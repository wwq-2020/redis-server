//use super::config::{Config};

pub struct Server {
    pub cluster_enabled:bool,
}

impl Server {
    pub fn new() -> Server {
        Server {
            cluster_enabled:false,
        }
    }
}