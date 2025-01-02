pub mod key;

use std::path::Path;
use std::path::PathBuf;

use anyhow::anyhow;
use anyhow::Result;
use kameo::mailbox::bounded::BoundedMailbox;
use kameo::messages;
use kameo::Actor;
use key::Key;
use liberum_core::parser::parse_typed;
use liberum_core::parser::ObjectEnum;
use liberum_core::proto::Hash;
use liberum_core::proto::TypedObject;
use liberum_core::types::TypedObjectInfo;
use rusqlite::params;
use tokio_rusqlite::Connection;
use tracing::{debug, error};

pub struct Vaultv3 {
    db: Connection,
}

impl Actor for Vaultv3 {
    type Mailbox = BoundedMailbox<Self>;

    async fn on_start(
        &mut self,
        _: kameo::actor::ActorRef<Self>,
    ) -> std::result::Result<(), kameo::error::BoxError> {
        let prepare_result = self.prepare_db().await;
        prepare_result.inspect_err(|e| error!(err = format!("{e}"), "Failed to prepare DB"))?;

        Ok(())
    }
}

#[messages]
impl Vaultv3 {
    // funkcjonalność signedObject
    #[message]
    pub async fn store_object(
        &self,
        hash: Hash,
        object: TypedObject, /* tylko taki obiekt można zapisać w db */
    ) -> Result<bool> /*true if added, false if exist*/ {
        let key: Key = hash.bytes.into();
        self.store_signed_object(key, object).await
    }
    #[message]
    pub async fn retrieve_object(&self, hash: Hash) -> Result<Option<TypedObject>> {
        let key: Key = hash.bytes.into();
        self.load_signed_object(key).await
    }

    #[message]
    pub async fn delete_object(&self, hash: Hash) -> Result<bool> {
        let key: Key = hash.bytes.into();
        self.delete_signed_object(key).await
    }

    // funkcjonalność pin'a
    #[message]
    pub async fn store_pin(
        &self,
        main_object_hash: Hash,
        from_object_hash: Hash,
        relation_object_hash: Option<Hash>,
    ) -> Result<()> {
        let main_object_hash: Key = main_object_hash.bytes.into();
        let from_object_hash: Key = from_object_hash.bytes.into();
        let relation_object_hash: Option<Key> = if let Some(relation) = relation_object_hash {
            Some(relation.bytes.into())
        } else {
            None
        };
        self.store_pin_object(main_object_hash, from_object_hash, relation_object_hash)
            .await
    }

    // helper
    // jak main_object_hashes == None return all matches, if Some then return subset of input that matches
    #[message]
    pub async fn matching_pins(
        &self,
        main_object_hashes: Option<Vec<Hash>>,
        from_object_hash: Option<Hash>,
        relation_object_hash: Option<Hash>,
    ) -> Result<Vec<Hash>> {
        let main_object_hashes: Option<Vec<Key>> =
            main_object_hashes.map(|hashes| hashes.iter().map(|hash| hash.bytes.into()).collect());
        let from_object_hash: Option<Key> = from_object_hash.map(|hash| hash.bytes.into());
        let relation_object_hash: Option<Key> = relation_object_hash.map(|hash| hash.bytes.into());

        Ok(self
            .matching_pins_internal(main_object_hashes, from_object_hash, relation_object_hash)
            .await?
            .iter()
            .map(|key| key.to_hash().unwrap())
            .collect())
    }

    #[message]
    pub async fn list_objects(&self) -> Result<Vec<TypedObjectInfo>> {
        todo!()
    }
}

impl Vaultv3 {
    const DEFAULT_VAULT_DATABASE_NAME: &'static str = "vault3.db3";

    pub async fn new_on_disk(vault_dir_path: &Path) -> Result<Vaultv3> {
        Self::ensure_dirs(vault_dir_path).await?;

        let db_path = Self::default_db_path(vault_dir_path);
        let db = Connection::open(db_path).await?;

        Ok(Vaultv3 { db })
    }

