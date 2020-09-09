use std::io;
use std::net;

#[derive(Debug)]

pub struct RedisError {
    msg: String,
}

impl RedisError {
    pub fn new(msg: String) -> RedisError {
        RedisError { msg: msg }
    }
}

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    NetParse(net::AddrParseError),
    Redis(RedisError),
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error::Io(err)
    }
}

impl From<net::AddrParseError> for Error {
    fn from(err: net::AddrParseError) -> Error {
        Error::NetParse(err)
    }
}
