//! Archives (`.zip`, `.tar`, `.tar.gz`, `.gz`): detected by their first bytes,
//! listed entry by entry, and read into memory or unpacked to the app's
//! archive cache (`archive_cache`). The Archive itself is never changed.

use crate::remote::compress::{Compression, decoder};
use flate2::read::GzDecoder;
use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::io::{self, BufReader, Read, Seek};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::path::Path;

/// An entry above this size gets a "Large file" warning but can still be added.
pub const LARGE_ENTRY_BYTES: u64 = 200 * 1024 * 1024;
/// The most one import may unpack, and the most one entry may ever grow to.
pub const MAX_UNCOMPRESSED_BYTES: u64 = 2 * 1024 * 1024 * 1024;
/// How many leading bytes of an entry are kept to tell text from binary.
pub const HEAD_BYTES: usize = 8 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveKind {
    Zip,
    Tar,
    TarGz,
    Gz,
    /// One file packed by zstd, xz or bzip2 (a Rotated File downloaded as it
    /// was on the Server).
    Zstd,
    Xz,
    Bzip2,
}

impl ArchiveKind {
    /// A lone packed file (`.gz`, `.zst`, `.xz`, `.bz2`): one entry, named
    /// after the file without the suffix.
    pub fn is_single_file(self) -> bool {
        self.single().is_some()
    }

    /// The compression of a lone packed file, `None` for the others.
    fn single(self) -> Option<Compression> {
        match self {
            ArchiveKind::Gz => Some(Compression::Gzip),
            ArchiveKind::Zstd => Some(Compression::Zstd),
            ArchiveKind::Xz => Some(Compression::Xz),
            ArchiveKind::Bzip2 => Some(Compression::Bzip2),
            _ => None,
        }
    }
}

/// A file inside an Archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryInfo {
    pub name: String,
    /// Size after unpacking, as the Archive declares it.
    pub size: u64,
    /// Up to `HEAD_BYTES` of the content.
    pub head: Vec<u8>,
}

#[derive(Debug)]
pub enum ArchiveError {
    Damaged(String),
    Encrypted,
    NotFound,
    TooLarge,
    /// Stopped before the end: nobody wants it any more.
    Cancelled,
    Io(io::Error),
}

impl fmt::Display for ArchiveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArchiveError::Damaged(why) => write!(f, "damaged archive ({why})"),
            ArchiveError::Encrypted => write!(f, "password-protected archive"),
            ArchiveError::NotFound => write!(f, "not found"),
            ArchiveError::TooLarge => write!(f, "larger than the archive declares or the 2 GB limit"),
            ArchiveError::Cancelled => write!(f, "cancelled"),
            ArchiveError::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for ArchiveError {}

impl From<ArchiveError> for io::Error {
    fn from(e: ArchiveError) -> Self {
        let kind = match &e {
            ArchiveError::Damaged(_) => io::ErrorKind::InvalidData,
            ArchiveError::Encrypted => io::ErrorKind::PermissionDenied,
            ArchiveError::NotFound => io::ErrorKind::NotFound,
            ArchiveError::TooLarge => io::ErrorKind::FileTooLarge,
            ArchiveError::Cancelled => io::ErrorKind::Interrupted,
            ArchiveError::Io(e) => return io::Error::new(e.kind(), e.to_string()),
        };
        io::Error::new(kind, e.to_string())
    }
}

pub(crate) fn io_err(e: io::Error) -> ArchiveError {
    match e.kind() {
        io::ErrorKind::NotFound => ArchiveError::NotFound,
        // A cut or garbled stream surfaces as one of these.
        io::ErrorKind::UnexpectedEof | io::ErrorKind::InvalidData | io::ErrorKind::InvalidInput => {
            ArchiveError::Damaged(e.to_string())
        }
        _ => ArchiveError::Io(e),
    }
}

fn zip_err(e: zip::result::ZipError) -> ArchiveError {
    use zip::result::ZipError;
    match e {
        ZipError::Io(e) => io_err(e),
        ZipError::UnsupportedArchive(msg) if msg == ZipError::PASSWORD_REQUIRED => ArchiveError::Encrypted,
        ZipError::InvalidPassword => ArchiveError::Encrypted,
        ZipError::FileNotFound => ArchiveError::NotFound,
        other => ArchiveError::Damaged(other.to_string()),
    }
}

