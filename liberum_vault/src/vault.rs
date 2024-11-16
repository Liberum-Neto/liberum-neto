use std::path::Path;
use std::path::PathBuf;

use anyhow::anyhow;
use anyhow::Result;
use kameo::mailbox::bounded::BoundedMailbox;
use kameo::message::{Context, Message};
use kameo::Actor;
use rusqlite::OptionalExtension;
use tokio::fs::File;
use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncSeekExt;
use tokio::io::Take;
use tokio_rusqlite::Connection;
use tokio_util::io::ReaderStream;

use crate::fragment;
use crate::fragment::key::Key;
use crate::fragment::Fragment;

pub struct Vault {
    db: Connection,
    vault_dir_path: PathBuf,
}

pub struct StoreFragment(Box<dyn AsyncRead + Send>);
pub struct Store(pub Fragment);
pub struct Get(pub Key);
pub struct Remove(pub Key);

impl Vault {
    pub async fn new(db_path: &Path, vault_dir_path: &Path) -> Result<Vault> {
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

    async fn load_fragment_info(&self, key: Key) -> Result<Option<Fragment>> {
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

    async fn store_fragment_info(&self, fragment: Fragment) -> Result<()> {
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
        msg: StoreFragment,
        ctx: Context<'_, Self, Self::Reply>,
    ) -> Self::Reply {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use tempdir::TempDir;
    use tokio::io::AsyncWriteExt;
    use tokio_stream::StreamExt;

    use super::*;

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

        println!("{:?}", stream_contents);
        assert!(stream_contents.iter().all(|b| *b == 65));

        stream_contents = Vec::new();
        while let Some(chunk) = fragments[1].next().await {
            stream_contents.extend_from_slice(&chunk.unwrap());
        }

        println!("{:?}", stream_contents);
        assert!(stream_contents.iter().all(|b| *b == 66));
    }
}
