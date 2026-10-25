//! Archive Entries unpacked to disk (decision 9 of `line-offset-index`): an
//! entry is written once to `<app data>/archive-cache/<archive>/`, then read
//! like a local file, so a 1GB entry does not take 1GB of memory. The
//! entries of one Archive are unpacked in one pass over it.
//!
//! An Archive's folder is named after its path, size and modification time:
//! a changed Archive is unpacked again into a new folder. An entry is first
//! written as `.part` and renamed when whole, so an entry cut off when the
//! app closed is never read; `sweep` clears such files and the folders no
//! Log Space uses.

use crate::archive::{ArchiveError, ArchiveKind, io_err, visit_entries};
use std::collections::{HashMap, HashSet};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

/// The archive cache in the app's data folder `base`.
pub fn root_in(base: &Path) -> PathBuf {
    base.join("archive-cache")
}

/// FNV-1a: a hash that stays the same across runs and builds.
fn fnv(parts: &[&[u8]]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for part in parts {
        for &b in *part {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
        h ^= 0xff;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// The folder of the Archive at `archive` as it is now on disk.
pub fn dir_of(root: &Path, archive: &Path) -> io::Result<PathBuf> {
    let meta = std::fs::metadata(archive)?;
    let modified = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos());
    let path = archive.to_string_lossy();
    let hash = fnv(&[path.as_bytes(), &meta.len().to_le_bytes(), &modified.to_le_bytes()]);
    Ok(root.join(format!("{hash:016x}")))
}

/// The file entry `entry` is unpacked to in `dir`.
pub fn entry_path(dir: &Path, entry: &str) -> PathBuf {
    dir.join(format!("{:016x}.log", fnv(&[entry.as_bytes()])))
}

/// Copy at most `declared` bytes, and never more than `limit`, from
/// `reader` to `out`; more than either is `TooLarge`.
fn copy_bounded(reader: &mut dyn Read, out: &mut impl Write, declared: u64, limit: u64) -> Result<(), ArchiveError> {
    let cap = declared.min(limit);
    let copied = io::copy(&mut reader.take(cap.saturating_add(1)), out).map_err(io_err)?;
    if copied > cap {
        return Err(ArchiveError::TooLarge);
    }
    Ok(())
}

/// Unpack `entries` of the Archive at `archive` into its folder under
/// `root`, in one pass, and give where each one is. An entry unpacked
/// before is not unpacked again. An entry larger than `limit` is
/// `TooLarge`, and nothing of it is kept.
pub fn unpack(
    root: &Path,
    archive: &Path,
    kind: ArchiveKind,
    entries: &[&str],
    limit: u64,
) -> Result<HashMap<String, Result<PathBuf, ArchiveError>>, ArchiveError> {
    let dir = dir_of(root, archive).map_err(io_err)?;
    let mut out = HashMap::new();
    let missing: Vec<&str> = entries
        .iter()
        .copied()
        .filter(|name| {
            let path = entry_path(&dir, name);
            let done = path.is_file();
            if done {
                out.insert(name.to_string(), Ok(path));
            }
            !done
        })
        .collect();
    if missing.is_empty() {
        return Ok(out);
    }
    std::fs::create_dir_all(&dir).map_err(io_err)?;
    let unpacked = visit_entries(archive, kind, &missing, limit, |name, reader, declared| {
        let path = entry_path(&dir, name);
        let part = path.with_extension("part");
        let written = std::fs::File::create(&part)
            .map_err(io_err)
            .and_then(|file| {
                let mut file = io::BufWriter::new(file);
                copy_bounded(reader, &mut file, declared, limit)?;
                file.flush().map_err(io_err)
            })
            .and_then(|()| std::fs::rename(&part, &path).map_err(io_err));
        if written.is_err() {
            let _ = std::fs::remove_file(&part);
        }
        written.map(|()| path)
    })?;
    out.extend(unpacked);
    Ok(out)
}

/// Remove the unpacked file of `entry` of `archive`, and the Archive's
/// folder once it holds nothing.
pub fn remove_entry(root: &Path, archive: &Path, entry: &str) {
    let Ok(dir) = dir_of(root, archive) else { return };
    let _ = std::fs::remove_file(entry_path(&dir, entry));
    // Only an empty folder goes.
    let _ = std::fs::remove_dir(&dir);
}

/// Clear what no Log Space uses: folders not in `in_use`, and in the others
/// the files cut off before they were whole (`.part`). How many went.
pub fn sweep(root: &Path, in_use: &HashSet<PathBuf>) -> usize {
    let Ok(dirs) = std::fs::read_dir(root) else { return 0 };
    let mut removed = 0;
    for dir in dirs.flatten().map(|e| e.path()) {
        if !dir.is_dir() {
            continue;
        }
        if !in_use.contains(&dir) {
            if std::fs::remove_dir_all(&dir).is_ok() {
                removed += 1;
            }
            continue;
        }
        for file in std::fs::read_dir(&dir).into_iter().flatten().flatten().map(|e| e.path()) {
            if file.extension().is_some_and(|e| e == "part") && std::fs::remove_file(&file).is_ok() {
                removed += 1;
            }
        }
    }
    removed
}
