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
    /// Why the file could not be read; it then counts as empty.
    pub load_error: Option<String>,
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
            load_error: None,
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

    /// A Remote Copy read from its folder: the Generations oldest first with
    /// a Break "rotated" between them, numbered as in their files on the
    /// Server. A folder without `meta.json` (the app closed during the read)
    /// is read as what it holds, cancelled.
    pub fn from_remote_copy(dir: &Path, name: &str) -> std::io::Result<Self> {
        use crate::live::BreakKind;
        use crate::remote::copy::{self, StopReason};
        let meta = match copy::read_meta(dir) {
            Ok(meta) => meta,
            Err(_) => copy::recover_meta(dir)?,
        };
        let mut parsed = Self::empty();
        let mut buffer = LiveBuffer::command(name);
        let count = meta.generations.len();
        for (i, generation) in meta.generations.iter().enumerate() {
            let text = copy::read_generation(dir, generation).unwrap_or_default();
            let lines: Vec<String> = String::from_utf8_lossy(&text).lines().map(str::to_string).collect();
            let label = format!("{name} rotated");
            buffer.begin_copy_generation(generation.first_line_no, i == 0, &label);
            let at = parsed.entries.len();
            buffer.apply_append_with(&mut parsed, &lines, &[]);
            if i == 0 && generation.older_not_read {
                buffer.mark_older_not_read(&mut parsed, &format!("{name}: older lines were not read"));
            }
            if let Some(stop) = generation.stop {
                let kind = match stop {
                    StopReason::Cancelled => BreakKind::ReadCancelled,
                    StopReason::FullReadLimit => BreakKind::ReadLimit,
                    StopReason::RotatedWhileReading => BreakKind::ReadRotated,
                    StopReason::OlderNotRead => BreakKind::OlderNotRead,
                };
                if parsed.entries.len() > at {
                    buffer.mark_read_end(&mut parsed, kind, &stop.label(meta.limit_lines));
                }
            }
            let _ = count;
        }
        parsed.live = Some(buffer);
        Ok(parsed)
    }

    /// An empty file that could not be read, and why.
    pub fn failed(error: String) -> Self {
        Self { load_error: Some(error), ..Self::empty() }
    }
}

/// What the canvas shows instead of rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmptyCanvas {
    /// No Log Space or File View is open.
    NoLogSpace,
    /// The open Log Space has no files; `can_add` when files can be added
    /// to it from here (an Imported Log Space).
    NoFiles { can_add: bool },
    /// Every Source File of the open view failed to load: why, per file.
    LoadFailed { errors: Vec<String> },
}

impl EmptyCanvas {
    pub fn title(&self) -> &'static str {
        match self {
            Self::NoLogSpace => "No Log Space open",
            Self::NoFiles { .. } => "This Log Space has no files",
            Self::LoadFailed { .. } => "The log could not be read",
        }
    }

    pub fn detail(&self) -> String {
        match self {
            Self::NoLogSpace => {
                "A Log Space groups log files and reads them as one timeline, merged by timestamp.".into()
            }
            Self::NoFiles { can_add: true } => "Add log files to read them here as one timeline.".into(),
            Self::NoFiles { can_add: false } => "Its lines show here once it has files.".into(),
            Self::LoadFailed { errors } => errors.join("\n"),
        }
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
