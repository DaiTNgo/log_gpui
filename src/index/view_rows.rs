//! The rows of a view as numbers (decision 6), and the parsed lines kept
//! for the rows on screen (decision 5).
//!
//! - A File View's rows are its file's lines, `0..len`: no array.
//! - A Log Space's rows are its files' lines merged by effective timestamp,
//!   each packed in a `u64` as (file, line). `Merger` merges as the files
//!   are indexed, a line once, in the order `timeline::merge` gives.
//! - `RowCache` keeps the entries of the rows last shown, parsed again from
//!   their source when needed.

use super::meta::{LineMeta, NO_TIME};
use crate::model::LogEntry;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use std::sync::Arc;

/// Bits of a packed row that hold the line.
const LINE_BITS: u32 = 48;

/// Row of line `line` of file `file` (its position in the Log Space).
pub fn pack(file: usize, line: usize) -> u64 {
    debug_assert!(file < 1 << 16 && (line as u64) < 1 << LINE_BITS);
    ((file as u64) << LINE_BITS) | line as u64
}

/// The file and line of a packed row.
pub fn unpack(row: u64) -> (usize, usize) {
    ((row >> LINE_BITS) as usize, (row & ((1 << LINE_BITS) - 1)) as usize)
}

/// What a view shows, row by row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewRows {
    /// A File View: line `n` is row `n`.
    File { lines: usize },
    /// A Log Space: the merged rows, packed.
    Merged(Vec<u64>),
}

impl Default for ViewRows {
    fn default() -> Self {
        Self::File { lines: 0 }
    }
}

impl ViewRows {
    pub fn len(&self) -> usize {
        match self {
            Self::File { lines } => *lines,
            Self::Merged(rows) => rows.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The file and line of `row`; a File View's file is 0.
    pub fn get(&self, row: usize) -> Option<(usize, usize)> {
        match self {
            Self::File { lines } => (row < *lines).then_some((0, row)),
            Self::Merged(rows) => rows.get(row).map(|&r| unpack(r)),
        }
    }

    /// The row showing line `line` of file `file`: the inverse of `get`.
    /// O(1) for a File View, a scan for a Log Space.
    pub fn row_of(&self, file: usize, line: usize) -> Option<usize> {
        match self {
            Self::File { lines } => (file == 0 && line < *lines).then_some(line),
            Self::Merged(rows) => {
                let want = pack(file, line);
                rows.iter().position(|&r| r == want)
            }
        }
    }
}

/// One file of a merge, as far as it is indexed.
#[derive(Debug, Clone, Copy)]
pub struct Lane<'a> {
    pub meta: &'a [LineMeta],
    /// No more lines will come (Indexing ended).
    pub done: bool,
}

#[derive(Debug, Clone, Default)]
struct LaneState {
    /// The next line not merged yet.
    next: usize,
    /// Its head is in the heap.
    queued: bool,
    /// All its lines are merged, or it waits at the end for the tail.
    finished: bool,
    /// It has no timestamp at all: its rows come after every other row.
    untimed: bool,
}

/// A k-way merge of files by effective timestamp, advanced as their lines
/// are indexed. Rows sort by (timestamp, file, line), a Continuation Line
/// stays right below its Parent Line, and the rows of a file with no
/// timestamp at all come last, in file order. A line is merged only once
/// every file still being indexed has a line to compare it with, so what
/// comes out is what the merge of the whole files gives.
#[derive(Debug, Clone, Default)]
pub struct Merger {
    lanes: Vec<LaneState>,
    heap: BinaryHeap<Reverse<(i64, usize, usize)>>,
    /// A lane whose Continuation Lines are being merged below their Parent
    /// Line; its next line was not indexed yet.
    open_run: Option<usize>,
    tail_done: bool,
}

impl Merger {
    pub fn new(files: usize) -> Self {
        Self { lanes: vec![LaneState::default(); files], ..Self::default() }
    }

    /// Whether every line of every file is merged.
    pub fn is_done(&self) -> bool {
        self.tail_done
    }

    /// Merge the Continuation Lines of `file` from its next line; whether
    /// the run ended (it may go on in lines not indexed yet).
    fn run(&mut self, file: usize, lane: &Lane, out: &mut Vec<u64>) -> bool {
        let state = &mut self.lanes[file];
        while lane.meta.get(state.next).is_some_and(|m| m.parent_dist > 0) {
            out.push(pack(file, state.next));
            state.next += 1;
        }
        state.next < lane.meta.len() || lane.done
    }

