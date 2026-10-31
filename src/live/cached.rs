//! A Remote File read through its Remote Cache (openspec change
//! `remote-local-cache`, stage 2): the lines its source sends are written
//! to the cache on disk and read through a Line Index, so their text is not
//! held in memory. It keeps what `LiveBuffer` keeps of a remote file (how
//! its lines are numbered, its Generations, the Breaks between its lines,
//! and Clear), and gives the same rows.
//!
//! A Remote Command's output is kept the same way, with the output each
//! line came from; each Run is a Generation, and the last one kept shows
//! again under a Break "previous run" when its tab is chosen again.

use super::buffer::{BreakMark, ClearMark, HeldStart, PROVISIONAL_BASE, Trimmed, size_text};
use super::source::{BreakKind, EarlierFile, EarlierStart, RestartKind, StreamMask, clock_text};
use crate::index::file::{GenerationStart, IndexedFile};
use crate::index::indexing::IndexOptions;
use crate::index::meta::Runs;
use crate::model::Stream;
use crate::index::{ChainSource, HandleBudget, LineSource};
use crate::remote::cache::RemoteCache;
use std::collections::BTreeMap;
use std::io;
use std::sync::Arc;

/// Breaks that only say something about the top of the lines held: they go
/// when older lines come above it.
fn is_top_mark(kind: BreakKind) -> bool {
    matches!(
        kind,
        BreakKind::LeftWindow
            | BreakKind::EarlierAvailable
            | BreakKind::EarlierWindowFull
            | BreakKind::StartTimeOverflow
    )
}

/// What applying an event changed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Applied {
    /// Lines were added at these positions, after the others.
    pub appended: Option<std::ops::Range<usize>>,
    /// The rows were renumbered, inserted above, or dropped: the view is
    /// built again.
    pub structural: bool,
    /// Every Generation held moved one up (an earlier one came above).
    pub generations_moved: bool,
}

pub struct CachedFile {
    name: String,
    cache: RemoteCache,
    budget: Arc<HandleBudget>,
    options: IndexOptions,
    chain: Arc<ChainSource>,
    file: Arc<IndexedFile>,
    /// Segments in the chain, to know when the cache started a new one.
    segments: usize,
    /// The newest Generation, as the source counts them.
    pub generation: u32,
    /// Number of the next line of the newest Generation.
    next_index: usize,
    numbers_known: bool,
    /// Number of the last line that came with `Tail` (what `TotalLines`
    /// counts up to).
    tail_last_index: Option<usize>,
    /// Offset after the last line of the newest Generation on the Server.
    next_offset: u64,
    pending: Option<BreakMark>,
    disconnected_at: Option<String>,
    pub clear_mark: Option<ClearMark>,
    inodes: Vec<(u32, Option<u64>)>,
    /// Breaks above lines, by (Generation, line number).
    marks: BTreeMap<(u32, usize), Arc<BreakMark>>,
    /// A Remote Command, and the output each line held came from, by
    /// position.
    is_command: bool,
    streams: Runs<Option<Stream>>,
}

/// Lines of the last Run handed to a Remote Command run again, to resume
/// from the time of the last one.
const PREVIOUS_LINES: usize = 1_000;

impl CachedFile {
    /// A Remote File with nothing yet, its lines kept in `cache`.
    pub fn new(name: impl Into<String>, mut cache: RemoteCache, options: IndexOptions, budget: Arc<HandleBudget>) -> io::Result<Self> {
        cache.clear()?;
        cache.begin_generation(PROVISIONAL_BASE as u64)?;
        let (chain, file, segments) = Self::index(&cache, &options, &budget, 0, &BTreeMap::new())?;
        Ok(Self {
            name: name.into(),
            cache,
            budget,
            options,
            chain,
            file,
            segments,
            generation: 0,
            next_index: PROVISIONAL_BASE,
            numbers_known: false,
            tail_last_index: None,
            next_offset: 0,
            pending: None,
            disconnected_at: None,
            clear_mark: None,
            inodes: Vec::new(),
            marks: BTreeMap::new(),
            is_command: false,
            streams: Runs::default(),
        })
    }

