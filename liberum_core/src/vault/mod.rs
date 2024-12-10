pub mod fragment;

use std::cmp;
use std::iter::once;
use std::iter::successors;
use std::path::Path;
use std::path::PathBuf;
use std::pin::Pin;
use std::str::FromStr;

use anyhow::anyhow;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use fragment::key::Key;
use fragment::FragmentInfo;
use futures::stream::BoxStream;
use futures::StreamExt;
use kameo::mailbox::bounded::BoundedMailbox;
use kameo::message::Message;
use kameo::messages;
use kameo::Actor;
use liberum_core::parser::ObjectEnum;
use liberum_core::proto::Hash;
use liberum_core::proto::PinObject;
use liberum_core::proto::TypedObject;
use liberum_core::types::TypedObjectInfo;
use rusqlite::params;
use rusqlite::params_from_iter;
use rusqlite::OptionalExtension;
use tokio::fs::remove_file;
use tokio::fs::File;
use tokio::io;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncSeekExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio_rusqlite::Connection;
use tokio_util::bytes::Bytes;
use tokio_util::io::ReaderStream;
use tracing::debug;
use uuid::Uuid;

pub struct Vault {
    db: Connection,
    // None will cause Vault to store data in memory
    vault_dir_path: Option<PathBuf>,
}

type FragmentData = BoxStream<'static, Result<Bytes, io::Error>>;

pub struct LoadFragment(Key);

impl Actor for Vault {
    type Mailbox = BoundedMailbox<Self>;

    async fn on_start(
        &mut self,
        _: kameo::actor::ActorRef<Self>,
    ) -> std::result::Result<(), kameo::error::BoxError> {
        self.prepare_db().await?;

        Ok(())
    }
}

#[messages]
impl Vault {
    #[message]
    async fn store_fragment(&self, key: Option<Key>, mut data: FragmentData) -> Result<Key> {
        let uid = Uuid::new_v4();
        // TODO: Storing fragments in memory not supported
        let random_fragment_path =
            Self::temp_dir_path(self.vault_dir_path.as_ref().unwrap()).join(uid.to_string());
        let mut fragment_file = File::create(&random_fragment_path).await?;
        let mut hasher = blake3::Hasher::new();
        let mut fragment_size = 0;

        while let Some(bytes) = data.next().await {
            let bytes = bytes?;
            hasher.update(&bytes);
            fragment_file.write(&bytes).await?;
            fragment_size += bytes.len();
        }

        let key_bytes = hasher.finalize().as_bytes().to_vec();
        let fragment_key = Key::try_from(key_bytes.clone())?;

        // Verify integrity if key was provided
        if let Some(key) = key {
            if key != fragment_key {
                remove_file(random_fragment_path).await?;
                bail!(
                    "Fragment integrity check failed, expected key to be {key}, was {fragment_key}"
                );
            }
        }

        let key_string = bs58::encode(&key_bytes).into_string();
        // TODO: Storing fragments in memory not supported
        let valid_fragment_path =
            Self::fragment_dir_path(self.vault_dir_path.as_ref().unwrap()).join(key_string);
        tokio::fs::rename(random_fragment_path, &valid_fragment_path).await?;

        let fragment_info = FragmentInfo::new(
            fragment_key.clone(),
            &valid_fragment_path,
            fragment_size as u64,
        );
        self.store_fragment_info(fragment_info).await?;

        Ok(fragment_key)
    }

    #[message]
    pub async fn store_object(&self, hash: Hash, object: ObjectEnum) -> Result<()> {
        let key: Key = hash.bytes.into();

        match object {
            ObjectEnum::Empty(_) => {}
            ObjectEnum::Typed(typed_object) => {
                self.store_hash_type_mapping(hash, TypedObject::UUID)
                    .await?;
                self.store_typed_object(key, typed_object).await?;
            }
            ObjectEnum::PinObject(pin_object) => {
                self.store_hash_type_mapping(hash, PinObject::UUID).await?;
                self.store_pin_object(key, pin_object).await?;
            }
            _ => return Result::Err(anyhow!("Storing this object type is not supported!")),
        }

        return Ok(());
    }

