use mio::net::TcpListener;
// use num_cpus;
use std::sync::{Arc, Mutex};
//use std::thread;


mod worker;
use worker::Worker;

mod client;

mod server;
use server::Server;

mod config;
use config::parse_config;

mod db;
use db::DB;

mod error;
use error::Error;
mod common;

fn main() -> Result<(), Error> {
    let config = parse_config();
    let addr = config.addr.parse()?;
    let listener = TcpListener::bind(&addr)?;

    let server = Arc::new(Mutex::new(Server::new()));
    let db = Arc::new(Mutex::new(DB::new()));
    let mut worker = Worker::new(listener, server, db, 1, true).unwrap();
    worker.run();
    // let num = num_cpus::get();
    // let mut threads: Vec<_> = Vec::with_capacity(num);
    // for id in 0..num {
    //     let is_main_thread = id == num - 1;
    //     let listener = listener.try_clone()?;
    //     let server = server.clone();
    //     let db = db.clone();
    //     let mut worker = Worker::new(listener, server, db, id, is_main_thread)?;
    //     let thread = thread::spawn(move || worker.run());
    //     threads.push(thread);
    // }

    // for t in threads {
    //     match t.join() {
    //         Ok(_) => {}
    //         Err(e) => println!("join err:{:?}", e),
    //     };
    // }

    Ok(())
}