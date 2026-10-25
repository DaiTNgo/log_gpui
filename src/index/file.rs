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

/// A file and what Indexing built of it.
pub struct IndexedFile {
    pub source: Arc<dyn LineSource>,
    pub indexed: Arc<Indexed>,
    options: IndexOptions,
    cache: Mutex<RowCache>,
}

impl std::fmt::Debug for IndexedFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IndexedFile").field("lines", &self.len()).finish()
    }
}

impl IndexedFile {
    /// The file `source` as Indexing (with `options`) read it into `indexed`.
    pub fn new(source: Arc<dyn LineSource>, indexed: Arc<Indexed>, options: IndexOptions) -> Self {
        Self { source, indexed, options, cache: Mutex::new(RowCache::new(ROW_CACHE)) }
    }

    /// Index all of `source` now, on this thread.
    pub fn index_now(source: Arc<dyn LineSource>, options: IndexOptions) -> Result<Self, String> {
        let (data, outcome) = index_all(&*source, &options);
        if let Outcome::Failed(error) = outcome {
            return Err(error);
        }
        Ok(Self::new(source, Arc::new(Indexed::from_data(data)), options))
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
