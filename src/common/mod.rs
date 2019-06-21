
use std::sync::{Arc, Mutex};
pub type AM<T> = Arc<Mutex<T>>;
pub const OK: &str = "+OK\r\n";
