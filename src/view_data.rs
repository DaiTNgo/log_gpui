//! What the open view shows, without a copy of its lines (openspec change
//! `line-offset-index`, task 4.3): each row is a line of one of its files,
//! as (file, position in that file's entries), and the entries stay in the
//! files' cache. What the view adds to a line (its Source File, its
//! Markers) is looked up per row.

use crate::index::view_rows::{pack, unpack};
use crate::marker::TextMarker;
use crate::model::LogEntry;
use crate::rows::{RowRef, Rows};
use crate::index::file::IndexedFile;
use crate::space_view::ParsedFile;
use crate::workspace::log_space::LogFile;
use std::collections::HashMap;
use std::sync::Arc;

/// A file of the view.
#[derive(Debug, Clone, Default)]
pub struct ViewFile {
    pub file_id: Arc<str>,
    /// The name the Source Column shows (Log Space rows only).
    pub source: Option<Arc<str>>,
    /// The stored Markers, by (line, Generations ago).
    markers: HashMap<(usize, u32), Vec<TextMarker>>,
    /// The file's Generation when the Markers were looked up.
    generation: u32,
}

impl ViewFile {
    pub fn new(file_id: &str, source: Option<&str>) -> Self {
        Self { file_id: Arc::from(file_id), source: source.map(Arc::from), ..Self::default() }
    }

    /// Take the Markers of `file`.
    pub fn set_markers(&mut self, file: &LogFile) {
        self.generation = file.generation;
        self.markers.clear();
        for m in &file.markers {
            self.markers
                .entry((m.line, m.generations_ago))
                .or_default()
                .push(TextMarker { range: m.range.clone(), color: m.color.clone() });
        }
    }

    /// The Markers of `entry`, a line of this file.
    pub fn markers_of(&self, entry: &LogEntry) -> &[TextMarker] {
        self.markers_of_line(entry.index, entry.generation)
    }

    /// The Markers of line `line` of Generation `generation`.
    pub fn markers_of_line(&self, line: usize, generation: u32) -> &[TextMarker] {
        if self.markers.is_empty() {
            return &[];
        }
        let ago = self.generation.saturating_sub(generation);
        self.markers.get(&(line, ago)).map_or(&[], Vec::as_slice)
    }
}

/// The rows of the open view.
#[derive(Debug, Clone, Default)]
pub struct ViewData {
    /// Each row as (file, position in the file's entries), packed.
    pub rows: Vec<u64>,
    pub files: Vec<ViewFile>,
}

impl ViewData {
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// The file and position of `row`.
    pub fn get(&self, row: usize) -> Option<(usize, usize)> {
        self.rows.get(row).map(|&r| unpack(r))
    }

    /// The position in `files` of the file `file_id`.
    pub fn file_index(&self, file_id: &str) -> Option<usize> {
        self.files.iter().position(|f| &*f.file_id == file_id)
    }

    /// Read the Markers of the view's files again from `files`.
    pub fn set_markers<'a>(&mut self, files: impl IntoIterator<Item = &'a LogFile>) {
        for file in files {
            if let Some(at) = self.file_index(&file.file_id) {
                self.files[at].set_markers(file);
            }
        }
    }

    /// The view with its files' entries, to read its rows.
    pub fn bind<'a>(&'a self, parsed: &'a HashMap<String, ParsedFile>) -> BoundRows<'a> {
        self.bind_files(|id| parsed.get(id).map(ParsedFile::file_entries))
    }

    /// The view with the entries `entries` gives for each file id.
    pub fn bind_with<'a>(&'a self, entries: impl Fn(&str) -> Option<&'a [LogEntry]>) -> BoundRows<'a> {
        self.bind_files(|id| entries(id).map(FileEntries::Held))
    }

    /// The view with the lines `files` gives for each file id.
    pub fn bind_files<'a>(&'a self, files: impl Fn(&str) -> Option<FileEntries<'a>>) -> BoundRows<'a> {
        let entries = self.files.iter().map(|f| files(&f.file_id).unwrap_or(FileEntries::Held(&[]))).collect();
        BoundRows { data: self, entries }
    }

    /// The rows of file `file` at `positions`, in order, packed.
    pub fn rows_of(file: usize, positions: impl IntoIterator<Item = usize>) -> impl Iterator<Item = u64> {
        positions.into_iter().map(move |p| pack(file, p))
    }
}

