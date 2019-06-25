use super::common::AM;
use super::error::Error;
use std::collections::HashMap;
use std::str;


const OK: &str = "+OK\r\n";
const COMMAND: &str = "COMMAND";
const _GET: &str = "GET";
const SET: &str = "SET";
const _EMPTY: &str = "$-1\r\n";

pub trait Command<R, E> {
    fn exec(&mut self, db: AM<DB>) -> Result<R, E>;
}

#[derive(Clone, Debug)]
pub struct CmdReq {
    pub command: Vec<u8>,
    pub args: Vec<Vec<u8>>,
}

impl CmdReq {
    pub fn new() -> CmdReq {
        CmdReq {
            command: vec![],
            args: vec![],
        }

    }
    pub fn reset(&mut self) {
        self.command.clear();
        self.args.clear();

    }
}

impl Command<CmdResp, Error> for CmdReq {
    fn exec(&mut self, db: AM<DB>) -> Result<CmdResp, Error> {
        let mut db = db.lock().unwrap();
        let data = db.process_command(&self.command, &self.args)?;
        let mut resp = CmdResp::new();
        resp.data.extend_from_slice(data);
        Ok(resp)
    }
}

pub struct CmdResp {
    pub data: Vec<u8>,
}

impl CmdResp {
    fn new() -> CmdResp {
        CmdResp { data: vec![] }
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.data.as_slice()
    }
}


pub struct DB {
    kv: HashMap<Vec<u8>, Vec<u8>>,
}

impl DB {
    pub fn new() -> DB {
        DB { kv: HashMap::new() }
    }

    pub fn process_command(
        &mut self,
        command: &Vec<u8>,
        arg: &Vec<Vec<u8>>,
    ) -> Result<&[u8], Error> {
        let command = command.as_slice();

        if command == COMMAND.as_bytes() {
            return Ok(OK.as_bytes());
        }


        if command == SET.as_bytes() {
            self.kv.insert(arg[0].clone(), arg[1].clone());
            return Ok(OK.as_bytes());
        }

        Ok(OK.as_bytes())
    }
}