    pub async fn new_in_memory() -> Result<Vaultv3> {
        let db = Connection::open_in_memory().await?;

        Ok(Vaultv3 { db })
    }

    async fn prepare_db(&self) -> Result<()> {
        const CREATE_TYPED_OBJECT_TABLE: &str = "
            CREATE TABLE IF NOT EXISTS typed_object (
                hash TEXT NOT NULL PRIMARY KEY,
                data BLOB NOT NULL
            )
        ";

        self.db
            .call(|conn| Ok(conn.execute(CREATE_TYPED_OBJECT_TABLE, ())?))
            .await?;

        const CREATE_PIN_OBJECT_TABLE: &str = "
            CREATE TABLE IF NOT EXISTS pin_object (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                main_object_hash TEXT NOT NULL,
                from_object_hash TEXT NOT NULL,
                relation_object_hash TEXT
            );
            CREATE INDEX pin_object_hash_from_idx ON pin_object (hash_from);
            CREATE INDEX pin_object_hash_to_idx ON pin_object (hash_to)
        ";

        self.db
            .call(|conn| Ok(conn.execute(CREATE_PIN_OBJECT_TABLE, ())?))
            .await?;

        Ok(())
    }

    async fn delete_signed_object(&self, key: Key) -> Result<bool> {
        const SELECT_TYPED_OBJECT_QUERY: &str = "
        DELETE * FROM typed_object
        WHERE hash = ?1
    ";

        let is_success = self
            .db
            .call(move |conn| {
                let mut stmt = conn.prepare(SELECT_TYPED_OBJECT_QUERY)?;
                let key_as_string = key.as_base58();
                let row_affected = stmt.execute(params![key_as_string])?;
                Ok(row_affected > 0)
            })
            .await
            .map_err(|e| anyhow!(e))?;

        Ok(is_success)
    }

    async fn load_signed_object(&self, key: Key) -> Result<Option<TypedObject>> {
        const SELECT_TYPED_OBJECT_QUERY: &str = "
            SELECT data
            FROM typed_object
            WHERE hash = ?1
        ";

        let db_response = self
            .db
            .call(move |conn| {
                let mut stmt = conn.prepare(SELECT_TYPED_OBJECT_QUERY)?;
                let key_as_string = key.as_base58();

                let object_content = stmt.query_row([key_as_string], |r| {
                    let data: Vec<u8> = r.get(0)?;
                    return Ok(data);
                })?;
                return Ok(object_content);
            })
            .await
            .map_err(|e| anyhow!(e))?;
        Ok(Some(TypedObject::try_from(&db_response)?))
    }

    async fn store_signed_object(&self, key: Key, object: TypedObject) -> Result<bool> {
        const SELECT_TYPED_OBJECT_QUERY: &str = "SELECT COUNT(*) FROM typed_object WHERE hash = ?1";
        const INSERT_TYPED_OBJECT_QUERY: &str = "INSERT INTO typed_object (hash, data)
             VALUES (?1, ?2)";

        let key_as_string = key.as_base58();

        let cnt = self
            .db
            .call(move |conn| {
                let cnt = conn.query_row(SELECT_TYPED_OBJECT_QUERY, [key_as_string], |r| {
                    let cnt: usize = r.get(0)?;
                    Ok(cnt)
                })?;

                Ok(cnt)
            })
            .await
            .inspect_err(|e| error!(e = format!("{e}"), "QUERY ERROR"))?;

        if cnt != 0 {
            // Already stored, no need to change as objects are immutable
            return Ok(false);
        }
        let typed: TypedObject;

        if let ObjectEnum::Signed(signed) = parse_typed(object)
            .await
            .inspect_err(|e| error!(err = format!("{e}"), "parse error"))?
        {
            typed = signed.into();
        } else {
            return Err(anyhow!("Object was not a SignedObject"));
        }

        let key_as_string = key.as_base58();
        let data: Vec<u8> = typed
            .try_into()
            .inspect_err(|e| error!(err = format!("{e}"), "serialize error"))?;

        self.db
            .call(move |conn| {
                conn.execute(INSERT_TYPED_OBJECT_QUERY, (key_as_string, data))
                    .inspect_err(|e| error!(err = format!("{e}"), "STORE ERROR 1"))?;
                Ok(())
            })
            .await
            .inspect_err(|e| error!(e = format!("{e}"), "STORE ERROR 2"))
            .map_err(|e| anyhow!(e))?;
        Ok(true)
    }

