use std::path::Path;

use anyhow::anyhow;
use anyhow::Result;
use kameo::mailbox::bounded::BoundedMailbox;
use kameo::message::{Context, Message};
use kameo::Actor;
use rusqlite::OptionalExtension;
use tokio_rusqlite::Connection;

use crate::fragment::key::Key;
use crate::fragment::Fragment;

pub struct Vault {
    db: Connection,
}

pub struct Store(pub Fragment);
pub struct Get(pub Key);
pub struct Remove(pub Key);

impl Vault {
    pub async fn new(db_path: &Path) -> Result<Vault> {
        let db = Connection::open(db_path).await?;

        Ok(Vault { db })
    }

    pub async fn in_memory() -> Result<Vault> {
        let db = Connection::open_in_memory().await?;

        Ok(Vault { db })
    }
}

impl Actor for Vault {
    type Mailbox = BoundedMailbox<Self>;

    async fn on_start(
        &mut self,
        _: kameo::actor::ActorRef<Self>,
    ) -> std::result::Result<(), kameo::error::BoxError> {
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
}

impl Message<Store> for Vault {
    type Reply = Result<()>;

    async fn handle(
        &mut self,
        Store(fragment): Store,
        _: Context<'_, Self, Self::Reply>,
    ) -> Self::Reply {
        const INSERT_FRAGMENT_QUERY: &str = "
                INSERT INTO fragment (hash0, hash1, hash2, hash3, path, size)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ";
        let hash_as_u64 = fragment.hash.as_u64_slice_be();

        self.db
            .call(move |conn| {
                conn.execute(
                    INSERT_FRAGMENT_QUERY,
                    (
                        hash_as_u64[0] as i64,
                        hash_as_u64[1] as i64,
                        hash_as_u64[2] as i64,
                        hash_as_u64[3] as i64,
                        fragment.path,
                        fragment.size,
                    ),
                )?;

                Ok(())
            })
            .await
            .map_err(|e| anyhow!(e))
    }
}

impl Message<Get> for Vault {
    type Reply = Result<Option<Fragment>>;

    async fn handle(&mut self, Get(key): Get, _: Context<'_, Self, Self::Reply>) -> Self::Reply {
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

                        Result::Ok(Fragment::new(key, path, size))
                    })
                    .optional()?;

                Ok(fragment)
            })
            .await
            .map_err(|e| anyhow!(e))
    }
}

impl Message<Remove> for Vault {
    type Reply = Result<()>;

    async fn handle(&mut self, _: Remove, _: Context<'_, Self, Self::Reply>) -> Self::Reply {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use kameo::request::MessageSend;

    use super::*;

    #[tokio::test]
    async fn basic_test() {
        let vault_ref = kameo::spawn(Vault::in_memory().await.unwrap());
        vault_ref.ask(Store(Fragment::random())).send().await.unwrap();
    }
}
