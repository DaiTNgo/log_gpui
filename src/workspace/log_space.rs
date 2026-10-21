//! Log Space: a named group of log files inside a Project.
//!
//! A Log Space is either *Imported* (files picked from disk, read-only) or
//! *Scratch* (files the app creates in its own data folder, editable in the
//! Raw View). The kind is a property of the space, so "Imported and Scratch
//! never mix" is enforced by the operations below rather than by convention.

use crate::filter::FilterState;
use crate::marker::{TextMarker, add_marker, recolor_marker_at, remove_marker_at};
use crate::model::LogEntry;
use crate::palette::HighlightColor;
use crate::remote::command::{Elevation, PresetFields, check_elevation, unedited_preset};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ops::Range;
use std::path::{Path, PathBuf};

/// A color the user painted on a stretch of one line of a file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredMarker {
    /// 1-based line index of the entry (`LogEntry::index`).
    pub line: usize,
    /// Bytes of the line's stored text.
    pub range: Range<usize>,
    pub color: HighlightColor,
    /// Hash of the line's content when the marker was set. A marker whose
    /// hash no longer matches the saved line is dropped (Raw View edits).
    #[serde(default)]
    pub line_hash: Option<u64>,
    /// How many Generations before the file's current one the line was read
    /// in: 0 is the current file, 1 the file before its last rotation.
    #[serde(default)]
    pub generations_ago: u32,
}

/// Read stored markers, skipping any in an older format (a Marker of a whole
/// line with a type and a note): those are not converted.
fn read_markers<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<StoredMarker>, D::Error> {
    let values: Vec<serde_json::Value> = Vec::deserialize(d)?;
    // Only the Markers of the current file come back: after a restart the
    // file is a new Generation and the lines of the older ones are gone.
    Ok(values
        .into_iter()
        .filter_map(|v| serde_json::from_value::<StoredMarker>(v).ok())
        .filter(|m| m.generations_ago == 0)
        .collect())
}

/// Read the members of a Log Space, skipping one that cannot be read (written
/// by a newer version) instead of failing the whole session. The lines of a
/// Remote Command differ from one Run to the next, so its Markers are not
/// brought back.
fn read_files<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<LogFile>, D::Error> {
    let values: Vec<serde_json::Value> = Vec::deserialize(d)?;
    Ok(values
        .into_iter()
        .filter_map(|v| serde_json::from_value::<LogFile>(v).ok())
        .map(|mut file| {
            if file.is_remote_command() {
                file.markers.clear();
            }
            file
        })
        .collect())
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

/// What a member of a Remote Log Space is on its Server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RemoteTarget {
    /// A file, by absolute path on the Server.
    File { path: String },
    /// A command whose output is read live.
    Command {
        command: String,
        /// The Command Preset it was written from, kept only while the command
        /// is exactly what the preset writes.
        #[serde(default)]
        preset: Option<PresetFields>,
        #[serde(default)]
        restart_on_exit: bool,
    },
}

/// A file or a command on a Server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RemoteRefRepr")]
pub struct RemoteRef {
    pub server_id: String,
    pub target: RemoteTarget,
    /// Starting Time of a Remote File as typed (a span or a time): older
    /// lines are not read. Empty: read the last lines as usual.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_time: Option<String>,
}

/// `RemoteRef` as stored: the older form has a `path` and no `target`.
#[derive(Deserialize)]
struct RemoteRefRepr {
    server_id: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    target: Option<RemoteTarget>,
    #[serde(default)]
    start_time: Option<String>,
}

impl TryFrom<RemoteRefRepr> for RemoteRef {
    type Error = String;

    fn try_from(repr: RemoteRefRepr) -> Result<Self, String> {
        let target = match (repr.target, repr.path) {
            (Some(target), _) => target,
            (None, Some(path)) => RemoteTarget::File { path },
            (None, None) => return Err("a remote member has neither a path nor a target".into()),
        };
        Ok(RemoteRef { server_id: repr.server_id, target, start_time: repr.start_time })
    }
}

impl RemoteRef {
    pub fn file(server_id: &str, path: &str) -> Self {
        Self { server_id: server_id.to_string(), target: RemoteTarget::File { path: path.to_string() }, start_time: None }
    }

    /// The path of a Remote File.
    pub fn path(&self) -> Option<&str> {
        match &self.target {
            RemoteTarget::File { path } => Some(path),
            RemoteTarget::Command { .. } => None,
        }
    }

