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

    /// Row `row` packed as (file, line).
    pub fn packed(&self, row: usize) -> Option<u64> {
        match self {
            Self::File { lines } => (row < *lines).then(|| pack(0, row)),
            Self::Merged(rows) => rows.get(row).copied(),
        }
    }

    /// Every row, packed, in order.
    pub fn iter(&self) -> impl Iterator<Item = u64> + '_ {
        let (lines, merged) = match self {
            Self::File { lines } => (*lines, &[][..]),
            Self::Merged(rows) => (0, rows.as_slice()),
        };
        (0..lines).map(|n| pack(0, n)).chain(merged.iter().copied())
    }

    /// The rows as a list, to change them in place (live rows arriving or
    /// leaving): a File View's rows are written out first.
    pub fn to_mut(&mut self) -> &mut Vec<u64> {
        if let Self::File { lines } = *self {
            *self = Self::Merged((0..lines).map(|n| pack(0, n)).collect());
        }
        match self {
            Self::Merged(rows) => rows,
            Self::File { .. } => unreachable!("written out above"),
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

/// The entries of the rows last shown, parsed again from their source. Each
/// entry weighs what it was kept with (`insert_weighed`; 1 by default), and
/// past the capacity the least recently used go.
#[derive(Debug, Default)]
pub struct RowCache {
    capacity: usize,
    weight: usize,
    entries: HashMap<RowKey, (Arc<LogEntry>, u64, usize)>,
    tick: u64,
}

/// Bytes of parsed rows the whole app keeps: a few screens around the ones
/// shown, however many files are open.
pub const ROW_CACHE_BYTES: usize = 64 << 20;

/// Shards of the shared Row Cache: a file's rows are all in one, so the UI
/// thread and the reads in the background rarely wait on each other.
const ROW_SHARDS: usize = 16;

/// What a parsed row of a line `len` bytes long is counted as.
pub fn row_weight(len: usize) -> usize {
    std::mem::size_of::<LogEntry>() + len + 64
}

impl RowCache {
    pub fn new(capacity: usize) -> Self {
        Self { capacity: capacity.max(1), weight: 0, entries: HashMap::new(), tick: 0 }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The weight of every entry kept.
    pub fn weight(&self) -> usize {
        self.weight
    }

    /// The number and weight of the entries of file `file`.
    pub fn of_file(&self, file: usize) -> (usize, usize) {
        self.entries.iter().filter(|(k, _)| k.0 == file).fold((0, 0), |(n, w), (_, e)| (n + 1, w + e.2))
    }

    /// The entry of `key`, when it is kept; it counts as used.
    pub fn get(&mut self, key: RowKey) -> Option<Arc<LogEntry>> {
        self.tick += 1;
        let tick = self.tick;
        self.entries.get_mut(&key).map(|(entry, used, _)| {
            *used = tick;
            entry.clone()
        })
    }

    pub fn contains(&self, key: RowKey) -> bool {
        self.entries.contains_key(&key)
    }

    /// Keep `entry` as the entry of `key`.
    pub fn insert(&mut self, key: RowKey, entry: Arc<LogEntry>) {
        self.insert_weighed(key, entry, 1);
    }

    /// Keep `entry` as the entry of `key`, weighing `weight`. Past a quarter
    /// over the capacity the least recently used go, down to the capacity.
    pub fn insert_weighed(&mut self, key: RowKey, entry: Arc<LogEntry>, weight: usize) {
        self.tick += 1;
        if let Some((_, _, old)) = self.entries.insert(key, (entry, self.tick, weight)) {
            self.weight -= old;
        }
        self.weight += weight;
        if self.weight > self.capacity + self.capacity / 4 {
            let mut by_age: Vec<(u64, usize)> = self.entries.values().map(|(_, t, w)| (*t, *w)).collect();
            by_age.sort_unstable();
            let mut weight = self.weight;
            let mut oldest_gone = 0;
            for (tick, w) in by_age {
                if weight <= self.capacity {
                    break;
                }
                weight -= w;
                oldest_gone = tick;
            }
            self.entries.retain(|_, (_, t, _)| *t > oldest_gone);
            self.weight = weight;
        }
    }

    /// Forget every line of file `file`.
    pub fn forget_file(&mut self, file: usize) {
        let mut gone = 0;
        self.entries.retain(|k, (_, _, w)| {
            let keep = k.0 != file;
            if !keep {
                gone += *w;
            }
            keep
        });
        self.weight -= gone;
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.weight = 0;
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

/// The Row Cache of the whole app: the parsed rows of every file, by the
/// file's key (`IndexedFile::file_key`), its Generation and its line,
/// within `ROW_CACHE_BYTES`.
#[derive(Debug)]
pub struct SharedRowCache {
    shards: Vec<std::sync::Mutex<RowCache>>,
}

impl SharedRowCache {
    /// A cache of `capacity` bytes in all.
    pub fn new(capacity: usize) -> Self {
        let each = (capacity / ROW_SHARDS).max(1);
        Self { shards: (0..ROW_SHARDS).map(|_| std::sync::Mutex::new(RowCache::new(each))).collect() }
    }

    /// The shard that holds the rows of file `file`.
    pub fn shard(&self, file: usize) -> std::sync::MutexGuard<'_, RowCache> {
        self.shards[file % ROW_SHARDS].lock().unwrap()
    }

    /// Forget every row of file `file`.
    pub fn forget(&self, file: usize) {
        self.shard(file).forget_file(file);
    }

    /// The weight of every row kept.
    pub fn weight(&self) -> usize {
        self.shards.iter().map(|s| s.lock().unwrap().weight()).sum()
    }
}

/// The app's Row Cache.
pub fn rows() -> &'static SharedRowCache {
    static ROWS: std::sync::OnceLock<SharedRowCache> = std::sync::OnceLock::new();
    ROWS.get_or_init(|| SharedRowCache::new(ROW_CACHE_BYTES))
}
