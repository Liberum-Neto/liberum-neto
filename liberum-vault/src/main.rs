use anyhow::Result;
use fragment::Fragment;
use tokio::sync::mpsc;
use tokio_rusqlite::Connection;

pub mod fragment;
pub mod vault;

#[tokio::main]
async fn main() -> Result<()> {
    let db = Connection::open("db").await.unwrap();
    let (tx, rx) = mpsc::channel(32);

    vault::prepare_db(&db).await?;
    let _ = vault::handle_queries(db, rx).await;

    for i in 0..1000 {
        match vault::store_fragment(&tx, Fragment::random()).await {
            Ok(_) => println!("{i} Stored!"),
            Err(e) => println!("Some error! {e}"),
        }
    }

    Ok(())
}
