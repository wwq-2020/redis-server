use super::common::{MySimpleResult, AM};
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
    req: Option<CmdReq>,
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
            req: Some(CmdReq::new()),
        }
    }
}

impl Client {
    pub async fn serve(&mut self) -> MySimpleResult {
        loop {
            self.serve_one().await?;
        }
    }

    async fn serve_one(&mut self) -> MySimpleResult {
        self.read_data().await?;
        self.parse_req_type()?;
        self.parse_req()?;
        self.handle_req().await?;
        Ok(())
    }

    async fn read_data(&mut self) -> MySimpleResult {
        let n = self.conn.read(&mut self.read_buf).await?;
        if n == 0 {
            return Err(Error::Redis(RedisError::new(format!(
                "client_id:{:?} conn closed",
                self.id
            ))));
        }
        self.data_pos += n;
        Ok(())
    }

    fn parse_req_type(&mut self) -> MySimpleResult {
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
        Ok(())
    }

    fn parse_req(&mut self) -> MySimpleResult {
        match self.req_type {
            ReqType::MULTIBULK => self.process_multibulk_buffer()?,
            ReqType::INLINE => self.process_inline_buffer()?,
            _ => {
                return Err(Error::Redis(RedisError::new(format!(
                    "client_id:{:?} unknown req_type",
                    self.id
                ))))
            }
        };
        Ok(())
    }

    async fn handle_req(&mut self) -> MySimpleResult {
        match &self.req {
            Some(cmd_req) => {
                let mut cmd_req = cmd_req.clone();
                let cmd_resp = cmd_req.exec(self.db.clone())?;
                self.conn.write_all(cmd_resp.as_bytes()).await?;
                self.conn.flush().await?;
                self.reset()?;
                return Ok(());
            }
            None => {
                return Ok(());
            }
        }
    }

    fn reset(&mut self) -> MySimpleResult {
        self.data_pos = self.data_pos - self.read_pos;
        self.read_pos = 0;
        self.req.as_mut().map(|req| {
            req.reset();
        });
        Ok(())
    }

    fn process_inline_buffer(&mut self) -> MySimpleResult {
        self.req = Some(CmdReq::new());
        Ok(())
    }

    fn process_multibulk_buffer(&mut self) -> MySimpleResult {
        self.process_multibulklen()?;
        self.process_bulks()?;
        Ok(())
    }

    fn process_multibulklen(&mut self) -> MySimpleResult {
        if self.multibulklen == 0 {
            let idx = match memchr('\r' as u8, self.cur_data()) {
                Some(idx) => idx,
                _ => {
                    return Ok(());
                }
            };

            if self.peek() != '*' as u8 {
                return Err(Error::Redis(RedisError::new(format!(
                    "client_id:{:?} protocol err, multibulklen start char not *",
                    self.id
                ))));
            }

            if idx == self.read_buf.len() - 1 {
                return Ok(());
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
                return Ok(());
            }
            self.multibulklen = multibulklen;
            self.req.as_mut().map(|req| {
                req.args = Vec::with_capacity(multibulklen as usize);
            });
        }
        Ok(())
    }

    fn process_bulks(&mut self) -> MySimpleResult {
        while self.multibulklen != 0 {
            if self.bulklen == -1 {
                let idx = match memchr('\r' as u8, self.cur_data()) {
                    Some(idx) => idx,
                    _ => {
                        return Ok(());
                    }
                };
                if self.peek() != '$' as u8 {
                    return Err(Error::Redis(RedisError::new(format!(
                        "client_id:{:?} protocol err, bulklen start char not $",
                        self.id
                    ))));
                }
                if idx == self.cur_data().len() - 1 {
                    return Ok(());
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
                return Ok(());
            }
            let buf = self.slice_len(self.bulklen as usize).to_vec();
            self.req.as_mut().map(|req| {
                if req.command.is_empty() {
                    req.command = buf
                } else {
                    req.args.push(buf);
                }
            });

            self.read_pos = self.read_pos + self.bulklen as usize + 2;

            self.bulklen = -1;
            self.multibulklen -= 1;
        }
        Ok(())
    }

    fn slice_len(&mut self, len: usize) -> &[u8] {
        &self.slice_to(self.read_pos + len)
    }

    fn slice_to(&mut self, end: usize) -> &[u8] {
        &self.read_buf[self.read_pos..end]
    }

    fn cur_data(&mut self) -> &[u8] {
        self.slice_to(self.data_pos)
    }

    fn peek(&mut self) -> u8 {
        self.read_buf[self.read_pos]
    }
}