/// Where the bytes of a log file come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileSource<'a> {
    Disk(&'a Path),
    ArchiveEntry { archive: &'a Path, entry: &'a str },
}

/// Read a whole file or Archive Entry into memory.
pub fn read_source(source: FileSource) -> io::Result<Vec<u8>> {
    match source {
        FileSource::Disk(path) => std::fs::read(path),
        FileSource::ArchiveEntry { archive, entry } => {
            let kind = detect(archive).ok_or_else(|| {
                io::Error::from(if archive.exists() {
                    ArchiveError::Damaged("not an archive".into())
                } else {
                    ArchiveError::NotFound
                })
            })?;
            let mut read = read_entries(archive, kind, &[entry], MAX_UNCOMPRESSED_BYTES)?;
            read.remove(entry).unwrap_or(Err(ArchiveError::NotFound)).map_err(Into::into)
        }
    }
}

const ZIP_MAGIC: [&[u8]; 2] = [b"PK\x03\x04", b"PK\x05\x06"];

fn has_ustar(block: &[u8]) -> bool {
    block.len() >= 262 && &block[257..262] == b"ustar"
}

fn read_up_to(reader: &mut impl Read, n: usize) -> io::Result<Vec<u8>> {
    let mut buf = Vec::with_capacity(n);
    reader.take(n as u64).read_to_end(&mut buf)?;
    Ok(buf)
}

/// Whether the first bytes of a file say it is an Archive or a compressed
/// file (zip, gzip, bzip2, xz, zstd or tar), not a log to read.
pub fn looks_like_archive(head: &[u8]) -> bool {
    ZIP_MAGIC.iter().any(|m| head.starts_with(m))
        || head.starts_with(&[0x1f, 0x8b])
        || head.starts_with(b"BZh")
        || head.starts_with(&[0xfd, b'7', b'z', b'X', b'Z', 0x00])
        || head.starts_with(&[0x28, 0xb5, 0x2f, 0xfd])
        || has_ustar(head)
}

/// What kind of Archive `path` is, by its first bytes; `None` for anything
/// else (including an unreadable or empty file).
pub fn detect(path: &Path) -> Option<ArchiveKind> {
    let mut file = File::open(path).ok()?;
    let head = read_up_to(&mut file, 512).ok()?;
    if ZIP_MAGIC.iter().any(|m| head.starts_with(m)) {
        return Some(ArchiveKind::Zip);
    }
    if head.starts_with(&[0x1f, 0x8b]) {
        // A gzip holding a tar is a `.tar.gz`; anything else is one file.
        let mut file = File::open(path).ok()?;
        let inner = read_up_to(&mut GzDecoder::new(&mut file), 512).unwrap_or_default();
        return Some(if has_ustar(&inner) { ArchiveKind::TarGz } else { ArchiveKind::Gz });
    }
    match Compression::detect(&head) {
        Compression::Zstd => return Some(ArchiveKind::Zstd),
        Compression::Xz => return Some(ArchiveKind::Xz),
        Compression::Bzip2 => return Some(ArchiveKind::Bzip2),
        _ => {}
    }
    has_ustar(&head).then_some(ArchiveKind::Tar)
}

/// The suffixes of a lone packed file.
pub const SINGLE_SUFFIXES: [&str; 4] = [".gz", ".zst", ".xz", ".bz2"];

/// The name a lone packed file (`.gz`, `.zst`, `.xz`, `.bz2`) shows: the
/// Archive's file name without the suffix.
pub fn gz_entry_name(path: &Path) -> String {
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    SINGLE_SUFFIXES
        .iter()
        .find_map(|s| name.strip_suffix(s))
        .filter(|n| !n.is_empty())
        .unwrap_or(&name)
        .to_string()
}

/// Entries that are never offered: macOS resource forks and Finder files.
fn skipped_name(name: &str) -> bool {
    name.starts_with("__MACOSX/")
        || name.rsplit('/').next().is_some_and(|base| base == ".DS_Store")
}

fn open_reader(path: &Path) -> Result<BufReader<Counted<File>>, ArchiveError> {
    open_counted(path, None)
}

/// The file at `path`, counting into `read` the bytes read from it.
fn open_counted(path: &Path, read: Option<Arc<AtomicU64>>) -> Result<BufReader<Counted<File>>, ArchiveError> {
    File::open(path).map(|inner| BufReader::new(Counted { inner, read })).map_err(io_err)
}

