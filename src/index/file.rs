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
    /// Every line's text, once something needed all of them (the Raw View).
    all_lines: Mutex<Option<Arc<Vec<gpui::SharedString>>>>,
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
}

impl std::fmt::Debug for IndexedFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IndexedFile").field("lines", &self.len()).finish()
    }
}

impl IndexedFile {
    /// The file `source` as Indexing (with `options`) read it into `indexed`.
    pub fn new(source: Arc<dyn LineSource>, indexed: Arc<Indexed>, options: IndexOptions) -> Self {
        Self { source, indexed, options, cache: Mutex::new(RowCache::new(ROW_CACHE)), all_lines: Mutex::default() }
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