/// Where a file's entries are: held, or read again from the file.
#[derive(Clone, Copy)]
pub enum FileEntries<'a> {
    Held(&'a [LogEntry]),
    Indexed(&'a IndexedFile),
}

impl<'a> FileEntries<'a> {
    pub fn len(&self) -> usize {
        match self {
            FileEntries::Held(entries) => entries.len(),
            FileEntries::Indexed(file) => file.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The entry at `pos`.
    pub fn get(&self, pos: usize) -> Option<RowRef<'a>> {
        match self {
            FileEntries::Held(entries) => entries.get(pos).map(RowRef::Borrowed),
            FileEntries::Indexed(file) => file.entry(pos).map(RowRef::Shared),
        }
    }

    // What the view asks of a line without reading it: from its entry when
    // it is held, from its `LineMeta` when it is not.

    /// The line's number in its file and its Generation.
    pub fn line_number(&self, pos: usize) -> Option<(usize, u32)> {
        match self {
            FileEntries::Held(entries) => entries.get(pos).map(|e| (e.index, e.generation)),
            FileEntries::Indexed(file) => (pos < file.len()).then_some((pos + 1, 0)),
        }
    }

    /// Whether the line has a time of its own.
    pub fn has_time(&self, pos: usize) -> bool {
        match self {
            FileEntries::Held(entries) => entries.get(pos).is_some_and(|e| e.has_time),
            FileEntries::Indexed(file) => file.meta(pos).is_some_and(|m| m.has_time()),
        }
    }

    /// The line's timestamp, when it has a time of its own.
    pub fn own_time(&self, pos: usize) -> Option<f64> {
        match self {
            FileEntries::Held(entries) => entries.get(pos).filter(|e| e.has_time).map(|e| e.timestamp),
            FileEntries::Indexed(file) => file
                .meta(pos)
                .filter(|m| m.has_time() && m.flags & crate::index::meta::flags::OWN_TS != 0)
                .map(|m| crate::index::meta::seconds(m.ts_us)),
        }
    }

    /// Whether the line is a Continuation Line.
    pub fn continues(&self, pos: usize) -> bool {
        match self {
            FileEntries::Held(entries) => entries.get(pos).is_some_and(|e| e.parent.is_some()),
            FileEntries::Indexed(file) => file.meta(pos).is_some_and(|m| m.parent_dist > 0),
        }
    }

    /// The line's level.
    pub fn level(&self, pos: usize) -> Option<crate::model::LogLevel> {
        match self {
            FileEntries::Held(entries) => entries.get(pos).map(|e| e.level),
            FileEntries::Indexed(file) => file.meta(pos).map(|m| m.level()),
        }
    }
}

/// Where a Scan reads a file's lines: held in memory, or from the file.
enum FileText {
    Held(Arc<Vec<gpui::SharedString>>),
    Indexed(crate::index::scan::FileTexts),
}

/// The texts of a view's rows, for a Scan on a worker thread: the rows, and
/// each file's lines, shared with the file (nothing is copied), or read
/// from it.
pub struct ViewTexts {
    rows: Vec<u64>,
    lines: Vec<FileText>,
}

impl ViewTexts {
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

impl crate::index::scan::ScanTexts for ViewTexts {
    fn read(&self, block: std::ops::Range<usize>, each: &mut dyn FnMut(&[&str])) -> Result<(), String> {
        let rows = self.rows.get(block).ok_or("the view has fewer rows")?;
        // A file's rows in a block are a run of its lines, in order: each
        // file read from disk is read once per block.
        let mut read: HashMap<usize, (usize, Vec<String>)> = HashMap::new();
        for (file, text) in self.lines.iter().enumerate() {
            let FileText::Indexed(texts) = text else { continue };
            let mut positions = rows.iter().map(|&r| unpack(r)).filter(|&(f, _)| f == file).map(|(_, p)| p);
            let Some(first) = positions.next() else { continue };
            let last = positions.last().unwrap_or(first);
            let mut lines = Vec::new();
            texts.read(first..last + 1, &mut |got| lines = got.iter().map(|s| s.to_string()).collect())?;
            read.insert(file, (first, lines));
        }
        let texts: Vec<&str> = rows
            .iter()
            .map(|&r| {
                let (file, pos) = unpack(r);
                match self.lines.get(file) {
                    Some(FileText::Held(lines)) => lines.get(pos).map_or("", |s| s.as_str()),
                    Some(FileText::Indexed(_)) => read
                        .get(&file)
                        .and_then(|(first, lines)| lines.get(pos.checked_sub(*first)?))
                        .map_or("", String::as_str),
                    None => "",
                }
            })
            .collect();
        each(&texts);
        Ok(())
    }
}

impl ViewData {
    /// The texts of the rows, to read them on a worker thread.
    pub fn texts(&self, parsed: &HashMap<String, ParsedFile>) -> ViewTexts {
        let lines = self
            .files
            .iter()
            .map(|f| match parsed.get(&*f.file_id) {
                Some(ParsedFile { indexed: Some(file), .. }) => FileText::Indexed(file.texts()),
                Some(p) => FileText::Held(p.lines.clone()),
                None => FileText::Held(Arc::default()),
            })
            .collect();
        ViewTexts { rows: self.rows.clone(), lines }
    }
}

/// The rows of a view, readable: the view and its files' entries.
pub struct BoundRows<'a> {
    pub data: &'a ViewData,
    entries: Vec<FileEntries<'a>>,
}

impl<'a> BoundRows<'a> {
    /// The entry of a packed row.
    pub fn entry_of(&self, packed: u64) -> Option<RowRef<'a>> {
        let (file, pos) = unpack(packed);
        self.entries.get(file)?.get(pos)
    }

    /// The lines of the view's file at `file`.
    pub fn file(&self, file: usize) -> Option<FileEntries<'a>> {
        self.entries.get(file).copied()
    }

    /// The file of `row` and its position there.
    fn at(&self, row: usize) -> Option<(FileEntries<'a>, usize)> {
        let (file, pos) = self.data.get(row)?;
        Some((self.file(file)?, pos))
    }

    /// The number of `row`'s line in its file, and its Generation.
    pub fn line_number(&self, row: usize) -> Option<(usize, u32)> {
        self.at(row).and_then(|(f, pos)| f.line_number(pos))
    }

    /// Whether `row` has a time of its own.
    pub fn has_time(&self, row: usize) -> bool {
        self.at(row).is_some_and(|(f, pos)| f.has_time(pos))
    }

    /// `row`'s own timestamp, when it has a time.
    pub fn own_time(&self, row: usize) -> Option<f64> {
        self.at(row).and_then(|(f, pos)| f.own_time(pos))
    }

    /// `row`'s level.
    pub fn level(&self, row: usize) -> Option<crate::model::LogLevel> {
        self.at(row).and_then(|(f, pos)| f.level(pos))
    }

    // The `Rows` methods, for references that outlive this value (it is
    // made for each read).

    pub fn len(&self) -> usize {
        self.data.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.rows.is_empty()
    }

    pub fn get(&self, row: usize) -> Option<RowRef<'a>> {
        self.entry_of(*self.data.rows.get(row)?)
    }

    pub fn file_id(&self, row: usize) -> Option<&'a str> {
        let data: &'a ViewData = self.data;
        let (file, _) = data.get(row)?;
        data.files.get(file).map(|f| &*f.file_id)
    }

    pub fn source(&self, row: usize) -> Option<&'a str> {
        let data: &'a ViewData = self.data;
        let (file, _) = data.get(row)?;
        data.files.get(file)?.source.as_deref()
    }

    pub fn markers(&self, row: usize) -> &'a [TextMarker] {
        let data: &'a ViewData = self.data;
        let Some((file, _)) = data.get(row) else { return &[] };
        let Some(f) = data.files.get(file) else { return &[] };
        match self.entry_of(data.rows[row]) {
            Some(entry) => f.markers_of_line(entry.index, entry.generation),
            None => &[],
        }
    }

    /// The rows `rows` (packed) of this view, read the same way.
    pub fn picked<'b>(&'b self, rows: &'b [u64]) -> PickedRows<'b, 'a> {
        PickedRows { bound: self, rows }
    }
}