    #[message]
    pub async fn load_object(&self, hash: Hash) -> Result<Option<ObjectEnum>> {
        let key: Key = hash.bytes.into();
        let type_id = self.load_hash_type_mapping(hash).await?;

        // No hash-type mapping means effectively that the object does not exist
        if let None = type_id {
            return Ok(None);
        }

        let type_id = type_id.unwrap();
        println!("{type_id}");

        let load_result = match type_id {
            TypedObject::UUID => self
                .load_typed_object(key)
                .await
                .map(|r| r.map(|o| ObjectEnum::Typed(o)))
                .map_err(|e| anyhow!(e)),
            PinObject::UUID => self
                .load_pin_object_by_hash(key)
                .await
                .map(|r| r.map(|o| ObjectEnum::PinObject(o)))
                .map_err(|e| anyhow!(e)),
            _ => Err(anyhow!("Loading this object type is not supported!")),
        };

        load_result
    }

    #[message]
    pub async fn list_typed_objects(&self) -> Result<Vec<TypedObjectInfo>> {
        const SELECT_TYPED_OBJECT_QUERY: &str = "
            SELECT hash0, hash1, hash2, hash3, type_id
            FROM typed_object;
        ";

        let object_infos = self
            .db
            .call(|conn| {
                let mut stmt = conn.prepare(SELECT_TYPED_OBJECT_QUERY)?;
                let rows = stmt.query_map([], |row| {
                    let key_i64s: [i64; 4] = [row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?];
                    let key = Key::from(key_i64s);
                    let type_id_str: String = row.get(4)?;
                    let type_id = Uuid::from_str(&type_id_str).expect("type id to be correct");

                    Ok(TypedObjectInfo {
                        id: key.to_string(),
                        type_id,
                    })
                })?;

                let mut objects = Vec::new();
                for obj in rows {
                    objects.push(obj?);
                }

                Ok(objects)
            })
            .await?;

        Ok(object_infos)
    }

    #[message]
    pub async fn delete_typed_object(&self, hash: Hash) -> Result<()> {
        const DELETE_TYPED_OBJECT_QUERY: &str = "
            DELETE FROM typed_object
            WHERE hash0 = ?1 AND hash1 = ?2 AND hash2 = ?3 AND hash3 = ?4
        ";

        self.db
            .call(move |conn| {
                let key_u64: [u64; 4] = Key::from(hash.bytes).into();
                let key_i64: [i64; 4] = [
                    key_u64[0] as i64,
                    key_u64[1] as i64,
                    key_u64[2] as i64,
                    key_u64[3] as i64,
                ];

                conn.execute(DELETE_TYPED_OBJECT_QUERY, params_from_iter(key_i64))?;

                Ok(())
            })
            .await?;

        Ok(())
    }

    #[message]
    async fn store_hash_type_mapping(&self, hash: Hash, type_id: Uuid) -> Result<()> {
        const INSERT_HASH_TYPE_MAPPING_QUERY: &str =
            "INSERT INTO hash_type_mapping (hash0, hash1, hash2, hash3, type_id)
             VALUES (?1, ?2, ?3, ?4, ?5)";

        let key_as_i64: [i64; 4] = Key::from(hash.bytes).into();

        self.db
            .call(move |conn| {
                conn.execute(
                    INSERT_HASH_TYPE_MAPPING_QUERY,
                    params![
                        key_as_i64[0],
                        key_as_i64[1],
                        key_as_i64[2],
                        key_as_i64[3],
                        type_id.to_string()
                    ],
                )?;

                Ok(())
            })
            .await
            .map_err(|e| anyhow!(e))
    }

    #[message]
    async fn load_hash_type_mapping(&self, hash: Hash) -> Result<Option<Uuid>> {
        const SELECT_HASH_TYPE_MAPPING_QUERY: &str =
            "SELECT type_id FROM hash_type_mapping WHERE hash0 = ?1 AND hash1 = ?2 AND hash2 = ?3 AND hash3 = ?4";

        let type_id_str = self
            .db
            .call(move |conn| {
                let key_as_i64: [i64; 4] = Key::from(hash.bytes).into();
                let type_id_str = conn
                    .query_row(
                        SELECT_HASH_TYPE_MAPPING_QUERY,
                        params_from_iter(key_as_i64),
                        |row| {
                            let type_id_str: String = row.get(0)?;
                            Ok(type_id_str)
                        },
                    )
                    .optional()?;

                Ok(type_id_str)
            })
            .await
            .map_err(|e| anyhow!(e))?;

        match type_id_str {
            Some(type_id_str) => {
                let type_id = Uuid::from_str(&type_id_str)
                    .context("Could not create UUID from type string")?;
                Ok(Some(type_id))
            }
            None => Ok(None),
        }
    }

