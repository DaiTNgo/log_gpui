//! Log Space: a named group of log files inside a Project.
//!
//! A Log Space is either *Imported* (files picked from disk, read-only) or
//! *Scratch* (files the app creates in its own data folder, editable in the
//! Raw View). The kind is a property of the space, so "Imported and Scratch
//! never mix" is enforced by the operations below rather than by convention.

use crate::filter::FilterState;
use crate::marker::MarkerType;
use crate::model::LogEntry;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// The markers and notes the user set on one line of a file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredMarker {
    /// 1-based line index of the entry (`LogEntry::index`).
    pub line: usize,
    pub marker: Option<MarkerType>,
    pub note: Option<String>,
    /// Hash of the line's content when the marker was set. A marker whose
    /// hash no longer matches the saved line is dropped (Raw View edits).
    #[serde(default)]
    pub content_hash: Option<u64>,
}

/// Stable hash of a line's content.
pub fn line_hash(text: &str) -> u64 {
    // FNV-1a: deterministic across runs, unlike `DefaultHasher`.
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// The markers and notes the user set on `entries`, leaving out the passive
/// error/warning markers derived from each entry's level.
pub fn collect_markers(entries: &[LogEntry]) -> Vec<StoredMarker> {
    entries
        .iter()
        .filter(|e| e.note.is_some() || e.marker != MarkerType::from_log_level(e.level))
        .map(|e| StoredMarker {
            line: e.index,
            marker: e.marker,
            note: e.note.clone(),
            content_hash: Some(line_hash(&e.raw_line)),
        })
        .collect()
}

/// A log file belonging to a Log Space. For an Imported space the file on
/// disk is never modified or deleted; for a Scratch space it lives in the
/// app's data folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogFile {
    /// Stable identifier, unique within the Project.
    pub file_id: String,
    /// Name shown in tabs, the sidebar and the Source Column.
    pub display_name: String,
    /// Absolute path on disk.
    pub path: Option<String>,
    /// Sequence number at which the file was added to its Log Space.
    #[serde(default)]
    pub added_seq: u64,
    /// The user-set markers and notes, remembered automatically.
    #[serde(default)]
    pub markers: Vec<StoredMarker>,
}

impl LogFile {
    pub fn new(file_id: String, display_name: String) -> Self {
        Self {
            file_id,
            display_name,
            path: None,
            added_seq: 0,
            markers: Vec::new(),
        }
    }

    /// Build a file from a filesystem path: id and display name come from it.
    pub fn from_path(path: &str) -> Self {
        let display_name = Path::new(path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string());
        Self {
            file_id: path.to_string(),
            display_name,
            path: Some(path.to_string()),
            added_seq: 0,
            markers: Vec::new(),
        }
    }

    /// Name of the folder holding the file, shown beside the name in a File
    /// Section header.
    pub fn folder(&self) -> Option<String> {
        self.path
            .as_deref()
            .and_then(|p| Path::new(p).parent())
            .map(|p| p.to_string_lossy().to_string())
            .filter(|p| !p.is_empty() && p != ".")
    }

    /// Remember the markers and notes the user set on `entries`. The passive
    /// error/warning markers derived from the level are not stored.
    pub fn capture_markers(&mut self, entries: &[LogEntry]) {
        self.markers = collect_markers(entries);
    }

    /// Restore the remembered markers and notes onto freshly loaded `entries`.
    pub fn apply_markers(&self, entries: &mut [LogEntry]) {
        for stored in &self.markers {
            if let Some(entry) = entries.iter_mut().find(|e| e.index == stored.line) {
                entry.marker = stored.marker;
                entry.note = stored.note.clone();
            }
        }
    }

