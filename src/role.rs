use std::{
    collections::HashMap,
    fmt::{self, Display, Formatter},
    sync::mpsc,
    thread,
    time::{Duration, SystemTime},
    usize,
};

extern crate rand;
use rand::Rng;

use crate::{
    error::Result,
    logger::{Logger, SequenceID},
    rpc::{Endpoint, PeerClientRPC},
};

#[derive(Debug)]
struct Signature {
    endpoint: Endpoint,
    term: usize,
    seq_id: Option<SequenceID>,
}

impl Signature {
    fn new(endpoint: Endpoint, term: usize, seq_id: Option<SequenceID>) -> Self {
        Self {
            endpoint,
            term,
            seq_id,
        }
    }
}

impl Display for Signature {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if let Some(log_seq) = &self.seq_id {
            write!(
                f,
                "{{ endpoint={}, term={}, log_seq={} }}",
                self.endpoint, self.term, log_seq
            )
        } else {
            write!(
                f,
                "{{ endpoint={}, term={}, log_seq=None }}",
                self.endpoint, self.term,
            )
        }
    }
}

#[derive(Debug)]
pub struct Vote {
    pub granted: bool,
    peer: Signature,
}

impl Display for Vote {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{{ granted?={} from peer={} }}", self.granted, self.peer,)
    }
}

impl Vote {
    fn grant(peer: Signature) -> Self {
        Self {
            granted: true,
            peer,
        }
    }

    fn deny(peer: Signature) -> Self {
        Self {
            granted: false,
            peer,
        }
    }
}

#[derive(Debug)]
pub struct Receipt {
    success: bool,
    term: usize,
    endpoint: Endpoint,
}

#[derive(Clone)]
pub struct Diverged {
    next: usize,
    matched: Option<SequenceID>,
}

impl Diverged {
    pub fn new(next: usize) -> Self {
        Self {
            next,
            matched: None,
        }
    }
}

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

    fn grant(&mut self, candidate: Signature) -> Vote {
        // XXX: will the case `self.voted == candidate.endpoint && self.term > candidate.term` happen?

        if candidate.term < self.state.logger.term() {
            return Vote::deny(self.state.sign());
        } else if candidate.term > self.state.logger.term() {
            self.state.logger.set_term(candidate.term);
        }

        if let Some(ref v) = self.voted {
            if v != &candidate.endpoint {
                return Vote::deny(self.state.sign());
            }
        };

        // `self.term <= candidate.term and not vote for anyone yet` for now
        if candidate.seq_id >= self.state.logger.last_seq_id() {
            return Vote::grant(self.state.sign());
        } else {
            return Vote::deny(self.state.sign());
        }
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
                self.state.endpoint.clone(),
                term,
                self.state.logger.last_seq_id(),
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
    peers_log_div: HashMap<Endpoint, Diverged>,
    interval: Duration,

    state: State<C>, // raft state
}

impl<C: PeerClientRPC> Leader<C> {
    fn new(state: State<C>) -> Self {
        let mut peers_log_div: HashMap<Endpoint, Diverged> =
            HashMap::with_capacity(state.peers.len());
        for (peer_endpoint, _) in &state.peers {
            peers_log_div.insert(peer_endpoint.clone(), Diverged::new(state.logger.applied()));
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
    endpoint: Endpoint,
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
            endpoint: host,
            peers,
            logger: Logger::new(seq_ids),
        }
    }

    fn sign(&self) -> Signature {
        Signature::new(
            self.endpoint.clone(),
            self.logger.term(),
            self.logger.last_seq_id(),
        )
    }
}

const ELECTION_INTERVAL_MIN: u64 = 100;
const ELECTION_INTERVAL_MAX: u64 = 500;

fn election_interval() -> Duration {
    Duration::from_millis(
        rand::thread_rng().gen_range(ELECTION_INTERVAL_MIN..=ELECTION_INTERVAL_MAX),
    )
}
