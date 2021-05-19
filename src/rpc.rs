use std::{
    fmt::{self, Display, Formatter},
    sync::mpsc::Sender,
    thread,
};

use crate::{
    error::Result,
    logger::LogSeq,
    peer::{Receipt, Vote},
};

#[derive(Default, Clone, Eq, PartialEq, Debug, Hash)]
pub struct EndPoint {
    pub ip: String,
    pub port: u16,
}

impl Display for EndPoint {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.ip, self.port)
    }
}

impl From<&(&str, u16)> for EndPoint {
    fn from(host: &(&str, u16)) -> Self {
        Self::from((host.0.to_owned(), host.1))
    }
}

impl From<(String, u16)> for EndPoint {
    fn from(host: (String, u16)) -> Self {
        Self {
            ip: host.0,
            port: host.1,
        }
    }
}

impl EndPoint {
    pub fn from_hosts(hosts: Vec<(String, u16)>) -> Vec<Self> {
        let mut endpoints: Vec<Self> = Vec::new();

        for host in hosts {
            endpoints.push(Self::from(host));
        }

        endpoints
    }
}

pub trait PeerClientRPC: Send + Clone + 'static {
    fn connect(host: EndPoint) -> Self;

    fn heart_beat(&self, leader: EndPoint, term: usize) -> Result<Receipt>;

    fn heart_beat_async(&self, leader: EndPoint, term: usize, ch: Sender<Result<Receipt>>) {
        let agent = self.clone();
        thread::spawn(move || {
            ch.send(agent.heart_beat(leader, term))
                .expect("failed to get response of heart beat from channel");
        });
    }

    fn request_vote(&self, host: EndPoint, term: usize, log_seq: Option<LogSeq>) -> Result<Vote>;

    fn request_vote_async(
        &self,
        host: EndPoint,
        term: usize,
        log_seq: Option<LogSeq>,
        ch: Sender<Result<Vote>>,
    ) {
        let agent = self.clone();
        thread::spawn(move || {
            ch.send(agent.request_vote(host, term, log_seq))
                .expect("failed to get vote from channel");
        });
    }
}
