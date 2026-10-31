//! A Remote Copy read through a Line Index (openspec change
//! `remote-local-cache`, task 7.1): its Generations are the `gen-*.log`
//! files of its folder, read end to end, oldest first, each numbered as in
//! its file on the Server. While Open in full reads, the lines it writes to
//! the newest file are indexed as they come; opened again, the folder is
//! indexed as it is. The text is never held in memory.

use super::buffer::{BreakMark, PROVISIONAL_BASE};
use super::source::BreakKind;
use crate::index::file::{GenerationStart, IndexedFile};
use crate::index::indexing::IndexOptions;
use crate::index::{BytesSource, ChainSource, DiskSource, HandleBudget, LineSource};
use crate::remote::copy::{self, StopReason};
use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// One Generation of the copy, as the Line Index sees it.
#[derive(Debug, Clone)]
struct Generation {
    /// Its first line's position among the copy's lines.
    pos: usize,
    first_number: usize,
    /// Its first number is known (not counted yet: shown with `~`).
    numbers_known: bool,
}

pub struct CopyFile {
    name: String,
    chain: Arc<ChainSource>,
    file: Arc<IndexedFile>,
    generations: Vec<Generation>,
    /// The newest Generation, counted from the oldest (0).
    pub generation: u32,
    /// Breaks above and below lines, by position.
    before: BTreeMap<usize, Arc<BreakMark>>,
    after: BTreeMap<usize, Arc<BreakMark>>,
    /// The Break above the first line of the next Generation.
    pending: Option<BreakMark>,
}

impl CopyFile {
    /// A copy about to be read into `dir`: nothing yet.
    pub fn reading(name: impl Into<String>, options: IndexOptions) -> io::Result<Self> {
        let chain = Arc::new(ChainSource::new(Arc::new(BytesSource::new(Vec::new()))));
        let file = IndexedFile::index_now(chain.clone() as Arc<dyn LineSource>, options).map_err(io::Error::other)?;
        Ok(Self {
            name: name.into(),
            chain,
            file: Arc::new(file),
            generations: Vec::new(),
            generation: 0,
            before: BTreeMap::new(),
            after: BTreeMap::new(),
            pending: None,
        })
    }

    /// The copy kept in `dir`. A folder without `meta.json` (the app closed
    /// during the read) is read as what it holds, cancelled.
    pub fn open(dir: &Path, name: impl Into<String>, options: IndexOptions, budget: Arc<HandleBudget>) -> io::Result<Self> {
        let meta = match copy::read_meta(dir) {
            Ok(meta) => meta,
            Err(_) => copy::recover_meta(dir)?,
        };
        let mut file = Self::reading(name, options)?;
        for (i, generation) in meta.generations.iter().enumerate() {
            file.begin_generation(&dir.join(&generation.file), generation.first_line_no, i == 0, &budget)?;
            let before = file.len();
            file.grow()?;
            if i == 0 && generation.older_not_read {
                file.mark_older_not_read(&format!("{}: older lines were not read", file.name));
            }
            if let Some(stop) = generation.stop
                && file.len() > before
            {
                file.mark_read_end(stop, &stop.label(meta.limit_lines));
            }
        }
        file.place();
        Ok(file)
    }

    /// The lines as a Line Index reads them.
    pub fn file(&self) -> Arc<IndexedFile> {
        self.file.clone()
    }

    pub fn len(&self) -> usize {
        self.file.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The Breaks above lines, by position, for the Raw View.
    pub fn marks_by_position(&self) -> BTreeMap<usize, Arc<BreakMark>> {
        self.before.clone()
    }

    /// The next Generation, read into `path` (oldest first): its first line
    /// is `first_line_no` in its file, when known.
    pub fn begin_generation(&mut self, path: &Path, first_line_no: Option<usize>, first: bool, budget: &Arc<HandleBudget>) -> io::Result<()> {
        if !first {
            self.generation += 1;
            self.pending = Some(BreakMark { kind: BreakKind::Rotated, label: format!("{} rotated", self.name) });
        }
        // The read may not have made it yet; never cut what it wrote.
        std::fs::OpenOptions::new().create(true).append(true).open(path)?;
        self.chain.push(Arc::new(DiskSource::on_demand(PathBuf::from(path), budget.clone())))?;
        self.generations.push(Generation {
            pos: self.len(),
            first_number: first_line_no.unwrap_or(PROVISIONAL_BASE),
            numbers_known: first_line_no.is_some(),
        });
        self.place();
        Ok(())
    }

    /// The number of the newest Generation's first line is known now: how
    /// far its numbers moved.
    pub fn set_first_line_no(&mut self, first_line_no: usize) -> Option<isize> {
        let g = self.generations.last_mut()?;
        if g.numbers_known {
            return None;
        }
        let delta = first_line_no as isize - g.first_number as isize;
        g.first_number = first_line_no;
        g.numbers_known = true;
        self.place();
        Some(delta)
    }

    /// Index what the read wrote since: the positions of the new lines.
    pub fn grow(&mut self) -> io::Result<std::ops::Range<usize>> {
        let from = self.len();
        loop {
            match self.file.extend()? {
                crate::index::file::Extended::Appended { more: true, .. } => {}
                _ => break,
            }
        }
        if self.len() > from
            && let Some(mark) = self.pending.take()
        {
            self.before.insert(from, Arc::new(mark));
        }
        self.place();
        Ok(from..self.len())
    }

    /// The read stopped short of the end of the newest Generation: a Break
    /// below its last line says why.
    pub fn mark_read_end(&mut self, reason: StopReason, label: &str) {
        let kind = match reason {
            StopReason::Cancelled => BreakKind::ReadCancelled,
            StopReason::FullReadLimit => BreakKind::ReadLimit,
            StopReason::RotatedWhileReading => BreakKind::ReadRotated,
            StopReason::OlderNotRead => BreakKind::OlderNotRead,
        };
        if let Some(last) = self.len().checked_sub(1) {
            self.after.insert(last, Arc::new(BreakMark { kind, label: label.to_string() }));
            self.place();
        }
    }

    /// Older lines were not read: a Break above the first line says so.
    pub fn mark_older_not_read(&mut self, label: &str) {
        if !self.is_empty() {
            self.before.insert(0, Arc::new(BreakMark { kind: BreakKind::OlderNotRead, label: label.to_string() }));
            self.place();
        }
    }

    /// The position of line `line` of the Generation `generations_ago`
    /// before the newest.
    pub fn position_of(&self, generations_ago: u32, line: usize) -> Option<usize> {
        let index = (self.generation.checked_sub(generations_ago))? as usize;
        let g = self.generations.get(index)?;
        let end = self.generations.get(index + 1).map_or(self.len(), |n| n.pos);
        let pos = g.pos + line.checked_sub(g.first_number)?;
        (pos < end).then_some(pos)
    }

    /// Tell the Line Index where the Generations start and the Breaks are.
    fn place(&self) {
        let starts = self
            .generations
            .iter()
            .enumerate()
            .map(|(i, g)| GenerationStart { pos: g.pos, first_number: g.first_number, generation: i as u32, mark: None })
            .collect();
        self.file.set_generations(starts);
        self.file.set_marks(self.before.clone());
        self.file.set_marks_after(self.after.clone());
    }
}

