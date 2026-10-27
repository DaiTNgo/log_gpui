//! How many files the app holds open. A process may hold only so many
//! (`RLIMIT_NOFILE`: 256 by default on macOS), and SSH sessions, fonts and
//! the window need some too, so file sources stop short of the limit and say
//! so instead of failing somewhere else.

use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Handles kept free for everything that is not a file source.
pub const RESERVED: usize = 64;

/// The handles file sources may hold open, and how many they hold.
#[derive(Debug)]
pub struct HandleBudget {
    limit: usize,
    open: AtomicUsize,
}

/// One open handle; given back when dropped.
#[derive(Debug)]
pub struct HandleLease {
    budget: Arc<HandleBudget>,
}

impl Drop for HandleLease {
    fn drop(&mut self) {
        self.budget.open.fetch_sub(1, Ordering::AcqRel);
    }
}

/// The error a source gets when the budget has no handle left.
pub fn too_many_open(limit: usize) -> io::Error {
    io::Error::other(format!(
        "too many files open: the app keeps at most {limit} open; close some tabs or files"
    ))
}

impl HandleBudget {
    /// A budget of `limit` handles for file sources.
    pub fn new(limit: usize) -> Arc<Self> {
        Arc::new(Self { limit: limit.max(1), open: AtomicUsize::new(0) })
    }

    /// The budget every file source of the app shares.
    pub fn shared() -> Arc<Self> {
        static SHARED: std::sync::OnceLock<Arc<HandleBudget>> = std::sync::OnceLock::new();
        SHARED.get_or_init(Self::for_process).clone()
    }

    /// A budget of what the process may open, less `RESERVED`.
    pub fn for_process() -> Arc<Self> {
        Self::new(process_limit().saturating_sub(RESERVED))
    }

    pub fn limit(&self) -> usize {
        self.limit
    }

    /// Handles open now.
    pub fn open(&self) -> usize {
        self.open.load(Ordering::Acquire)
    }

    /// Whether at least nine in ten handles are open: time to tell the user
    /// before an open fails.
    pub fn near_limit(&self) -> bool {
        self.open() * 10 >= self.limit * 9
    }

    /// A handle, or the "too many files open" error when none is left.
    pub fn acquire(self: &Arc<Self>) -> io::Result<HandleLease> {
        let mut open = self.open.load(Ordering::Acquire);
        loop {
            if open >= self.limit {
                return Err(too_many_open(self.limit));
            }
            match self.open.compare_exchange_weak(open, open + 1, Ordering::AcqRel, Ordering::Acquire) {
                Ok(_) => return Ok(HandleLease { budget: self.clone() }),
                Err(now) => open = now,
            }
        }
    }
}

/// The soft limit of open files of this process.
#[cfg(unix)]
fn process_limit() -> usize {
    let mut limit = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
    // SAFETY: `limit` is a valid, writable rlimit.
    if unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit) } != 0 {
        return 256;
    }
    usize::try_from(limit.rlim_cur).unwrap_or(usize::MAX).min(1 << 20)
}

/// Windows has no small per-process limit on file handles.
#[cfg(not(unix))]
fn process_limit() -> usize {
    8192
}
