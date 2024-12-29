pub mod key;

use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::anyhow;
use anyhow::Context;
use anyhow::Result;
use kameo::mailbox::bounded::BoundedMailbox;
use kameo::messages;
use kameo::Actor;
use key::Key;
use liberum_core::parser::ObjectEnum;
use liberum_core::proto::Hash;
use liberum_core::proto::PinObject;
use liberum_core::proto::TypedObject;
use liberum_core::proto::TypedObjectRef;
use liberum_core::types::TypedObjectInfo;
use rusqlite::params;
use rusqlite::params_from_iter;
use rusqlite::OptionalExtension;
use tokio_rusqlite::Connection;
use tracing::debug;
use uuid::Uuid;

pub struct Vault {
    db: Connection,
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

#[messages]
impl Vault {
    #[message]
    pub async fn store_object(
        &self,
        hash: Hash,
        parent_hash: Option<Hash>,
        object: ObjectEnum,
    ) -> Result<()> {
        let key: Key = hash.bytes.into();
        let parent_key = parent_hash.map(|h| Key::from(h.bytes));

        match object {
            ObjectEnum::Empty(_) => {}
            ObjectEnum::Typed(typed_object) => {
                self.store_hash_type_mapping(hash, typed_object.uuid)
                    .await?;
                self.store_typed_object(key, parent_key, typed_object)
                    .await?;
            }
            ObjectEnum::PinObject(pin_object) => {
                self.store_hash_type_mapping(hash, PinObject::UUID).await?;
                self.store_pin_object(key, parent_key, pin_object).await?;
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
            SELECT hash, type_id
            FROM typed_object;
        ";

        let object_infos = self
            .db
            .call(|conn| {
                let mut stmt = conn.prepare(SELECT_TYPED_OBJECT_QUERY)?;
                let rows = stmt.query_map([], |row| {
                    let key_string: String = row.get(0)?;
                    let key = Key::try_from(key_string).unwrap();
                    let type_id_str: String = row.get(1)?;
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
    async fn store_hash_type_mapping(&self, hash: Hash, type_id: Uuid) -> Result<()> {
        const INSERT_HASH_TYPE_MAPPING_QUERY: &str = "INSERT INTO hash_type_mapping (hash, type_id)
             VALUES (?1, ?2)";

        self.db
            .call(move |conn| {
                conn.execute(
                    INSERT_HASH_TYPE_MAPPING_QUERY,
                    params![hash.to_string(), type_id.to_string()],
                )?;

                Ok(())
            })
            .await
            .map_err(|e| anyhow!(e))
    }

    #[message]
    async fn load_hash_type_mapping(&self, hash: Hash) -> Result<Option<Uuid>> {
        const SELECT_HASH_TYPE_MAPPING_QUERY: &str =
            "SELECT type_id FROM hash_type_mapping WHERE hash = ?1";

        let type_id_str = self
            .db
            .call(move |conn| {
                let type_id_str = conn
                    .query_row(
                        SELECT_HASH_TYPE_MAPPING_QUERY,
                        params![hash.to_string()],
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
            WHERE hash = ?1
        ";

        self.db
            .call(move |conn| {
                conn.execute(DELETE_HASH_TYPE_MAPPING_QUERY, params![hash.to_string()])?;

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
        let query;
        let params_list;

        match (from, to) {
            (Some(from), Some(to)) => {
                query = "
                    SELECT hash, hash_from, hash_to
                    FROM pin_object
                    WHERE hash_from = ?1 AND hash_to = ?2
                ";
                let from_str = from.to_string();
                let to_str = to.to_string();
                params_list = vec![from_str, to_str];
            }
            (Some(from), None) => {
                query = "
                    SELECT hash, hash_from, hash_to
                    FROM pin_object
                    WHERE hash_from = ?1
                ";
                let from_str = from.to_string();
                params_list = vec![from_str];
            }
            (None, Some(to)) => {
                query = "
                    SELECT hash, hash_from, hash_to
                    FROM pin_object
                    WHERE hash_to = ?1
                ";
                let to_str = to.to_string();
                params_list = vec![to_str];
            }
            (None, None) => {
                query = "
                    SELECT hash, hash_from, hash_to
                    FROM pin_object
                ";
                params_list = vec![];
            }
        }

        let pin_objects_triples = self
            .db
            .call(move |conn| {
                let mut stmt = conn.prepare(query)?;
                let pin_objects_iter = stmt.query_map(params_from_iter(params_list), |row| {
                    let hash_str: String = row.get(0)?;
                    let hash_from_str: String = row.get(1)?;
                    let hash_to_str: String = row.get(2)?;

                    Ok((hash_str, hash_from_str, hash_to_str))
                })?;

                let mut pin_objects = Vec::new();

                for pin_obj in pin_objects_iter {
                    pin_objects.push(pin_obj?);
                }

                Ok(pin_objects)
            })
            .await?;

        let mut pin_objects = Vec::new();

        for triple in pin_objects_triples {
            let hash_from = Hash::try_from(&triple.1)?;
            let hash_to = Hash::try_from(&triple.2)?;

            let pin_obj = PinObject {
                from: hash_from,
                to: TypedObjectRef::ByHash(hash_to),
            };

            pin_objects.push(pin_obj);
        }

        Ok(pin_objects)
    }
}

impl Vault {
    const DEFAULT_VAULT_DATABASE_NAME: &'static str = "vault.db3";

    pub async fn new_on_disk(vault_dir_path: &Path) -> Result<Vault> {
        Self::ensure_dirs(vault_dir_path).await?;

        let db_path = Self::default_db_path(vault_dir_path);
        let db = Connection::open(db_path).await?;

        Ok(Vault { db })
    }

    pub async fn new_in_memory() -> Result<Vault> {
        let db = Connection::open_in_memory().await?;

        Ok(Vault { db })
    }

    async fn prepare_db(&self) -> Result<()> {
        const CREATE_TYPED_OBJECT_TABLE_QUERY: &str = "
            CREATE TABLE IF NOT EXISTS typed_object (
                hash TEXT NOT NULL,
                parent_hash TEXT NOT NULL,
                type_id TEXT NOT NULL,
                data BLOB NOT NULL,
                PRIMARY KEY(hash, parent_hash),
                FOREIGN KEY(parent_hash) REFERENCES typed_object(hash)
            )
        ";

        self.db
            .call(|conn| Ok(conn.execute(CREATE_TYPED_OBJECT_TABLE_QUERY, ())?))
            .await?;

        const CREATE_HASH_TYPE_MAPPING_TABLE_QUERY: &str = "
            CREATE TABLE IF NOT EXISTS hash_type_mapping (
                hash TEXT NOT NULL PRIMARY KEY,
                type_id TEXT NOT NULL
            )
        ";

        self.db
            .call(|conn| Ok(conn.execute(CREATE_HASH_TYPE_MAPPING_TABLE_QUERY, ())?))
            .await?;

        const CREATE_PIN_OBJECT_TABLE_QUERY: &str = "
            CREATE TABLE IF NOT EXISTS pin_object (
                hash TEXT NOT NULL,
                parent_hash TEXT NOT NULL,
                hash_from TEXT NOT NULL UNIQUE,
                hash_to TEXT NOT NULL UNIQUE,
                PRIMARY KEY(hash, parent_hash),
                FOREIGN KEY(parent_hash) REFERENCES typed_object(hash)
            );
            CREATE INDEX pin_object_hash_from_idx ON pin_object (hash_from);
            CREATE INDEX pin_object_hash_to_idx ON pin_object (hash_to)
        ";

        self.db
            .call(|conn| Ok(conn.execute(CREATE_PIN_OBJECT_TABLE_QUERY, ())?))
            .await?;

        Ok(())
    }

    async fn load_typed_object(&self, key: Key) -> Result<Option<TypedObject>> {
        const SELECT_TYPED_OBJECT_QUERY: &str = "
            SELECT type_id, data
            FROM typed_object
            WHERE hash = ?1
        ";

        self.db
            .call(move |conn| {
                let mut stmt = conn.prepare(SELECT_TYPED_OBJECT_QUERY)?;

                let typed_object = stmt
                    .query_row(params![key.to_string()], |r| {
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

    async fn store_typed_object(
        &self,
        key: Key,
        parent_key: Option<Key>,
        object: TypedObject,
    ) -> Result<()> {
        const SELECT_TYPED_OBJECT_QUERY: &str =
            "SELECT COUNT(*) FROM typed_object WHERE hash = ?1 AND parent_hash = ?2";
        const INSERT_TYPED_OBJECT_QUERY: &str =
            "INSERT INTO typed_object (hash, parent_hash, type_id, data) VALUES (?1, ?2, ?3, ?4)";

        let parent_key = parent_key.unwrap_or(key);
        let cnt = self
            .db
            .call(move |conn| {
                let cnt = conn.query_row(
                    SELECT_TYPED_OBJECT_QUERY,
                    params![key.to_string(), parent_key.to_string()],
                    |r| {
                        let cnt: usize = r.get(0)?;

                        Ok(cnt)
                    },
                )?;

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
                        key.to_string(),
                        parent_key.to_string(),
                        object.uuid.to_string(),
                        object.data,
                    ),
                )?;

                Ok(())
            })
            .await
            .map_err(|e| anyhow!(e))
    }

    async fn store_pin_object(
        &self,
        key: Key,
        parent_key: Option<Key>,
        object: PinObject,
    ) -> Result<()> {
        const INSERT_PIN_OBJECT_QUERY: &str = "
            INSERT INTO pin_object (hash, parent_hash, hash_from, hash_to)
            VALUES (?1, ?2, ?3, ?4)
        ";

        let parent_key = parent_key.unwrap_or(key);
        let object_hash_b58str = key.as_base58();
        let from_hash_b58str = Key::from(object.from.bytes).as_base58();

        let to_hash: Hash = match object.to {
            TypedObjectRef::Direct(typed_object) => (&typed_object).try_into()?,
            TypedObjectRef::ByHash(hash) => hash,
        };

        let to_hash_b58str = Key::from(to_hash.bytes).as_base58();

        self.db
            .call(move |conn| {
                conn.execute(
                    INSERT_PIN_OBJECT_QUERY,
                    params![
                        object_hash_b58str,
                        parent_key.to_string(),
                        from_hash_b58str,
                        to_hash_b58str
                    ],
                )?;

                Ok(())
            })
            .await
            .map_err(|e| anyhow!(e))
    }

    async fn load_pin_object_by_hash(&self, key: Key) -> Result<Option<PinObject>> {
        const SELECT_PIN_OBJECT_BY_HASH_QUERY: &str = "
            SELECT hash_from, hash_to FROM pin_object
            WHERE hash = ?1
        ";

        let object_hash_str = key.as_base58();

        let hashes = self
            .db
            .call(move |conn| {
                let hashes = conn
                    .query_row(
                        SELECT_PIN_OBJECT_BY_HASH_QUERY,
                        params![object_hash_str],
                        |row| {
                            let hash_from: String = row.get(0)?;
                            let hash_to: String = row.get(1)?;

                            Ok((hash_from, hash_to))
                        },
                    )
                    .optional()?;

                Ok(hashes)
            })
            .await?;

        if let None = hashes {
            return Ok(None);
        }

        let hashes = hashes.unwrap();

        let hash_from = hashes.0;
        let hash_from = Hash::try_from(&hash_from)?;
        let hash_to = hashes.1;
        let hash_to = Hash::try_from(&hash_to)?;

        let pin_object = PinObject {
            from: hash_from,
            to: TypedObjectRef::ByHash(hash_to),
        };

        Ok(Some(pin_object))
    }

    async fn ensure_dirs(vault_dir_path: &Path) -> Result<()> {
        debug!(
            path = vault_dir_path.display().to_string(),
            "ensuring vault dir"
        );
        tokio::fs::create_dir_all(vault_dir_path).await?;

        Ok(())
    }

    fn default_db_path(base_path: &Path) -> PathBuf {
        base_path.join(Self::DEFAULT_VAULT_DATABASE_NAME)
    }
}

#[cfg(test)]
mod tests {
    use kameo::request::MessageSend;
    use pretty_assertions::assert_eq;
    use tempdir::TempDir;

    use super::*;

    #[tokio::test]
    async fn typed_object_load_store_test() {
        let tmp_dir = TempDir::new("liberum_tests").unwrap();
        let vault_dir_path = tmp_dir.path();
        let vault = Vault::new_on_disk(vault_dir_path).await.unwrap();
        let vault = kameo::spawn(vault);

        vault
            .ask(StoreObject {
                hash: Hash { bytes: [1; 32] },
                parent_hash: None,
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
    async fn pin_object_load_store_test() {
        let tmp_dir = TempDir::new("liberum_tests").unwrap();
        let vault_dir_path = tmp_dir.path();
        let vault = Vault::new_on_disk(vault_dir_path).await.unwrap();
        let vault = kameo::spawn(vault);

        vault
            .ask(StoreObject {
                hash: Hash { bytes: [1; 32] },
                parent_hash: None,
                object: ObjectEnum::PinObject(PinObject {
                    from: Hash { bytes: [2; 32] },
                    to: TypedObjectRef::ByHash(Hash { bytes: [3; 32] }),
                }),
            })
            .send()
            .await
            .unwrap();

        let left_results = vault
            .ask(LoadPinObjects {
                from: Some(Hash::from(&[2; 32])),
                to: None,
            })
            .send()
            .await
            .unwrap();

        assert_eq!(left_results.len(), 1);
        assert_eq!(left_results[0].from, Hash::from(&[2; 32]));
        let left_result = left_results[0].clone();
        match left_result.to {
            TypedObjectRef::Direct(_) => panic!(),
            TypedObjectRef::ByHash(hash) => {
                assert_eq!(hash, Hash::from(&[3; 32]));
            }
        }

        let right_results = vault
            .ask(LoadPinObjects {
                from: None,
                to: Some(Hash::from(&[3; 32])),
            })
            .send()
            .await
            .unwrap();

        assert_eq!(right_results.len(), 1);
        assert_eq!(right_results[0].from, Hash::from(&[2; 32]));
        let right_result = right_results[0].clone();
        match right_result.to {
            TypedObjectRef::Direct(_) => panic!(),
            TypedObjectRef::ByHash(hash) => {
                assert_eq!(hash, Hash::from(&[3; 32]));
            }
        }

        let all_results = vault
            .ask(LoadPinObjects {
                from: None,
                to: None,
            })
            .send()
            .await
            .unwrap();

        assert_eq!(all_results.len(), 1);
        assert_eq!(all_results[0].from, Hash::from(&[2; 32]));
        let all_result = all_results[0].clone();
        match all_result.to {
            TypedObjectRef::Direct(_) => panic!(),
            TypedObjectRef::ByHash(hash) => {
                assert_eq!(hash, Hash::from(&[3; 32]));
            }
        }
    }
}
