use std::{
    fmt::{self, Display, Formatter},
    fs,
    path::Path,
};

/// FIXME: bugs occur when self.term > other.term but self.index < other.index under derived `PartialOrd`
#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
pub struct LogSeq {
    pub term: usize,
    pub index: usize,
}

impl Display for LogSeq {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{{ term={}, index={} }}", self.term, self.index)
    }
}

#[derive(Clone)]
struct Entry {
    seq: LogSeq,
    data: Vec<u8>,
}

#[derive(Default, Clone)]
pub struct Logger {
    // persistent state
    pub term: usize,
    entries: Vec<Entry>,

    // volatile state
    committed: usize,
    pub applied: usize,
}

impl Logger {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load(dir: &str) -> Self {
        let path = Path::new(dir);
        assert!(path.is_dir());

        let mut paths = Vec::new();
        for entry in fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            if let Some(ext) = path.extension() {
                if ext == "wal" {
                    paths.push(path);
                }
            }
        }

        debug!("loading logs from {} files: {:?}", paths.len(), paths);
        if paths.is_empty() {
            Self::new()
        } else {
            unimplemented!()
        }
    }

    pub fn get_last_seq(&self) -> Option<LogSeq> {
        if let Some(entry) = self.entries.get(self.applied) {
            Some(entry.seq.clone())
        } else {
            None
        }
    }
}
