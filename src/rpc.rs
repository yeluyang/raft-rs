use std::{collections::HashMap, sync::mpsc::Sender, thread};

use crate::{
    error::Result,
    logger::SequenceID,
    peer::{Receipt, Vote},
};

pub type Endpoint = String;

pub trait PeerClientRPC: Send + Clone + 'static {
    fn connect(host: Endpoint) -> Self;

    fn append(
        &self,
        leader: Endpoint,
        term: usize,
        seq_ids: Option<Vec<SequenceID>>,
    ) -> Result<Receipt>;

    fn append_async(
        &self,
        leader: Endpoint,
        term: usize,
        seq_ids: Option<Vec<SequenceID>>,
        ch: Sender<Result<Receipt>>,
    ) {
        let agent = self.clone();
        thread::spawn(move || {
            ch.send(agent.append(leader, term, seq_ids))
                .expect("failed to get response of heart beat from channel");
        });
    }

    fn request_vote(&self, host: Endpoint, term: usize, seq_id: Option<SequenceID>)
        -> Result<Vote>;

    fn request_vote_async(
        &self,
        host: Endpoint,
        term: usize,
        seq_id: Option<SequenceID>,
        ch: Sender<Result<Vote>>,
    ) {
        let agent = self.clone();
        thread::spawn(move || {
            ch.send(agent.request_vote(host, term, seq_id))
                .expect("failed to get vote from channel");
        });
    }
}
