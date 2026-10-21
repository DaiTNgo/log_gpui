//! Headless state of the New Log Space dialog. The kind of the space is not
//! chosen: no files means Scratch, one or more files means Imported.

use crate::archive::{self, ArchiveError, ArchiveKind, LARGE_ENTRY_BYTES, MAX_UNCOMPRESSED_BYTES};
use crate::workspace::log_space::{ImportSource, LogFile, LogSpaceKind, SpaceNameError, validate_space_name};
use crate::workspace::project::Project;
use std::io::Read;
use std::path::Path;

/// How many leading bytes decide whether a file is text.
const SNIFF_BYTES: usize = 8 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingStatus {
    Ok,
    /// "Not a text file": shown with a warning and never added.
    Binary,
    /// Already in the list.
    Duplicate,
    /// An Archive Entry above 200 MB: warned about, but still added.
    LargeFile,
    /// An Archive that cannot be read: never added.
    Damaged,
    /// A password-protected Archive: never added.
    Encrypted,
    /// An Archive being listed in the background.
    Reading,
}

/// Where a pending file comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingSource {
    Disk(String),
    /// A file inside an Archive, with its declared size after unpacking.
    Entry { archive: String, entry: String, size: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingFile {
    pub source: PendingSource,
    pub status: PendingStatus,
}

impl PendingFile {
    pub fn warning(&self) -> Option<&'static str> {
        match self.status {
            PendingStatus::Ok | PendingStatus::Reading => None,
            PendingStatus::Binary => Some("Not a text file"),
            PendingStatus::Duplicate => Some("Already in the list"),
            PendingStatus::LargeFile => Some("Large file"),
            PendingStatus::Damaged => Some("Damaged archive"),
            PendingStatus::Encrypted => Some("Password-protected archive"),
        }
    }

    /// Whether the file is added when the space is created.
    pub fn is_importable(&self) -> bool {
        matches!(self.status, PendingStatus::Ok | PendingStatus::LargeFile)
    }

    /// The name the list shows: the file name, `<archive> › <entry>` for an
    /// Archive Entry, and "Reading <name>…" while an Archive is listed.
    pub fn display_name(&self) -> String {
        let name = match &self.source {
            PendingSource::Disk(path) => crate::local_path::file_name(path, crate::local_path::PathStyle::native())
                .unwrap_or(path)
                .to_string(),
            PendingSource::Entry { archive, entry, .. } => {
                LogFile::from_archive_entry(archive, entry).display_name
            }
        };
        if self.status == PendingStatus::Reading { format!("Reading {name}…") } else { name }
    }

    fn unpacked_size(&self) -> u64 {
        match &self.source {
            PendingSource::Entry { size, .. } if self.is_importable() => *size,
            _ => 0,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct NewLogSpaceForm {
    pub name: String,
    pub files: Vec<PendingFile>,
}

/// A file is binary when its first 8 KB hold a NUL byte or are not valid UTF-8.
pub fn is_binary(bytes: &[u8]) -> bool {
    if bytes.contains(&0) {
        return true;
    }
    match std::str::from_utf8(bytes) {
        Ok(_) => false,
        // The sniffed window may end in the middle of a character.
        Err(e) => e.error_len().is_some() || e.valid_up_to() + 4 < bytes.len(),
    }
}

/// Whether the file at `path` is binary. Unreadable files count as binary.
pub fn path_is_binary(path: &Path) -> bool {
    let Ok(file) = std::fs::File::open(path) else {
        return true;
    };
    let mut buf = Vec::with_capacity(SNIFF_BYTES);
    if file.take(SNIFF_BYTES as u64).read_to_end(&mut buf).is_err() {
        return true;
    }
    is_binary(&buf)
}

/// The entries of an Archive as pending files, in Archive order. An Archive
/// that cannot be read becomes a single pending file with the reason.
/// Reads the whole Archive: call it off the UI thread.
pub fn list_archive(path: &str, kind: ArchiveKind) -> Vec<PendingFile> {
    let disk = |status| vec![PendingFile { source: PendingSource::Disk(path.to_string()), status }];
    match archive::list(Path::new(path), kind) {
        Ok(entries) => entries
            .into_iter()
            .map(|e| {
                let status = if is_binary(&e.head) {
                    PendingStatus::Binary
                } else if e.size > LARGE_ENTRY_BYTES {
                    PendingStatus::LargeFile
                } else {
                    PendingStatus::Ok
                };
                let source = PendingSource::Entry { archive: path.to_string(), entry: e.name, size: e.size };
                PendingFile { source, status }
            })
            .collect(),
        Err(ArchiveError::Encrypted) => disk(PendingStatus::Encrypted),
        Err(_) => disk(PendingStatus::Damaged),
    }
}

/// Expand dropped or picked paths: a folder gives its direct child files
/// (sorted by name, no subfolders); a file is taken as is. Archives are
/// opened later, by `NewLogSpaceForm`.
pub fn expand_paths(paths: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for path in paths {
        let p = Path::new(path);
        if p.is_dir() {
            let mut children: Vec<String> = std::fs::read_dir(p)
                .into_iter()
                .flatten()
                .flatten()
                .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
                .map(|e| e.path().to_string_lossy().to_string())
                .collect();
            children.sort();
            out.extend(children);
        } else {
            out.push(path.clone());
        }
    }
    out
}

/// Everything the paths stand for, Archives opened: what a dialog would list.
/// Reads Archives whole: call it off the UI thread.
pub fn resolve_paths(paths: &[String]) -> Vec<PendingFile> {
    let mut form = NewLogSpaceForm::new();
    form.add_paths(paths);
    form.files
}

impl NewLogSpaceForm {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_name(&mut self, name: &str) {
        self.name = name.to_string();
    }

    /// Add dropped or picked paths (folders expanded one level), listing
    /// every Archive before returning.
    pub fn add_paths(&mut self, paths: &[String]) {
        for (archive, kind) in self.begin_paths(paths) {
            let items = list_archive(&archive, kind);
            self.finish_archive(&archive, items);
        }
    }

    /// Add dropped or picked paths without opening Archives: each Archive
    /// gets a "Reading…" row and is returned, to be listed in the background
    /// and then given to `finish_archive`.
    pub fn begin_paths(&mut self, paths: &[String]) -> Vec<(String, ArchiveKind)> {
        let mut archives = Vec::new();
        for path in expand_paths(paths) {
            if let Some(kind) = archive::detect(Path::new(&path)) {
                let source = PendingSource::Disk(path.clone());
                if self.push_source(source, PendingStatus::Reading) {
                    archives.push((path, kind));
                }
                continue;
            }
            let status = if path_is_binary(Path::new(&path)) {
                PendingStatus::Binary
            } else {
                PendingStatus::Ok
            };
            self.push(path, status);
        }
        archives
    }

    /// Replace the "Reading…" row of `archive` by its entries.
    pub fn finish_archive(&mut self, archive: &str, items: Vec<PendingFile>) {
        let reading = PendingSource::Disk(archive.to_string());
        let Some(at) = self
            .files
            .iter()
            .position(|f| f.source == reading && f.status == PendingStatus::Reading)
        else {
            return;
        };
        self.files.remove(at);
        let mut at = at;
        for item in items {
            if !self.files.iter().any(|f| f.source == item.source) {
                self.files.insert(at, item);
                at += 1;
            }
        }
    }

    /// Whether an Archive is still being listed.
    pub fn is_reading(&self) -> bool {
        self.files.iter().any(|f| f.status == PendingStatus::Reading)
    }

    /// Add a path with a known status (no disk access).
    pub fn push(&mut self, path: String, status: PendingStatus) {
        self.push_source(PendingSource::Disk(path), status);
    }

    /// Add a file or Archive Entry; false when it is already listed.
    pub fn push_source(&mut self, source: PendingSource, status: PendingStatus) -> bool {
        if self.files.iter().any(|f| f.source == source) {
            return false;
        }
        self.files.push(PendingFile { source, status });
        true
    }

    pub fn remove_file(&mut self, index: usize) {
        if index < self.files.len() {
            self.files.remove(index);
        }
    }

    /// What will be imported: text files and text Archive Entries.
    pub fn importable_sources(&self) -> Vec<ImportSource> {
        self.files
            .iter()
            .filter(|f| f.is_importable())
            .map(|f| match &f.source {
                PendingSource::Disk(path) => ImportSource::Disk(path.clone()),
                PendingSource::Entry { archive, entry, .. } => {
                    ImportSource::Entry { archive: archive.clone(), entry: entry.clone() }
                }
            })
            .collect()
    }

    /// Size after unpacking of everything that will be imported from Archives.
    pub fn total_uncompressed(&self) -> u64 {
        self.files.iter().map(PendingFile::unpacked_size).sum()
    }

    /// Why Create is off because of size, if it is.
    pub fn size_limit_message(&self) -> Option<String> {
        (self.total_uncompressed() > MAX_UNCOMPRESSED_BYTES)
            .then(|| "Too large: the files add up to more than 2 GB once unpacked".to_string())
    }

    pub fn kind(&self) -> LogSpaceKind {
        if self.importable_sources().is_empty() {
            LogSpaceKind::Scratch
        } else {
            LogSpaceKind::Imported
        }
    }

    /// Why the name is refused, if it is.
    pub fn name_error(&self, project: &Project) -> Option<SpaceNameError> {
        validate_space_name(&self.name, project.log_spaces.iter().map(|s| s.name.as_str())).err()
    }

    /// The message shown under the name field: only for a duplicate name,
    /// since an empty field is not an error yet.
    pub fn name_message(&self, project: &Project) -> Option<&'static str> {
        match self.name_error(project) {
            Some(SpaceNameError::Duplicate) => Some(SpaceNameError::Duplicate.message()),
            _ => None,
        }
    }

    pub fn can_create(&self, project: &Project) -> bool {
        self.name_error(project).is_none() && !self.is_reading() && self.size_limit_message().is_none()
    }

    /// "Create empty" or "Create with N file(s)".
    pub fn create_label(&self) -> String {
        match self.importable_sources().len() {
            0 => "Create empty".to_string(),
            1 => "Create with 1 file".to_string(),
            n => format!("Create with {n} files"),
        }
    }
}
