//! The daemon's URL: `http://host[:port]` only.

use super::exit::Failure;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub host: String,
    pub port: u16,
    pub loopback: bool,
}

pub fn parse(_url: &str) -> Result<Target, Failure> {
    Err(Failure::usage("not built yet"))
}