impl Rows for BoundRows<'_> {
    fn len(&self) -> usize {
        BoundRows::len(self)
    }

    fn get(&self, row: usize) -> Option<RowRef<'_>> {
        BoundRows::get(self, row)
    }

    fn file_id(&self, row: usize) -> Option<&str> {
        BoundRows::file_id(self, row)
    }

    fn source(&self, row: usize) -> Option<&str> {
        BoundRows::source(self, row)
    }

    fn markers(&self, row: usize) -> &[TextMarker] {
        BoundRows::markers(self, row)
    }

    fn continues(&self, row: usize) -> bool {
        self.at(row).is_some_and(|(f, pos)| f.continues(pos))
    }
}

/// Some rows of a view (packed), read as rows of their own.
pub struct PickedRows<'b, 'a> {
    bound: &'b BoundRows<'a>,
    rows: &'b [u64],
}

impl Rows for PickedRows<'_, '_> {
    fn len(&self) -> usize {
        self.rows.len()
    }

    fn get(&self, row: usize) -> Option<RowRef<'_>> {
        self.bound.entry_of(*self.rows.get(row)?)
    }

    fn file_id(&self, row: usize) -> Option<&str> {
        let (file, _) = unpack(*self.rows.get(row)?);
        self.bound.data.files.get(file).map(|f| &*f.file_id)
    }

    fn source(&self, row: usize) -> Option<&str> {
        let (file, _) = unpack(*self.rows.get(row)?);
        self.bound.data.files.get(file)?.source.as_deref()
    }

    fn markers(&self, row: usize) -> &[TextMarker] {
        let Some(&packed) = self.rows.get(row) else { return &[] };
        let (file, _) = unpack(packed);
        let Some(f) = self.bound.data.files.get(file) else { return &[] };
        match self.bound.entry_of(packed) {
            Some(entry) => f.markers_of_line(entry.index, entry.generation),
            None => &[],
        }
    }
}

/// Copies of the rows of `rows` with what the view adds to them set on
/// them: the form the view had before it stopped copying its lines (tests
/// compare with it).
pub fn materialize<R: Rows + ?Sized>(rows: &R) -> Vec<LogEntry> {
    (0..rows.len())
        .filter_map(|r| {
            let mut entry = (*rows.get(r)?).clone();
            entry.file_id = rows.file_id(r).map(Arc::from);
            entry.source = rows.source(r).map(Arc::from);
            entry.markers = rows.markers(r).to_vec();
            Some(entry)
        })
        .collect()
}
