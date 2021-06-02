use std::{sync::mpsc::Sender, thread};

extern crate async_trait;
use async_trait::async_trait;

use crate::{
    error::Result,
    logger::SequenceID,
    role::{Receipt, Vote},
};

pub type Endpoint = String;

#[async_trait]
pub trait PeerClientRPC: Send + Clone + 'static {
    fn connect(host: Endpoint) -> Self;

    async fn append(
        &self,
        leader: Endpoint,
        term: usize,
        seq_ids: Option<Vec<SequenceID>>,
    ) -> Result<Receipt>;

    async fn request_vote(
        &self,
        host: Endpoint,
        term: usize,
        seq_id: Option<SequenceID>,
    ) -> Result<Vote>;
}