    /// The output of a Remote Command with nothing yet, kept in `cache`:
    /// its lines are numbered from 1 in the order it printed them.
    pub fn command(name: impl Into<String>, cache: RemoteCache, options: IndexOptions, budget: Arc<HandleBudget>) -> io::Result<Self> {
        let mut file = Self::new(name, cache, options, budget)?;
        file.cache.set_ended(None)?;
        file.cache.set_first_line_no(0, 1)?;
        file.is_command = true;
        file.next_index = 1;
        file.numbers_known = true;
        file.place_marks();
        Ok(file)
    }

    /// A Remote Command as its cache kept it: the output of its Runs below
    /// a Break "previous run", and the last lines of the newest, oldest
    /// first, for the command to resume from. `None` when nothing is kept.
    pub fn restore_command(
        name: impl Into<String>,
        cache: RemoteCache,
        options: IndexOptions,
        budget: Arc<HandleBudget>,
    ) -> io::Result<Option<(Self, Vec<(Stream, String)>)>> {
        let name = name.into();
        let meta = cache.meta();
        let Some(newest) = meta.generations.last() else { return Ok(None) };
        if cache.lines() == 0 {
            return Ok(None);
        }
        let next_index = (newest.first_line_no + newest.lines() as u64) as usize;
        let generation = meta.generations.iter().skip(1).filter(|g| !g.part).count() as u32;
        let ended = meta.ended.clone();
        let (chain, file, segments) = Self::index(&cache, &options, &budget, generation, &BTreeMap::new())?;
        let mut restored = Self {
            name,
            cache,
            budget,
            options,
            chain,
            file,
            segments,
            generation,
            next_index,
            numbers_known: true,
            tail_last_index: None,
            next_offset: 0,
            pending: None,
            disconnected_at: None,
            clear_mark: None,
            inodes: Vec::new(),
            marks: BTreeMap::new(),
            is_command: true,
            streams: Runs::default(),
        };
        restored.streams = restored.streams_from_cache();
        let gens = restored.cache.meta().generations.clone();
        for (i, g) in gens.iter().enumerate().skip(1) {
            if g.lines() > 0 && !g.part {
                let label = format!("{} restarted", restored.name);
                let kind = BreakKind::Restarted(RestartKind::Manual);
                restored.marks.insert((restored.number_of(i), g.first_line_no as usize), Arc::new(BreakMark { kind, label }));
            }
        }
        let when = ended.map(|t| format!(" {t}")).unwrap_or_default();
        let label = format!("{} previous run{when}", restored.name);
        restored.set_top_mark(BreakMark { kind: BreakKind::PreviousRun, label });
        restored.place_marks();
        let len = restored.len();
        // The newest Generation: its last parts and the one they go on from.
        let first_of_newest = gens.iter().rposition(|g| !g.part).unwrap_or(0);
        let newest_from = len - gens[first_of_newest..].iter().map(|g| g.lines()).sum::<usize>();
        let from = newest_from.max(len.saturating_sub(PREVIOUS_LINES));
        let previous = (from..len)
            .filter_map(|i| {
                let line = restored.file.line(i)?;
                Some((restored.streams.get(i).flatten().unwrap_or(Stream::Stdout), line.to_string()))
            })
            .collect();
        Ok(Some((restored, previous)))
    }

    pub fn is_command(&self) -> bool {
        self.is_command
    }

    /// The output of each line held, from the cache.
    fn streams_from_cache(&self) -> Runs<Option<Stream>> {
        let mut runs = Runs::default();
        for (i, code) in self.cache.streams().into_iter().enumerate() {
            runs.set(i, crate::remote::cache::stream_of(code));
        }
        runs
    }

