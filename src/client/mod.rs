use super::common::{MyResult, MySimpleResult, AM};
use super::db::{CmdReq, DB};
use super::error::{Error, RedisError};
use memchr::memchr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

pub struct Client {
    id: i64,
    conn: TcpStream,
    read_buf: Vec<u8>,
    read_pos: usize,
    data_pos: usize,
    req_type: ReqType,
    multibulklen: i64,
    bulklen: i64,
    db: AM<DB>,
    req: CmdReq,
}

enum ReqType {
    Init,
    MULTIBULK,
    INLINE,
}

impl Client {
    pub fn new(id: i64, conn: TcpStream, db: AM<DB>) -> Client {
        Client {
            id: id,
            conn: conn,
            read_buf: [0_u8; 4096].to_vec(),
            read_pos: 0,
            data_pos: 0,
            req_type: ReqType::Init,
            multibulklen: 0,
            bulklen: -1,
            db: db,
            req: CmdReq::new(),
        }
    }
}

impl Client {
    pub async fn serve(&mut self) -> MySimpleResult {
        loop {
            let n = self.conn.read(&mut self.read_buf).await?;
            if n == 0 {
                return Err(Error::Redis(RedisError::new(format!(
                    "client_id:{:?} conn closed",
                    self.id
                ))));
            }
            self.data_pos += n;
            self.req_type = match self.req_type {
                ReqType::Init => {
                    if self.read_buf[self.read_pos] == '*' as u8 {
                        ReqType::MULTIBULK
                    } else {
                        ReqType::INLINE
                    }
                }
                ReqType::INLINE => ReqType::INLINE,
                ReqType::MULTIBULK => ReqType::MULTIBULK,
            };
            let cmd_req = match self.req_type {
                ReqType::MULTIBULK => self.process_multibulk_buffer()?,
                ReqType::INLINE => self.process_inline_buffer()?,
                _ => panic!("nerver reached"),
            };
            match cmd_req {
                Some(mut cmd_req) => {
                    let cmd_resp = cmd_req.exec(self.db.clone())?;
                    self.conn.write_all(cmd_resp.as_bytes()).await?;
                    self.conn.flush().await?;
                    self.data_pos = self.data_pos - self.read_pos;
                    self.read_pos = 0;
                }
                None => continue,
            }
        }
    }

    fn process_inline_buffer(&mut self) -> MyResult<Option<CmdReq>> {
        Ok(Some(CmdReq::new()))
    }

    fn process_multibulk_buffer(&mut self) -> MyResult<Option<CmdReq>> {
        if self.multibulklen == 0 {
            let idx = match memchr('\r' as u8, self.cur_data()) {
                Some(idx) => idx,
                _ => {
                    return Ok(None);
                }
            };

            if self.peek() != '*' as u8 {
                return Err(Error::Redis(RedisError::new(format!(
                    "client_id:{:?} protocol err, multibulklen start char not *",
                    self.id
                ))));
            }

            if idx == self.read_buf.len() - 1 {
                return Ok(None);
            }

            let mut multibulklen: i64 = 0;

            for &item in &self.read_buf[..idx] {
                if item >= '0' as u8 && item <= '9' as u8 {
                    multibulklen *= 10;
                    let cur = item - '0' as u8;
                    multibulklen += cur as i64;
                }
            }
            self.read_pos = self.read_pos + idx + 2;
            if multibulklen <= 0 {
                return Ok(None);
            }
            self.multibulklen = multibulklen;
            self.req.args = Vec::with_capacity(multibulklen as usize);
        }

        while self.multibulklen != 0 {
            if self.bulklen == -1 {
                let idx = match memchr('\r' as u8, self.cur_data()) {
                    Some(idx) => idx,
                    _ => {
                        return Ok(None);
                    }
                };
                if self.peek() != '$' as u8 {
                    return Err(Error::Redis(RedisError::new(format!(
                        "client_id:{:?} protocol err, bulklen start char not $",
                        self.id
                    ))));
                }
                if idx == self.cur_data().len() - 1 {
                    return Ok(None);
                }

                let mut bulklen: i64 = 0;

                for &item in &self.cur_data()[..idx] {
                    if item >= '0' as u8 && item <= '9' as u8 {
                        bulklen *= 10;
                        let cur = item - '0' as u8;
                        bulklen += cur as i64;
                    }
                }
                if bulklen == 0 {
                    return Err(Error::Redis(RedisError::new(format!(
                        "client_id:{:?} protocol err, bulklen len 0",
                        self.id
                    ))));
                }
                self.read_pos = self.read_pos + idx + 2;

                self.bulklen = bulklen;
            }

            if self.read_buf.len() < (self.bulklen + 2) as usize {
                return Ok(None);
            }

            if self.req.command.is_empty() {
                self.req.command = self.slice_len(self.bulklen as usize).to_vec();
            } else {
                let mut buf = Vec::with_capacity(self.bulklen as usize);
                buf.extend_from_slice(self.slice_len(self.bulklen as usize));
                self.req.args.push(buf);
            }

            self.read_pos = self.read_pos + self.bulklen as usize + 2;

            self.bulklen = -1;
            self.multibulklen -= 1;
        }

        let req = self.req.clone();
        self.req.reset();
        return Ok(Some(req));
    }

    fn slice_len(&self, len: usize) -> &[u8] {
        &self.slice_to(self.read_pos + len)
    }

    fn slice_to(&self, end: usize) -> &[u8] {
        &self.read_buf[self.read_pos..end]
    }

    fn cur_data(&self) -> &[u8] {
        self.slice_to(self.data_pos)
    }

    fn peek(&self) -> u8 {
        self.read_buf[self.read_pos]
    }
}