    pub fn is_command(&self) -> bool {
        matches!(self.target, RemoteTarget::Command { .. })
    }
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
    /// The colors painted on text, remembered automatically.
    #[serde(default, deserialize_with = "read_markers")]
    pub markers: Vec<StoredMarker>,
    /// Where a Remote File is: its Server and path on it.
    #[serde(default)]
    pub remote: Option<RemoteRef>,
    /// For an Archive Entry, its path inside the Archive at `path`.
    #[serde(default)]
    pub entry: Option<String>,
    /// The Live Glob that brought a Remote File in (`None` when it was added
    /// by its path).
    #[serde(default)]
    pub from_glob: Option<String>,
    /// For a Remote Copy, the folder in the app's data folder that holds it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copy: Option<String>,
    /// The file's current Generation, counted from 0 each time it is opened
    /// and raised on every rotation, truncation or deletion.
    #[serde(skip)]
    pub generation: u32,
}

impl LogFile {
    pub fn new(file_id: String, display_name: String) -> Self {
        Self {
            file_id,
            display_name,
            path: None,
            added_seq: 0,
            markers: Vec::new(),
            remote: None,
            entry: None,
            from_glob: None,
            copy: None,
            generation: 0,
        }
    }

    /// A file on a Server. Its identity is the Server and the path; it is
    /// shown as `<server>:<path>`.
    pub fn remote(server_id: &str, server_name: &str, path: &str) -> Self {
        Self {
            file_id: format!("remote:{server_id}:{path}"),
            display_name: format!("{server_name}:{path}"),
            path: None,
            added_seq: 0,
            markers: Vec::new(),
            remote: Some(RemoteRef::file(server_id, path)),
            entry: None,
            from_glob: None,
            copy: None,
            generation: 0,
        }
    }

    /// A command run on a Server. Its identity is the Server and the command;
    /// it is shown as `<server>$ <command>`.
    pub fn remote_command(
        server_id: &str,
        server_name: &str,
        command: &str,
        preset: Option<&PresetFields>,
    ) -> Self {
        Self {
            file_id: format!("remote-cmd:{server_id}:{:016x}", line_hash(command)),
            display_name: format!("{server_name}$ {command}"),
            path: None,
            added_seq: 0,
            markers: Vec::new(),
            remote: Some(RemoteRef {
                server_id: server_id.to_string(),
                target: RemoteTarget::Command {
                    command: command.to_string(),
                    preset: unedited_preset(command, preset),
                    restart_on_exit: false,
                },
                start_time: None,
            }),
            entry: None,
            from_glob: None,
            copy: None,
            generation: 0,
        }
    }

    /// Whether the file is a Remote Command.
    pub fn is_remote_command(&self) -> bool {
        self.remote.as_ref().is_some_and(RemoteRef::is_command)
    }

    /// Whether both read the same thing on the same Server: the same path, or
    /// the same command. An edited file keeps its `file_id`, so this (and not
    /// the id) tells whether two members are the same.
    fn same_remote_target(&self, other: &LogFile) -> bool {
        match (&self.remote, &other.remote) {
            (Some(a), Some(b)) => {
                a.server_id == b.server_id
                    && match (&a.target, &b.target) {
                        (RemoteTarget::File { path: x }, RemoteTarget::File { path: y }) => x == y,
                        (RemoteTarget::Command { command: x, .. }, RemoteTarget::Command { command: y, .. }) => x == y,
                        _ => false,
                    }
            }
            _ => false,
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
            remote: None,
            entry: None,
            from_glob: None,
            copy: None,
            generation: 0,
        }
    }

    /// A Remote Copy: the content of a Remote File and its Rotated Files read
    /// whole into `dir`. It is shown as `<server>:<path>`.
    pub fn remote_copy(dir: &str, server_name: &str, path: &str) -> Self {
        let id = Path::new(dir).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let mut file = Self::new(format!("remote-copy:{id}"), format!("{server_name}:{path}"));
        file.copy = Some(dir.to_string());
        file
    }

    /// Whether the file is a Remote Copy.
    pub fn is_remote_copy(&self) -> bool {
        self.copy.is_some()
    }

