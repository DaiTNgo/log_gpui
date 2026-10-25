//! The rows of the open view, read one row at a time (openspec change
//! `line-offset-index`, task 4.3). What a row shows that is not the line's
//! own (its Source File, its Markers) is read here too, so the view can stop
//! holding a copy of every line with those set on it.

use crate::marker::TextMarker;
use crate::model::LogEntry;
use std::sync::Arc;

/// The entry of a row: borrowed from where its file holds it, or shared
/// from the cache of lines read again from the file.
#[derive(Debug, Clone)]
pub enum RowRef<'a> {
    Borrowed(&'a LogEntry),
    Shared(Arc<LogEntry>),
}

impl std::ops::Deref for RowRef<'_> {
    type Target = LogEntry;

    fn deref(&self) -> &LogEntry {
        match self {
            RowRef::Borrowed(entry) => entry,
            RowRef::Shared(entry) => entry,
        }
    }
}

impl<'a> From<&'a LogEntry> for RowRef<'a> {
    fn from(entry: &'a LogEntry) -> Self {
        RowRef::Borrowed(entry)
    }
}

/// The rows of a view, in the order shown.
pub trait Rows: Sync {
    fn len(&self) -> usize;

    /// The entry of `row`.
    fn get(&self, row: usize) -> Option<RowRef<'_>>;

    /// The `file_id` of the Source File of `row`.
    fn file_id(&self, row: usize) -> Option<&str>;

    /// The name of the Source File of `row`, in a Log Space (a File View's
    /// rows have none).
    fn source(&self, row: usize) -> Option<&str>;

    /// The Markers painted on `row`.
    fn markers(&self, row: usize) -> &[TextMarker];

    /// Whether `row` is a Continuation Line.
    fn continues(&self, row: usize) -> bool {
        self.get(row).is_some_and(|e| e.parent.is_some())
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Rows for [LogEntry] {
    fn len(&self) -> usize {
        <[LogEntry]>::len(self)
    }

    fn get(&self, row: usize) -> Option<RowRef<'_>> {
        <[LogEntry]>::get(self, row).map(RowRef::Borrowed)
    }

    fn file_id(&self, row: usize) -> Option<&str> {
        <[LogEntry]>::get(self, row).and_then(|e| e.file_id.as_deref())
    }

    fn source(&self, row: usize) -> Option<&str> {
        <[LogEntry]>::get(self, row).and_then(|e| e.source.as_deref())
    }

    fn markers(&self, row: usize) -> &[TextMarker] {
        <[LogEntry]>::get(self, row).map_or(&[], |e| &e.markers)
    }
}

impl Rows for Vec<LogEntry> {
    fn len(&self) -> usize {
        self.as_slice().len()
    }

    fn get(&self, row: usize) -> Option<RowRef<'_>> {
        Rows::get(self.as_slice(), row)
    }

    fn file_id(&self, row: usize) -> Option<&str> {
        Rows::file_id(self.as_slice(), row)
    }

    fn source(&self, row: usize) -> Option<&str> {
        Rows::source(self.as_slice(), row)
    }

    fn markers(&self, row: usize) -> &[TextMarker] {
        Rows::markers(self.as_slice(), row)
    }
}

/// A view with no rows.
pub const NO_ROWS: &Vec<LogEntry> = &Vec::new();

/// The entries of `rows` from row `from` on, in order.
pub fn entries_from<R: Rows + ?Sized>(rows: &R, from: usize) -> impl Iterator<Item = RowRef<'_>> {
    (from..rows.len()).filter_map(move |r| rows.get(r))
}

/// The entries of every row of `rows`, in order.
pub fn entries<R: Rows + ?Sized>(rows: &R) -> impl Iterator<Item = RowRef<'_>> {
    entries_from(rows, 0)
}
