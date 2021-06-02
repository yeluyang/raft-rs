#[macro_use]
extern crate log;

#[macro_use]
extern crate futures;

mod error;
pub use error::{Error, Result};

mod rpc;
pub use rpc::{Endpoint, PeerClientRPC};

mod logger;
pub use logger::SequenceID;

// mod peer;
// pub use peer::{Peer, Receipt, Vote};

mod role;
