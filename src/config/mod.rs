pub struct Config {
    pub addr: String,
}

impl Config {
    fn new() -> Config {
        Config {
            addr: "127.0.0.1:6379".to_string(),
        }
    }

}

pub fn parse_config() -> Config {
    Config::new()
}