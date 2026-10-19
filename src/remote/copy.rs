//! Remote Copy on disk: what Open in full read from a Server, kept in the
//! app's data folder so a Project opens again without a connection.
//!
//! ```text
//! remote-copies/<id>/
//!   meta.json     what was read, and how each Generation ended
//!   gen-0.log     the file itself (or the part read), as text
//!   gen-1.log     app.log.1
//!   gen-2.log     app.log.2.gz, already decompressed
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io;
use std::path::{Path, PathBuf};

pub const META_FILE: &str = "meta.json";

/// Why the lines of a Generation stop short of the end of its file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StopReason {
    /// The user cancelled the read.
    Cancelled,
    /// The Full Read Limit was reached.
    FullReadLimit,
    /// The file was rotated while it was read and could not be read on.
    RotatedWhileReading,
    /// Older lines were left out to fit the Full Read Limit.
    OlderNotRead,
}

impl StopReason {
    /// The text of the Break after the last line read.
    pub fn label(self, limit: usize) -> String {
        match self {
            StopReason::Cancelled => "read cancelled".to_string(),
            StopReason::FullReadLimit => format!("stopped at Full Read Limit ({} lines)", group(limit)),
            StopReason::RotatedWhileReading => "file rotated while reading".to_string(),
            StopReason::OlderNotRead => "older lines were not read".to_string(),
        }
    }
}

/// One Generation of a Remote Copy: the file, or one of its Rotated Files.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GenerationMeta {
    /// 0 for the file itself, 1 for the Rotated File just before it, …
    pub distance: u32,
    /// File name on the Server, e.g. `app.log.2.gz`.
    pub source_name: String,
    pub compressed: bool,
    /// Where in the (decompressed) file reading began.
    pub start_offset: u64,
    /// Where reading ended, when it did.
    pub end_offset: Option<u64>,
    /// Number in its file of the first line kept; `None` when it could not be
    /// counted (the lines are numbered from 1 and shown with `~`).
    pub first_line_no: Option<usize>,
    /// Times (on the clock of the file) the read was limited to.
    pub from: Option<f64>,
    pub to: Option<f64>,
    /// The Generation was read up to where it was meant to end.
    pub complete: bool,
    pub stop: Option<StopReason>,
    pub lines: usize,
    /// Older lines of this file (or older files) were left out to fit the Full
    /// Read Limit: the Break at the top says so.
    #[serde(default)]
    pub older_not_read: bool,
    /// Name of the file in the folder.
    pub file: String,
}

impl GenerationMeta {
    pub fn file_name(distance: u32) -> String {
        format!("gen-{distance}.log")
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CopyMeta {
    pub server_id: String,
    pub server_name: String,
    /// Path of the Remote File the copy was made of.
    pub path: String,
    /// When it was read, `YYYY-MM-DD HH:MM:SS` in local time.
    pub read_at: String,
    /// Starting Time and To as typed, for Read again.
    pub start_text: Option<String>,
    pub to_text: Option<String>,
    /// The Full Read Limit the read ran under.
    pub limit_lines: usize,
    /// Oldest Generation first.
    pub generations: Vec<GenerationMeta>,
}

impl CopyMeta {
    /// `<server>:<path>`.
    pub fn name(&self) -> String {
        format!("{}:{}", self.server_name, self.path)
    }

    pub fn total_lines(&self) -> usize {
        self.generations.iter().map(|g| g.lines).sum()
    }

    pub fn is_complete(&self) -> bool {
        self.generations.iter().all(|g| g.complete)
    }

    /// The span of time read, for the tooltip: `from` of the oldest to `to`
    /// of the newest when the read was limited, otherwise nothing.
    pub fn span_text(&self) -> String {
        match (&self.start_text, &self.to_text) {
            (Some(a), Some(b)) => format!("{a} – {b}"),
            (Some(a), None) => format!("from {a}"),
            (None, Some(b)) => format!("until {b}"),
            (None, None) => "whole file".to_string(),
        }
    }
}

fn group(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// The folder of Remote Copies in the data folder `base`.
pub fn root_in(base: &Path) -> PathBuf {
    base.join("remote-copies")
}

/// A new, empty folder for a Remote Copy.
pub fn create_dir(root: &Path) -> io::Result<PathBuf> {
    let id = unique_id();
    let dir = root.join(id);
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn unique_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    format!("{nanos:x}-{:x}-{:x}", std::process::id(), COUNTER.fetch_add(1, Ordering::Relaxed))
}

pub fn write_meta(dir: &Path, meta: &CopyMeta) -> io::Result<()> {
    let json = serde_json::to_vec_pretty(meta).map_err(io::Error::other)?;
    // Written whole and renamed, so a crash leaves the old one.
    let tmp = dir.join("meta.json.tmp");
    std::fs::write(&tmp, json)?;
    std::fs::rename(tmp, dir.join(META_FILE))
}

pub fn read_meta(dir: &Path) -> io::Result<CopyMeta> {
    let bytes = std::fs::read(dir.join(META_FILE))?;
    serde_json::from_slice(&bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// What a folder holds when its `meta.json` was never written (the app closed
/// during the read): its `gen-*.log` files, oldest first, each cancelled.
pub fn recover_meta(dir: &Path) -> io::Result<CopyMeta> {
    let mut found: Vec<(u32, String)> = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let name = entry?.file_name().to_string_lossy().to_string();
        if let Some(n) = name.strip_prefix("gen-").and_then(|r| r.strip_suffix(".log")).and_then(|n| n.parse().ok()) {
            found.push((n, name));
        }
    }
    if found.is_empty() {
        return Err(io::Error::new(io::ErrorKind::NotFound, "no Remote Copy here"));
    }
    // The biggest distance is the oldest.
    found.sort_by(|a, b| b.0.cmp(&a.0));
    let generations = found
        .iter()
        .map(|(distance, file)| GenerationMeta {
            distance: *distance,
            source_name: file.clone(),
            compressed: false,
            start_offset: 0,
            end_offset: None,
            first_line_no: None,
            from: None,
            to: None,
            complete: false,
            stop: Some(StopReason::Cancelled),
            lines: 0,
            older_not_read: false,
            file: file.clone(),
        })
        .collect();
    Ok(CopyMeta {
        server_id: String::new(),
        server_name: String::new(),
        path: String::new(),
        read_at: String::new(),
        start_text: None,
        to_text: None,
        limit_lines: 0,
        generations,
    })
}

/// Remove a Remote Copy's folder. Only a folder that holds a `meta.json` or
/// only `gen-*.log` files is removed, so a wrong path never deletes more.
pub fn remove(dir: &Path) -> io::Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let name = entry?.file_name().to_string_lossy().to_string();
        let ours = name == META_FILE || name == "meta.json.tmp" || (name.starts_with("gen-") && name.ends_with(".log"));
        if !ours {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, format!("not a Remote Copy folder: {}", dir.display())));
        }
    }
    std::fs::remove_dir_all(dir)
}

