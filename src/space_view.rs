//! Building what a Log Space or a File View shows: each file is parsed on its
//! own, then the rows of a Log Space are merged by timestamp.

use crate::column::FieldMaxChars;
use crate::filter_index::FilterIndex;
use crate::marker::MarkerType;
use crate::model::LogEntry;
use crate::parser::text::parse_text_bytes_with_max_chars;
use crate::timeline;
use crate::workspace::log_space::{LogFile, LogSpace, StoredMarker, line_hash};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

/// One file, parsed once and cached by `file_id`: the entries for the Parsed
/// View and the text lines for the Raw View.
#[derive(Debug, Clone)]
pub struct ParsedFile {
    pub entries: Vec<LogEntry>,
    pub max_chars: FieldMaxChars,
    /// Every line of the file as stored, empty lines included.
    pub lines: Arc<Vec<String>>,
}

/// The lines of `text` as shown in the Raw View.
pub fn split_lines(text: &str) -> Vec<String> {
    text.lines().map(str::to_string).collect()
}

impl ParsedFile {
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let parsed = parse_text_bytes_with_max_chars(bytes)
            .expect("parsing text bytes never fails");
        Self {
            entries: parsed.entries,
            max_chars: parsed.max_chars,
            lines: Arc::new(split_lines(&String::from_utf8_lossy(bytes))),
        }
    }

    pub fn from_text(text: &str) -> Self {
        Self::from_bytes(text.as_bytes())
    }

    pub fn from_path(path: &Path) -> std::io::Result<Self> {
        Ok(Self::from_bytes(&std::fs::read(path)?))
    }

    pub fn empty() -> Self {
        Self::from_text("")
    }
}

/// What the canvas shows for one view.
#[derive(Debug, Clone, Default)]
pub struct BuiltView {
    pub entries: Vec<LogEntry>,
    pub index: FilterIndex,
    pub max_chars: FieldMaxChars,
    /// `file_id`s of the files with no timestamp at all.
    pub no_timestamp_files: Vec<String>,
}

/// Merge the files of a Log Space by timestamp (in the space's file order),
/// restore the files' Markers and detect the Filter columns on the merged rows.
pub fn build_space_view(space: &LogSpace, parsed: &HashMap<String, ParsedFile>) -> BuiltView {
    let empty = ParsedFile::empty();
    let files = space.ordered_files();
    let entry_lists: Vec<&[LogEntry]> = files
        .iter()
        .map(|f| parsed.get(&f.file_id).unwrap_or(&empty).entries.as_slice())
        .collect();
    let timeline = timeline::merge(&entry_lists);
    let named: Vec<(&str, &str, &[LogEntry])> = files
        .iter()
        .zip(&entry_lists)
        .map(|(f, e)| (f.file_id.as_str(), f.display_name.as_str(), *e))
        .collect();
    let mut entries = timeline::materialize(&named, &timeline);
    apply_markers_merged(space, &mut entries);

    let field_order = timeline::union_field_names(&entry_lists);
    let index = FilterIndex::build_merged(&entries, &field_order, &timeline.effective_ts);
    let mut max_chars = FieldMaxChars::new();
    for entry in &entries {
        max_chars.record_entry(entry);
    }
    BuiltView {
        entries,
        index,
        max_chars,
        no_timestamp_files: timeline
            .no_timestamp_files
            .iter()
            .map(|i| files[*i].file_id.clone())
            .collect(),
    }
}

/// The rows of a single file (a File View): no Source Column.
pub fn build_file_view(file: &LogFile, parsed: &ParsedFile) -> BuiltView {
    let mut entries = parsed.entries.clone();
    file.apply_markers(&mut entries);
    let index = FilterIndex::build(&entries);
    BuiltView {
        max_chars: parsed.max_chars,
        index,
        no_timestamp_files: Vec::new(),
        entries,
    }
}

/// Restore each file's stored Markers onto the merged rows.
pub fn apply_markers_merged(space: &LogSpace, entries: &mut [LogEntry]) {
    let stored: HashMap<&str, HashMap<usize, &StoredMarker>> = space
        .files
        .iter()
        .filter(|f| !f.markers.is_empty())
        .map(|f| (f.file_id.as_str(), f.markers.iter().map(|m| (m.line, m)).collect()))
        .collect();
    if stored.is_empty() {
        return;
    }
    for entry in entries {
        let Some(file_id) = entry.file_id.as_deref() else { continue };
        if let Some(marker) = stored.get(file_id).and_then(|m| m.get(&entry.index)) {
            entry.marker = marker.marker;
            entry.note = marker.note.clone();
        }
    }
}

/// Remember the Markers set on the merged rows, file by file. Only files in
/// `loaded` are touched, so a file that failed to load keeps its Markers.
pub fn capture_markers_merged(
    space: &mut LogSpace,
    entries: &[LogEntry],
    loaded: &HashSet<String>,
) {
    let mut by_file: HashMap<String, Vec<StoredMarker>> = HashMap::new();
    for entry in entries {
        let Some(file_id) = entry.file_id.as_deref() else { continue };
        if entry.note.is_none() && entry.marker == MarkerType::from_log_level(entry.level) {
            continue;
        }
        by_file.entry(file_id.to_string()).or_default().push(StoredMarker {
            line: entry.index,
            marker: entry.marker,
            note: entry.note.clone(),
            content_hash: Some(line_hash(&entry.raw_line)),
        });
    }
    for file in &mut space.files {
        if loaded.contains(&file.file_id) {
            file.markers = by_file.remove(&file.file_id).unwrap_or_default();
        }
    }
}
