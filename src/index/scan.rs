//! Scans (decision 8): reading lines again from their source in the
//! background, in parallel blocks of 10,000 lines, for whatever needs their
//! text (Search, a Custom Value, a new Detected Field). The blocks around the
//! line shown are read first, results come as they are found with a running
//! total, and a new Scan cancels the one it replaces (dropping a `Scan`
//! cancels it).

use super::line_index::{line_text, split_lines};
use super::{Indexed, LineIndex, LineSource};
use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Lines a Scan reads at a time.
pub const SCAN_LINES: usize = 10_000;
/// How often a Scan tells its listener at most.
const PUBLISH_EVERY: Duration = Duration::from_millis(50);

/// Where the lines of a file are, for a Scan reading them.
pub trait LineSpans: Send + Sync {
    /// Lines indexed.
    fn lines(&self) -> usize;
    /// Where lines `from..to` are in the source, line breaks included.
    fn byte_range(&self, from: usize, to: usize) -> Option<Range<u64>>;
}

impl LineSpans for LineIndex {
    fn lines(&self) -> usize {
        self.len()
    }

    fn byte_range(&self, from: usize, to: usize) -> Option<Range<u64>> {
        LineIndex::byte_range(self, from, to)
    }
}

impl LineSpans for Indexed {
    fn lines(&self) -> usize {
        self.data.read().unwrap().file.len()
    }

    fn byte_range(&self, from: usize, to: usize) -> Option<Range<u64>> {
        self.data.read().unwrap().file.lines.byte_range(from, to)
    }
}

/// What a Scan looks for: given the first line number of a block and the
/// texts of its lines, the lines that match.
pub type Matcher = dyn Fn(usize, &[&str]) -> Vec<u32> + Send + Sync;

/// What a Scan found so far.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Found {
    /// Matching lines, ascending.
    pub lines: Vec<u32>,
    /// Lines read so far, and to read.
    pub scanned: usize,
    pub total: usize,
    /// Every line was read.
    pub done: bool,
    /// Why the Scan stopped before the end, if it failed.
    pub error: Option<String>,
}

impl Found {
    /// The total so far: final once `done`.
    pub fn count(&self) -> usize {
        self.lines.len()
    }

    /// Take in the Scan of the lines after the ones this Scan read: the new
    /// lines of a live file, read alone.
    pub fn append(&mut self, more: Found) {
        debug_assert!(self.lines.last().is_none_or(|l| more.lines.first().is_none_or(|m| m > l)));
        self.lines.extend(more.lines);
        self.scanned += more.scanned;
        self.total += more.total;
        self.done &= more.done;
        if self.error.is_none() {
            self.error = more.error;
        }
    }
}

/// The blocks of `lines`, those nearest the block of `around` first.
pub fn block_order(lines: Range<usize>, around: usize) -> Vec<Range<usize>> {
    let blocks: Vec<Range<usize>> = lines
        .clone()
        .step_by(SCAN_LINES)
        .map(|start| start..(start + SCAN_LINES).min(lines.end))
        .collect();
    let Some(mid) = blocks.iter().position(|b| b.contains(&around)) else { return blocks };
    let mut out = Vec::with_capacity(blocks.len());
    out.push(blocks[mid].clone());
    for step in 1..blocks.len() {
        if let Some(after) = blocks.get(mid + step) {
            out.push(after.clone());
        }
        if let Some(before) = mid.checked_sub(step).and_then(|i| blocks.get(i)) {
            out.push(before.clone());
        }
    }
    out
}

