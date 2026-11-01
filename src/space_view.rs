//! Building what a Log Space or a File View shows: each file is parsed on its
//! own, then the rows of a Log Space are merged by timestamp.

use crate::archive::{FileSource, read_source};
use crate::column::FieldMaxChars;
use crate::filter_index::FilterIndex;
use crate::model::LogEntry;
use crate::view_data::{ViewData, ViewFile};
use crate::marker::TextMarker;
use crate::workspace::log_space::{LogFile, LogSpace};
use crate::live::BreakMark;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::Arc;


/// One file, cached by `file_id`: read through its Line Index, so its text
/// stays where it is (on disk, in a Remote Cache, or the bytes it was made
/// from) and its rows are read again when shown.
#[derive(Debug, Clone)]
pub struct ParsedFile {
    pub max_chars: FieldMaxChars,
    /// Breaks of a live file, by position of the line they are drawn above.
    pub raw_breaks: Arc<BTreeMap<usize, Arc<BreakMark>>>,
    /// Why the file could not be read; it then counts as empty.
    pub load_error: Option<String>,
    /// Id of the Log Format the entries were read with (`None`: Auto-detect).
    pub format_id: Option<String>,
    /// The file's Line Index.
    pub indexed: Option<Arc<crate::index::file::IndexedFile>>,
}

/// The lines of `text` as shown in the Raw View.
pub fn split_lines(text: &str) -> Vec<gpui::SharedString> {
    text.lines().map(gpui::SharedString::from).collect()
}

impl ParsedFile {
    /// The lines of `bytes`, kept in memory, read through a Line Index.
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let source = Arc::new(crate::index::BytesSource::new(bytes.to_vec()));
        let file = crate::index::file::IndexedFile::index_now(source, Default::default())
            .expect("bytes in memory are always read");
        Self::from_indexed(file)
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
            None => crate::view_data::FileEntries::Empty,
        }
    }

    /// The number of lines.
    pub fn len(&self) -> usize {
        self.indexed.as_ref().map_or(0, |f| f.len())
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Entry `n` (0-based), read through the Line Index.
    pub fn entry(&self, n: usize) -> Option<Arc<LogEntry>> {
        self.indexed.as_ref()?.entry(n)
    }

    /// The text of line `n` (0-based).
    pub fn line(&self, n: usize) -> Option<gpui::SharedString> {
        self.indexed.as_ref()?.line(n)
    }

    /// Every entry, read now and copied: for tests and small files only.
    pub fn entries(&self) -> Vec<LogEntry> {
        (0..self.len()).filter_map(|n| self.entry(n)).map(|e| (*e).clone()).collect()
    }

    /// Every line's text, read now: for tests and small files only.
    pub fn lines(&self) -> Vec<gpui::SharedString> {
        (0..self.len()).filter_map(|n| self.line(n)).collect()
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

    /// The file at `path`, read through its Line Index where it is.
    pub fn from_path(path: &Path) -> std::io::Result<Self> {
        let source = Arc::new(crate::index::DiskSource::on_demand(path, crate::index::HandleBudget::shared()));
        let file = crate::index::file::IndexedFile::index_now(source, Default::default()).map_err(std::io::Error::other)?;
        Ok(Self::from_indexed(file))
    }

    pub fn from_source(source: FileSource) -> std::io::Result<Self> {
        match source {
            FileSource::Disk(path) => Self::from_path(path),
            other => Ok(Self::from_bytes(&read_source(other)?)),
        }
    }

    /// A file with no lines.
    pub fn empty() -> Self {
        Self {
            max_chars: FieldMaxChars::default(),
            raw_breaks: Arc::default(),
            load_error: None,
            format_id: None,
            indexed: None,
        }
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
    let (rows, effective_ts, no_timestamp) = merge_by_meta(&parsed_of);
    let mut view = ViewData {
        rows: crate::index::view_rows::ViewRows::Merged(rows),
        files: files.iter().map(|f| ViewFile::new(&f.file_id, Some(&f.display_name))).collect(),
    };
    view.set_markers(files.iter().copied());

    let field_order = union_field_names(&parsed_of);
    let rows: Vec<u64> = view.rows.iter().collect();
    let index = merge_filter_indexes(&files, &parsed_of, &rows, &field_order, effective_ts);
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
        let Some(indexed) = &file.indexed else { continue };
        let in_file: Vec<String> = {
            let data = indexed.indexed.data.read().unwrap();
            data.filter.field_columns().filter_map(|c| c.field()).map(|k| k.as_str().to_string()).collect()
        };
        for name in in_file {
            if seen.insert(name.clone()) {
                names.push(name);
            }
        }
    }
    names
}

/// The Filter columns of a Log Space's merged rows, from its files' own,
/// which Indexing built.
fn merge_filter_indexes(
    files: &[&LogFile],
    parsed: &[&ParsedFile],
    rows: &[u64],
    field_order: &[String],
    effective_ts: Vec<f64>,
) -> FilterIndex {
    use crate::filter_index::MergeFile;
    let guards: Vec<_> = parsed.iter().map(|p| p.indexed.as_ref().map(|f| f.indexed.data.read().unwrap())).collect();
    let empty = FilterIndex::default();
    let merge: Vec<MergeFile> = files
        .iter()
        .enumerate()
        .map(|(i, f)| MergeFile {
            index: guards[i].as_ref().map_or(&empty, |data| &data.filter),
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
        None => None,
    };
    let continues = |file: usize, pos: usize| match &guards[file] {
        Some(data) => data.file.meta.get(pos).is_some_and(|m| m.parent_dist > 0),
        None => false,
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
        if let Some(indexed) = &p.indexed {
            max.merge(&indexed.indexed.data.read().unwrap().max_chars);
        }
        max.source = max.source.max(file.display_name.chars().count());
        max.log_name = max.log_name.max(crate::rotation_name::log_name(&file.display_name).chars().count());
    }
    max
}

use crate::index::meta::seconds;

/// Merge the files by the `LineMeta` of their Line Index, with `Merger`:
/// the merged rows, the timestamp each sorts by (`NAN` for none), and the
/// files with no timestamp at all. A file not read has no lines.
fn merge_by_meta(files: &[&ParsedFile]) -> (Vec<u64>, Vec<f64>, Vec<usize>) {
    use crate::index::view_rows::{Lane, Merger, unpack};
    let guards: Vec<_> = files.iter().map(|p| p.indexed.as_ref().map(|f| f.indexed.data.read().unwrap())).collect();
    let lanes: Vec<Lane> = guards
        .iter()
        .map(|g| match g {
            Some(data) => Lane { meta: data.file.meta.as_slice(), done: true },
            None => Lane { meta: &[], done: true },
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
            guards[file].as_ref().and_then(|data| data.file.meta.get(pos)).map_or(f64::NAN, |m| seconds(m.ts_us))
        })
        .collect();
    (rows, effective_ts, untimed)
}

/// The rows of a single file (a File View): no Source Column.
pub fn build_file_view(file: &LogFile, parsed: &ParsedFile) -> BuiltView {
    let mut view = ViewData {
        rows: crate::index::view_rows::ViewRows::File { lines: parsed.len() },
        files: vec![ViewFile::new(&file.file_id, None)],
    };
    view.set_markers([file]);
    // Indexing built it as a build from every row would.
    let index = parsed.indexed.as_ref().map(|indexed| indexed.indexed.data.read().unwrap().filter.clone()).unwrap_or_default();
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