    /// Drop the markers whose line content changed since they were set.
    /// `lines` is the saved text, one element per line. Returns how many
    /// markers were dropped.
    pub fn prune_markers(&mut self, lines: &[&str]) -> usize {
        let before = self.markers.len();
        self.markers.retain(|m| match (m.content_hash, lines.get(m.line.wrapping_sub(1))) {
            (Some(hash), Some(line)) => hash == line_hash(line),
            (Some(_), None) => false,
            // Markers stored without a hash are kept.
            (None, _) => true,
        });
        before - self.markers.len()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LogSpaceKind {
    Imported,
    Scratch,
}

/// How the files of a Log Space are ordered everywhere: sidebar, File
/// Sections, and the tie-break for equal timestamps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum FileOrder {
    #[default]
    Added,
    Name,
}

/// Why a Log Space name was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpaceNameError {
    Blank,
    Duplicate,
}

impl SpaceNameError {
    pub fn message(self) -> &'static str {
        match self {
            SpaceNameError::Blank => "Enter a name",
            SpaceNameError::Duplicate => "A Log Space with this name already exists",
        }
    }
}

/// An operation was attempted on the wrong kind of Log Space.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WrongKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoveFileError {
    NotFound,
    /// A Scratch Log Space always keeps at least one file.
    LastScratchFile,
}

/// Outcome of importing files into an Imported Log Space.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportOutcome {
    /// `file_id`s of the files that were added.
    pub added: Vec<String>,
    /// Paths skipped because the space already holds them.
    pub skipped: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogSpace {
    pub id: String,
    pub name: String,
    pub kind: LogSpaceKind,
    #[serde(default)]
    pub file_order: FileOrder,
    pub files: Vec<LogFile>,
    /// The Filter shared by every file in the Log Space's Parsed View.
    #[serde(default)]
    pub filter: FilterState,
    /// The separate Filter of each File View.
    #[serde(default)]
    pub file_filters: HashMap<String, FilterState>,
    /// Whether the Source Column is shown; `None` means the default (shown
    /// when the space has two or more files).
    #[serde(default)]
    pub show_source: Option<bool>,
    /// The view last used on this space, reopened as it was.
    #[serde(default)]
    pub last_mode: crate::workspace::working_set::ViewMode,
    /// Next `added_seq` to hand out.
    #[serde(default)]
    next_seq: u64,
}

impl LogSpace {
    pub fn new(id: String, name: String, kind: LogSpaceKind) -> Self {
        Self {
            id,
            name,
            kind,
            file_order: FileOrder::Added,
            files: Vec::new(),
            filter: FilterState::default(),
            file_filters: HashMap::new(),
            show_source: None,
            last_mode: Default::default(),
            next_seq: 1,
        }
    }

    pub fn file(&self, file_id: &str) -> Option<&LogFile> {
        self.files.iter().find(|f| f.file_id == file_id)
    }

    pub fn file_mut(&mut self, file_id: &str) -> Option<&mut LogFile> {
        self.files.iter_mut().find(|f| f.file_id == file_id)
    }

    pub fn has_file(&self, file_id: &str) -> bool {
        self.file(file_id).is_some()
    }

    fn take_seq(&mut self) -> u64 {
        let seq = self.next_seq.max(self.files.iter().map(|f| f.added_seq + 1).max().unwrap_or(1));
        self.next_seq = seq + 1;
        seq
    }

    /// The files in the Log Space's single file order.
    pub fn ordered_files(&self) -> Vec<&LogFile> {
        let mut files: Vec<&LogFile> = self.files.iter().collect();
        match self.file_order {
            FileOrder::Added => files.sort_by_key(|f| f.added_seq),
            FileOrder::Name => files.sort_by(|a, b| {
                a.display_name
                    .to_lowercase()
                    .cmp(&b.display_name.to_lowercase())
                    .then(a.added_seq.cmp(&b.added_seq))
            }),
        }
        files
    }

    pub fn set_file_order(&mut self, order: FileOrder) {
        self.file_order = order;
    }

    /// Whether the Source Column is shown.
    pub fn source_column_shown(&self) -> bool {
        self.show_source.unwrap_or(self.files.len() >= 2)
    }