    /// A file inside an Archive. Its identity is the Archive and the entry;
    /// it is shown as `<archive name> › <entry>` (a lone `.gz` shows its
    /// name without `.gz`).
    pub fn from_archive_entry(archive: &str, entry: &str) -> Self {
        let archive_path = Path::new(archive);
        let archive_name = archive_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| archive.to_string());
        let lone_gz = crate::archive::SINGLE_SUFFIXES.iter().any(|s| archive_name.ends_with(s)) && entry == crate::archive::gz_entry_name(archive_path);
        Self {
            file_id: format!("{archive}!/{entry}"),
            display_name: if lone_gz { entry.to_string() } else { format!("{archive_name} › {entry}") },
            path: Some(archive.to_string()),
            added_seq: 0,
            markers: Vec::new(),
            remote: None,
            entry: Some(entry.to_string()),
            from_glob: None,
            copy: None,
            generation: 0,
        }
    }

    /// Where the file's bytes are read from, if it has a local source.
    pub fn source(&self) -> Option<crate::archive::FileSource<'_>> {
        let path = Path::new(self.path.as_deref()?);
        Some(match self.entry.as_deref() {
            Some(entry) => crate::archive::FileSource::ArchiveEntry { archive: path, entry },
            None => crate::archive::FileSource::Disk(path),
        })
    }

    /// Name of the folder holding the file (for an Archive Entry, the
    /// folder holding the Archive), shown beside the name in a File
    /// Section header.
    pub fn folder(&self) -> Option<String> {
        self.path
            .as_deref()
            .and_then(|p| Path::new(p).parent())
            .map(|p| p.to_string_lossy().to_string())
            .filter(|p| !p.is_empty() && p != ".")
    }

    /// The file was rotated, truncated or deleted: what is read next is a new
    /// Generation, and every stored Marker is one Generation further back.
    pub fn begin_generation(&mut self) {
        self.generation += 1;
        for m in &mut self.markers {
            m.generations_ago += 1;
        }
    }

    /// Set the Starting Time of a Remote File (`None`: read as usual).
    pub fn set_start_time(&mut self, start: Option<String>) {
        if let Some(remote) = self.remote.as_mut() {
            remote.start_time = start.filter(|s| !s.trim().is_empty());
        }
    }

    /// A Rotated File was loaded above everything held: it is a Generation
    /// before all the others. The file's own count moves with the lines'
    /// Generations, so Markers keep their distance (and the new Generation is
    /// one further back than the oldest held).
    pub fn begin_earlier_generation(&mut self) {
        self.generation += 1;
    }

    /// How many Generations back a line read in `generation` is.
    pub fn generations_ago(&self, generation: u32) -> u32 {
        self.generation.saturating_sub(generation)
    }

    /// The Markers painted on one line of the current Generation.
    pub fn markers_on_line(&self, line: usize) -> Vec<TextMarker> {
        self.markers_on_line_in(line, 0)
    }

    /// The Markers painted on one line, `ago` Generations back.
    pub fn markers_on_line_in(&self, line: usize, ago: u32) -> Vec<TextMarker> {
        self.markers
            .iter()
            .filter(|m| m.line == line && m.generations_ago == ago)
            .map(|m| TextMarker { range: m.range.clone(), color: m.color.clone() })
            .collect()
    }

    fn set_line_markers(&mut self, line: usize, ago: u32, line_text: &str, markers: Vec<TextMarker>) {
        let hash = line_hash(line_text);
        self.markers.retain(|m| !(m.line == line && m.generations_ago == ago));
        self.markers.extend(markers.into_iter().map(|m| StoredMarker {
            line,
            range: m.range,
            color: m.color,
            line_hash: Some(hash),
            generations_ago: ago,
        }));
        self.markers.sort_by_key(|m| (m.generations_ago, m.line, m.range.start));
    }

    /// Forget the Markers of lines that left the Live Window: those of
    /// `generation` with a number below `first_kept`, and every older
    /// Generation.
    pub fn drop_markers_before(&mut self, generation: u32, first_kept: usize) {
        let ago = self.generations_ago(generation);
        self.markers.retain(|m| {
            m.generations_ago < ago || (m.generations_ago == ago && m.line >= first_kept)
        });
    }

    /// Paint `color` on `range` of a line; a Marker under it is cut. Returns
    /// whether anything was painted.
    pub fn paint_marker(&mut self, line: usize, line_text: &str, range: Range<usize>, color: HighlightColor) -> bool {
        self.paint_marker_in(0, line, line_text, range, color)
    }

    /// `paint_marker` for a line read `ago` Generations back.
    pub fn paint_marker_in(
        &mut self,
        ago: u32,
        line: usize,
        line_text: &str,
        range: Range<usize>,
        color: HighlightColor,
    ) -> bool {
        let mut on_line = self.markers_on_line_in(line, ago);
        if !add_marker(&mut on_line, range, color) {
            return false;
        }
        self.set_line_markers(line, ago, line_text, on_line);
        true
    }

    /// Remove the whole Marker containing `byte` of a line.
    pub fn remove_marker_at(&mut self, line: usize, line_text: &str, byte: usize) -> bool {
        self.remove_marker_in(0, line, line_text, byte)
    }

    /// `remove_marker_at` for a line read `ago` Generations back.
    pub fn remove_marker_in(&mut self, ago: u32, line: usize, line_text: &str, byte: usize) -> bool {
        let mut on_line = self.markers_on_line_in(line, ago);
        if !remove_marker_at(&mut on_line, byte) {
            return false;
        }
        self.set_line_markers(line, ago, line_text, on_line);
        true
    }

    /// Give the Marker containing `byte` of a line another color.
    pub fn recolor_marker_at(&mut self, line: usize, line_text: &str, byte: usize, color: HighlightColor) -> bool {
        self.recolor_marker_in(0, line, line_text, byte, color)
    }

    /// `recolor_marker_at` for a line read `ago` Generations back.
    pub fn recolor_marker_in(
        &mut self,
        ago: u32,
        line: usize,
        line_text: &str,
        byte: usize,
        color: HighlightColor,
    ) -> bool {
        let mut on_line = self.markers_on_line_in(line, ago);
        if !recolor_marker_at(&mut on_line, byte, color) {
            return false;
        }
        self.set_line_markers(line, ago, line_text, on_line);
        true
    }

    /// Restore the remembered Markers onto freshly loaded `entries`.
    pub fn apply_markers(&self, entries: &mut [LogEntry]) {
        for entry in entries.iter_mut() {
            entry.markers.clear();
        }
        if self.markers.is_empty() {
            return;
        }
        let mut by_line: HashMap<(usize, u32), Vec<TextMarker>> = HashMap::new();
        for m in &self.markers {
            by_line
                .entry((m.line, m.generations_ago))
                .or_default()
                .push(TextMarker { range: m.range.clone(), color: m.color.clone() });
        }
        for entry in entries.iter_mut() {
            if let Some(markers) = by_line.get(&(entry.index, self.generations_ago(entry.generation))) {
                entry.markers = markers.clone();
            }
        }
    }

    /// Drop the markers whose line content changed since they were set.
    /// `lines` is the saved text, one element per line. Returns how many
    /// markers were dropped.
    pub fn prune_markers(&mut self, lines: &[&str]) -> usize {
        let before = self.markers.len();
        self.markers.retain(|m| match (m.line_hash, lines.get(m.line.wrapping_sub(1))) {
            // Raw View edits only reach the current file.
            _ if m.generations_ago > 0 => true,
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
    /// Files on Servers, read live over SSH; read-only.
    Remote,
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

/// What to import into an Imported Log Space.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportSource {
    Disk(String),
    Entry { archive: String, entry: String },
}

impl ImportSource {
    fn to_file(&self) -> LogFile {
        match self {
            ImportSource::Disk(path) => LogFile::from_path(path),
            ImportSource::Entry { archive, entry } => LogFile::from_archive_entry(archive, entry),
        }
    }
}

/// Why a Remote Command could not be added.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddCommandError {
    /// Only a Remote Log Space takes Remote Commands.
    WrongKind,
    /// The command starts with `sudo`, `su`, …
    Elevation(Elevation),
}

/// Outcome of importing files into an Imported Log Space.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportOutcome {
    /// `file_id`s of the files that were added.
    pub added: Vec<String>,
    /// Paths skipped because the space already holds them.
    pub skipped: Vec<String>,
}

