use super::error::Error;
use std::sync::{Arc, Mutex};
pub type AM<T> = Arc<Mutex<T>>;
pub type MySimpleResult = Result<(), Error>;
