use mio::net::TcpListener;
use mio::*;
use std::collections::HashMap;
use std::io::ErrorKind::WouldBlock;

const MAX_ACCEPTS_PER_CALL: i32 = 1000;
const SERVER_TOKEN: Token = Token(0);
use std::os::unix::io::AsRawFd;

use super::client::Client;

use super::common::AM;
use super::db::DB;
use super::error::Error;
use super::server::Server;

use std::io;

pub struct Worker {
    listener: TcpListener,
    server_id: usize,
    client_id: usize,
    conns: HashMap<usize, Client>,
    poll: Poll,
    server: AM<Server>,
    db: AM<DB>,
    is_main_thread: bool,
}

impl Worker {
    pub fn new(
        listener: TcpListener,
        server: AM<Server>,
        db: AM<DB>,
        server_id: usize,
        is_main_thread: bool,
    ) -> Result<Worker, Error> {
        let poll = Poll::new()?;
        poll.register(&listener, SERVER_TOKEN, Ready::readable(), PollOpt::level())?;
        Ok(Worker {
            listener: listener,
            poll: poll,
            server_id: server_id,
            client_id: 0,
            conns: HashMap::new(),
            server: server,
            db: db,
            is_main_thread: is_main_thread,
        })
    }

    fn cluster_before_sleep(&mut self) {}

    fn cluster_before_sleep_if_needed(&mut self) {
        let server = self.server.lock().unwrap();
        if server.cluster_enabled {
            drop(server);
            self.cluster_before_sleep();
        }
    }

    fn before_sleep_main(&mut self) {
        self.cluster_before_sleep_if_needed();
        self.flush_appendonly_file(false);
        self.before_sleep_lite();
    }

    fn flush_appendonly_file(&mut self, _force: bool) {}

    fn free_clients_in_async_free_queue(&mut self) {}

    fn before_sleep_lite(&mut self) {
        self.handle_clients_with_pending_writes();
        self.free_clients_in_async_free_queue();
    }

    fn before_sleep(&mut self) {
        if self.is_main_thread {
            self.before_sleep_main();
        } else {
            self.before_sleep_lite();
        }

    }

    fn handle_clients_with_pending_writes(&mut self) {}

    fn after_sleep(&mut self) {}


    fn accept(&mut self) -> io::Result<()> {
        let (stream, _) = self.listener.accept()?;
        let token = stream.as_raw_fd() as usize;
        self.poll
            .register(&stream, Token(token), Ready::readable(), PollOpt::level())?;

        let db = self.db.clone();
        self.conns
            .insert(token, Client::new(stream, self.client_id, db));
        self.client_id += 1;
        Ok(())
    }


    fn on_accept(&mut self) {
        let mut max = MAX_ACCEPTS_PER_CALL;
        for _ in 0..MAX_ACCEPTS_PER_CALL {
            match self.accept() {
                Ok(_) => max -= 1,
                Err(e) => match e.kind() {
                    WouldBlock => return,
                    _ => {
                        println!("server_id:{:?}, on_accept err:{:?}", self.server_id, e);
                        return;
                    }
                },
            };
        }
    }


    fn on_data(&mut self, token: usize) {
        let client = match self.conns.get_mut(&token) {
            Some(client) => client,
            None => {
                println!("got unexpected client:{:?}", token);
                return;
            }
        };
        match client.read_command() {
            Err(Error::Io(e)) => {
                match e.kind() {
                    io::ErrorKind::UnexpectedEof => {}
                    _ => {
                        println!("client:{:?} got err:{:?}", client.id, e);
                    }
                }
                self.conns.remove(&token);
            }


            Err(e) => {
                println!("client:{:?} got err:{:?}", client.id, e);
                self.conns.remove(&token);
            }
            _ => return,
        }
    }

    fn on_event(&mut self, event: Event) {
        match event.token() {
            SERVER_TOKEN => {
                self.on_accept();
            }

            Token(token) => self.on_data(token),
        }
    }

    pub fn run(&mut self) {
        let mut events = Events::with_capacity(1024);
        loop {
            self.poll.poll(&mut events, None).unwrap();
            self.after_sleep();
            events.iter().for_each(|event| self.on_event(event));
            self.before_sleep();
        }
    }
}
