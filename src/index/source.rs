//! Where the bytes of a file are read from (decision 2): a file on disk or
//! bytes in memory, behind one `LineSource`. The Line Index, Scans and the
//! rows drawn on screen only see this trait, so a source that later moves
//! from memory to a cache file on disk only changes its implementation.

use super::handles::{HandleBudget, HandleLease};
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

/// The bytes of one file, read at any position.
pub trait LineSource: Send + Sync {
    /// Bytes the source has now; a live source grows.
    fn len(&self) -> io::Result<u64>;

    /// Read into `buf` from `offset`; fewer bytes than `buf` holds only at
    /// the end of the source.
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> io::Result<usize>;

    /// Done reading for now: a source that does not stay open lets go of
    /// what it holds open. It opens again on the next read.
    fn release(&self) {}

    /// The identity of the file read (its inode on Unix), when the source
    /// is a file and it can be told.
    fn identity(&self) -> Option<crate::live::local::FileId> {
        None
    }

    fn is_empty(&self) -> io::Result<bool> {
        Ok(self.len()? == 0)
    }

    /// The `len` bytes at `offset`, fewer at the end of the source.
    fn read_range(&self, offset: u64, len: usize) -> io::Result<Vec<u8>> {
        let mut buf = vec![0; len];
        let n = self.read_at(offset, &mut buf)?;
        buf.truncate(n);
        Ok(buf)
    }
}

/// Read until `buf` is full or the file ends.
fn read_full_at(file: &File, mut offset: u64, buf: &mut [u8]) -> io::Result<usize> {
    let mut got = 0;
    while got < buf.len() {
        #[cfg(unix)]
        let n = std::os::unix::fs::FileExt::read_at(file, &mut buf[got..], offset);
        #[cfg(windows)]
        let n = std::os::windows::fs::FileExt::seek_read(file, &mut buf[got..], offset);
        match n {
            Ok(0) => break,
            Ok(n) => {
                got += n;
                offset += n as u64;
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(got)
}

/// A file on disk, read with `read_at` (`seek_read` on Windows). std opens
/// files sharing read, write and delete on Windows, so a writer can keep
/// appending and a rotation can rename or delete the file while it is open.
///
/// A file that does not stay open (one that is not live) opens on the first
/// read and closes on `release`, after a pass over it. Every open handle
/// holds a lease of the `HandleBudget` its source was given.
pub struct DiskSource {
    path: PathBuf,
    budget: Arc<HandleBudget>,
    stay_open: bool,
    open: Mutex<Option<(File, HandleLease)>>,
}

impl DiskSource {
    /// A file opened when read and closed after each pass.
    pub fn on_demand(path: impl Into<PathBuf>, budget: Arc<HandleBudget>) -> Self {
        Self { path: path.into(), budget, stay_open: false, open: Mutex::new(None) }
    }

    /// A file kept open until dropped: a live file, or an older Generation
    /// that still has lines in a view. It keeps reading the file it opened
    /// even after the path names another file.
    pub fn stay_open(path: impl Into<PathBuf>, budget: Arc<HandleBudget>) -> io::Result<Self> {
        let source = Self { path: path.into(), budget, stay_open: true, open: Mutex::new(None) };
        source.with_file(|_| Ok(()))?;
        Ok(source)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Whether a handle is open now.
    pub fn is_open(&self) -> bool {
        self.open.lock().unwrap().is_some()
    }

    fn with_file<T>(&self, f: impl FnOnce(&File) -> io::Result<T>) -> io::Result<T> {
        let mut open = self.open.lock().unwrap();
        if open.is_none() {
            let lease = self.budget.acquire()?;
            *open = Some((File::open(&self.path)?, lease));
        }
        f(&open.as_ref().expect("opened above").0)
    }
}

impl LineSource for DiskSource {
    fn len(&self) -> io::Result<u64> {
        self.with_file(|file| Ok(file.metadata()?.len()))
    }

    fn read_at(&self, offset: u64, buf: &mut [u8]) -> io::Result<usize> {
        self.with_file(|file| read_full_at(file, offset, buf))
    }

    fn identity(&self) -> Option<crate::live::local::FileId> {
        self.with_file(|file| Ok(crate::live::local::id_of_open(file, &file.metadata()?))).ok().flatten()
    }

    fn release(&self) {
        if !self.stay_open {
            *self.open.lock().unwrap() = None;
        }
    }
}

/// Bytes in memory: a Scratch's text, or the Live Window of a Remote File
/// until Remote is written to a local cache (a later change).
#[derive(Default)]
pub struct BytesSource {
    bytes: RwLock<Vec<u8>>,
}

impl BytesSource {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { bytes: RwLock::new(bytes) }
    }

    /// Add `more` at the end.
    pub fn append(&self, more: &[u8]) {
        self.bytes.write().unwrap().extend_from_slice(more);
    }

    /// Hold `bytes` instead of what it held.
    pub fn replace(&self, bytes: Vec<u8>) {
        *self.bytes.write().unwrap() = bytes;
    }
}

impl LineSource for BytesSource {
    fn len(&self) -> io::Result<u64> {
        Ok(self.bytes.read().unwrap().len() as u64)
    }

    fn read_at(&self, offset: u64, buf: &mut [u8]) -> io::Result<usize> {
        let bytes = self.bytes.read().unwrap();
        let Ok(start) = usize::try_from(offset) else { return Ok(0) };
        let Some(have) = bytes.get(start..) else { return Ok(0) };
        let n = have.len().min(buf.len());
        buf[..n].copy_from_slice(&have[..n]);
        Ok(n)
    }
}
