//! Building what a Log Space or a File View shows: each file is parsed on its
//! own, then the rows of a Log Space are merged by timestamp.

use crate::archive::{FileSource, read_source};
use crate::column::FieldMaxChars;
use crate::filter_index::FilterIndex;
use crate::model::LogEntry;
use crate::index::view_rows::pack;
use crate::timeline;
use crate::view_data::{ViewData, ViewFile};
use crate::marker::TextMarker;
use crate::workspace::log_space::{LogFile, LogSpace};
use crate::live::{BreakMark, LiveBuffer};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::Arc;

/// Bytes of a file parsed at a time on each core.
const PARSE_BLOCK: usize = 4 << 20;

/// One file, parsed once and cached by `file_id`: the entries for the Parsed
/// View and the text lines for the Raw View.
#[derive(Debug, Clone)]
pub struct ParsedFile {
    pub entries: Vec<LogEntry>,
    pub max_chars: FieldMaxChars,
    /// Every line of the file as stored, empty lines included.
    pub lines: Arc<Vec<gpui::SharedString>>,
    /// Breaks of a live file, by position of the line they are drawn above.
    pub raw_breaks: Arc<BTreeMap<usize, Arc<BreakMark>>>,
    /// What a live file keeps besides its lines.
    pub live: Option<LiveBuffer>,
    /// Why the file could not be read; it then counts as empty.
    pub load_error: Option<String>,
    /// Id of the Log Format the entries were read with (`None`: Auto-detect).
    pub format_id: Option<String>,
    /// The entries the Live Window just trimmed off the front, until the
    /// open view took their rows out (they are what its counts held).
    pub trimmed_off: Vec<LogEntry>,
    /// The file read through its Line Index: its entries are read again from
    /// it when shown, and `entries` and `lines` hold none.
    pub indexed: Option<Arc<crate::index::file::IndexedFile>>,
}

/// The lines of `text` as shown in the Raw View.
pub fn split_lines(text: &str) -> Vec<gpui::SharedString> {
    text.lines().map(gpui::SharedString::from).collect()
}