/// A reader that adds the bytes read from it to a count: how far through
/// an Archive an unpacking is.
pub struct Counted<R> {
    inner: R,
    read: Option<Arc<AtomicU64>>,
}

impl<R: Read> Read for Counted<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        if let Some(read) = &self.read {
            read.fetch_add(n as u64, Ordering::Relaxed);
        }
        Ok(n)
    }
}

impl<R: Seek> Seek for Counted<R> {
    fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
        self.inner.seek(pos)
    }
}

/// Read at most `declared` bytes, and never more than `limit`; more than
/// either is `TooLarge`.
fn read_bounded(reader: impl Read, declared: u64, limit: u64) -> Result<Vec<u8>, ArchiveError> {
    let cap = declared.min(limit).saturating_add(1);
    let mut buf = Vec::new();
    reader.take(cap).read_to_end(&mut buf).map_err(io_err)?;
    if buf.len() as u64 > declared.min(limit) {
        return Err(ArchiveError::TooLarge);
    }
    Ok(buf)
}

fn head_of(reader: impl Read) -> Result<Vec<u8>, ArchiveError> {
    read_up_to(&mut { reader }, HEAD_BYTES).map_err(io_err)
}

/// The regular files of an Archive: no folders, links, `__MACOSX/` or
/// `.DS_Store`, each with its declared size and its first bytes.
pub fn list(path: &Path, kind: ArchiveKind) -> Result<Vec<EntryInfo>, ArchiveError> {
    match kind {
        ArchiveKind::Zip => list_zip(path),
        ArchiveKind::Tar => list_tar(open_reader(path)?),
        ArchiveKind::TarGz => list_tar(GzDecoder::new(open_reader(path)?)),
        ArchiveKind::Gz | ArchiveKind::Zstd | ArchiveKind::Xz | ArchiveKind::Bzip2 => {
            let mut decoder = single_decoder(path, kind, None)?;
            let head = head_of(&mut decoder)?;
            // Drain the rest to find the real size and any damage.
            let rest = io::copy(&mut (&mut decoder).take(MAX_UNCOMPRESSED_BYTES), &mut io::sink())
                .map_err(io_err)?;
            Ok(vec![EntryInfo { name: gz_entry_name(path), size: head.len() as u64 + rest, head }])
        }
    }
}

fn single_decoder(path: &Path, kind: ArchiveKind, read: Option<Arc<AtomicU64>>) -> Result<Box<dyn Read + Send>, ArchiveError> {
    let compression = kind.single().expect("a lone packed file");
    decoder(compression, open_counted(path, read)?).map_err(io_err)
}

fn list_zip(path: &Path) -> Result<Vec<EntryInfo>, ArchiveError> {
    let mut archive = zip::ZipArchive::new(open_reader(path)?).map_err(zip_err)?;
    let mut out = Vec::new();
    for i in 0..archive.len() {
        {
            let raw = archive.by_index_raw(i).map_err(zip_err)?;
            if raw.encrypted() {
                return Err(ArchiveError::Encrypted);
            }
            let is_link = raw.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000);
            let name = raw.name().map_err(zip_err)?;
            if !raw.is_file() || is_link || skipped_name(&name) {
                continue;
            }
        }
        let file = archive.by_index(i).map_err(zip_err)?;
        let (name, size) = (file.name().map_err(zip_err)?.to_string(), file.size());
        out.push(EntryInfo { name, size, head: head_of(file)? });
    }
    Ok(out)
}

fn list_tar(reader: impl Read) -> Result<Vec<EntryInfo>, ArchiveError> {
    let mut archive = tar::Archive::new(reader);
    let mut out = Vec::new();
    for entry in archive.entries().map_err(io_err)? {
        let mut entry = entry.map_err(io_err)?;
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let name = entry.path().map_err(io_err)?.to_string_lossy().to_string();
        if skipped_name(&name) {
            continue;
        }
        let size = entry.size();
        out.push(EntryInfo { name, size, head: head_of(&mut entry)? });
    }
    Ok(out)
}