    #[message]
    async fn delete_hash_type_mapping(&self, hash: Hash) -> Result<()> {
        const DELETE_HASH_TYPE_MAPPING_QUERY: &str = "
            DELETE FROM hash_type_mapping
            WHERE hash0 = ?1 AND hash1 = ?2 AND hash2 = ?3 AND hash3 = ?4
        ";

        self.db
            .call(move |conn| {
                let key_as_i64: [i64; 4] = Key::from(hash.bytes).into();

                conn.execute(DELETE_HASH_TYPE_MAPPING_QUERY, params_from_iter(key_as_i64))?;

                Ok(())
            })
            .await?;

        Ok(())
    }

    #[message]
    async fn load_pin_objects(
        &self,
        from: Option<Hash>,
        to: Option<Hash>,
    ) -> Result<Vec<PinObject>> {
        todo!()
    }
}

impl Message<LoadFragment> for Vault {
    type Reply = Result<Option<FragmentData>>;

    fn handle(
        &mut self,
        msg: LoadFragment,
        _: kameo::message::Context<'_, Self, Self::Reply>,
    ) -> impl std::future::Future<Output = Self::Reply> + Send {
        async move { self.load_fragment(msg.0).await }
    }
}

impl Vault {
    const DEFAULT_VAULT_DATABASE_NAME: &'static str = "vault.db3";
    const FRAGMENT_DIR_NAME: &'static str = "fragments";
    const TEMP_DIR_NAME: &'static str = "temp";
    const MIN_FRAGMENT_SIZE: u64 = 4096;

    pub async fn new_on_disk(vault_dir_path: &Path) -> Result<Vault> {
        Self::ensure_dirs(vault_dir_path).await?;

        let db_path = Self::default_db_path(vault_dir_path);
        let db = Connection::open(db_path).await?;

        Ok(Vault {
            db,
            vault_dir_path: Some(vault_dir_path.to_path_buf()),
        })
    }

    pub async fn new_in_memory() -> Result<Vault> {
        let db = Connection::open_in_memory().await?;

        Ok(Vault {
            db,
            vault_dir_path: None,
        })
    }

    pub async fn fragment(path: &Path) -> Result<Vec<FragmentData>> {
        let file_size = tokio::fs::metadata(path).await?.len();
        let fragment_sizes = Self::fragment_sizes(file_size);
        let mut current_pos = 0;
        let mut result: Vec<FragmentData> = Vec::new();

        for fragment_size in fragment_sizes {
            let mut f = File::open(path).await?;
            f.seek(std::io::SeekFrom::Start(current_pos)).await?;

            let buf_reader = BufReader::new(f);
            let reader_stream = ReaderStream::new(buf_reader.take(fragment_size));

            result.push(reader_stream.boxed());
            current_pos += fragment_size;
        }

        Ok(result)
    }

    fn fragment_sizes(target: u64) -> Vec<u64> {
        let mut fragment_sizes = Vec::new();
        let mut current_size = cmp::max(Self::MIN_FRAGMENT_SIZE, Self::power_2_upto(target));
        let mut current_target = target;

        while current_target != 0 {
            current_size = cmp::max(
                Self::MIN_FRAGMENT_SIZE,
                Self::power_2_desc_from_power_2(current_size, current_target),
            );

            fragment_sizes.push(current_size);
            current_target = current_target.saturating_sub(current_size);
        }

        fragment_sizes
    }

    fn power_2_upto(limit: u64) -> u64 {
        let powers_from_1 = successors(Some(1u64), |&n| Some(n * 2))
            .take_while(|x| x <= &limit)
            .last();

        once(0u64).chain(powers_from_1).last().unwrap()
    }

