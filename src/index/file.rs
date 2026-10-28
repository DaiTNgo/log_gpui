//! A local file read through its Line Index (task 4b.2): what is held for
//! every line is its position, `LineMeta` and Filter value ids; a line's
//! entry is parsed again from the file when it is asked for, and the
//! entries asked for last are kept (`RowCache`). Lines are read in blocks
//! of `READ_AROUND`, so the lines next to one asked for come with it.

use super::indexing::{IndexOptions, Indexed, Outcome, index_all};
use super::meta::LineMeta;
use super::scan::FileTexts;
use super::view_rows::{ROW_CACHE, RowCache};
use super::{LineSource, read_entries};
use crate::model::LogEntry;
use crate::parser::text::LineParser;
use std::sync::{Arc, Mutex};

/// Lines read from the file at a time when one is not in the cache.
pub const READ_AROUND: usize = 256;

/// Bytes hashed at each end of what was indexed, to tell a file rewritten
/// in place from one only written on (how klogg tells them).
pub const FINGERPRINT_BYTES: u64 = 4096;

/// A file and what Indexing built of it.
pub struct IndexedFile {
    pub source: Arc<dyn LineSource>,
    pub indexed: Arc<Indexed>,
    options: IndexOptions,
    cache: Mutex<RowCache>,
    /// Every line's text, once something needed all of them (the Raw View).
    all_lines: Mutex<Option<Arc<Vec<gpui::SharedString>>>>,
    fingerprint: Mutex<Fingerprint>,
}

/// What the indexed part of a file was: which file, how long, and a hash of
/// its first and last `FINGERPRINT_BYTES`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Fingerprint {
    pub identity: Option<crate::live::local::FileId>,
    pub len: u64,
    pub head: u64,
    pub tail: u64,
    /// The size and time of change the path had when last found unchanged:
    /// while they stay so, nothing is read to check.
    seen: Option<(u64, std::time::SystemTime)>,
}

/// How the file at a path is, next to what was indexed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnDisk {
    Same,
    /// Only written on: what was indexed is still there, unchanged.
    Appended,
    /// Another file, cut shorter, or rewritten: read it again from its start.
    Replaced,
    /// No file at the path.
    Gone,
}

/// What reading on in a file found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Extended {
    /// Nothing new.
    Same,
    /// New lines at positions `range`. With `replaced_last`, the line before
    /// them was cut off without its line break and is read again, whole,
    /// as the first of them.
    Appended { range: std::ops::Range<usize>, replaced_last: bool },
    /// The file is shorter than what was indexed: it was cut or rewritten.
    Shrunk,
    /// The file is longer, but what was indexed was written over in place:
    /// the first or last bytes read are not the same any more.
    Rewritten,
}

impl std::fmt::Debug for IndexedFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IndexedFile").field("lines", &self.len()).finish()
    }
}

impl IndexedFile {
    /// The file `source` as Indexing (with `options`) read it into `indexed`.
    pub fn new(source: Arc<dyn LineSource>, indexed: Arc<Indexed>, options: IndexOptions) -> Self {
        let fingerprint = {
            let end = indexed.data.read().unwrap().file.lines.end();
            Mutex::new(fingerprint_of(&*source, end))
        };
        Self { source, indexed, options, cache: Mutex::new(RowCache::new(ROW_CACHE)), all_lines: Mutex::default(), fingerprint }
    }

    /// What the indexed part of the file was.
    pub fn fingerprint(&self) -> Fingerprint {
        self.fingerprint.lock().unwrap().clone()
    }

