use mio::net::TcpStream;
use std::io::{Read, Write};

use super::error::{Error, RedisError};
const PROTO_IOBUF_LEN: i64 = 16 << 10;
use super::common::AM;

use super::common::OK;
use super::db::DB;
use std::io;

enum ReqType {
    Init,
    MULTIBULK,
    INLINE,
}

pub struct Client {
    pub id: usize,
    query_buf: Vec<u8>,
    arg: Vec<Vec<u8>>,
    stream: TcpStream,
    req_type: ReqType,
    multibulklen: i64,
    buf_len: i64,
    bulklen: i64,
    query_pos: i64,
    db: AM<DB>,
}

impl Client {
    pub fn new(stream: TcpStream, id: usize, db: AM<DB>) -> Client {
        Client {
            query_buf: vec![0u8; PROTO_IOBUF_LEN as usize],
            stream: stream,
            arg: vec![],
            id: id,
            req_type: ReqType::Init,
            multibulklen: 0,
            bulklen: -1,
            buf_len: 0,
            query_pos: 0,
            db: db,
        }
    }

    pub fn read_command(&mut self) -> Result<(), Error> {

        let size = self
            .stream
            .read(&mut self.query_buf[self.buf_len as usize..])?;

        if size == 0 {
            return Err(Error::Io(io::Error::new(io::ErrorKind::UnexpectedEof,"eof")));
        }


        self.buf_len += size as i64;
        return self.process_input_buffer();

    }

    fn process_input_buffer_each(&mut self) -> Result<bool, Error> {
        self.req_type = match self.req_type {
            ReqType::Init => {
                if self.query_buf[self.query_pos as usize] == '*' as u8 {
                    ReqType::MULTIBULK
                } else {
                    ReqType::INLINE
                }

            }
            ReqType::INLINE => ReqType::INLINE,
            ReqType::MULTIBULK => ReqType::MULTIBULK,
        };

        match self.req_type {
            ReqType::MULTIBULK => return self.process_multibulk_buffer(),
            ReqType::INLINE => return self.process_inline_buffer(),
            _ => panic!("nerver reached"),
        }
    }

    fn process_input_buffer(&mut self) -> Result<(), Error> {
        while self.query_pos < self.buf_len {
            if !self.process_input_buffer_each()? {
                return Ok(());
            }


            if self.arg.len() == 0 {
                self.req_type = ReqType::Init;
                self.multibulklen = 0;
                self.bulklen = -1;
                break;
            } else {
                self.process_command()?
            }
            self.req_type = ReqType::Init;

        }
        self.query_pos = 0;
        self.buf_len = 0;
        Ok(())
    }

    fn process_inline_buffer(&mut self) -> Result<bool, Error> {
        let buf = &self.query_buf[self.query_pos as usize..self.buf_len as usize];
        let mut idx = match buf.into_iter().position(|e| e == &('\r' as u8)) {
            Some(idx) => idx,
            _ => return Ok(false),
        };
        let mut linefeed_chars = 1;
        if self.query_buf[self.query_pos as usize + idx] == '\r' as u8 {
            idx -= 1;
            linefeed_chars += 1;
        }
        let querylen = idx as i64 - self.query_pos;
        self.query_pos += querylen + linefeed_chars;

        self.stream.write(OK.as_bytes())?;
        self.req_type = ReqType::Init;
        Ok(true)
    }

    fn process_multibulk_buffer(&mut self) -> Result<bool, Error> {
        if self.multibulklen == 0 {
            let buf = &self.query_buf[self.query_pos as usize..self.buf_len as usize];
            let idx = match buf.into_iter().position(|e| e == &('\r' as u8)) {
                Some(idx) => idx,
                _ => return Ok(false),
            };

            if self.query_buf[self.query_pos as usize] != '*' as u8 {
                return Ok(false);
            }


            if idx as i64 - self.query_pos > self.buf_len - self.query_pos - 2 {
                return Ok(false);
            }

            let low = (self.query_pos + 1) as usize;
            let high = (idx as i64 + self.query_pos) as usize;


            let mut multibulklen: i64 = 0;

            for &item in &self.query_buf[low..high] {
                if item >= '0' as u8 && item <= '9' as u8 {
                    multibulklen *= 10;
                    let cur = item - '0' as u8;
                    multibulklen += cur as i64;
                }
            }
            self.query_pos = self.query_pos + idx as i64 + 2;
            if multibulklen <= 0 {
                return Ok(true);
            }
            self.multibulklen = multibulklen;
            self.arg = Vec::with_capacity(multibulklen as usize);

        }

        while self.multibulklen != 0 {

            if self.bulklen == -1 {

                let buf = &self.query_buf[self.query_pos as usize..self.buf_len as usize];
                let idx = match buf.into_iter().position(|e| e == &('\r' as u8)) {
                    Some(idx) => idx,
                    _ => break,
                };

                if idx as i64 - self.query_pos > self.buf_len - self.query_pos - 2 {
                    break;
                }

                if self.query_buf[self.query_pos as usize] != '$' as u8 {
                    return Ok(false);
                }

                let low = (self.query_pos + 1) as usize;
                let high = (idx as i64 + self.query_pos) as usize;


                let mut bulklen: i64 = 0;

                for &item in &self.query_buf[low..high] {
                    if item >= '0' as u8 && item <= '9' as u8 {
                        bulklen *= 10;
                        let cur = item - '0' as u8;
                        bulklen += cur as i64;
                    }
                }


                self.query_pos = self.query_pos + idx as i64 + 2;
                self.bulklen = bulklen;
            }


            if self.buf_len - self.query_pos < self.bulklen + 2 {
                break;
            }


            let mut buf = Vec::with_capacity(self.bulklen as usize);
            buf.extend_from_slice(
                &self.query_buf
                    [self.query_pos as usize..self.query_pos as usize + self.bulklen as usize],
            );

            self.arg.push(buf);
            self.query_pos += self.bulklen + 2;

            self.bulklen = -1;
            self.multibulklen -= 1;

        }
        return Ok(self.multibulklen == 0);
    }

    fn process_command(&mut self) -> Result<(), Error> {
        let mut db = self.db.lock().unwrap();

        let resp = db.process_command(&self.arg)?;
        self.stream.write(resp)?;
        Ok(())
    }
}