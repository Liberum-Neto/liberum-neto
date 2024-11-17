use core::hash;
use std::path::Path;
use std::path::PathBuf;

use anyhow::anyhow;
use anyhow::Result;
use kameo::mailbox::bounded::BoundedMailbox;
use kameo::message::{Context, Message};
use kameo::Actor;
use rand::random;
use rusqlite::OptionalExtension;
use tokio::fs::File;
use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncSeekExt;
use tokio::io::AsyncWriteExt;
use tokio::io::Take;
use tokio_rusqlite::Connection;
use tokio_stream::StreamExt;
use tokio_util::io::ReaderStream;
use tokio_util::io::StreamReader;
use tracing::debug;

use crate::fragment;
use crate::fragment::key::Key;
use crate::fragment::FragmentInfo;

pub struct Vault {
    db: Connection,
    vault_dir_path: PathBuf,
}

type FragmentData = Box<ReaderStream<Take<File>>>;

pub struct StoreFragment(FragmentData);
pub struct LoadFragment(Key);

impl Vault {
    const DEFAULT_VAULT_DATABASE_NAME: &'static str = "vault.db3";
    const FRAGMENT_DIR_NAME: &'static str = "fragments";
    const TEMP_DIR_NAME: &'static str = "temp";

    pub async fn new(vault_dir_path: &Path) -> Result<Vault> {
        Self::ensure_dirs(vault_dir_path).await?;

        let db_path = Self::default_db_path(vault_dir_path);
        let db = Connection::open(db_path).await?;

        Ok(Vault {
            db,
            vault_dir_path: vault_dir_path.to_path_buf(),
        })
    }

    // TODO: Add logarithmic fragment sizes
    pub async fn fragment(path: &Path) -> Result<Vec<Box<ReaderStream<Take<File>>>>> {
        let file_size = tokio::fs::metadata(path).await?.len();
        let mut current_pos = 0;
        let mut result = Vec::new();

        while current_pos < file_size {
            let mut f = File::open(path).await?;
            f.seek(std::io::SeekFrom::Start(current_pos)).await?;

            let reader_stream = ReaderStream::new(f.take(4096));

            result.push(Box::new(reader_stream));
            current_pos += 4096;
        }

        Ok(result)
    }

