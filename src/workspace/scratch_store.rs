//! Disk side of Scratch Log Spaces: scratch files live in the app's own data
//! folder (`<config>/log_gpui/spaces/<space_id>/`). Every function takes
//! explicit paths so it can be tested against a temporary directory.

use std::io;
use std::path::Path;

/// Create an empty scratch file (and its folder). An existing file is kept.
pub fn create_file(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    match std::fs::OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        Err(e) => Err(e),
    }
}

/// Write the whole text of a scratch file.
pub fn save_text(path: &Path, text: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, text)
}

/// Read a scratch file; a missing file reads as empty.
pub fn read_text(path: &Path) -> io::Result<String> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(String::from_utf8_lossy(&bytes).into_owned()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(e),
    }
}

/// Rename a scratch file inside its folder.
pub fn rename_file(from: &Path, to: &Path) -> io::Result<()> {
    if from == to {
        return Ok(());
    }
    if to.exists() {
        return Err(io::Error::new(io::ErrorKind::AlreadyExists, "file name in use"));
    }
    match std::fs::rename(from, to) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

/// Delete a scratch file. A file that is already gone is fine.
pub fn delete_file(path: &Path) -> io::Result<()> {
    match std::fs::remove_file(path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

/// Delete a Scratch Log Space's whole folder.
pub fn delete_space_dir(dir: &Path) -> io::Result<()> {
    match std::fs::remove_dir_all(dir) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

/// Save As: write a copy of `text` to `dest`. The scratch file is untouched.
pub fn save_as(text: &str, dest: &Path) -> io::Result<()> {
    std::fs::write(dest, text)
}
