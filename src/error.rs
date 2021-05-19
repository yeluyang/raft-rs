use std::{
    error,
    fmt::{self, Display, Formatter},
    result,
};

use crate::EndPoint;

pub type Result<T> = result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    RPC(EndPoint, String), // TODO add endpoint into error
}

impl error::Error for Error {}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::RPC(endpoint, err) => write!(f, "RPC error from {}: {}", endpoint, err),
        }
    }
}

impl From<(EndPoint, String)> for Error {
    fn from(err: (EndPoint, String)) -> Self {
        Self::RPC(err.0, err.1)
    }
}