    async fn prepare_db(&self) -> Result<()> {
        const CREATE_FRAGMENT_TABLE_QUERY: &str = "
            CREATE TABLE IF NOT EXISTS fragment (
                hash0 INTEGER,
                hash1 INTEGER,
                hash2 INTEGER,
                hash3 INTEGER,
                path VARCHAR(255),
                size INTEGER,
                PRIMARY KEY (hash0, hash1, hash2, hash3)
            )
        ";

        self.db
            .call(|conn| Ok(conn.execute(CREATE_FRAGMENT_TABLE_QUERY, ())?))
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
                let key_u64_slice = key.as_u64_slice_be();
                let key_as_i64 = [
                    key_u64_slice[0] as i64,
                    key_u64_slice[1] as i64,
                    key_u64_slice[2] as i64,
                    key_u64_slice[3] as i64,
                ];

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

        let hash_as_u64 = fragment.hash.as_u64_slice_be();

        let cnt = self
            .db
            .call(move |conn| {
                let key_as_i64 = [
                    hash_as_u64[0] as i64,
                    hash_as_u64[1] as i64,
                    hash_as_u64[2] as i64,
                    hash_as_u64[3] as i64,
                ];

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
                        hash_as_u64[0] as i64,
                        hash_as_u64[1] as i64,
                        hash_as_u64[2] as i64,
                        hash_as_u64[3] as i64,
                        fragment.path.to_str(),
                        fragment.size,
                    ),
                )?;

                Ok(())
            })
            .await
            .map_err(|e| anyhow!(e))
    }

    async fn load_fragment(&self, key: Key) -> Result<Option<FragmentData>> {
        let fragment_info = self.load_fragment_info(key.clone()).await?;

        if let None = fragment_info {
            return Ok(None);
        }

        let fragment_info = fragment_info.unwrap();
        let fragment_path = fragment_info.path;
        let fragment_file = File::open(&fragment_path).await?;

        // TODO: Fix, is there any way to not use this take here?
        Ok(Some(Box::new(ReaderStream::new(
            fragment_file.take(fragment_info.size),
        ))))
    }

    async fn store_fragment(&self, data: &mut FragmentData) -> Result<Key> {
        let uid = uuid::Uuid::new_v4();
        let random_fragment_path = Self::temp_dir_path(&self.vault_dir_path).join(uid.to_string());
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
        let key_string = bs58::encode(&key_bytes).into_string();

        let valid_fragment_path = Self::fragment_dir_path(&self.vault_dir_path).join(key_string);
        tokio::fs::rename(random_fragment_path, &valid_fragment_path).await?;

        let fragment_key = Key::try_from(key_bytes)?;
        let fragment_info = FragmentInfo::new(
            fragment_key.clone(),
            &valid_fragment_path,
            fragment_size as u64,
        );
        self.store_fragment_info(fragment_info).await?;

        Ok(fragment_key)
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
        tokio::fs::create_dir(fragment_dir_path).await?;

        let temp_dir_path = Self::temp_dir_path(vault_dir_path);
        debug!(
            path = temp_dir_path.display().to_string(),
            "ensuring temp dir"
        );
        tokio::fs::create_dir(temp_dir_path).await?;

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

impl Message<StoreFragment> for Vault {
    type Reply = Result<Key>;

    async fn handle(
        &mut self,
        mut msg: StoreFragment,
        _: Context<'_, Self, Self::Reply>,
    ) -> Self::Reply {
        Ok(self.store_fragment(&mut msg.0).await?)
    }
}

impl Message<LoadFragment> for Vault {
    type Reply = Result<Option<FragmentData>>;

    async fn handle(
        &mut self,
        msg: LoadFragment,
        _: Context<'_, Self, Self::Reply>,
    ) -> Self::Reply {
        self.load_fragment(msg.0).await
    }
}

#[cfg(test)]
mod tests {
    use kameo::request::MessageSend;
    use tempdir::TempDir;
    use tokio::io::AsyncWriteExt;
    use tokio_stream::StreamExt;

    use super::*;

    #[tokio::test]
    async fn load_store_test() {
        let tmp_dir = TempDir::new("liberum_tests").unwrap();
        let vault = Vault::new(&tmp_dir.path()).await.unwrap();
        let vault = kameo::spawn(vault);

        let src_file_path = tmp_dir.path().join("src_file");
        let mut src_file = File::create(&src_file_path).await.unwrap();
        src_file.write(&[0; 9000]).await.unwrap();

        let mut last_key_stored: Option<Key> = None;
        let fragments = Vault::fragment(&src_file_path).await.unwrap();
        for fragment in fragments {
            let key_stored = vault.ask(StoreFragment(fragment)).send().await.unwrap();
            println!("Stored {}", key_stored.as_base64());
            last_key_stored = Some(key_stored);
        }

        let last_key_stored = last_key_stored.unwrap();
        let mut fragment_stream = vault
            .ask(LoadFragment(last_key_stored))
            .send()
            .await
            .unwrap()
            .unwrap();

        let is_ok = fragment_stream
            .all(|x| x.unwrap().iter().all(|b| *b == 0))
            .await;

        assert!(is_ok);
    }

    #[tokio::test]
    async fn fragment_test() {
        let tmp_dir = TempDir::new("liberum_tests").unwrap();
        let file_path = tmp_dir.path().join("to_fragment.txt");
        let mut file = File::create(&file_path).await.unwrap();

        file.write_all(&[65; 4096]).await.unwrap();
        file.write_all(&[66; 4096]).await.unwrap();

        let mut fragments = Vault::fragment(&file_path).await.unwrap();

        assert_eq!(fragments.len(), 2);

        let mut stream_contents = Vec::new();
        while let Some(chunk) = fragments[0].next().await {
            stream_contents.extend_from_slice(&chunk.unwrap());
        }

        assert!(stream_contents.iter().all(|b| *b == 65));

        stream_contents = Vec::new();
        while let Some(chunk) = fragments[1].next().await {
            stream_contents.extend_from_slice(&chunk.unwrap());
        }

        assert!(stream_contents.iter().all(|b| *b == 66));
    }
}