/// Remove the folders of `root` that no Log Space refers to. Returns how many.
pub fn sweep(root: &Path, in_use: &HashSet<PathBuf>) -> usize {
    let Ok(entries) = std::fs::read_dir(root) else { return 0 };
    let mut removed = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && !in_use.contains(&path) && remove(&path).is_ok() {
            removed += 1;
        }
    }
    removed
}

/// Bytes the Remote Copies take, for Settings.
pub fn total_size(root: &Path) -> u64 {
    fn walk(dir: &Path) -> u64 {
        std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| match e.metadata() {
                Ok(m) if m.is_dir() => walk(&e.path()),
                Ok(m) => m.len(),
                Err(_) => 0,
            })
            .sum()
    }
    walk(root)
}

/// The text of one Generation, as it was read. A missing or cut file is an
/// error the caller reports; the lines of the others still show.
pub fn read_generation(dir: &Path, generation: &GenerationMeta) -> io::Result<Vec<u8>> {
    std::fs::read(dir.join(&generation.file))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta() -> CopyMeta {
        let make = |distance, name: &str, lines, complete| GenerationMeta {
            distance,
            source_name: name.into(),
            compressed: name.ends_with(".gz"),
            start_offset: 0,
            end_offset: Some(100),
            first_line_no: Some(1),
            from: None,
            to: None,
            complete,
            stop: (!complete).then_some(StopReason::Cancelled),
            lines,
            older_not_read: false,
            file: GenerationMeta::file_name(distance),
        };
        CopyMeta {
            server_id: "s1".into(),
            server_name: "web1".into(),
            path: "/app/app.log".into(),
            read_at: "2026-10-19 10:00:00".into(),
            start_text: None,
            to_text: None,
            limit_lines: 2_000_000,
            generations: vec![make(1, "app.log.1", 20, true), make(0, "app.log", 10, false)],
        }
    }

    #[test]
    fn meta_round_trips_and_names_the_copy() {
        let root = tempfile::tempdir().unwrap();
        let dir = create_dir(root.path()).unwrap();
        write_meta(&dir, &meta()).unwrap();
        let back = read_meta(&dir).unwrap();
        assert_eq!(back, meta());
        assert_eq!(back.name(), "web1:/app/app.log");
        assert_eq!(back.total_lines(), 30);
        assert!(!back.is_complete());
        assert_eq!(back.span_text(), "whole file");
    }

    #[test]
    fn stop_labels() {
        assert_eq!(StopReason::Cancelled.label(10), "read cancelled");
        assert_eq!(StopReason::FullReadLimit.label(2_000_000), "stopped at Full Read Limit (2,000,000 lines)");
        assert_eq!(StopReason::RotatedWhileReading.label(1), "file rotated while reading");
    }

    #[test]
    fn remove_deletes_only_a_remote_copy_folder() {
        let root = tempfile::tempdir().unwrap();
        let dir = create_dir(root.path()).unwrap();
        write_meta(&dir, &meta()).unwrap();
        std::fs::write(dir.join("gen-0.log"), "x\n").unwrap();
        remove(&dir).unwrap();
        assert!(!dir.exists());
        // A folder with something else in it is left alone.
        let other = root.path().join("mine");
        std::fs::create_dir_all(&other).unwrap();
        std::fs::write(other.join("notes.txt"), "keep").unwrap();
        assert!(remove(&other).is_err());
        assert!(other.join("notes.txt").exists());
    }

    #[test]
    fn sweep_removes_folders_nobody_uses_and_size_adds_up() {
        let root = tempfile::tempdir().unwrap();
        let used = create_dir(root.path()).unwrap();
        let orphan = create_dir(root.path()).unwrap();
        for d in [&used, &orphan] {
            write_meta(d, &meta()).unwrap();
            std::fs::write(d.join("gen-0.log"), vec![b'a'; 1000]).unwrap();
        }
        assert!(total_size(root.path()) >= 2000);
        let keep: HashSet<PathBuf> = [used.clone()].into_iter().collect();
        assert_eq!(sweep(root.path(), &keep), 1);
        assert!(used.exists() && !orphan.exists());
        assert_eq!(sweep(&root.path().join("nothing"), &keep), 0);
    }
}
