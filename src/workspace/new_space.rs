//! Headless state of the New Log Space dialog. The kind of the space is not
//! chosen: no files means Scratch, one or more files means Imported.

use crate::workspace::log_space::{LogSpaceKind, SpaceNameError, validate_space_name};
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingFile {
    pub path: String,
    pub status: PendingStatus,
}

impl PendingFile {
    pub fn warning(&self) -> Option<&'static str> {
        match self.status {
            PendingStatus::Ok => None,
            PendingStatus::Binary => Some("Not a text file"),
            PendingStatus::Duplicate => Some("Already in the list"),
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

/// Expand dropped or picked paths: a folder gives its direct child files
/// (sorted by name, no subfolders); a file is taken as is.
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

impl NewLogSpaceForm {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_name(&mut self, name: &str) {
        self.name = name.to_string();
    }

    /// Add dropped or picked paths (folders expanded one level).
    pub fn add_paths(&mut self, paths: &[String]) {
        for path in expand_paths(paths) {
            let status = if path_is_binary(Path::new(&path)) {
                PendingStatus::Binary
            } else {
                PendingStatus::Ok
            };
            self.push(path, status);
        }
    }

    /// Add a path with a known status (no disk access).
    pub fn push(&mut self, path: String, status: PendingStatus) {
        if self.files.iter().any(|f| f.path == path) {
            return;
        }
        self.files.push(PendingFile { path, status });
    }

    pub fn remove_file(&mut self, index: usize) {
        if index < self.files.len() {
            self.files.remove(index);
        }
    }

    /// Paths that will be imported: text files only.
    pub fn importable_paths(&self) -> Vec<String> {
        self.files
            .iter()
            .filter(|f| f.status == PendingStatus::Ok)
            .map(|f| f.path.clone())
            .collect()
    }

    pub fn kind(&self) -> LogSpaceKind {
        if self.importable_paths().is_empty() {
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
        self.name_error(project).is_none()
    }

    /// "Create empty" or "Create with N file(s)".
    pub fn create_label(&self) -> String {
        match self.importable_paths().len() {
            0 => "Create empty".to_string(),
            1 => "Create with 1 file".to_string(),
            n => format!("Create with {n} files"),
        }
    }
}
