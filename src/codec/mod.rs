use super::db::{CmdReq, CmdResp};
use super::error::{Error, RedisError};
use bytes::BytesMut;

use memchr::memchr;

use tokio::codec::{Decoder, Encoder};

pub struct Codec {
    req: CmdReq,
    req_type: ReqType,
    multibulklen: i64,
    bulklen: i64,
}

enum ReqType {
    Init,
    MULTIBULK,
    INLINE,
}

impl Codec {
    pub fn new() -> Codec {
        Codec {
            req: CmdReq::new(),
            req_type: ReqType::Init,
            multibulklen: 0,
            bulklen: -1,
        }
    }

    fn process_inline_buffer(&mut self, _src: &mut BytesMut) -> Result<Option<CmdReq>, Error> {
        Ok(Some(CmdReq::new()))
    }

    fn process_multibulk_buffer(&mut self, src: &mut BytesMut) -> Result<Option<CmdReq>, Error> {
        if self.multibulklen == 0 {
            let idx = match memchr('\r' as u8, src) {
                Some(idx) => idx,
                _ => {
                    return Ok(None);
                }
            };


            if src[0] != '*' as u8 {
                return Err(Error::Redis(RedisError::new()));
            }

            if idx == src.len() - 1 {
                return Ok(None);
            }


            let mut multibulklen: i64 = 0;

            for &item in &src[..idx] {
                if item >= '0' as u8 && item <= '9' as u8 {
                    multibulklen *= 10;
                    let cur = item - '0' as u8;
                    multibulklen += cur as i64;
                }
            }
            src.advance(idx + 2);
            if multibulklen <= 0 {
                return Ok(None);
            }
            self.multibulklen = multibulklen;
            self.req.args = Vec::with_capacity(multibulklen as usize);
        }

        while self.multibulklen != 0 {

            if self.bulklen == -1 {
                let idx = match memchr('\r' as u8, src) {
                    Some(idx) => idx,
                    _ => {
                        return Ok(None);
                    }
                };
                if src[0] != '$' as u8 {
                    return Err(Error::Redis(RedisError::new()));
                }
                if idx == src.len() - 1 {
                    return Ok(None);
                }

                let mut bulklen: i64 = 0;

                for &item in &src[..idx] {
                    if item >= '0' as u8 && item <= '9' as u8 {
                        bulklen *= 10;
                        let cur = item - '0' as u8;
                        bulklen += cur as i64;
                    }
                }
                if bulklen == 0 {
                    return Err(Error::Redis(RedisError::new()));
                }

                src.advance(idx + 2);
                self.bulklen = bulklen;
            }

            if src.len() < (self.bulklen + 2) as usize {
                return Ok(None);
            }

            if self.req.command.is_empty() {
                self.req
                    .command
                    .extend_from_slice(&src[..self.bulklen as usize]);
            } else {
                let mut buf = Vec::with_capacity(self.bulklen as usize);
                buf.extend_from_slice(&src[..self.bulklen as usize]);
                self.req.args.push(buf);
            }

            src.advance((self.bulklen + 2) as usize);

            self.bulklen = -1;
            self.multibulklen -= 1;

        }
        let req = self.req.clone();
        self.req.reset();
        return Ok(Some(req));
    }

}

impl Decoder for Codec {
    type Item = CmdReq;
    type Error = Error;
    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.len() == 0 {
            return Ok(None);
        }
        self.req_type = match self.req_type {
            ReqType::Init => {
                if src[0] == '*' as u8 {
                    ReqType::MULTIBULK
                } else {
                    ReqType::INLINE
                }

            }
            ReqType::INLINE => ReqType::INLINE,
            ReqType::MULTIBULK => ReqType::MULTIBULK,
        };

        match self.req_type {
            ReqType::MULTIBULK => return self.process_multibulk_buffer(src),
            ReqType::INLINE => return self.process_inline_buffer(src),
            _ => panic!("nerver reached"),
        }
    }
}

impl Encoder for Codec {
    type Item = CmdResp;
    type Error = Error;
    fn encode(&mut self, item: Self::Item, dst: &mut BytesMut) -> Result<(), Self::Error> {
        dst.extend_from_slice(item.as_bytes());
        Ok(())
    }
}

