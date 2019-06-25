extern crate rand;
use tokio::net::{TcpListener, TcpStream};

use net2::TcpBuilder;
use tokio::prelude::*;

use rand::prelude::*;
use std::net::SocketAddr;
use tokio::runtime::current_thread;
mod client;
use client::Client;

mod codec;
use codec::Codec;

mod common;
mod error;

mod server;
use server::Server;


use std::sync::{Arc, Mutex};

use net2::unix::UnixTcpBuilderExt;

use common::AM;
use error::Error;
use tokio::codec::Decoder;

use std::thread;

use num_cpus;
mod config;
use config::parse_config;
mod db;
use db::DB;


fn main() -> Result<(), Error> {

    let config = parse_config();
    let addr = config.addr.parse()?;

    let server = Arc::new(Mutex::new(Server::new()));
    let db = Arc::new(Mutex::new(DB::new()));

    let num = num_cpus::get();
    let mut threads: Vec<_> = Vec::with_capacity(num);
    for _ in 0..num {
        //let is_main_thread = id == num - 1;

        let listener = create_reuse_port_listener(&addr)?;
        let db = db.clone();
        let server = server.clone();

        let fut = listener
            .incoming()
            .for_each(move |sock| {
                accept_conn(sock, db.clone(), server.clone());
                Ok(())
            })
            .map_err(|err| {
                eprintln!("got err {:?}", err);
            });

        let thread = thread::spawn(move || current_thread::block_on_all(fut).unwrap());
        threads.push(thread);
    }

    for t in threads {
        match t.join() {
            Ok(_) => {}
            Err(e) => eprintln!("join err:{:?}", e),
        };
    }

    Ok(())

}

pub fn create_reuse_port_listener(addr: &SocketAddr) -> Result<TcpListener, std::io::Error> {
    let builder = TcpBuilder::new_v4()?;
    let std_listener = builder
        .reuse_address(true)?
        .reuse_port(true)?
        .bind(addr)?
        .listen(65535)?;
    let hd = tokio::reactor::Handle::default();
    TcpListener::from_std(std_listener, &hd)
}

fn accept_conn(sock: TcpStream, db: AM<DB>, server: AM<Server>) {
    sock.set_nodelay(true).unwrap();
    let mut rng = rand::thread_rng();
    let id: i64 = rng.gen();
    let codec = Codec::new();
    let (sink, stream) = codec.framed(sock).split();
    let client = Client::new(sink, stream, id, db, server).map_err(|err| eprintln!("err{:?}", err));
    current_thread::spawn(client);
}