    fn power_2_desc_from_power_2(start: u64, limit: u64) -> u64 {
        assert!(Self::is_power_of_2(start));

        if limit == 0 {
            return 0;
        }

        successors(Some(start), |&n| Some(n / 2))
            .take_while(|x| x > &(limit / 2) || x == &limit)
            .last()
            .unwrap()
    }

    fn is_power_of_2(number: u64) -> bool {
        number > 0 && (((number) & (number - 1)) == 0)
    }

    async fn prepare_db(&self) -> Result<()> {
        const CREATE_FRAGMENT_TABLE_QUERY: &str = "
            CREATE TABLE IF NOT EXISTS fragment (
                hash0 INTEGER NOT NULL,
                hash1 INTEGER NOT NULL,
                hash2 INTEGER NOT NULL,
                hash3 INTEGER NOT NULL,
                path VARCHAR(255) NOT NULL,
                size INTEGER NOT NULL,
                PRIMARY KEY (hash0, hash1, hash2, hash3)
            )
        ";

        self.db
            .call(|conn| Ok(conn.execute(CREATE_FRAGMENT_TABLE_QUERY, ())?))
            .await?;

        const CREATE_TYPED_OBJECT_TABLE_QUERY: &str = "
            CREATE TABLE IF NOT EXISTS typed_object (
                hash0 INTEGER NOT NULL,
                hash1 INTEGER NOT NULL,
                hash2 INTEGER NOT NULL,
                hash3 INTEGER NOT NULL,
                type_id TEXT NOT NULL,
                data BLOB NOT NULL,
                PRIMARY KEY (hash0, hash1, hash2, hash3)
            )
        ";

        self.db
            .call(|conn| Ok(conn.execute(CREATE_TYPED_OBJECT_TABLE_QUERY, ())?))
            .await?;

        const CREATE_HASH_TYPE_MAPPING_TABLE_QUERY: &str = "
            CREATE TABLE IF NOT EXISTS hash_type_mapping (
                hash0 INTEGER NOT NULL,
                hash1 INTEGER NOT NULL,
                hash2 INTEGER NOT NULL,
                hash3 INTEGER NOT NULL,
                type_id TEXT NOT NULL,
                PRIMARY KEY (hash0, hash1, hash2, hash3)
            )
        ";

        self.db
            .call(|conn| Ok(conn.execute(CREATE_HASH_TYPE_MAPPING_TABLE_QUERY, ())?))
            .await?;

        const CREATE_PIN_OBJECT_TABLE_QUERY: &str = "
            CREATE TABLE IF NOT EXISTS pin_object (
                hash TEXT NOT NULL PRIMARY KEY,
                hash_from TEXT NOT NULL UNIQUE,
                hash_to TEXT NOT NULL UNIQUE, 
            );
            CREATE INDEX pin_object_hash_from_idx ON pin_object (hash_from);
            CREATE INDEX pin_object_hash_to_idx ON pin_object (hash_to)
        ";

        self.db
            .call(|conn| Ok(conn.execute(CREATE_PIN_OBJECT_TABLE_QUERY, ())?))
            .await?;

        Ok(())
    }

    async fn load_fragment_info(&self, key: Key) -> Result<Option<FragmentInfo>> {
        const SELECT_FRAGMENT_QUERY: &str = "
            SELECT path, size
            FROM fragment
            WHERE hash0 = ?1 AND hash1 = ?2 AND hash2 = ?3 AND hash3 = ?4
        ";

        self.db
            .call(move |conn| {
                let mut stmt = conn.prepare(SELECT_FRAGMENT_QUERY)?;
                let key_as_i64: [i64; 4] = key.into();

                let fragment = stmt
                    .query_row(key_as_i64, |r| {
                        let path: String = match r.get(0) {
                            Ok(p) => p,
                            Err(e) => return Result::Err(e),
                        };

                        let size: u64 = match r.get(1) {
                            Ok(s) => s,
                            Err(e) => return Result::Err(e),
                        };

                        Result::Ok(FragmentInfo::new(key, Path::new(&path), size))
                    })
                    .optional()?;

                Ok(fragment)
            })
            .await
            .map_err(|e| anyhow!(e))
    }

