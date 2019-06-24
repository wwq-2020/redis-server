use std::io;
use tokio::net::{TcpListener, TcpStream};
use tokio::prelude::*;
mod client;
use client::Client;
use std::sync::{Arc, Mutex};

use net2::unix::UnixTcpBuilderExt;
use net2::TcpBuilder;

use std::net::SocketAddr;

mod common;
mod db;
use db::DB;
mod error;
pub const OK: &str = "+OK\r\n";

fn main() {
    let addr: SocketAddr = "0.0.0.0:6379".parse().unwrap();
    let listener = TcpListener::bind(&addr).expect("unable to bind TCP listener");

    let db = Arc::new(Mutex::new(DB::new()));
    let client_id = Arc::new(Mutex::new(1));
    let server = listener
        .incoming()
        .map_err(|e| eprintln!("accept failed = {:?}", e))
        .for_each(move |sock| {
            let db = db.clone();
            let mut client_id = client_id.lock().unwrap();
            *client_id += 1;

            let client = Client::new(sock, *client_id, db).map_err(|err| eprintln!("err{:?}", err));
            tokio::spawn(client);
            Ok(())
        });

    tokio::run(server);
}