/// What a Live Glob brought into a space.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GlobAdded {
    /// `file_id`s of the new Remote Files.
    pub added: Vec<String>,
}

/// A glob on one Server, kept by a Remote Log Space: the files matching it,
/// including those that appear later, are the space's Remote Files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveGlob {
    pub glob_id: String,
    pub server_id: String,
    /// Absolute folder; the glob is only in the file name.
    pub dir: String,
    pub pattern: String,
    /// Paths the user removed from the glob: it does not add them again.
    #[serde(default)]
    pub excluded: Vec<String>,
    /// The user stopped watching: no new file is added.
    #[serde(default)]
    pub stopped: bool,
}

impl LiveGlob {
    /// The glob as typed: `<dir>/<pattern>`.
    pub fn full_pattern(&self) -> String {
        crate::remote::browse::join(&self.dir, &self.pattern)
    }

    /// Shown as `<server>:<path>`.
    pub fn label(&self, server_name: &str) -> String {
        format!("{server_name}:{}", self.full_pattern())
    }

    pub fn path_of(&self, name: &str) -> String {
        crate::remote::browse::join(&self.dir, name)
    }

    /// This glob as it is after an edit to `server_id`, `dir` and `pattern`:
    /// the same `glob_id` and `stopped`, and only the exclusions the new glob
    /// still matches (none when the Server changed).
    pub fn edited(&self, server_id: &str, dir: &str, pattern: &str) -> LiveGlob {
        let same_server = self.server_id == server_id;
        LiveGlob {
            glob_id: self.glob_id.clone(),
            server_id: server_id.to_string(),
            dir: dir.to_string(),
            pattern: pattern.to_string(),
            excluded: self
                .excluded
                .iter()
                .filter(|path| {
                    same_server
                        && crate::remote::browse::dirname(path) == dir
                        && crate::remote::browse::glob_match(pattern, path.rsplit('/').next().unwrap_or(""))
                })
                .cloned()
                .collect(),
            stopped: self.stopped,
        }
    }
}

