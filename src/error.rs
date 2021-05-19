use std::{
    error,
    fmt::{self, Display, Formatter},
    result,
};

use crate::Endpoint;

pub type Result<T> = result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    RPC(Endpoint, String), // TODO add endpoint into error
}

impl error::Error for Error {}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::RPC(endpoint, err) => write!(f, "RPC error from {}: {}", endpoint, err),
        }
    }
}

impl From<(Endpoint, String)> for Error {
    fn from(err: (Endpoint, String)) -> Self {
        Self::RPC(err.0, err.1)
    }
}
