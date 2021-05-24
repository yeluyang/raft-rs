use std::time::{Duration, SystemTime};

extern crate rand;
use rand::Rng;

use crate::{
    logger::{Logger, SequenceID},
    rpc::Endpoint,
};

enum Role {
    Follower(Follower),
    Candidate(Candidate),
    Leader(Leader),
}

impl Role {
    fn new(seq_ids: Vec<SequenceID>) -> Self {
        Self::Follower(Follower::new(Logger::new(seq_ids)))
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

struct Follower {
    // raft follower state
    voted: Option<Endpoint>,
    interval: Duration,
    last_heart_beat: SystemTime,

    logger: Logger, // raft log state
}

impl Follower {
    fn new(logger: Logger) -> Self {
        Self {
            voted: None,
            interval: election_interval(),
            last_heart_beat: SystemTime::now(),
            logger,
        }
    }

    fn step(mut self) -> Role {
        if self.last_heart_beat.elapsed().unwrap() > self.interval {
            Role::Candidate(Candidate::new(self.logger))
        } else {
            Role::Follower(self)
        }
    }

    fn interval(&self) -> Option<Duration> {
        Some(self.interval)
    }
}

struct Candidate {
    // raft candidate state
    logger: Logger, // raft log state
}

impl Candidate {
    fn new(logger: Logger) -> Self {
        Self { logger }
    }

    fn step(mut self) -> Role {
        unimplemented!()
    }

    fn interval(&self) -> Option<Duration> {
        None
    }
}

struct Leader {
    // raft leader state
    interval: Duration,

    logger: Logger, // raft log state
}

impl Leader {
    fn new(logger: Logger) -> Self {
        Self {
            interval: Duration::from_millis(ELECTION_INTERVAL_MIN / 2),
            logger,
        }
    }

    fn step(mut self) -> Role {
        unimplemented!()
    }

    fn interval(&self) -> Option<Duration> {
        Some(self.interval)
    }
}

const ELECTION_INTERVAL_MIN: u64 = 100;
const ELECTION_INTERVAL_MAX: u64 = 500;

fn election_interval() -> Duration {
    Duration::from_millis(
        rand::thread_rng().gen_range(ELECTION_INTERVAL_MIN..=ELECTION_INTERVAL_MAX),
    )
}