    async fn store_fragment_info(&self, fragment: FragmentInfo) -> Result<()> {
        const SELECT_FRAGMENT_QUERY: &str = "SELECT COUNT(*) FROM fragment WHERE hash0 = ?1 AND hash1 = ?2 AND hash2 = ?3 AND hash3 = ?4";
        const INSERT_FRAGMENT_QUERY: &str = "
                INSERT INTO fragment (hash0, hash1, hash2, hash3, path, size)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ";

        let key_as_i64: [i64; 4] = fragment.hash.into();
        let cnt = self
            .db
            .call(move |conn| {
                let cnt = conn.query_row(SELECT_FRAGMENT_QUERY, key_as_i64, |r| {
                    let cnt: usize = r.get(0)?;

                    Ok(cnt)
                })?;

                Ok(cnt)
            })
            .await?;

        if cnt != 0 {
            return Ok(());
        }

        self.db
            .call(move |conn| {
                conn.execute(
                    INSERT_FRAGMENT_QUERY,
                    (
                        key_as_i64[0],
                        key_as_i64[1],
                        key_as_i64[2],
                        key_as_i64[3],
                        fragment.path.to_str(),
                        fragment.size,
                    ),
                )?;

                Ok(())
            })
            .await
            .map_err(|e| anyhow!(e))
    }

    // TODO: Loading fragments from in memory not supported
    async fn load_fragment(&self, key: Key) -> Result<Option<FragmentData>> {
        let fragment_info = self.load_fragment_info(key.clone()).await?;

        if let None = fragment_info {
            return Ok(None);
        }

        let fragment_info = fragment_info.unwrap();
        let fragment_path = fragment_info.path;
        let fragment_file = File::open(&fragment_path).await?;

        Ok(Some(ReaderStream::new(fragment_file).boxed()))
    }

    async fn load_typed_object(&self, key: Key) -> Result<Option<TypedObject>> {
        const SELECT_TYPED_OBJECT_QUERY: &str = "
            SELECT type_id, data
            FROM typed_object
            WHERE hash0 = ?1 AND hash1 = ?2 AND hash2 = ?3 AND hash3 = ?4
        ";

        self.db
            .call(move |conn| {
                let mut stmt = conn.prepare(SELECT_TYPED_OBJECT_QUERY)?;
                let key_as_i64: [i64; 4] = key.into();

                let typed_object = stmt
                    .query_row(key_as_i64, |r| {
                        let uuid: String = r.get(0)?;
                        let data: Vec<u8> = r.get(1)?;

                        Result::Ok(TypedObject {
                            uuid: uuid::Uuid::from_str(&uuid).unwrap(),
                            data,
                        })
                    })
                    .optional()?;

                Ok(typed_object)
            })
            .await
            .map_err(|e| anyhow!(e))
    }

    async fn store_typed_object(&self, key: Key, object: TypedObject) -> Result<()> {
        const SELECT_TYPED_OBJECT_QUERY: &str =
            "SELECT COUNT(*) FROM typed_object WHERE hash0 = ?1 AND hash1 = ?2 AND hash2 = ?3 AND hash3 = ?4";
        const INSERT_TYPED_OBJECT_QUERY: &str =
            "INSERT INTO typed_object (hash0, hash1, hash2, hash3, type_id, data)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)";

        let key_as_i64: [i64; 4] = key.into();
        let cnt = self
            .db
            .call(move |conn| {
                let cnt = conn.query_row(SELECT_TYPED_OBJECT_QUERY, key_as_i64, |r| {
                    let cnt: usize = r.get(0)?;

                    Ok(cnt)
                })?;

                Ok(cnt)
            })
            .await?;

        if cnt != 0 {
            // Already stored, no need to change as objects are immutable
            return Ok(());
        }

        self.db
            .call(move |conn| {
                conn.execute(
                    INSERT_TYPED_OBJECT_QUERY,
                    (
                        key_as_i64[0] as i64,
                        key_as_i64[1] as i64,
                        key_as_i64[2] as i64,
                        key_as_i64[3] as i64,
                        object.uuid.to_string(),
                        object.data,
                    ),
                )?;

                Ok(())
            })
            .await
            .map_err(|e| anyhow!(e))
    }

