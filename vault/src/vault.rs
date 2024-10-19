use anyhow::anyhow;
use anyhow::Result;
use rusqlite::OptionalExtension;
use tokio::task::JoinHandle;
use tokio_rusqlite::Connection;
use tokio::sync::{oneshot, mpsc};

use crate::fragment::key::Key;
use crate::fragment::Fragment;

type StoreResult = Result<()>;
type GetResult = Result<Option<Fragment>>;
type RemoveResult = Result<()>;

type StoreResponder = tokio::sync::oneshot::Sender<StoreResult>;
type GetResponder = tokio::sync::oneshot::Sender<GetResult>;
type RemoveResponder = tokio::sync::oneshot::Sender<RemoveResult>;

pub enum Query {
    Store {
        fragment: Fragment,
        resp: StoreResponder,
    },
    Get {
        key: Key,
        resp: GetResponder,
    },
    Remove {
        key: Key,
        resp: RemoveResponder,
    },
}

pub async fn prepare_db(db: &Connection) -> Result<()> {
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

    db.call(|conn| {
        conn.execute(CREATE_FRAGMENT_TABLE_QUERY, ())?;

        Ok(())
    })
    .await?;

    Ok(())
}

pub async fn handle_queries(db: Connection, mut queries: mpsc::Receiver<Query>) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            if let Some(query) = queries.recv().await {
                handle_query(&db, query).await;
            } else {
                break;
            }
        }
    })
}

async fn handle_query(db: &Connection, query: Query) {
    match query {
        Query::Store { fragment, resp } => handle_store(db, fragment, resp).await,
        Query::Get { key, resp } => handle_get(db, key, resp).await,
        Query::Remove { key, resp } => handle_remove(db, key, resp).await,
    }
}

async fn handle_store(db: &Connection, fragment: Fragment, resp: StoreResponder) {
    const INSERT_FRAGMENT_QUERY: &str = "
        INSERT INTO fragment (hash0, hash1, hash2, hash3, path, size)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
    ";
    let hash_as_u64 = fragment.hash.as_u64_slice_be();

    let result = db
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
        .await;

    let _ = resp.send(result.map_err(|e| anyhow!(e)));
}

async fn handle_get(db: &Connection, key: Key, resp: GetResponder) {
    const SELECT_FRAGMENT_QUERY: &str = "
        SELECT path, size
        FROM fragment
        WHERE hash0 = ?1 AND hash1 = ?2 AND hash2 = ?3 AND hash3 = ?4
    ";

    let result = db
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
        .await;

    let _ = resp.send(result.map_err(|e| anyhow!(e)));
}

async fn handle_remove(_: &Connection, _: Key, _: RemoveResponder) {
    todo!()
}

pub async fn store_fragment(queries: &mpsc::Sender<Query>, fragment: Fragment) -> Result<()> {
    let (tx, rx) = oneshot::channel();

    queries.send(Query::Store { fragment, resp: tx }).await.map_err(|e| anyhow!(e))?;

    rx.await.map_err(|e| anyhow!(e))?
}
