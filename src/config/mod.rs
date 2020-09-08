use super::error::Error;

pub struct Config {
    pub addr: String,
}

impl Config {
    pub fn new() -> Config {
        Config {
            addr: "127.0.0.1:6379".to_string(),
        }
    }
}

impl Clone for Config {
    fn clone(&self) -> Config {
        Config {
            addr: self.addr.clone(),
        }
    }
}

pub fn parse_config() -> Result<Config, Error> {
    Ok(Config::new())
}
