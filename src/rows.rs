//! The rows of the open view, read one row at a time (openspec change
//! `line-offset-index`, task 4.3). What a row shows that is not the line's
//! own (its Source File, its Markers) is read here too, so the view can stop
//! holding a copy of every line with those set on it.

use crate::marker::TextMarker;
use crate::model::LogEntry;

/// The rows of a view, in the order shown.
pub trait Rows: Sync {
    fn len(&self) -> usize;

    /// The entry of `row`.
    fn get(&self, row: usize) -> Option<&LogEntry>;

    /// The `file_id` of the Source File of `row`.
    fn file_id(&self, row: usize) -> Option<&str>;

    /// The name of the Source File of `row`, in a Log Space (a File View's
    /// rows have none).
    fn source(&self, row: usize) -> Option<&str>;

    /// The Markers painted on `row`.
    fn markers(&self, row: usize) -> &[TextMarker];

    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Rows for [LogEntry] {
    fn len(&self) -> usize {
        <[LogEntry]>::len(self)
    }

    fn get(&self, row: usize) -> Option<&LogEntry> {
        <[LogEntry]>::get(self, row)
    }

    fn file_id(&self, row: usize) -> Option<&str> {
        self.get(row).and_then(|e| e.file_id.as_deref())
    }

    fn source(&self, row: usize) -> Option<&str> {
        self.get(row).and_then(|e| e.source.as_deref())
    }

    fn markers(&self, row: usize) -> &[TextMarker] {
        self.get(row).map_or(&[], |e| &e.markers)
    }
}

impl Rows for Vec<LogEntry> {
    fn len(&self) -> usize {
        self.as_slice().len()
    }

    fn get(&self, row: usize) -> Option<&LogEntry> {
        self.as_slice().get(row)
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
pub fn entries_from<R: Rows + ?Sized>(rows: &R, from: usize) -> impl Iterator<Item = &LogEntry> {
    (from..rows.len()).filter_map(move |r| rows.get(r))
}

/// The entries of every row of `rows`, in order.
pub fn entries<R: Rows + ?Sized>(rows: &R) -> impl Iterator<Item = &LogEntry> {
    entries_from(rows, 0)
}