/// Read several entries in one pass over the Archive. An entry missing from
/// the Archive is `NotFound` in the map; one that outgrows its declared size
/// or `limit` is `TooLarge`.
pub fn read_entries(
    path: &Path,
    kind: ArchiveKind,
    entries: &[&str],
    limit: u64,
) -> Result<HashMap<String, Result<Vec<u8>, ArchiveError>>, ArchiveError> {
    visit_entries(path, kind, entries, limit, |_, reader, declared| read_bounded(reader, declared, limit))
}

/// Hand each of `entries` to `each` in one pass over the Archive, with its
/// declared size (`limit` for a lone packed file, which declares none). An
/// entry missing from the Archive is `NotFound` in the map.
pub fn visit_entries<T>(
    path: &Path,
    kind: ArchiveKind,
    entries: &[&str],
    limit: u64,
    each: impl FnMut(&str, &mut dyn Read, u64) -> Result<T, ArchiveError>,
) -> Result<HashMap<String, Result<T, ArchiveError>>, ArchiveError> {
    visit_entries_counting(path, kind, entries, limit, None, each)
}

/// `visit_entries`, adding to `read` the bytes read from the Archive as it
/// goes: how far through it the visit is.
pub fn visit_entries_counting<T>(
    path: &Path,
    kind: ArchiveKind,
    entries: &[&str],
    limit: u64,
    read: Option<Arc<AtomicU64>>,
    mut each: impl FnMut(&str, &mut dyn Read, u64) -> Result<T, ArchiveError>,
) -> Result<HashMap<String, Result<T, ArchiveError>>, ArchiveError> {
    let mut out: HashMap<String, Result<T, ArchiveError>> = HashMap::new();
    match kind {
        ArchiveKind::Zip => {
            let mut archive = zip::ZipArchive::new(open_counted(path, read.clone())?).map_err(zip_err)?;
            for name in entries {
                let result = match archive.by_name(name) {
                    Ok(mut file) => {
                        let declared = file.size();
                        each(name, &mut file, declared)
                    }
                    Err(e) => Err(zip_err(e)),
                };
                out.insert((*name).to_string(), result);
            }
        }
        ArchiveKind::Tar => visit_tar_entries(open_counted(path, read.clone())?, entries, &mut each, &mut out)?,
        ArchiveKind::TarGz => visit_tar_entries(GzDecoder::new(open_counted(path, read.clone())?), entries, &mut each, &mut out)?,
        ArchiveKind::Gz | ArchiveKind::Zstd | ArchiveKind::Xz | ArchiveKind::Bzip2 => {
            // The entry name is not needed: a lone packed file holds exactly one.
            let mut decoder = single_decoder(path, kind, read.clone())?;
            let mut first = true;
            for name in entries {
                let result = if first { each(name, &mut decoder, limit) } else { Err(ArchiveError::TooLarge) };
                first = false;
                out.insert((*name).to_string(), result);
            }
        }
    }
    for name in entries {
        out.entry((*name).to_string()).or_insert(Err(ArchiveError::NotFound));
    }
    Ok(out)
}

fn visit_tar_entries<T>(
    reader: impl Read,
    wanted: &[&str],
    each: &mut impl FnMut(&str, &mut dyn Read, u64) -> Result<T, ArchiveError>,
    out: &mut HashMap<String, Result<T, ArchiveError>>,
) -> Result<(), ArchiveError> {
    let mut archive = tar::Archive::new(reader);
    let mut remaining = wanted.len();
    for entry in archive.entries().map_err(io_err)? {
        let mut entry = entry.map_err(io_err)?;
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let name = entry.path().map_err(io_err)?.to_string_lossy().to_string();
        if wanted.contains(&name.as_str()) && !out.contains_key(&name) {
            let declared = entry.size();
            let result = each(&name, &mut entry, declared);
            if matches!(result, Err(ArchiveError::Cancelled)) {
                return Err(ArchiveError::Cancelled);
            }
            out.insert(name, result);
            remaining -= 1;
            if remaining == 0 {
                break;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_bounded_stops_past_declared_size() {
        let data = vec![b'x'; 2048];
        assert!(matches!(read_bounded(&data[..], 1024, u64::MAX), Err(ArchiveError::TooLarge)));
        assert_eq!(read_bounded(&data[..], 2048, u64::MAX).unwrap().len(), 2048);
    }

    #[test]
    fn read_bounded_stops_past_limit() {
        let data = vec![b'x'; 2048];
        assert!(matches!(read_bounded(&data[..], 4096, 1024), Err(ArchiveError::TooLarge)));
    }
}