    async fn store_pin_object(&self, key: Key, object: PinObject) -> Result<()> {
        const INSERT_PIN_OBJECT_QUERY: &str = "
            INSERT INTO pin_object (hash, hash_from, hash_to)
            VALUES (?1, ?2, ?3)
        ";

        let object_hash_b58str = key.as_base58();
        let from_hash_b58str = Key::from(object.from.bytes).as_base58();
        let to_hash: Hash = (&object.to).try_into()?;
        let to_hash_b58str = Key::from(to_hash.bytes).as_base58();

        self.db
            .call(move |conn| {
                conn.execute(
                    INSERT_PIN_OBJECT_QUERY,
                    params![object_hash_b58str, from_hash_b58str, to_hash_b58str],
                )?;

                Ok(())
            })
            .await
            .map_err(|e| anyhow!(e))
    }

    async fn load_pin_object_by_hash(&self, key: Key) -> Result<Option<PinObject>> {
        todo!()
    }

    async fn ensure_dirs(vault_dir_path: &Path) -> Result<()> {
        debug!(
            path = vault_dir_path.display().to_string(),
            "ensuring vault dir"
        );
        tokio::fs::create_dir_all(vault_dir_path).await?;

        let fragment_dir_path = Self::fragment_dir_path(vault_dir_path);
        debug!(
            path = fragment_dir_path.display().to_string(),
            "ensuring fragment dir"
        );
        tokio::fs::create_dir_all(fragment_dir_path).await?;

        let temp_dir_path = Self::temp_dir_path(vault_dir_path);
        debug!(
            path = temp_dir_path.display().to_string(),
            "ensuring temp dir"
        );
        tokio::fs::create_dir_all(temp_dir_path).await?;

        Ok(())
    }

    fn fragment_dir_path(vault_dir_path: &Path) -> PathBuf {
        vault_dir_path.join(Self::FRAGMENT_DIR_NAME)
    }

    fn temp_dir_path(vault_dir_path: &Path) -> PathBuf {
        vault_dir_path.join(Self::TEMP_DIR_NAME)
    }

    fn default_db_path(base_path: &Path) -> PathBuf {
        base_path.join(Self::DEFAULT_VAULT_DATABASE_NAME)
    }
}

#[cfg(test)]
mod tests {
    use futures::StreamExt as FuturesStreamExt;
    use kameo::request::MessageSend;
    use pretty_assertions::assert_eq;
    use rand::{rngs::StdRng, Rng, SeedableRng};
    use tempdir::TempDir;
    use tokio::io::AsyncWriteExt;

    use super::*;

    #[tokio::test]
    async fn fragment_test() {
        let tmp_dir = TempDir::new("liberum_tests").unwrap();
        let file_path = tmp_dir.path().join("to_fragment.txt");
        let mut file = File::create(&file_path).await.unwrap();

        file.write_all(&[65; 4096]).await.unwrap();
        file.write_all(&[66; 2048]).await.unwrap();
        file.flush().await.unwrap();

        let mut fragments = Vault::fragment(&file_path).await.unwrap();

        assert_eq!(fragments.len(), 2);

        let mut stream_contents = Vec::new();
        while let Some(chunk) = fragments[0].next().await {
            stream_contents.extend_from_slice(&chunk.unwrap());
        }

        assert_eq!(stream_contents.len(), 4096);
        assert!(stream_contents.iter().all(|b| *b == 65));

        stream_contents = Vec::new();
        while let Some(chunk) = fragments[1].next().await {
            stream_contents.extend_from_slice(&chunk.unwrap());
        }

        // Minimum fragment size of 4096
        // TODO: Find other way to check for real fragment size, not data got from stream
        assert_eq!(stream_contents.len(), 2048);
        assert!(stream_contents.iter().all(|b| *b == 66));
    }