    /// Whether the file at `path` is still the one indexed, only written on,
    /// or another one (rewritten in place counts: its first or last indexed
    /// bytes differ). Reads only metadata while the path's size and time of
    /// change stay those last found unchanged.
    pub fn changed_on_disk(&self, path: &std::path::Path) -> std::io::Result<OnDisk> {
        let meta = match std::fs::metadata(path) {
            Ok(meta) => meta,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(OnDisk::Gone),
            Err(e) => return Err(e),
        };
        let mut known = self.fingerprint.lock().unwrap();
        let seen = meta.modified().ok().map(|at| (meta.len(), at));
        if seen.is_some() && seen == known.seen {
            return Ok(OnDisk::Same);
        }
        if crate::live::local::is_rotated(known.identity, crate::live::local::id_of_path(path, &meta)) {
            return Ok(OnDisk::Replaced);
        }
        let now = fingerprint_of(&*self.source, known.len);
        if now.len < known.len || now.head != known.head || now.tail != known.tail {
            return Ok(OnDisk::Replaced);
        }
        if self.source.len()? > known.len {
            return Ok(OnDisk::Appended);
        }
        known.seen = seen;
        Ok(OnDisk::Same)
    }

    /// Index all of `source` now, on this thread.
    pub fn index_now(source: Arc<dyn LineSource>, options: IndexOptions) -> Result<Self, String> {
        let (data, outcome) = index_all(&*source, &options);
        if let Outcome::Failed(error) = outcome {
            return Err(error);
        }
        Ok(Self::new(source, Arc::new(Indexed::from_data(data)), options))
    }

    /// How the file was read: its Log Format and Detected Fields.
    pub fn options(&self) -> &IndexOptions {
        &self.options
    }