    /// Put `file`'s next line in the heap when it is known; whether the lane
    /// can go on without waiting for more lines.
    fn queue(&mut self, file: usize, lane: &Lane) -> bool {
        let state = &mut self.lanes[file];
        if state.finished || state.queued {
            return true;
        }
        match lane.meta.get(state.next) {
            Some(m) if m.ts_us != NO_TIME => {
                self.heap.push(Reverse((m.ts_us, file, state.next)));
                state.queued = true;
                true
            }
            // Lines before the first timestamp: wait for it, or for the end.
            Some(_) if !lane.done => false,
            Some(_) => {
                state.untimed = true;
                state.finished = true;
                true
            }
            None if lane.done => {
                state.finished = true;
                true
            }
            None => false,
        }
    }

    /// Merge what `lanes` (in the Log Space's file order) allow into `out`.
    pub fn advance(&mut self, lanes: &[Lane], out: &mut Vec<u64>) {
        debug_assert_eq!(lanes.len(), self.lanes.len());
        if self.tail_done {
            return;
        }
        if let Some(file) = self.open_run {
            if !self.run(file, &lanes[file], out) {
                return;
            }
            self.open_run = None;
        }
        loop {
            let mut ready = true;
            for (file, lane) in lanes.iter().enumerate() {
                ready &= self.queue(file, lane);
            }
            if !ready {
                return;
            }
            let Some(Reverse((_, file, line))) = self.heap.pop() else { break };
            out.push(pack(file, line));
            let state = &mut self.lanes[file];
            state.queued = false;
            state.next = line + 1;
            if !self.run(file, &lanes[file], out) {
                self.open_run = Some(file);
                return;
            }
        }
        // Every timed line is merged: the files with no timestamp follow.
        for (file, lane) in lanes.iter().enumerate() {
            if self.lanes[file].untimed {
                out.extend((self.lanes[file].next..lane.meta.len()).map(|line| pack(file, line)));
            }
        }
        self.tail_done = true;
    }

    /// Whether `file` turned out to have no timestamp at all.
    pub fn is_untimed(&self, file: usize) -> bool {
        self.lanes.get(file).is_some_and(|l| l.untimed)
    }
}

/// Where a parsed line comes from: its file in the view, its Generation and
/// its line number.
pub type RowKey = (usize, u32, usize);

/// The entries of the rows last shown, parsed again from their source.
#[derive(Debug, Default)]
pub struct RowCache {
    capacity: usize,
    entries: HashMap<RowKey, (Arc<LogEntry>, u64)>,
    tick: u64,
}

/// Rows kept by default: a few screens around the one shown.
pub const ROW_CACHE: usize = 4096;

impl RowCache {
    pub fn new(capacity: usize) -> Self {
        Self { capacity: capacity.max(1), entries: HashMap::new(), tick: 0 }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The entry of `key`, when it is kept; it counts as used.
    pub fn get(&mut self, key: RowKey) -> Option<Arc<LogEntry>> {
        self.tick += 1;
        let tick = self.tick;
        self.entries.get_mut(&key).map(|(entry, used)| {
            *used = tick;
            entry.clone()
        })
    }

    pub fn contains(&self, key: RowKey) -> bool {
        self.entries.contains_key(&key)
    }

    /// Keep `entry` as the entry of `key`. Past the capacity the least
    /// recently used go, a quarter at a time.
    pub fn insert(&mut self, key: RowKey, entry: Arc<LogEntry>) {
        self.tick += 1;
        self.entries.insert(key, (entry, self.tick));
        if self.entries.len() > self.capacity + self.capacity / 4 {
            let mut ticks: Vec<u64> = self.entries.values().map(|(_, t)| *t).collect();
            let cut = self.entries.len() - self.capacity;
            ticks.select_nth_unstable(cut - 1);
            let oldest_kept = ticks[cut - 1];
            self.entries.retain(|_, (_, t)| *t > oldest_kept);
        }
    }

    /// Forget every line of file `file`.
    pub fn forget_file(&mut self, file: usize) {
        self.entries.retain(|k, _| k.0 != file);
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// The stretches of `lines` (ascending line numbers of one file and
    /// Generation) not kept: what to read, each in one read.
    pub fn missing(&self, file: usize, generation: u32, lines: &[usize]) -> Vec<std::ops::Range<usize>> {
        let mut out: Vec<std::ops::Range<usize>> = Vec::new();
        for &line in lines {
            if self.contains((file, generation, line)) {
                continue;
            }
            match out.last_mut() {
                Some(r) if r.end == line => r.end += 1,
                _ => out.push(line..line + 1),
            }
        }
        out
    }
}