/// A running Scan.
pub struct Scan {
    found: Arc<Mutex<Found>>,
    cancel: Arc<AtomicBool>,
    pub updates: async_channel::Receiver<Found>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Scan {
    /// Read `lines` of `source` (where `spans` says they are) with `matcher`,
    /// the blocks around line `around` first.
    pub fn start(
        source: Arc<dyn LineSource>,
        spans: Arc<dyn LineSpans>,
        lines: Range<usize>,
        around: usize,
        matcher: Arc<Matcher>,
    ) -> Self {
        let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
        Self::start_with(source, spans, lines, around, matcher, workers)
    }

    /// `start` with `workers` threads reading blocks.
    pub fn start_with(
        source: Arc<dyn LineSource>,
        spans: Arc<dyn LineSpans>,
        lines: Range<usize>,
        around: usize,
        matcher: Arc<Matcher>,
        workers: usize,
    ) -> Self {
        let found = Arc::new(Mutex::new(Found { total: lines.len(), ..Found::default() }));
        let cancel = Arc::new(AtomicBool::new(false));
        let (tx, updates) = async_channel::unbounded();
        let thread = {
            let (found, cancel) = (found.clone(), cancel.clone());
            std::thread::Builder::new()
                .name("scan".into())
                .spawn(move || {
                    run(&*source, &*spans, lines, around, &*matcher, workers, &found, &cancel, &tx);
                    source.release();
                })
                .expect("spawn the scan thread")
        };
        Self { found, cancel, updates, thread: Some(thread) }
    }

    /// What was found so far.
    pub fn found(&self) -> Found {
        self.found.lock().unwrap().clone()
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Release);
    }

    /// Wait for the end; what was found.
    pub fn wait(mut self) -> Found {
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        self.found()
    }
}

impl Drop for Scan {
    fn drop(&mut self) {
        self.cancel();
    }
}

/// The same Scan on the calling thread, to the end.
pub fn scan_all(source: &dyn LineSource, spans: &dyn LineSpans, lines: Range<usize>, matcher: &Matcher) -> Found {
    let found = Mutex::new(Found { total: lines.len(), ..Found::default() });
    let (tx, _rx) = async_channel::unbounded();
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
    run(source, spans, lines.clone(), lines.start, matcher, workers, &found, &AtomicBool::new(false), &tx);
    source.release();
    found.into_inner().unwrap()
}

fn read_block(source: &dyn LineSource, spans: &dyn LineSpans, block: &Range<usize>, matcher: &Matcher) -> Result<Vec<u32>, String> {
    let bytes = spans.byte_range(block.start, block.end).ok_or("the lines are no longer indexed")?;
    let want = usize::try_from(bytes.end - bytes.start).map_err(|e| e.to_string())?;
    let data = source.read_range(bytes.start, want).map_err(|e| e.to_string())?;
    if data.len() < want {
        return Err("the file no longer has these lines".into());
    }
    let texts: Vec<std::borrow::Cow<str>> =
        split_lines(&data).map(|bytes| String::from_utf8_lossy(line_text(bytes))).collect();
    let texts: Vec<&str> = texts.iter().map(|t| t.as_ref()).collect();
    Ok(matcher(block.start, &texts))
}

#[allow(clippy::too_many_arguments)]
fn run(
    source: &dyn LineSource,
    spans: &dyn LineSpans,
    lines: Range<usize>,
    around: usize,
    matcher: &Matcher,
    workers: usize,
    found: &Mutex<Found>,
    cancel: &AtomicBool,
    tx: &async_channel::Sender<Found>,
) {
    let order = block_order(lines, around);
    let next = AtomicUsize::new(0);
    let parts: Mutex<BTreeMap<usize, Vec<u32>>> = Mutex::new(BTreeMap::new());
    let published = Mutex::new(None::<Instant>);
    let workers = workers.clamp(1, order.len().max(1));
    std::thread::scope(|s| {
        for _ in 0..workers {
            s.spawn(|| {
                loop {
                    if cancel.load(Ordering::Acquire) {
                        return;
                    }
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(block) = order.get(i) else { return };
                    let result = read_block(source, spans, block, matcher);
                    let mut parts = parts.lock().unwrap();
                    let mut f = found.lock().unwrap();
                    match result {
                        Ok(hits) => {
                            parts.insert(block.start, hits);
                            f.scanned += block.len();
                        }
                        Err(error) => {
                            f.error.get_or_insert(error);
                            cancel.store(true, Ordering::Release);
                            return;
                        }
                    }
                    let mut published = published.lock().unwrap();
                    let last = f.scanned == f.total;
                    if last || published.is_none_or(|at| at.elapsed() >= PUBLISH_EVERY) {
                        *published = Some(Instant::now());
                        f.lines = parts.values().flatten().copied().collect();
                        f.done = last;
                        let _ = tx.try_send(f.clone());
                    }
                }
            });
        }
    });
    let mut f = found.lock().unwrap();
    f.lines = parts.into_inner().unwrap().into_values().flatten().collect();
    f.done = f.scanned == f.total && f.error.is_none();
    let _ = tx.try_send(f.clone());
}
