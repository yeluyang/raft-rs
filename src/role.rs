use std::{
    collections::HashMap,
    sync::mpsc,
    thread,
    time::{Duration, SystemTime},
};

extern crate rand;
use rand::Rng;

use crate::{
    error::Result,
    logger::{LogDiverged, Logger, SequenceID},
    peer::Vote,
    rpc::{Endpoint, PeerClientRPC},
};

enum Role<C: PeerClientRPC> {
    Follower(Follower<C>),
    Candidate(Candidate<C>),
    Leader(Leader<C>),
}

impl<C: PeerClientRPC> Role<C> {
    fn new(host: Endpoint, seq_ids: Vec<SequenceID>, peer_hosts: Vec<Endpoint>) -> Self {
        Self::Follower(Follower::new(State::<C>::new(host, seq_ids, peer_hosts)))
    }

    fn step(mut self) -> Self {
        match self {
            Self::Follower(r) => r.step(),
            Self::Candidate(r) => r.step(),
            Self::Leader(r) => r.step(),
        }
    }

    fn interval(&self) -> Option<Duration> {
        match self {
            Self::Follower(r) => r.interval(),
            Self::Candidate(r) => r.interval(),
            Self::Leader(r) => r.interval(),
        }
    }
}

struct Follower<C: PeerClientRPC> {
    // raft follower state
    voted: Option<Endpoint>,
    interval: Duration,
    last_heart_beat: SystemTime,

    state: State<C>, // raft state
}

impl<C: PeerClientRPC> Follower<C> {
    fn new(state: State<C>) -> Self {
        Self {
            voted: None,
            interval: election_interval(),
            last_heart_beat: SystemTime::now(),
            state,
        }
    }

    fn step(mut self) -> Role<C> {
        if self.last_heart_beat.elapsed().unwrap() > self.interval {
            Role::Candidate(Candidate::new(self.state))
        } else {
            Role::Follower(self)
        }
    }

    fn interval(&self) -> Option<Duration> {
        Some(self.interval)
    }
}

struct Candidate<C: PeerClientRPC> {
    // raft candidate state
    state: State<C>, // raft state
}

impl<C: PeerClientRPC> Candidate<C> {
    fn new(state: State<C>) -> Self {
        Self { state }
    }

    fn step(mut self) -> Role<C> {
        let term = self.state.logger.new_term();
        let timeout_duration = election_interval();
        debug!(
            "running as Candidate: term={}, timeout_after={:?}",
            term, timeout_duration,
        );

        let (vote_sender, vote_recver) = mpsc::channel::<Result<Vote>>();
        let (tick_sender, tick_recver) = mpsc::channel::<()>();

        thread::spawn(move || {
            thread::sleep(timeout_duration);
            tick_sender
                .send(())
                .unwrap_or_else(|err| error!("candidate timeout ticker error: {}", err));
        });
        // TODO add `append` notify

        for (_, peer) in &self.state.peers {
            peer.request_vote_async(
                self.state.host.clone(),
                term,
                self.state.logger.get_last_seq(),
                vote_sender.clone(),
            );
        }

        let mut granted: usize = 0;
        let mut deny: usize = 0;
        loop {
            if let Ok(vote) = vote_recver.try_recv() {
                match vote {
                    Err(err) => debug!("failed to get vote: {}", err),
                    Ok(vote) => {
                        debug!("receive vote: {}", vote,);
                        if vote.granted {
                            granted += 1;
                        } else {
                            deny += 1;
                        }
                        if granted + deny >= self.state.peers.len() {
                            break;
                        }
                    }
                };
            } else if let Ok(_) = tick_recver.try_recv() {
                debug!("timeout when election");
                break;
            }
        }
        if granted >= self.state.peers.len() / 2 {
            // become leader
            debug!(
                "receive {}/{} granted vote from peers, become leader",
                granted,
                self.state.peers.len(),
            );
            Role::Leader(Leader::new(self.state))
        } else {
            Role::Candidate(self)
        }
    }

    fn interval(&self) -> Option<Duration> {
        None
    }
}

struct Leader<C: PeerClientRPC> {
    // raft leader state
    peers_log_div: HashMap<Endpoint, LogDiverged>,
    interval: Duration,

    state: State<C>, // raft state
}

impl<C: PeerClientRPC> Leader<C> {
    fn new(state: State<C>) -> Self {
        let mut peers_log_div: HashMap<Endpoint, LogDiverged> =
            HashMap::with_capacity(state.peers.len());
        for (peer_endpoint, _) in &state.peers {
            peers_log_div.insert(
                peer_endpoint.clone(),
                LogDiverged::new(state.logger.applied()),
            );
        }

        Self {
            interval: Duration::from_millis(ELECTION_INTERVAL_MIN / 2),
            peers_log_div,
            state,
        }
    }

    fn step(mut self) -> Role<C> {
        unimplemented!()
    }

    fn interval(&self) -> Option<Duration> {
        Some(self.interval)
    }
}

struct State<C: PeerClientRPC> {
    host: Endpoint,
    peers: HashMap<Endpoint, C>,
    logger: Logger, // raft log state
}

impl<C: PeerClientRPC> State<C> {
    fn new(host: Endpoint, seq_ids: Vec<SequenceID>, peer_hosts: Vec<Endpoint>) -> Self {
        let mut peers = HashMap::new();
        for h in peer_hosts {
            let client = PeerClientRPC::connect(h.clone());
            peers.insert(h, client);
        }
        Self {
            host,
            peers,
            logger: Logger::new(seq_ids),
        }
    }
}

const ELECTION_INTERVAL_MIN: u64 = 100;
const ELECTION_INTERVAL_MAX: u64 = 500;

fn election_interval() -> Duration {
    Duration::from_millis(
        rand::thread_rng().gen_range(ELECTION_INTERVAL_MIN..=ELECTION_INTERVAL_MAX),
    )
}
