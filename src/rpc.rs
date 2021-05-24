use std::{sync::mpsc::Sender, thread};

use crate::{
    error::Result,
    logger::SequenceID,
    peer::{Receipt, Vote},
};

pub type Endpoint = String;

pub trait PeerClientRPC: Send + Clone + 'static {
    fn connect(host: Endpoint) -> Self;

    fn heart_beat(&self, leader: Endpoint, term: usize) -> Result<Receipt>;

    fn heart_beat_async(&self, leader: Endpoint, term: usize, ch: Sender<Result<Receipt>>) {
        let agent = self.clone();
        thread::spawn(move || {
            ch.send(agent.heart_beat(leader, term))
                .expect("failed to get response of heart beat from channel");
        });
    }

    fn request_vote(&self, host: Endpoint, term: usize, log_seq: Option<SequenceID>) -> Result<Vote>;

    fn request_vote_async(
        &self,
        host: Endpoint,
        term: usize,
        log_seq: Option<SequenceID>,
        ch: Sender<Result<Vote>>,
    ) {
        let agent = self.clone();
        thread::spawn(move || {
            ch.send(agent.request_vote(host, term, log_seq))
                .expect("failed to get vote from channel");
        });
    }
}
