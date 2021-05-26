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

enum Role {
    Follower {
        voted: Option<Endpoint>,
        last_heart_beat: SystemTime,
    },
    Candidate,
    Leader {
        followers: HashMap<Endpoint, Diverged>,
    },
}

impl Role {
    fn new() -> Self {
        Self::follower()
    }

    fn follower() -> Self {
        Self::Follower {
            voted: None,
            last_heart_beat: SystemTime::now(),
        }
    }

    fn candidate() -> Self {
        Self::Candidate
    }

    fn leader(peers: Vec<Endpoint>, applied: usize) -> Self {
        let mut followers: HashMap<Endpoint, Diverged> = HashMap::with_capacity(peers.len());
        for peer in peers {
            followers.insert(peer, Diverged::new(applied));
        }

        Self::Leader { followers }
    }
}

struct State<C: PeerClientRPC> {
    endpoint: Endpoint,
    peers: HashMap<Endpoint, C>,
    logger: Logger, // raft log state

    role: Role,
    interval: Option<Duration>,
}

impl<C: PeerClientRPC> State<C> {
    fn new(endpoint: Endpoint, seq_ids: Vec<SequenceID>, peer_hosts: Vec<Endpoint>) -> Self {
        let mut peers = HashMap::new();
        for h in peer_hosts {
            let client = PeerClientRPC::connect(h.clone());
            peers.insert(h, client);
        }
        Self {
            endpoint,
            peers,
            logger: Logger::new(seq_ids),

            role: Role::follower(),
            interval: Some(election_interval()),
        }
    }

    fn interval(&self) -> Option<Duration> {
        self.interval
    }

    fn sign(&self) -> Signature {
        Signature::new(
            self.endpoint.clone(),
            self.logger.term(),
            self.logger.last_seq_id(),
        )
    }

    fn become_follower(&mut self) {
        self.interval = Some(election_interval());
        self.role = Role::follower();
    }

    fn become_candidate(&mut self) {
        self.interval = None;
        self.role = Role::candidate();
    }

    fn become_leader(&mut self) {
        self.interval = Some(Duration::from_millis(ELECTION_INTERVAL_MIN / 2));
        self.role = Role::leader(self.peers.keys().cloned().collect(), self.logger.applied());
    }

    fn grant(&mut self, candidate: Signature) -> Vote {
        // XXX: will the case `self.voted == candidate.endpoint && self.term > candidate.term` happen?

        match &self.role {
            Role::Follower { voted, .. } => {
                if candidate.term < self.logger.term() {
                    return Vote::deny(self.sign());
                } else if candidate.term > self.logger.term() {
                    self.logger.set_term(candidate.term);
                }

                if let Some(ref v) = voted {
                    if v != &candidate.endpoint {
                        return Vote::deny(self.sign());
                    }
                };

                // `self.term <= candidate.term and not vote for anyone yet` for now
                if candidate.seq_id >= self.logger.last_seq_id() {
                    Vote::grant(self.sign())
                } else {
                    Vote::deny(self.sign())
                }
            }
            _ => unimplemented!(),
        }
    }

    fn follower_step(&mut self) {
        if let Role::Follower {
            last_heart_beat, ..
        } = &self.role
        {
            if last_heart_beat.elapsed().unwrap() > self.interval.unwrap() {
                self.become_candidate();
                return self.candidate_step();
            }
        } else {
            unreachable!();
        };
    }

    fn candidate_step(&mut self) {
        let term = self.logger.new_term();
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

        for (_, peer) in &self.peers {
            peer.request_vote_async(
                self.endpoint.clone(),
                term,
                self.logger.last_seq_id(),
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
                        if granted + deny >= self.peers.len() {
                            break;
                        }
                    }
                };
            } else if let Ok(_) = tick_recver.try_recv() {
                debug!("timeout when election");
                break;
            }
        }
        if granted >= self.peers.len() / 2 {
            // become leader
            debug!(
                "receive {}/{} granted vote from peers, become leader",
                granted,
                self.peers.len(),
            );
            self.become_leader();
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
