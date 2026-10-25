//! What the open view shows, without a copy of its lines (openspec change
//! `line-offset-index`, task 4.3): each row is a line of one of its files,
//! as (file, position in that file's entries), and the entries stay in the
//! files' cache. What the view adds to a line (its Source File, its
//! Markers) is looked up per row.

use crate::index::view_rows::{pack, unpack};
use crate::marker::TextMarker;
use crate::model::LogEntry;
use crate::rows::Rows;
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
        if self.markers.is_empty() {
            return &[];
        }
        let ago = self.generation.saturating_sub(entry.generation);
        self.markers.get(&(entry.index, ago)).map_or(&[], Vec::as_slice)
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
        self.bind_with(|id| parsed.get(id).map(|p| p.entries.as_slice()))
    }

    /// The view with the entries `entries` gives for each file id.
    pub fn bind_with<'a>(&'a self, entries: impl Fn(&str) -> Option<&'a [LogEntry]>) -> BoundRows<'a> {
        let entries = self.files.iter().map(|f| entries(&f.file_id).unwrap_or(&[])).collect();
        BoundRows { data: self, entries }
    }

    /// The rows of file `file` at `positions`, in order, packed.
    pub fn rows_of(file: usize, positions: impl IntoIterator<Item = usize>) -> impl Iterator<Item = u64> {
        positions.into_iter().map(move |p| pack(file, p))
    }
}

/// The texts of a view's rows, for a Scan on a worker thread: the rows, and
/// each file's lines as stored, shared with the file (nothing is copied).
pub struct ViewTexts {
    rows: Vec<u64>,
    lines: Vec<Arc<Vec<gpui::SharedString>>>,
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
        let texts: Vec<&str> = rows
            .iter()
            .map(|&r| {
                let (file, pos) = unpack(r);
                self.lines.get(file).and_then(|l| l.get(pos)).map_or("", |s| s.as_str())
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
            .map(|f| parsed.get(&*f.file_id).map(|p| p.lines.clone()).unwrap_or_default())
            .collect();
        ViewTexts { rows: self.rows.clone(), lines }
    }
}

/// The rows of a view, readable: the view and its files' entries.
pub struct BoundRows<'a> {
    pub data: &'a ViewData,
    entries: Vec<&'a [LogEntry]>,
}

impl<'a> BoundRows<'a> {
    /// The entry of a packed row.
    pub fn entry_of(&self, packed: u64) -> Option<&'a LogEntry> {
        let (file, pos) = unpack(packed);
        self.entries.get(file)?.get(pos)
    }

    // The `Rows` methods, for references that outlive this value (it is
    // made for each read).

    pub fn len(&self) -> usize {
        self.data.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.rows.is_empty()
    }

    pub fn get(&self, row: usize) -> Option<&'a LogEntry> {
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
        match (data.files.get(file), self.get(row)) {
            (Some(f), Some(entry)) => f.markers_of(entry),
            _ => &[],
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

    fn get(&self, row: usize) -> Option<&LogEntry> {
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

    fn get(&self, row: usize) -> Option<&LogEntry> {
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
        match (self.bound.data.files.get(file), self.bound.entry_of(packed)) {
            (Some(f), Some(entry)) => f.markers_of(entry),
            _ => &[],
        }
    }
}

/// Copies of the rows of `rows` with what the view adds to them set on
/// them: the form the view had before it stopped copying its lines (tests
/// compare with it).
pub fn materialize<R: Rows + ?Sized>(rows: &R) -> Vec<LogEntry> {
    (0..rows.len())
        .filter_map(|r| {
            let mut entry = rows.get(r)?.clone();
            entry.file_id = rows.file_id(r).map(Arc::from);
            entry.source = rows.source(r).map(Arc::from);
            entry.markers = rows.markers(r).to_vec();
            Some(entry)
        })
        .collect()
}
