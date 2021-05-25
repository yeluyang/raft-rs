use std::fmt::{self, Display, Formatter};

/// FIXME: bugs occur when self.term > other.term but self.index < other.index under derived `PartialOrd`
#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
pub struct SequenceID {
    term: usize,
    index: usize,
}

impl Display for SequenceID {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{{ term={}, index={} }}", self.term, self.index)
    }
}

#[derive(Default, Clone)]
pub struct Logger {
    // persistent state
    term: usize,
    entries: Vec<SequenceID>,

    // volatile state
    committed: usize,
    applied: usize,
}

impl Logger {
    pub fn new(seq_ids: Vec<SequenceID>) -> Self {
        trace!("loading logger from {:?}", seq_ids);
        if seq_ids.is_empty() {
            Self::default()
        } else {
            let last = seq_ids.last().unwrap();
            debug!("loading logger into {}", last);
            let term = last.term;
            Self {
                term,
                entries: seq_ids,
                committed: 0, // TODO setup Self.committed
                applied: 0,   // TODO setup Self.applied
            }
        }
    }

    pub fn last_seq_id(&self) -> Option<SequenceID> {
        if let Some(entry) = self.entries.get(self.applied) {
            Some(entry.clone())
        } else {
            None
        }
    }

    pub fn applied(&self) -> usize {
        self.applied
    }

    pub fn term(&self) -> usize {
        self.term
    }

    pub fn new_term(&mut self) -> usize {
        self.term += 1;
        self.term
    }
    pub fn set_term(&mut self, term: usize) {
        assert!(self.term > term);
        self.term = term;
    }
}