    /// Import files by path into an Imported Log Space. Paths the space
    /// already holds are skipped and reported.
    pub fn add_imported_files(&mut self, paths: &[String]) -> Result<ImportOutcome, WrongKind> {
        if self.kind != LogSpaceKind::Imported {
            return Err(WrongKind);
        }
        let mut outcome = ImportOutcome::default();
        for path in paths {
            if self.has_file(path) {
                outcome.skipped.push(path.clone());
                continue;
            }
            let mut file = LogFile::from_path(path);
            file.added_seq = self.take_seq();
            outcome.added.push(file.file_id.clone());
            self.files.push(file);
        }
        Ok(outcome)
    }

    /// The name of the next scratch file: `untitled.log`, then
    /// `untitled-2.log`, `untitled-3.log`, … using the smallest unused number.
    pub fn next_scratch_name(&self) -> String {
        let name_for = |n: usize| {
            if n == 1 { "untitled.log".to_string() } else { format!("untitled-{n}.log") }
        };
        (1..)
            .map(name_for)
            .find(|name| !self.files.iter().any(|f| &f.display_name == name))
            .expect("unbounded range")
    }

    /// Add a new empty scratch file stored under `dir`.
    pub fn new_scratch_file(&mut self, dir: &Path) -> Result<&LogFile, WrongKind> {
        if self.kind != LogSpaceKind::Scratch {
            return Err(WrongKind);
        }
        let name = self.next_scratch_name();
        let seq = self.take_seq();
        let mut file = LogFile::new(format!("{}#{}", self.id, seq), name.clone());
        file.path = Some(dir.join(&name).to_string_lossy().to_string());
        file.added_seq = seq;
        self.files.push(file);
        Ok(self.files.last().expect("just pushed"))
    }

    /// Remove a file from the space. For an Imported space this only drops it
    /// from the list; the caller deletes scratch files from disk.
    pub fn remove_file(&mut self, file_id: &str) -> Result<LogFile, RemoveFileError> {
        let pos = self
            .files
            .iter()
            .position(|f| f.file_id == file_id)
            .ok_or(RemoveFileError::NotFound)?;
        if self.kind == LogSpaceKind::Scratch && self.files.len() == 1 {
            return Err(RemoveFileError::LastScratchFile);
        }
        self.file_filters.remove(file_id);
        Ok(self.files.remove(pos))
    }

    /// Rename a file's display name. Refuses blank names and names another
    /// file of the space already uses. Returns whether the rename happened.
    pub fn rename_file(&mut self, file_id: &str, name: &str) -> bool {
        let name = name.trim();
        if name.is_empty()
            || self.files.iter().any(|f| f.file_id != file_id && f.display_name == name)
        {
            return false;
        }
        match self.file_mut(file_id) {
            Some(file) => {
                file.display_name = name.to_string();
                true
            }
            None => false,
        }
    }

    /// The Filter of a File View, empty until one is set.
    pub fn file_filter(&self, file_id: &str) -> FilterState {
        self.file_filters.get(file_id).cloned().unwrap_or_default()
    }

    pub fn set_file_filter(&mut self, file_id: &str, filter: FilterState) {
        self.file_filters.insert(file_id.to_string(), filter);
    }

    /// Directory holding this space's scratch files.
    pub fn scratch_dir(&self) -> PathBuf {
        scratch_dir_for(&self.id)
    }
}

/// `<config>/log_gpui/spaces/<space_id>/`, next to `session.json`.
pub fn scratch_dir_for(space_id: &str) -> PathBuf {
    spaces_root().join(space_id)
}

/// `<config>/log_gpui/spaces/`.
pub fn spaces_root() -> PathBuf {
    crate::workspace::session::app_data_dir().join("spaces")
}

/// Validate a Log Space name against the other names in the Project.
pub fn validate_space_name<'a>(
    name: &str,
    existing: impl IntoIterator<Item = &'a str>,
) -> Result<String, SpaceNameError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(SpaceNameError::Blank);
    }
    if existing.into_iter().any(|n| n == trimmed) {
        return Err(SpaceNameError::Duplicate);
    }
    Ok(trimmed.to_string())
}
