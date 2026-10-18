//! Building what a Log Space or a File View shows: each file is parsed on its
//! own, then the rows of a Log Space are merged by timestamp.

use crate::archive::{FileSource, read_source};
use crate::column::FieldMaxChars;
use crate::filter_index::FilterIndex;
use crate::model::LogEntry;
use crate::parser::text::parse_text_bytes_with_max_chars;
use crate::timeline;
use crate::marker::TextMarker;
use crate::workspace::log_space::{LogFile, LogSpace};
use crate::live::{BreakMark, LiveBuffer};
use std::collections::{BTreeMap, HashMap};
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
    /// Breaks of a live file, by position of the line they are drawn above.
    pub raw_breaks: BTreeMap<usize, Arc<BreakMark>>,
    /// What a live file keeps besides its lines.
    pub live: Option<LiveBuffer>,
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
            raw_breaks: BTreeMap::new(),
            live: None,
        }
    }

    /// The lines, to change them in place.
    pub fn lines_mut(&mut self) -> &mut Vec<String> {
        Arc::make_mut(&mut self.lines)
    }

    /// Move the Breaks at line positions `from` and below by `by` lines;
    /// Breaks that move below position 0 are dropped.
    pub fn shift_breaks_from(&mut self, from: usize, by: isize) {
        let moved: Vec<(usize, Arc<BreakMark>)> = std::mem::take(&mut self.raw_breaks)
            .into_iter()
            .collect();
        for (pos, mark) in moved {
            let pos = if pos >= from { pos as isize + by } else { pos as isize };
            if pos >= 0 {
                self.raw_breaks.insert(pos as usize, mark);
            }
        }
    }

    pub fn from_text(text: &str) -> Self {
        Self::from_bytes(text.as_bytes())
    }

    pub fn from_path(path: &Path) -> std::io::Result<Self> {
        Self::from_source(FileSource::Disk(path))
    }

    pub fn from_source(source: FileSource) -> std::io::Result<Self> {
        Ok(Self::from_bytes(&read_source(source)?))
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
    // Lines are told apart by file and number, in a File View too.
    let file_id: Arc<str> = Arc::from(file.file_id.as_str());
    for entry in &mut entries {
        entry.file_id = Some(file_id.clone());
    }
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
    let stored: HashMap<&str, (&LogFile, HashMap<(usize, u32), Vec<TextMarker>>)> = space
        .files
        .iter()
        .filter(|f| !f.markers.is_empty())
        .map(|f| {
            let mut by_line: HashMap<(usize, u32), Vec<TextMarker>> = HashMap::new();
            for m in &f.markers {
                by_line
                    .entry((m.line, m.generations_ago))
                    .or_default()
                    .push(TextMarker { range: m.range.clone(), color: m.color.clone() });
            }
            (f.file_id.as_str(), (f, by_line))
        })
        .collect();
    for entry in entries {
        entry.markers = entry
            .file_id
            .as_deref()
            .and_then(|file_id| stored.get(file_id))
            .and_then(|(file, by_line)| by_line.get(&(entry.index, file.generations_ago(entry.generation))))
            .cloned()
            .unwrap_or_default();
    }
}