    /// The tab of a Remote Command is left at `time`: everything held is
    /// kept, shown under a Break "previous run" next time.
    pub fn close_command(mut self, time: &str) -> io::Result<()> {
        self.cache.set_ended(Some(time.to_string()))
    }

    /// A Remote File as its cache kept it when its tab was left: its lines,
    /// numbered as on the Server, and where its source reads on from.
    /// `None` when the cache holds nothing it can read on from.
    pub fn restore(
        name: impl Into<String>,
        cache: RemoteCache,
        options: IndexOptions,
        budget: Arc<HandleBudget>,
    ) -> io::Result<Option<(Self, super::remote::ResumeFrom)>> {
        let name = name.into();
        let meta = cache.meta();
        let Some(newest) = meta.generations.last() else { return Ok(None) };
        let cp = newest.checkpoint.clone();
        let (Some(head_hash), Some(tail_hash)) = (cp.head_hash, cp.tail_hash) else { return Ok(None) };
        if cache.lines() == 0 || meta.generations.iter().any(|g| !super::buffer::is_real_line_number(g.first_line_no as usize)) {
            return Ok(None);
        }
        let next_index = (newest.first_line_no + newest.lines() as u64) as usize;
        let (chain, file, segments) = Self::index(&cache, &options, &budget, cp.generation, &BTreeMap::new())?;
        let mut restored = Self {
            name,
            cache,
            budget,
            options,
            chain,
            file,
            segments,
            generation: cp.generation,
            next_index,
            numbers_known: true,
            tail_last_index: None,
            next_offset: cp.server_end,
            pending: None,
            disconnected_at: None,
            clear_mark: None,
            inodes: vec![(cp.generation, cp.inode)],
            marks: BTreeMap::new(),
            is_command: false,
            streams: Runs::default(),
        };
        // A Break "rotated" between the Generations kept.
        let gens = restored.cache.meta().generations.clone();
        for (i, g) in gens.iter().enumerate().skip(1) {
            if g.lines() > 0 {
                let label = format!("{} rotated", restored.name);
                restored.marks.insert((restored.number_of(i), g.first_line_no as usize), Arc::new(BreakMark { kind: BreakKind::Rotated, label }));
            }
        }
        restored.place_marks();
        let from = super::remote::ResumeFrom { generation: cp.generation, inode: cp.inode, server_end: cp.server_end, head_hash, tail_hash };
        Ok(Some((restored, from)))
    }

    /// The tab is left: the lines received after the last Checkpoint go
    /// (the source sends them again next time), so the cache ends where
    /// the Checkpoint says. When they do not line up, the cache is left
    /// with no Checkpoint and is read anew next time.
    pub fn close(mut self) -> io::Result<()> {
        let newest = self.newest();
        let cp = self.cache.meta().generations[newest].checkpoint.clone();
        let lens = self.cache.lens_of(newest);
        let lined_up = cp.generation == self.generation && cp.head_hash.is_some() && {
            let mut extra = self.next_offset.checked_sub(cp.server_end);
            let mut count = 0;
            for len in lens.iter().rev() {
                match extra {
                    Some(0) | None => break,
                    Some(e) if u64::from(*len) <= e && *len > 0 => {
                        extra = Some(e - u64::from(*len));
                        count += 1;
                    }
                    _ => {
                        extra = None;
                        break;
                    }
                }
            }
            match extra {
                Some(0) => {
                    self.cache.drop_bottom(count)?;
                    true
                }
                _ => false,
            }
        };
        if !lined_up {
            self.cache.set_checkpoint(crate::remote::cache::Checkpoint::default())?;
        }
        Ok(())
    }

    /// The lines as a Line Index reads them.
    pub fn file(&self) -> Arc<IndexedFile> {
        self.file.clone()
    }