impl ParsedFile {
    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self::from_bytes_with_progress(bytes, |_| true).expect("never stopped")
    }

    /// `from_bytes`, telling `progress` the bytes read now and then; `None`
    /// when it answers false (stop).
    pub fn from_bytes_with_progress(bytes: &[u8], progress: impl FnMut(usize) -> bool) -> Option<Self> {
        let content = String::from_utf8_lossy(bytes);
        let entries = crate::parser::text::parse_entries_parallel(&content, PARSE_BLOCK, progress)?;
        let max_chars = FieldMaxChars::scan_entries(&entries);
        let parsed = crate::parser::text::ParseResult { entries, max_chars };
        // Every line is an entry: the Raw View's line is the entry's own text.
        let lines = parsed.entries.iter().map(|e| e.raw_line.clone()).collect();
        Some(Self {
            entries: parsed.entries,
            max_chars: parsed.max_chars,
            lines: Arc::new(lines),
            raw_breaks: Arc::default(),
            live: None,
            load_error: None,
            format_id: None,
            trimmed_off: Vec::new(),
            indexed: None,
        })
    }

    /// A file read through its Line Index.
    pub fn from_indexed(file: crate::index::file::IndexedFile) -> Self {
        let max_chars = file.indexed.data.read().unwrap().max_chars.clone();
        Self { max_chars, indexed: Some(Arc::new(file)), ..Self::empty() }
    }

    /// Where the file's entries are.
    pub fn file_entries(&self) -> crate::view_data::FileEntries<'_> {
        match &self.indexed {
            Some(file) => crate::view_data::FileEntries::Indexed(file),
            None => crate::view_data::FileEntries::Held(&self.entries),
        }
    }

    /// The number of lines.
    pub fn len(&self) -> usize {
        self.file_entries().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Read the entries again with the Log Format `format` of id `format_id`
    /// (`None`: Auto-detect); a live file reads its new lines with it too.
    pub fn reformat(&mut self, format_id: Option<String>, format: Option<Arc<crate::log_format::CompiledFormat>>) {
        crate::parser::text::reread_entries(&mut self.entries, format.as_deref());
        self.adopt_format(format_id, format);
    }

    /// The entries were read again with a format: remember it, and read new
    /// lines with it.
    pub fn adopt_format(&mut self, format_id: Option<String>, format: Option<Arc<crate::log_format::CompiledFormat>>) {
        self.max_chars = FieldMaxChars::scan_entries(&self.entries);
        if let Some(live) = self.live.as_mut() {
            live.set_format(format);
        }
        self.format_id = format_id;
    }

    /// The lines, to change them in place.
    pub fn lines_mut(&mut self) -> &mut Vec<gpui::SharedString> {
        Arc::make_mut(&mut self.lines)
    }

    /// Move the Breaks at line positions `from` and below by `by` lines;
    /// Breaks that move below position 0 are dropped.
    pub fn shift_breaks_from(&mut self, from: usize, by: isize) {
        let moved: Vec<(usize, Arc<BreakMark>)> = std::mem::take(Arc::make_mut(&mut self.raw_breaks))
            .into_iter()
            .collect();
        for (pos, mark) in moved {
            let pos = if pos >= from { pos as isize + by } else { pos as isize };
            if pos >= 0 {
                Arc::make_mut(&mut self.raw_breaks).insert(pos as usize, mark);
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
    /// The view's files are being read: each name with how far, in percent.
    Reading { files: Vec<(String, u8)> },
}

impl EmptyCanvas {
    pub fn title(&self) -> &'static str {
        match self {
            Self::NoLogSpace => "No Log Space open",
            Self::NoFiles { .. } => "This Log Space has no files",
            Self::LoadFailed { .. } => "The log could not be read",
            Self::Reading { .. } => "Reading the log",
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
            Self::Reading { files } => {
                files.iter().map(|(name, percent)| format!("{name} · {percent}%")).collect::<Vec<_>>().join("\n")
            }
        }
    }
}

/// What the canvas shows for one view.
#[derive(Debug, Clone, Default)]
pub struct BuiltView {
    pub view: ViewData,
    pub index: FilterIndex,
    pub max_chars: FieldMaxChars,
    /// `file_id`s of the files with no timestamp at all.
    pub no_timestamp_files: Vec<String>,
}

impl BuiltView {
    /// The rows with what the view adds to them, copied (for tests).
    #[doc(hidden)]
    pub fn materialize(&self, parsed: &HashMap<String, ParsedFile>) -> Vec<LogEntry> {
        crate::view_data::materialize(&self.view.bind(parsed))
    }
}

/// Merge the files of a Log Space by timestamp (in the space's file order),
/// look up the files' Markers and detect the Filter columns on the merged
/// rows. No line is copied: the rows point into `parsed`.
pub fn build_space_view(space: &LogSpace, parsed: &HashMap<String, ParsedFile>) -> BuiltView {
    let empty = ParsedFile::empty();
    let files = space.ordered_files();
    let parsed_of: Vec<&ParsedFile> = files.iter().map(|f| parsed.get(&f.file_id).unwrap_or(&empty)).collect();
    let (rows, effective_ts, no_timestamp) = if parsed_of.iter().any(|p| p.indexed.is_some()) {
        merge_by_meta(&parsed_of)
    } else {
        let entry_lists: Vec<&[LogEntry]> = parsed_of.iter().map(|p| p.entries.as_slice()).collect();
        let timeline = timeline::merge(&entry_lists);
        let rows = timeline.rows.iter().map(|r| pack(r.file as usize, r.line as usize)).collect();
        (rows, timeline.effective_ts, timeline.no_timestamp_files)
    };
    let mut view = ViewData {
        rows,
        files: files.iter().map(|f| ViewFile::new(&f.file_id, Some(&f.display_name))).collect(),
    };
    view.set_markers(files.iter().copied());

    let field_order = union_field_names(&parsed_of);
    let index = merge_filter_indexes(&files, &parsed_of, &view.rows, &field_order, effective_ts);
    let max_chars = merged_max_chars(&files, &parsed_of);
    BuiltView {
        view,
        index,
        max_chars,
        no_timestamp_files: no_timestamp.iter().map(|i| files[*i].file_id.clone()).collect(),
    }
}

/// The fields of the files by name, the first file's order first (as
/// `timeline::union_field_names`): a file read through its Line Index gives
/// its Filter columns' fields, which are in the order they first show.
fn union_field_names(files: &[&ParsedFile]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut names = Vec::new();
    for file in files {
        let in_file: Vec<String> = match &file.indexed {
            Some(indexed) => {
                let data = indexed.indexed.data.read().unwrap();
                data.filter.field_columns().filter_map(|c| c.field()).map(|k| k.as_str().to_string()).collect()
            }
            None => timeline::union_field_names(&[file.entries.as_slice()]),
        };
        for name in in_file {
            if seen.insert(name.clone()) {
                names.push(name);
            }
        }
    }
    names
}

/// The Filter columns of a Log Space's merged rows, from its files' own:
/// a file read through its Line Index has them from Indexing, a held file's
/// are detected from its entries.
fn merge_filter_indexes(
    files: &[&LogFile],
    parsed: &[&ParsedFile],
    rows: &[u64],
    field_order: &[String],
    effective_ts: Vec<f64>,
) -> FilterIndex {
    use crate::filter_index::MergeFile;
    let guards: Vec<_> = parsed.iter().map(|p| p.indexed.as_ref().map(|f| f.indexed.data.read().unwrap())).collect();
    let built: Vec<Option<FilterIndex>> =
        parsed.iter().map(|p| p.indexed.is_none().then(|| FilterIndex::build(&p.entries))).collect();
    let merge: Vec<MergeFile> = files
        .iter()
        .enumerate()
        .map(|(i, f)| MergeFile {
            index: match (&guards[i], &built[i]) {
                (Some(data), _) => &data.filter,
                (None, Some(index)) => index,
                (None, None) => unreachable!("a file is held or indexed"),
            },
            source: Some(f.display_name.as_str()),
        })
        .collect();
    let own_ts = |file: usize, pos: usize| match &guards[file] {
        Some(data) => data
            .file
            .meta
            .get(pos)
            .filter(|m| m.flags & crate::index::meta::flags::OWN_TS != 0)
            .map(|m| seconds(m.ts_us)),
        None => parsed[file].entries.get(pos).filter(|e| crate::filter_index::has_timestamp(e)).map(|e| e.timestamp),
    };
    let continues = |file: usize, pos: usize| match &guards[file] {
        Some(data) => data.file.meta.get(pos).is_some_and(|m| m.parent_dist > 0),
        None => parsed[file].entries.get(pos).is_some_and(|e| e.parent.is_some()),
    };
    FilterIndex::merge_files(&merge, rows, field_order, effective_ts, own_ts, continues)
}

/// The longest texts of a Log Space's rows: each file's, and its name in
/// the Source column.
fn merged_max_chars(files: &[&LogFile], parsed: &[&ParsedFile]) -> FieldMaxChars {
    let mut max = FieldMaxChars::new();
    for (file, p) in files.iter().zip(parsed) {
        if p.is_empty() {
            continue;
        }
        match &p.indexed {
            Some(indexed) => max.merge(&indexed.indexed.data.read().unwrap().max_chars),
            None => max.merge(&FieldMaxChars::scan_entries(&p.entries)),
        }
        max.source = max.source.max(file.display_name.chars().count());
    }
    max
}

use crate::index::meta::seconds;

/// Merge files of which some are read through their Line Index, with
/// `Merger` on their `LineMeta`: the merged rows, the timestamp each sorts
/// by (`NAN` for none), and the files with no timestamp at all.
fn merge_by_meta(files: &[&ParsedFile]) -> (Vec<u64>, Vec<f64>, Vec<usize>) {
    use crate::index::meta::{LineMeta, NO_TIME, micros};
    use crate::index::view_rows::{Lane, Merger, unpack};
    // A held file's lines as a merge reads them: its effective timestamps,
    // and which lines continue the one above.
    let held: Vec<Option<(Vec<LineMeta>, Option<Vec<f64>>)>> = files
        .iter()
        .map(|p| {
            p.indexed.is_none().then(|| {
                let ts = timeline::effective_timestamps(&p.entries);
                let metas = p
                    .entries
                    .iter()
                    .enumerate()
                    .map(|(i, e)| LineMeta {
                        ts_us: ts.as_ref().map_or(NO_TIME, |ts| micros(ts[i])),
                        parent_dist: u32::from(e.parent.is_some()),
                        ..LineMeta::default()
                    })
                    .collect();
                (metas, ts)
            })
        })
        .collect();
    let guards: Vec<_> = files.iter().map(|p| p.indexed.as_ref().map(|f| f.indexed.data.read().unwrap())).collect();
    let lanes: Vec<Lane> = files
        .iter()
        .enumerate()
        .map(|(i, _)| match (&guards[i], &held[i]) {
            (Some(data), _) => Lane { meta: data.file.meta.as_slice(), done: true },
            (None, Some((metas, _))) => Lane { meta: metas, done: true },
            (None, None) => Lane { meta: &[], done: true },
        })
        .collect();
    let mut merger = Merger::new(lanes.len());
    let mut rows = Vec::new();
    merger.advance(&lanes, &mut rows);
    let untimed: Vec<usize> = (0..files.len()).filter(|&f| merger.is_untimed(f)).collect();
    let effective_ts = rows
        .iter()
        .map(|&r| {
            let (file, pos) = unpack(r);
            if untimed.contains(&file) {
                return f64::NAN;
            }
            match (&guards[file], &held[file]) {
                (Some(data), _) => data.file.meta.get(pos).map_or(f64::NAN, |m| seconds(m.ts_us)),
                (None, Some((_, Some(ts)))) => ts[pos],
                _ => f64::NAN,
            }
        })
        .collect();
    (rows, effective_ts, untimed)
}

/// The rows of a single file (a File View): no Source Column.
pub fn build_file_view(file: &LogFile, parsed: &ParsedFile) -> BuiltView {
    let mut view = ViewData {
        rows: ViewData::rows_of(0, 0..parsed.len()).collect(),
        files: vec![ViewFile::new(&file.file_id, None)],
    };
    view.set_markers([file]);
    let index = match &parsed.indexed {
        // Indexing built it as a build from every row would.
        Some(indexed) => indexed.indexed.data.read().unwrap().filter.clone(),
        None => FilterIndex::build(&view.bind_with(|_| Some(parsed.entries.as_slice()))),
    };
    BuiltView { max_chars: parsed.max_chars.clone(), index, no_timestamp_files: Vec::new(), view }
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
