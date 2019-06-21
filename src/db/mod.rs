
use super::common::OK;
use super::error::Error;
use std::collections::HashMap;
pub struct DB {
    kv: HashMap<Vec<u8>, Vec<u8>>,
}

impl DB {
    pub fn new() -> DB {
        DB { kv: HashMap::new() }
    }

    pub fn process_command(&mut self, arg: &Vec<Vec<u8>>) -> Result<&[u8], Error> {
        self.kv.insert(arg[0].clone(), arg[1].clone());
        Ok(OK.as_bytes())
    }
}