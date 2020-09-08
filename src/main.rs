use tokio::net::TcpListener;
mod config;
use config::parse_config;
mod client;
use client::Client;
mod error;
use error::Error;
mod common;
mod db;
use db::DB;
use std::sync::{Arc, Mutex};

#[tokio::main]
async fn main() -> Result<(), Error> {
    let config = parse_config()?;
    let db = Arc::new(Mutex::new(DB::new()));
    let mut listener = TcpListener::bind(config.addr).await?;
    let mut client_id: i64 = 0;
    while let Ok((conn, _)) = listener.accept().await {
        client_id += 1;
        let db = db.clone();

        tokio::spawn(async move {
            let mut client = Client::new(client_id, conn, db);
            if let Err(err) = client.serve().await {
                eprintln!(
                    "failed to serve for client:{:?}; err = {:?}",
                    client_id, err
                );
                return;
            }
        });
    }
    Ok(())
}