    #[tokio::test]
    async fn store_load_test() {
        let tmp_dir = TempDir::new("liberum_tests").unwrap();
        let vault_dir_path = tmp_dir.path();

        let file_path = tmp_dir.path().join("to_fragment.txt");
        let mut file = File::create(&file_path).await.unwrap();
        let mut rng = StdRng::seed_from_u64(1234);
        let random_bytes = (0..954521)
            .map(|_| rng.gen_range(65..91))
            .collect::<Vec<u8>>();
        file.write_all(&random_bytes).await.unwrap();
        file.flush().await.unwrap();

        let fragments = Vault::fragment(&file_path).await.unwrap();
        // vec![524288, 262144, 131072, 32768, 4096, 4096]
        assert_eq!(fragments.len(), 6);

        let vault = Vault::new_on_disk(vault_dir_path).await.unwrap();
        let vault = kameo::spawn(vault);
        let mut stored_keys = Vec::new();

        for frag in fragments {
            let stored_key = vault
                .ask(StoreFragment {
                    key: None,
                    data: frag,
                })
                .send()
                .await
                .unwrap();
            stored_keys.push(stored_key);
        }

        let mut bytes_recollected = Vec::new();

        for key in stored_keys {
            let mut fragment_bytes = vault
                .ask(LoadFragment(key))
                .send()
                .await
                .unwrap()
                .unwrap()
                .flat_map(|bt| tokio_stream::iter(bt.unwrap()))
                .collect::<Vec<u8>>()
                .await;

            // TODO: Assert fragments sizes

            bytes_recollected.append(&mut fragment_bytes);
        }

        assert_eq!(random_bytes, bytes_recollected);
    }

    #[test]
    fn fragment_sizes_test() {
        let some_file_size = 45000;
        let fragment_sizes = Vault::fragment_sizes(some_file_size);

        assert_eq!(fragment_sizes, vec![32768, 8192, 4096]);

        let some_file_size = 954521;
        let fragment_sizes = Vault::fragment_sizes(some_file_size);

        assert_eq!(
            fragment_sizes,
            vec![524288, 262144, 131072, 32768, 4096, 4096]
        );

        let some_file_size = 4096;
        let fragment_sizes = Vault::fragment_sizes(some_file_size);

        assert_eq!(fragment_sizes, vec![4096]);

        let some_file_size = 5;
        let fragment_sizes = Vault::fragment_sizes(some_file_size);

        assert_eq!(fragment_sizes, vec![4096]);
    }

    #[tokio::test]
    async fn typed_object_load_store_test() {
        let tmp_dir = TempDir::new("liberum_tests").unwrap();
        let vault_dir_path = tmp_dir.path();
        let vault = Vault::new_on_disk(vault_dir_path).await.unwrap();
        let vault = kameo::spawn(vault);

        vault
            .ask(StoreObject {
                hash: Hash { bytes: [1; 32] },
                object: ObjectEnum::Typed(TypedObject {
                    uuid: TypedObject::UUID,
                    data: vec![1, 2, 3],
                }),
            })
            .send()
            .await
            .unwrap();

        let typed_object = vault
            .ask(LoadObject {
                hash: Hash { bytes: [1; 32] },
            })
            .send()
            .await
            .unwrap()
            .unwrap();

        if let ObjectEnum::Typed(TypedObject { uuid, data }) = typed_object {
            assert_eq!(uuid, TypedObject::UUID);
            assert_eq!(data, vec![1, 2, 3]);
        } else {
            panic!("Object enum is not TypedObject, but it should be")
        }
    }

    #[tokio::test]
    async fn typed_object_delete_test() {
        let tmp_dir = TempDir::new("liberum_tests").unwrap();
        let vault_dir_path = tmp_dir.path();
        let vault = Vault::new_on_disk(vault_dir_path).await.unwrap();
        let vault = kameo::spawn(vault);

        vault
            .ask(StoreObject {
                hash: Hash { bytes: [1; 32] },
                object: ObjectEnum::Typed(TypedObject {
                    uuid: TypedObject::UUID,
                    data: vec![1, 2, 3],
                }),
            })
            .send()
            .await
            .unwrap();

        vault
            .ask(DeleteTypedObject {
                hash: Hash { bytes: [1; 32] },
            })
            .send()
            .await
            .unwrap();

        let loaded_obj = vault
            .ask(LoadObject {
                hash: Hash { bytes: [1; 32] },
            })
            .send()
            .await
            .unwrap();

        assert!(loaded_obj.is_none());
    }
}
