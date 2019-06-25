
use super::common::AM;
use super::db::{Command, DB};
use futures::{try_ready, Sink, Stream};
use std::collections::VecDeque;
use std::fmt::Debug;
use tokio::prelude::*;

use super::server::Server;


use super::error::Error;

pub struct Client<I, O, S, R>
where
    I: Stream<Item = S, Error = Error>,
    O: Sink<SinkItem = R, SinkError = Error>,
    S: Command<R, Error>,
{
    sink: O,
    stream: I,
    cmds: VecDeque<S>,
    _id: i64,
    pending: bool,
    db: AM<DB>,
    _server: AM<Server>,
}


impl<I, O, S, R> Client<I, O, S, R>
where
    I: Stream<Item = S, Error = Error>,
    O: Sink<SinkItem = R, SinkError = Error>,
    S: Command<R, Error>,
{
    pub fn new(sink: O, stream: I, id: i64, db: AM<DB>, server: AM<Server>) -> Client<I, O, S, R> {
        Client {
            sink: sink,
            stream: stream,
            _id: id,
            cmds: VecDeque::new(),
            pending: false,
            db: db,
            _server: server,
        }

    }
}

impl<I, O, S, R> Future for Client<I, O, S, R>
where
    I: Stream<Item = S, Error = Error>,
    O: Sink<SinkItem = R, SinkError = Error>,
    S: Command<R, Error> + Debug,
{
    type Item = ();
    type Error = Error;
    fn poll(&mut self) -> Poll<(), Error> {
        // Ok(Async::Ready(()))

        let mut can_read = true;
        let mut can_send = true;
        loop {
            if !can_read && !can_send {
                return Ok(Async::NotReady);
            }

            if can_read {
                loop {
                    match self.stream.poll() {
                        Ok(Async::Ready(Some(req))) => {
                            self.cmds.push_back(req);
                        }
                        Ok(Async::Ready(None)) => {
                            can_read = false;
                            break;
                        }
                        Ok(Async::NotReady) => {
                            can_read = false;
                            break;
                        }
                        Err(e) => {
                            return Err(e);
                        }
                    }
                }

            }
            if self.cmds.is_empty() {
                can_send = false;
                continue;
            }
            if can_send {
                while !self.cmds.is_empty() {
                    let mut req = self.cmds.pop_front().unwrap();
                    let resp = req.exec(self.db.clone())?;
                    match self.sink.start_send(resp)? {
                        AsyncSink::NotReady(_) => {
                            can_send = false;
                            break;
                        }
                        AsyncSink::Ready => {

                            self.pending = true;
                        }
                    }
                }
                if self.pending {

                    try_ready!(self.sink.poll_complete());
                    self.pending = false;
                }
            }
        }

    }
}