/// Why a member of a Remote Log Space could not be edited.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditError {
    /// Only a Remote Log Space has members to edit.
    WrongKind,
    /// The member (or the Live Glob) is not in the space, or is not the kind
    /// the edit is for.
    NotFound,
    /// Another member of the space already reads this.
    Duplicate,
    /// The command starts with `sudo`, `su`, …
    Elevation(Elevation),
}

/// What an edit of a Live Glob does to its files, decided from the folder's
/// listing (see `remote::glob_plan::plan_glob_edit`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GlobEditPlan {
    /// Paths of the files to add.
    pub add: Vec<String>,
    /// Paths of the glob's files that stop matching.
    pub remove: Vec<String>,
    /// Paths of the glob's files that go on as they are.
    pub keep: Vec<String>,
}

/// Why a Live Glob was not added.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddGlobError {
    WrongKind,
    /// The space already has a Live Glob with this Server and pattern.
    Duplicate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogSpace {
    pub id: String,
    pub name: String,
    pub kind: LogSpaceKind,
    #[serde(default)]
    pub file_order: FileOrder,
    #[serde(deserialize_with = "read_files")]
    pub files: Vec<LogFile>,
    /// The Live Globs of a Remote Log Space.
    #[serde(default)]
    pub globs: Vec<LiveGlob>,
    /// The Filter shared by every file in the Log Space's Parsed View.
    #[serde(default)]
    pub filter: FilterState,
    /// The separate Filter of each File View.
    #[serde(default)]
    pub file_filters: HashMap<String, FilterState>,
    /// The Focus Tabs shared by every file in the Log Space's views.
    #[serde(default)]
    pub focus: crate::focus::FocusTabs,
    /// The separate Focus Tabs of each File View.
    #[serde(default)]
    pub file_focus: HashMap<String, crate::focus::FocusTabs>,
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
            globs: Vec::new(),
            filter: FilterState::default(),
            file_filters: HashMap::new(),
            focus: Default::default(),
            file_focus: HashMap::new(),
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

    /// Add a Remote Copy (from `LogFile::remote_copy`) to an Imported Log
    /// Space. It comes only through Open in full.
    pub fn add_remote_copy(&mut self, mut file: LogFile) -> Result<String, WrongKind> {
        if self.kind != LogSpaceKind::Imported || !file.is_remote_copy() {
            return Err(WrongKind);
        }
        file.added_seq = self.take_seq();
        let id = file.file_id.clone();
        self.files.push(file);
        Ok(id)
    }

    /// Import files by path into an Imported Log Space. Paths the space
    /// already holds are skipped and reported.
    pub fn add_imported_files(&mut self, paths: &[String]) -> Result<ImportOutcome, WrongKind> {
        let sources: Vec<ImportSource> = paths.iter().cloned().map(ImportSource::Disk).collect();
        self.add_imported_sources(&sources)
    }

    /// Import files and Archive Entries. A source the space already holds
    /// (same path, or same Archive and entry) is skipped and reported by name.
    pub fn add_imported_sources(&mut self, sources: &[ImportSource]) -> Result<ImportOutcome, WrongKind> {
        if self.kind != LogSpaceKind::Imported {
            return Err(WrongKind);
        }
        let mut outcome = ImportOutcome::default();
        for source in sources {
            let mut file = source.to_file();
            if self.has_file(&file.file_id) {
                outcome.skipped.push(match source {
                    ImportSource::Disk(path) => path.clone(),
                    ImportSource::Entry { .. } => file.display_name.clone(),
                });
                continue;
            }
            file.added_seq = self.take_seq();
            outcome.added.push(file.file_id.clone());
            self.files.push(file);
        }
        Ok(outcome)
    }

    /// Whether a member already reads what `file` reads.
    fn has_remote_target(&self, file: &LogFile) -> bool {
        self.files.iter().any(|f| f.same_remote_target(file))
    }

    /// `base`, or `base` with a number when a member (an edited one that kept
    /// an id made from its old path) already has it.
    fn free_file_id(&self, base: String) -> String {
        if !self.has_file(&base) {
            return base;
        }
        (2..)
            .map(|n| format!("{base}#{n}"))
            .find(|id| !self.has_file(id))
            .expect("unbounded range")
    }

    /// The `file_id` of the Remote File on a Server at `path`.
    pub fn remote_file_id(&self, server_id: &str, path: &str) -> Option<String> {
        self.files
            .iter()
            .find(|f| f.remote.as_ref().is_some_and(|r| r.server_id == server_id && r.path() == Some(path)))
            .map(|f| f.file_id.clone())
    }

    /// Point a Remote File or Remote Command at another path or command, or
    /// another Server. It stays the same member: `file_id`, `added_seq`, its
    /// place in the list and its Live Glob stay, and `display_name` follows.
    /// Its Markers go (they were on another file) and it is read from
    /// Generation 0. A Remote File keeps its `start_time` unless the caller
    /// sets it again; a Remote Command keeps `restart_on_exit`.
    ///
    /// Refused when another member already reads the new target.
    pub fn replace_remote_target(
        &mut self,
        file_id: &str,
        server_id: &str,
        server_name: &str,
        target: RemoteTarget,
    ) -> Result<(), EditError> {
        if self.kind != LogSpaceKind::Remote {
            return Err(EditError::WrongKind);
        }
        let Some(current) = self.file(file_id) else { return Err(EditError::NotFound) };
        let Some(old) = current.remote.as_ref() else { return Err(EditError::NotFound) };
        let target = match (&old.target, target) {
            (RemoteTarget::File { .. }, target @ RemoteTarget::File { .. }) => target,
            (RemoteTarget::Command { restart_on_exit, .. }, RemoteTarget::Command { command, preset, .. }) => {
                check_elevation(&command).map_err(EditError::Elevation)?;
                let preset = unedited_preset(&command, preset.as_ref());
                RemoteTarget::Command { command, preset, restart_on_exit: *restart_on_exit }
            }
            // An edit does not turn a file into a command or the other way.
            _ => return Err(EditError::NotFound),
        };
        let display_name = match &target {
            RemoteTarget::File { path } => format!("{server_name}:{path}"),
            RemoteTarget::Command { command, .. } => format!("{server_name}$ {command}"),
        };
        let candidate = LogFile {
            remote: Some(RemoteRef { server_id: server_id.to_string(), target: target.clone(), start_time: None }),
            ..LogFile::new(String::new(), String::new())
        };
        if self.files.iter().any(|f| f.file_id != file_id && f.same_remote_target(&candidate)) {
            return Err(EditError::Duplicate);
        }
        let file = self.file_mut(file_id).expect("found above");
        let is_command = matches!(target, RemoteTarget::Command { .. });
        let remote = file.remote.as_mut().expect("found above");
        remote.server_id = server_id.to_string();
        remote.target = target;
        file.display_name = display_name;
        // A Remote Command keeps the lines and Markers of its earlier Runs.
        if !is_command {
            file.markers.clear();
            file.generation = 0;
        }
        Ok(())
    }

    /// Change the Starting Time of a Remote File (`None`: read as usual),
    /// keeping its Markers. Returns whether the file is a Remote File.
    pub fn set_start_time(&mut self, file_id: &str, start: Option<String>) -> bool {
        match self.file_mut(file_id) {
            Some(file) if file.remote.as_ref().is_some_and(|r| r.path().is_some()) => {
                file.set_start_time(start);
                true
            }
            _ => false,
        }
    }

    /// Edit a Live Glob in place: it keeps its `glob_id`, so the files that
    /// go on stay its files. `plan` says which of its files go and which come
    /// (nothing comes while the glob is stopped). Paths it excluded that the
    /// new glob no longer matches are forgotten. Returns the removed files,
    /// for the caller to close.
    ///
    /// Refused when another Live Glob of the space is the same.
    pub fn edit_live_glob(
        &mut self,
        glob_id: &str,
        server_id: &str,
        server_name: &str,
        dir: &str,
        pattern: &str,
        plan: &GlobEditPlan,
    ) -> Result<Vec<LogFile>, EditError> {
        if self.kind != LogSpaceKind::Remote {
            return Err(EditError::WrongKind);
        }
        let Some(glob) = self.glob(glob_id) else { return Err(EditError::NotFound) };
        let stopped = glob.stopped;
        if self.globs.iter().any(|g| g.glob_id != glob_id && g.server_id == server_id && g.dir == dir && g.pattern == pattern)
        {
            return Err(EditError::Duplicate);
        }
        let mut removed = Vec::new();
        for file in std::mem::take(&mut self.files) {
            let goes = file.from_glob.as_deref() == Some(glob_id)
                && file.remote.as_ref().and_then(|r| r.path()).is_some_and(|p| plan.remove.iter().any(|r| r == p));
            if goes {
                self.file_filters.remove(&file.file_id);
                self.file_focus.remove(&file.file_id);
                removed.push(file);
            } else {
                self.files.push(file);
            }
        }
        let glob = self.glob_mut(glob_id).expect("found above");
        *glob = glob.edited(server_id, dir, pattern);
        if !stopped {
            self.add_glob_files(glob_id, server_name, &plan.add);
        }
        Ok(removed)
    }

    /// Add Remote Files, given as `(server id, server name, path)`. A file
    /// already in the space (same Server and path) is skipped and reported by
    /// its name. A path a Live Glob of the same Server had excluded is taken
    /// off its exclusions and comes back as one of that glob's files.
    pub fn add_remote_files(&mut self, files: &[(String, String, String)]) -> Result<ImportOutcome, WrongKind> {
        if self.kind != LogSpaceKind::Remote {
            return Err(WrongKind);
        }
        let mut outcome = ImportOutcome::default();
        for (server_id, server_name, path) in files {
            let mut file = LogFile::remote(server_id, server_name, path);
            if self.has_remote_target(&file) {
                outcome.skipped.push(file.display_name.clone());
                continue;
            }
            file.file_id = self.free_file_id(file.file_id);
            if let Some(glob) = self
                .globs
                .iter_mut()
                .find(|g| g.server_id == *server_id && g.excluded.iter().any(|e| e == path))
            {
                glob.excluded.retain(|e| e != path);
                file.from_glob = Some(glob.glob_id.clone());
            }
            file.added_seq = self.take_seq();
            outcome.added.push(file.file_id.clone());
            self.files.push(file);
        }
        Ok(outcome)
    }

    /// Add a Live Glob on a Server: `dir` is an existing folder and `pattern`
    /// has the glob characters. Returns its id.
    pub fn add_live_glob(&mut self, server_id: &str, dir: &str, pattern: &str) -> Result<String, AddGlobError> {
        if self.kind != LogSpaceKind::Remote {
            return Err(AddGlobError::WrongKind);
        }
        if self.globs.iter().any(|g| g.server_id == server_id && g.dir == dir && g.pattern == pattern) {
            return Err(AddGlobError::Duplicate);
        }
        let glob_id = format!("glob-{}", self.take_seq());
        self.globs.push(LiveGlob {
            glob_id: glob_id.clone(),
            server_id: server_id.to_string(),
            dir: dir.to_string(),
            pattern: pattern.to_string(),
            excluded: Vec::new(),
            stopped: false,
        });
        Ok(glob_id)
    }

    pub fn glob(&self, glob_id: &str) -> Option<&LiveGlob> {
        self.globs.iter().find(|g| g.glob_id == glob_id)
    }

    pub fn glob_mut(&mut self, glob_id: &str) -> Option<&mut LiveGlob> {
        self.globs.iter_mut().find(|g| g.glob_id == glob_id)
    }

    /// The files a Live Glob brought.
    pub fn glob_files(&self, glob_id: &str) -> Vec<&LogFile> {
        self.files.iter().filter(|f| f.from_glob.as_deref() == Some(glob_id)).collect()
    }

    /// Add the files a Live Glob found, as `(server name, path)`. A path the
    /// space already holds keeps its first origin.
    pub fn add_glob_files(&mut self, glob_id: &str, server_name: &str, paths: &[String]) -> GlobAdded {
        let mut out = GlobAdded::default();
        let Some(glob) = self.glob(glob_id) else { return out };
        let server_id = glob.server_id.clone();
        for path in paths {
            let mut file = LogFile::remote(&server_id, server_name, path);
            if self.has_remote_target(&file) {
                continue;
            }
            file.file_id = self.free_file_id(file.file_id);
            file.from_glob = Some(glob_id.to_string());
            file.added_seq = self.take_seq();
            out.added.push(file.file_id.clone());
            self.files.push(file);
        }
        out
    }

    /// Stop or resume watching. Returns whether the glob exists.
    pub fn set_glob_stopped(&mut self, glob_id: &str, stopped: bool) -> bool {
        match self.glob_mut(glob_id) {
            Some(glob) => {
                glob.stopped = stopped;
                true
            }
            None => false,
        }
    }

    /// Remove a Live Glob and every file it brought; files added by path
    /// stay. Returns the removed files.
    pub fn remove_live_glob(&mut self, glob_id: &str) -> Vec<LogFile> {
        let Some(pos) = self.globs.iter().position(|g| g.glob_id == glob_id) else { return Vec::new() };
        self.globs.remove(pos);
        let mut removed = Vec::new();
        let mut kept = Vec::new();
        for file in std::mem::take(&mut self.files) {
            if file.from_glob.as_deref() == Some(glob_id) {
                self.file_filters.remove(&file.file_id);
                self.file_focus.remove(&file.file_id);
                removed.push(file);
            } else {
                kept.push(file);
            }
        }
        self.files = kept;
        removed
    }

    /// Drop files of a Live Glob without excluding them (they are gone from
    /// its folder when the space is reopened).
    pub fn drop_glob_files(&mut self, file_ids: &[String]) {
        for id in file_ids {
            self.file_filters.remove(id);
            self.file_focus.remove(id);
        }
        self.files.retain(|f| !(f.from_glob.is_some() && file_ids.contains(&f.file_id)));
    }

    /// Names of the Remote Files on a Server.
    pub fn files_on_server(&self, server_id: &str) -> Vec<&LogFile> {
        self.files
            .iter()
            .filter(|f| f.remote.as_ref().is_some_and(|r| r.server_id == server_id))
            .collect()
    }

    /// Add Remote Commands, given as `(server id, server name, command,
    /// preset)`. One the space already has (same Server and command) is
    /// skipped and reported by its name; one that elevates is refused.
    pub fn add_remote_commands(
        &mut self,
        commands: &[(String, String, String, Option<PresetFields>)],
    ) -> Result<ImportOutcome, AddCommandError> {
        if self.kind != LogSpaceKind::Remote {
            return Err(AddCommandError::WrongKind);
        }
        let mut outcome = ImportOutcome::default();
        for (server_id, server_name, command, preset) in commands {
            check_elevation(command).map_err(AddCommandError::Elevation)?;
            let mut file = LogFile::remote_command(server_id, server_name, command, preset.as_ref());
            if self.has_remote_target(&file) {
                outcome.skipped.push(file.display_name.clone());
                continue;
            }
            file.file_id = self.free_file_id(file.file_id);
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
        self.file_focus.remove(file_id);
        let file = self.files.remove(pos);
        // A file the user takes out of a Live Glob is not added again by it.
        if let (Some(glob_id), Some(path)) =
            (file.from_glob.as_deref(), file.remote.as_ref().and_then(|r| r.path()))
        {
            if let Some(glob) = self.glob_mut(glob_id) {
                if !glob.excluded.iter().any(|p| p == path) {
                    glob.excluded.push(path.to_string());
                }
            }
        }
        Ok(file)
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

    /// The Focus Tabs of a File View, one empty tab until some are stored.
    pub fn file_focus(&self, file_id: &str) -> crate::focus::FocusTabs {
        self.file_focus.get(file_id).cloned().unwrap_or_default()
    }

    pub fn set_file_focus(&mut self, file_id: &str, focus: crate::focus::FocusTabs) {
        self.file_focus.insert(file_id.to_string(), focus);
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