    pub fn len(&self) -> usize {
        self.cache.lines()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn cache(&self) -> &RemoteCache {
        &self.cache
    }

    // ── Generations and positions ───────────────────────────────────────────

    /// The source's number of the Generation at index `i` of the cache: a
    /// part has the number of the one before.
    fn number_of(&self, i: usize) -> u32 {
        let later = self.cache.meta().generations.iter().skip(i + 1).filter(|g| !g.part).count();
        self.generation.saturating_sub(later as u32)
    }

    /// Index in the cache of the newest Generation.
    fn newest(&self) -> usize {
        self.cache.meta().generations.len() - 1
    }

    /// The oldest line held, as (Generation, number).
    fn top(&self) -> Option<(u32, usize)> {
        let gens = &self.cache.meta().generations;
        let i = gens.iter().position(|g| g.lines() > 0)?;
        Some((self.number_of(i), gens[i].first_line_no as usize))
    }

    /// Where each line key is now, and the Generation starts.
    fn layout(&self) -> (Vec<GenerationStart>, BTreeMap<usize, Arc<BreakMark>>) {
        let mut starts = Vec::new();
        let mut at = 0;
        let mut by_number: BTreeMap<u32, Vec<(usize, usize, usize)>> = BTreeMap::new();
        for (i, g) in self.cache.meta().generations.iter().enumerate() {
            let number = self.number_of(i);
            starts.push(GenerationStart { pos: at, first_number: g.first_line_no as usize, generation: number, mark: None });
            by_number.entry(number).or_default().push((at, g.first_line_no as usize, g.lines()));
            at += g.lines();
        }
        let marks = self
            .marks
            .iter()
            .filter_map(|((generation, line), mark)| {
                let parts = by_number.get(generation)?;
                let (pos, first, _) = parts.iter().find(|(_, first, count)| *line >= *first && *line < first + count)?;
                Some((pos + line - first, mark.clone()))
            })
            .collect();
        (starts, marks)
    }

    /// The Breaks by position, for the Raw View.
    pub fn marks_by_position(&self) -> BTreeMap<usize, Arc<BreakMark>> {
        self.layout().1
    }

    fn index(
        cache: &RemoteCache,
        options: &IndexOptions,
        budget: &Arc<HandleBudget>,
        _generation: u32,
        _marks: &BTreeMap<(u32, usize), Arc<BreakMark>>,
    ) -> io::Result<(Arc<ChainSource>, Arc<IndexedFile>, usize)> {
        let (chain, _) = cache.chain(budget.clone())?;
        let chain = Arc::new(chain);
        let file = IndexedFile::index_now(chain.clone() as Arc<dyn LineSource>, options.clone()).map_err(io::Error::other)?;
        Ok((chain, Arc::new(file), cache.segment_paths().len()))
    }

    /// Read the cache again into a new Line Index: lines came above, left,
    /// or were renumbered.
    fn rebuild(&mut self) -> io::Result<()> {
        let (chain, file, segments) = Self::index(&self.cache, &self.options, &self.budget, self.generation, &self.marks)?;
        (self.chain, self.file, self.segments) = (chain, file, segments);
        if self.is_command {
            self.streams = self.streams_from_cache();
        }
        self.place_marks();
        Ok(())
    }

    /// Tell the Line Index where the Generations start and the Breaks are.
    fn place_marks(&self) {
        let (starts, marks) = self.layout();
        self.file.set_generations(starts);
        self.file.set_marks(marks);
        if self.is_command {
            self.file.set_streams(self.streams.clone());
        }
    }

    /// Lines were added at the end: index them, or read everything again
    /// when the cache started a new segment.
    fn grow(&mut self) -> io::Result<()> {
        let segments = self.cache.segment_paths();
        if segments.len() != self.segments {
            // The new segments join the chain where the last one ends.
            for (_, path) in &segments[self.segments..] {
                let source = crate::index::DiskSource::on_demand(path.clone(), self.budget.clone());
                self.chain.push(Arc::new(source))?;
            }
            self.segments = segments.len();
        }
        self.file.extend()?;
        self.place_marks();
        Ok(())
    }

    fn add_pending(&mut self, mark: BreakMark) {
        self.pending = Some(match self.pending.take() {
            Some(old) => BreakMark { kind: mark.kind, label: format!("{}; {}", old.label, mark.label) },
            None => mark,
        });
    }

    fn set_top_mark(&mut self, mark: BreakMark) {
        if let Some(top) = self.top() {
            self.marks.insert(top, Arc::new(mark));
        }
    }

    fn top_mark(&self) -> Option<&Arc<BreakMark>> {
        self.top().and_then(|t| self.marks.get(&t))
    }

    fn remove_top_marks(&mut self) {
        if let Some(top) = self.top()
            && self.marks.get(&top).is_some_and(|m| is_top_mark(m.kind))
        {
            self.marks.remove(&top);
        }
    }

    /// Lines at the end of the newest Generation, numbered on from
    /// `next_index`; a pending Break goes above the first.
    fn push(&mut self, lines: &[String], bytes: &[u32]) -> io::Result<std::ops::Range<usize>> {
        self.push_streams(lines, bytes, None)
    }

    fn push_streams(&mut self, lines: &[String], bytes: &[u32], streams: Option<&StreamMask>) -> io::Result<std::ops::Range<usize>> {
        let from = self.len();
        if lines.is_empty() {
            return Ok(from..from);
        }
        if let Some(mark) = self.pending.take() {
            self.marks.insert((self.generation, self.next_index), Arc::new(mark));
        }
        let newest = self.newest();
        if self.cache.meta().generations[newest].lines() == 0 {
            self.cache.set_first_line_no(newest, self.next_index as u64)?;
        }
        let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
        if self.is_command {
            let of = |i: usize| streams.and_then(|s| s.get(i));
            let codes: Vec<u8> = (0..lines.len()).map(|i| crate::remote::cache::stream_code(of(i))).collect();
            self.cache.append_streams(&refs, bytes, &codes)?;
            for i in 0..lines.len() {
                self.streams.set(from + i, of(i));
            }
        } else {
            self.cache.append_with(&refs, bytes)?;
        }
        self.next_index += lines.len();
        self.next_offset += bytes.iter().map(|b| u64::from(*b)).sum::<u64>();
        Ok(from..self.len())
    }

    // ── Events ──────────────────────────────────────────────────────────────

    /// The last lines of the file, at the end.
    pub fn apply_tail(&mut self, lines: &[String], bytes: &[u32], end_offset: u64) -> io::Result<Applied> {
        self.push(lines, bytes)?;
        self.next_offset = end_offset;
        if !lines.is_empty() {
            self.tail_last_index = Some(self.next_index - 1);
        }
        self.rebuild()?;
        Ok(Applied { structural: true, ..Applied::default() })
    }

    /// Older lines, above everything in the newest Generation.
    pub fn apply_backfill(&mut self, lines: &[String], bytes: &[u32]) -> io::Result<Applied> {
        if lines.is_empty() {
            return Ok(Applied::default());
        }
        let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
        let newest = self.newest();
        self.cache.prepend_with(newest, &refs, bytes)?;
        self.rebuild()?;
        Ok(Applied { structural: true, ..Applied::default() })
    }

    /// New lines at the end.
    pub fn apply_append(&mut self, lines: &[String], bytes: &[u32]) -> io::Result<Applied> {
        let range = self.push(lines, bytes)?;
        if range.is_empty() {
            return Ok(Applied::default());
        }
        self.grow()?;
        Ok(Applied { appended: Some(range), ..Applied::default() })
    }

    /// New lines of a Remote Command at the end, with the output each came
    /// from.
    pub fn apply_append_streams(&mut self, lines: &[String], bytes: &[u32], streams: Option<&StreamMask>) -> io::Result<Applied> {
        let range = self.push_streams(lines, bytes, streams)?;
        if range.is_empty() {
            return Ok(Applied::default());
        }
        self.grow()?;
        Ok(Applied { appended: Some(range), ..Applied::default() })
    }

    /// The real number of the last line of the tail is known.
    pub fn apply_total(&mut self, total: Option<usize>) -> io::Result<Option<isize>> {
        let (Some(total), Some(last)) = (total, self.tail_last_index) else { return Ok(None) };
        if self.numbers_known {
            return Ok(None);
        }
        let delta = total as isize - last as isize;
        let newest = self.newest();
        let first = self.cache.meta().generations[newest].first_line_no as isize + delta;
        self.cache.set_first_line_no(newest, first as u64)?;
        let generation = self.generation;
        let moved: Vec<_> = self.marks.keys().filter(|(g, _)| *g == generation).copied().collect();
        for key in moved {
            if let Some(mark) = self.marks.remove(&key) {
                self.marks.insert((key.0, (key.1 as isize + delta) as usize), mark);
            }
        }
        self.next_index = (self.next_index as isize + delta) as usize;
        self.tail_last_index = Some(total);
        self.numbers_known = true;
        self.rebuild()?;
        Ok(Some(delta))
    }

    /// The file is not continuous here: a Break above the next line, and a
    /// new Generation after a rotation, truncation or deletion, or a Remote
    /// Command run again. The labels are `LiveBuffer::apply_break`'s.
    pub fn apply_break(&mut self, kind: BreakKind, time: &str) -> io::Result<()> {
        let name = self.name.clone();
        let label = match kind {
            BreakKind::Disconnected => {
                self.disconnected_at = Some(time.to_string());
                return Ok(());
            }
            BreakKind::Reconnected => {
                let since = self.disconnected_at.take().unwrap_or_else(|| time.to_string());
                format!("{name} disconnected {since} – reconnected {time}")
            }
            BreakKind::Resumed(seconds) => {
                let since = self.disconnected_at.take().unwrap_or_else(|| time.to_string());
                format!("{name} disconnected {since} – reconnected {time}, resumed from {}", clock_text(seconds))
            }
            BreakKind::Exited(code) => {
                let how = match code {
                    Some(code) => format!("exited ({code})"),
                    None => "exited (killed)".to_string(),
                };
                format!("{name} {how} {time}")
            }
            BreakKind::Skipped(count) => {
                // The numbers of the dropped lines are not given again: the
                // lines after them are a part of their own.
                self.next_index += count as usize;
                let newest = self.newest();
                if self.cache.meta().generations[newest].lines() > 0 {
                    self.cache.begin_part(self.next_index as u64)?;
                    self.rebuild()?;
                }
                format!("{name} {} {time}", kind.label())
            }
            BreakKind::Restarted(why) => match why {
                RestartKind::Manual | RestartKind::OnExit => format!("{name} restarted {time}"),
                RestartKind::AfterDisconnect => {
                    let since = self.disconnected_at.take().unwrap_or_else(|| time.to_string());
                    format!("{name} disconnected {since} – reconnected {time}, command restarted, lines may be missing or repeated")
                }
                RestartKind::ResumeFailed => {
                    format!("{name} restarted {time}, resume not supported, lines may be missing or repeated")
                }
                RestartKind::Reopened(from) => super::buffer::reopened_label(&name, time, from),
            },
            _ => format!("{name} {} {time}", kind.label()),
        };
        if kind.starts_generation() {
            self.begin_generation()?;
        }
        self.add_pending(BreakMark { kind, label });
        Ok(())
    }

    /// The lines after this are a new Generation, numbered from 1.
    fn begin_generation(&mut self) -> io::Result<()> {
        self.generation += 1;
        self.next_index = 1;
        self.next_offset = 0;
        self.numbers_known = true;
        self.tail_last_index = None;
        self.cache.begin_generation(1)?;
        self.rebuild()
    }

    /// The Remote Command was edited: the Break names both commands, and
    /// the lines after it are a new Generation.
    pub fn apply_command_changed(&mut self, from: &str, to: &str, time: &str) -> io::Result<()> {
        self.begin_generation()?;
        self.name = to.to_string();
        self.disconnected_at = None;
        self.add_pending(BreakMark { kind: BreakKind::CommandChanged, label: format!("command changed {time}: {from} → {to}") });
        Ok(())
    }

    /// Lines of a Rotated File above everything held; with `start` they
    /// begin an earlier Generation and every Generation moves one up.
    pub fn apply_earlier(
        &mut self,
        start: Option<&EarlierStart>,
        lines: &[String],
        bytes: &[u32],
        last_line_no: Option<usize>,
    ) -> io::Result<Applied> {
        if lines.is_empty() && start.is_none() {
            return Ok(Applied::default());
        }
        let count = lines.len();
        let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
        match start {
            Some(start) => {
                let old_top = self.top();
                self.remove_top_marks();
                // Every Generation moves one up.
                self.generation += 1;
                self.marks = std::mem::take(&mut self.marks).into_iter().map(|((g, l), m)| ((g + 1, l), m)).collect();
                for (g, _) in self.inodes.iter_mut() {
                    *g += 1;
                }
                if let Some(mark) = self.clear_mark.as_mut() {
                    mark.generation += 1;
                }
                let first = match last_line_no {
                    Some(last) => (last + 1).saturating_sub(count),
                    None => PROVISIONAL_BASE,
                };
                self.cache.add_earlier_generation_with(&start.name, first as u64, &refs, bytes)?;
                if let Some((g, l)) = old_top {
                    let when = start.modified.as_deref().map(|m| format!(" {m}")).unwrap_or_default();
                    let mark = BreakMark { kind: BreakKind::Rotated, label: format!("{} rotated{when}", self.name) };
                    self.marks.insert((g + 1, l), Arc::new(mark));
                }
                self.rebuild()?;
                Ok(Applied { structural: true, generations_moved: true, ..Applied::default() })
            }
            None => {
                let Some(top) = self.cache.meta().generations.iter().position(|g| g.lines() > 0) else {
                    return Ok(Applied::default());
                };
                self.remove_top_marks();
                self.cache.prepend_with(top, &refs, bytes)?;
                self.rebuild()?;
                Ok(Applied { structural: true, ..Applied::default() })
            }
        }
    }

    /// The Break at the top that offers the next Rotated File.
    pub fn set_earlier_available(&mut self, file: Option<&EarlierFile>, window_full: bool) -> io::Result<()> {
        match file {
            Some(file) => {
                if self.top_mark().is_none() && self.top().is_some() {
                    let size = size_text(file.size);
                    let (kind, label) = if window_full {
                        (BreakKind::EarlierWindowFull, format!("Live Window is full; earlier generation: {}, {size}", file.name))
                    } else {
                        (BreakKind::EarlierAvailable, format!("Load earlier generation ({}, {size})", file.name))
                    };
                    self.set_top_mark(BreakMark { kind, label });
                }
            }
            None => {
                if let Some(top) = self.top()
                    && self.marks.get(&top).is_some_and(|m| matches!(m.kind, BreakKind::EarlierAvailable | BreakKind::EarlierWindowFull))
                {
                    self.marks.remove(&top);
                }
            }
        }
        self.place_marks();
        Ok(())
    }

    /// The Live Window filled before the Starting Time.
    pub fn set_start_overflow(&mut self, lines: Option<usize>, since: &str) {
        let count = lines.map(|n| format!("{} lines", crate::count::format_count(n))).unwrap_or_else(|| "Lines".to_string());
        let label = format!("{count} since {since} do not fit the Live Window");
        self.set_top_mark(BreakMark { kind: BreakKind::StartTimeOverflow, label });
        self.place_marks();
    }

    pub fn set_inode(&mut self, inode: Option<u64>) {
        let generation = self.generation;
        match self.inodes.iter_mut().find(|(g, _)| *g == generation) {
            Some(slot) => slot.1 = inode,
            None => self.inodes.push((generation, inode)),
        }
    }

    pub fn inode_of(&self, generation: u32) -> Option<u64> {
        self.inodes.iter().find(|(g, _)| *g == generation).and_then(|(_, i)| *i)
    }

    /// Where the newest Generation's file stood on the Server.
    pub fn set_checkpoint(&mut self, checkpoint: crate::remote::cache::Checkpoint) -> io::Result<()> {
        self.cache.set_checkpoint(checkpoint)
    }

    // ── The Live Window and Clear ───────────────────────────────────────────

    /// Keep at most `window` lines, as `LiveBuffer::trim_to_window`.
    pub fn trim_to_window(&mut self, window: usize) -> io::Result<Option<Trimmed>> {
        let len = self.len();
        if window == 0 || len <= window + window / 10 {
            return Ok(None);
        }
        self.trim_to(window).map(Some)
    }

    fn trim_to(&mut self, keep: usize) -> io::Result<Trimmed> {
        let removed = self.len() - keep;
        self.cache.drop_top(removed)?;
        // The Breaks of the lines that left go with them.
        let kept = self.layout_keys();
        self.marks.retain(|key, _| kept(key));
        let (generation, first_kept) = self.top().unwrap_or((self.generation, self.next_index));
        let label = format!("{} older lines left the window", self.name);
        self.set_top_mark(BreakMark { kind: BreakKind::LeftWindow, label });
        self.rebuild()?;
        Ok(Trimmed { removed, generation, first_kept, text_bytes: 0 })
    }

    /// Whether a line key is still held.
    fn layout_keys(&self) -> impl Fn(&(u32, usize)) -> bool + use<> {
        let held: Vec<(u32, usize, usize)> = self
            .cache
            .meta()
            .generations
            .iter()
            .enumerate()
            .map(|(i, g)| (self.number_of(i), g.first_line_no as usize, g.lines()))
            .collect();
        move |(g, l)| held.iter().any(|(hg, first, count)| hg == g && *l >= *first && *l < first + count)
    }

    /// Drop every line, with a Break "cleared" above the next one.
    pub fn clear(&mut self, time: &str) -> io::Result<Trimmed> {
        let removed = self.len();
        self.cache.clear()?;
        let newest = self.newest();
        self.cache.set_first_line_no(newest, self.next_index as u64)?;
        self.marks.clear();
        self.clear_mark = Some(ClearMark { generation: self.generation, line_no: self.next_index, offset: self.next_offset, time: time.to_string() });
        self.add_pending(BreakMark { kind: BreakKind::Cleared, label: format!("{} {} {time}", self.name, BreakKind::Cleared.label()) });
        self.rebuild()?;
        Ok(Trimmed { removed, generation: self.generation, first_kept: usize::MAX, text_bytes: 0 })
    }

    /// Where the oldest held line starts in the file on the Server.
    pub fn held_start(&self) -> HeldStart {
        let gens = &self.cache.meta().generations;
        let Some(i) = gens.iter().position(|g| g.lines() > 0) else {
            return HeldStart { generation: self.generation, line_no: self.next_index, offset: Some(self.next_offset) };
        };
        let (generation, line_no) = (self.number_of(i), gens[i].first_line_no as usize);
        if generation != self.generation {
            return HeldStart { generation, line_no, offset: None };
        }
        let held: u64 = self.cache.lens_of(i).iter().map(|b| u64::from(*b)).sum();
        HeldStart { generation, line_no, offset: self.next_offset.checked_sub(held) }
    }

    /// Bytes on the Server of the lines held of the oldest Generation held.
    pub fn top_generation_bytes(&self) -> u64 {
        let gens = &self.cache.meta().generations;
        let Some(i) = gens.iter().position(|g| g.lines() > 0) else { return 0 };
        self.cache.lens_of(i).iter().map(|b| u64::from(*b)).sum()
    }

    /// Whether the file holds up to the first line of its Generation.
    pub fn at_generation_start(&self) -> bool {
        self.numbers_known && self.held_start().line_no <= 1
    }

    /// The text of the oldest line held.
    pub fn first_line(&self) -> Option<gpui::SharedString> {
        self.file.line(0)
    }
}