    async fn store_pin_object(
        &self,
        main_object_hash: Key,
        from_object_hash: Key,
        relation_object_hash: Option<Key>,
    ) -> Result<()> {
        const INSERT_PIN_OBJECT_QUERY: &str = "
            INSERT INTO pin_object (main_object_hash, from_object_hash, relation_object_hash)
            VALUES (?1, ?2, ?3)
        ";

        let main_object_hash = main_object_hash.as_base58();
        let from_object_hash = from_object_hash.as_base58();
        let relation_object_hash = if let Some(relation) = relation_object_hash {
            relation.as_base58()
        } else {
            "NULL".to_string()
        };

        self.db
            .call(move |conn| {
                conn.execute(
                    INSERT_PIN_OBJECT_QUERY,
                    params![main_object_hash, from_object_hash, relation_object_hash],
                )?;

                Ok(())
            })
            .await
            .map_err(|e| anyhow!(e))
    }

    pub async fn matching_pins_internal(
        &self,
        main_object_hashes: Option<Vec<Key>>,
        from_object_hash: Option<Key>,
        relation_object_hash: Option<Key>,
    ) -> Result<Vec<Key>> {
        let select = "SELECT main_object_hash From pin_object ";
        // main_object_hash in ?1
        // && from_object_hash = ?2
        // relation_object_hash = ?3
        let group = " GROUP BY main_object_hash";
        let result: Vec<String> = self.db.call(move |conn| {

    let (query,params) = match (main_object_hashes, from_object_hash,relation_object_hash) {
    (None, None, None) => ("", params![]),
    (None, None, Some(relation)) => ("WHERE relation_object_hash = ?1",params![relation.as_base58()]),
    (None, Some(from), None) => ("WHERE from_object_hash = ?1",params![from.as_base58()]),
    (None, Some(from), Some(relation)) => ("WHERE from_object_hash = ?1 && relation_object_hash = ?2",params![from.as_base58(),relation.as_base58()]),
    (Some(scope), None, None) => ("WHERE main_object_hash in ?1",params![scope_to_string(scope)]),
    (Some(scope), None, Some(relation)) => ("WHERE main_object_hash in ?1 && relation_object_hash = ?2",params![scope_to_string(scope),relation.as_base58()]),
    (Some(scope), Some(from), None) => ("WHERE main_object_hash in ?1 && from_object_hash = ?2",params![scope_to_string(scope),from.as_base58()]),
    (Some(scope), Some(from), Some(relation)) => ("WHERE main_object_hash in ?1 && from_object_hash = ?2 && from_object_hash = ?3",params![scope_to_string(scope),from.as_base58(),relation.as_base58()]),
    };

    let query = format!("{select}{query}{group};");

        let mut statement = conn.prepare(&query)?;
        let matching_keyes = statement.query_map(params,|row| row.get::<usize,String>(0))?;

        let mut key_string = Vec::new();

        for key in matching_keyes {
            key_string.push(key?);
        }

        Ok(key_string)
    }).await?;

        let mut keys = Vec::new();

        for key in result {
            let key = Key::from_base58(key).map_err(|err| anyhow!(err))?;
            keys.push(key);
        }

        return Ok(keys);
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

fn scope_to_string(keyes: Vec<Key>) -> String {
    let inner = keyes
        .iter()
        .map(|key| key.as_base58())
        .collect::<Vec<_>>()
        .join(", ");
    format! {"[{inner}]"}
}