    /// Lines indexed.
    pub fn len(&self) -> usize {
        self.indexed.data.read().unwrap().file.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The metadata of line `n` (0-based).
    pub fn meta(&self, n: usize) -> Option<LineMeta> {
        self.indexed.data.read().unwrap().file.meta.get(n).copied()
    }

    /// The entry of line `n` (0-based), from the cache or read from the
    /// file with the lines around it. `None` when the file no longer has it.
    pub fn entry(&self, n: usize) -> Option<Arc<LogEntry>> {
        if let Some(entry) = self.cache.lock().unwrap().get((0, 0, n)) {
            return Some(entry);
        }
        let data = self.indexed.data.read().unwrap();
        let len = data.file.len();
        if n >= len {
            return None;
        }
        let start = n - n % READ_AROUND;
        let end = (start + READ_AROUND).min(len);
        let mut parser = LineParser::new();
        parser.set_config(self.options.config.clone());
        parser.set_format(self.options.format.clone());
        let entries = read_entries(&*self.source, &data.file, start..end, &mut parser).ok()?;
        drop(data);
        let mut cache = self.cache.lock().unwrap();
        let mut found = None;
        for (i, entry) in entries.into_iter().enumerate() {
            let entry = Arc::new(entry);
            if start + i == n {
                found = Some(entry.clone());
            }
            cache.insert((0, 0, start + i), entry);
        }
        found
    }

    /// The text of every line, read from the file the first time and then
    /// kept with the file (empty when the file no longer has them).
    pub fn all_lines(&self) -> Arc<Vec<gpui::SharedString>> {
        let mut all = self.all_lines.lock().unwrap();
        all.get_or_insert_with(|| {
            let data = self.indexed.data.read().unwrap();
            let lines = super::read_lines(&*self.source, &data.file.lines, 0..data.file.len()).unwrap_or_default();
            Arc::new(lines.into_iter().map(gpui::SharedString::from).collect())
        })
        .clone()
    }

    /// The text of line `n`.
    pub fn line(&self, n: usize) -> Option<gpui::SharedString> {
        let all = self.all_lines.lock().unwrap().clone();
        match all {
            Some(lines) => lines.get(n).cloned(),
            None => self.entry(n).map(|e| e.raw_line.clone()),
        }
    }

    /// Index what was written to the file since: its whole new lines, and
    /// its last line again when it was cut off without a line break. A line
    /// still being written (no line break yet) waits for its line break.
    pub fn extend(&self) -> std::io::Result<Extended> {
        let len = self.source.len()?;
        let mut data = self.indexed.data.write().unwrap();
        let end = data.file.lines.end();
        if len < end {
            return Ok(Extended::Shrunk);
        }
        if len == end {
            return Ok(Extended::Same);
        }
        // Written over (`cp bigger.log app.log`): reading on from `end` would
        // show lines cut at the old offsets.
        {
            let known = self.fingerprint.lock().unwrap();
            let now = fingerprint_of(&*self.source, end);
            if now.head != known.head || now.tail != known.tail {
                return Ok(Extended::Rewritten);
            }
        }
        // A last line without its line break is read again with what came.
        let count = data.file.len();
        let cut_last = match count.checked_sub(1).and_then(|n| data.file.lines.span(n)) {
            Some((start, line_len)) => {
                let last = self.source.read_range(start + line_len - 1, 1)?;
                (last != b"\n").then_some((count - 1, start))
            }
            None => None,
        };
        let from = cut_last.map_or(end, |(_, start)| start);
        let mut bytes = self.source.read_range(from, (len - from) as usize)?;
        let whole = memchr::memrchr(b'\n', &bytes).map_or(0, |at| at + 1);
        if whole == 0 {
            return Ok(Extended::Same);
        }
        bytes.truncate(whole);
        if let Some((last, _)) = cut_last {
            let removed = self.read_back(&data, last)?;
            data.file.lines.pop();
            data.file.meta.truncate(last);
            let parent_row = (0..last).rev().find(|&r| data.file.meta.get(r).is_some_and(|m| m.parent_dist == 0)).unwrap_or(0);
            data.filter.truncate_to(last, parent_row, &[removed][..]);
            self.cache.lock().unwrap().forget_file(0);
        }
        let locked = self.indexed.locked.load(std::sync::atomic::Ordering::Acquire);
        super::indexing::index_more(&mut data, bytes, &self.options, locked);
        let new_len = data.file.len();
        *self.fingerprint.lock().unwrap() = fingerprint_of(&*self.source, data.file.lines.end());
        *self.all_lines.lock().unwrap() = None;
        let first = cut_last.map_or(count, |(last, _)| last);
        Ok(Extended::Appended { range: first..new_len, replaced_last: cut_last.is_some() })
    }

    /// Line `n` as it was indexed, read again (the file only grew since).
    fn read_back(&self, data: &super::indexing::IndexedData, n: usize) -> std::io::Result<LogEntry> {
        let mut parser = LineParser::new();
        parser.set_config(self.options.config.clone());
        parser.set_format(self.options.format.clone());
        read_entries(&*self.source, &data.file, n..n + 1, &mut parser)?
            .pop()
            .ok_or_else(|| std::io::Error::other("the line is gone"))
    }

    /// How many entries are kept now.
    pub fn cached(&self) -> usize {
        self.cache.lock().unwrap().len()
    }

    /// The file's lines to read in a Scan.
    pub fn texts(&self) -> FileTexts {
        FileTexts { source: self.source.clone(), spans: self.indexed.clone() }
    }

    /// Memory the index of the file holds (not the cache).
    pub fn heap_bytes(&self) -> usize {
        let data = self.indexed.data.read().unwrap();
        let ids = data.filter.columns.iter().filter(|c| c.has_row_ids()).count() * 2 * data.file.len();
        data.file.heap_bytes() + ids
    }
}

/// The fingerprint of the first `end` bytes of `source` (fewer when the
/// source is shorter now: `len` says how many were there).
fn fingerprint_of(source: &dyn LineSource, end: u64) -> Fingerprint {
    let len = source.len().unwrap_or(0).min(end);
    let n = len.min(FINGERPRINT_BYTES);
    let hash = |at: u64| source.read_range(at, n as usize).map(|b| fnv(&b)).unwrap_or(0);
    Fingerprint { identity: source.identity(), len, head: hash(0), tail: hash(len - n), seen: None }
}

/// FNV-1a: the same across runs.
fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |hash, &b| (hash ^ u64::from(b)).wrapping_mul(0x100000001b3))
}
