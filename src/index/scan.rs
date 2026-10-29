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
use std::sync::atomic::{AtomicBool, Ordering};
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

/// Where a Scan reads the texts of the lines it looks at.
pub trait ScanTexts: Send + Sync {
    /// Hand the texts of lines `block` to `each`, in one call.
    fn read(&self, block: Range<usize>, each: &mut dyn FnMut(&[&str])) -> Result<(), String>;

    /// Done reading for now.
    fn release(&self) {}
}

/// The lines of a file read from its source, where its Line Index says.
pub struct FileTexts {
    pub source: Arc<dyn LineSource>,
    pub spans: Arc<dyn LineSpans>,
}

impl ScanTexts for FileTexts {
    fn read(&self, block: Range<usize>, each: &mut dyn FnMut(&[&str])) -> Result<(), String> {
        let bytes = self.spans.byte_range(block.start, block.end).ok_or("the lines are no longer indexed")?;
        let want = usize::try_from(bytes.end - bytes.start).map_err(|e| e.to_string())?;
        let data = self.source.read_range(bytes.start, want).map_err(|e| e.to_string())?;
        if data.len() < want {
            return Err("the file no longer has these lines".into());
        }
        let texts: Vec<std::borrow::Cow<str>> =
            split_lines(&data).map(|bytes| String::from_utf8_lossy(line_text(bytes))).collect();
        let texts: Vec<&str> = texts.iter().map(|t| t.as_ref()).collect();
        each(&texts);
        Ok(())
    }

    fn release(&self) {
        self.source.release();
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
    /// Read `lines` from `texts` with `matcher`, the blocks around line
    /// `around` first.
    pub fn start(texts: Arc<dyn ScanTexts>, lines: Range<usize>, around: usize, matcher: Arc<Matcher>) -> Self {
        let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
        Self::start_with(texts, lines, around, matcher, workers)
    }

    /// `start` with `workers` threads reading blocks.
    pub fn start_with(
        texts: Arc<dyn ScanTexts>,
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
                    run(&*texts, lines, around, &*matcher, workers, &found, &cancel, &tx);
                    texts.release();
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
pub fn scan_all(texts: &dyn ScanTexts, lines: Range<usize>, matcher: &Matcher) -> Found {
    let found = Mutex::new(Found { total: lines.len(), ..Found::default() });
    let (tx, _rx) = async_channel::unbounded();
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
    run(texts, lines.clone(), lines.start, matcher, workers, &found, &AtomicBool::new(false), &tx);
    texts.release();
    found.into_inner().unwrap()
}

fn read_block(texts: &dyn ScanTexts, block: &Range<usize>, matcher: &Matcher) -> Result<Vec<u32>, String> {
    let mut hits = Vec::new();
    texts.read(block.clone(), &mut |lines| hits = matcher(block.start, lines))?;
    Ok(hits)
}

#[allow(clippy::too_many_arguments)]
fn run(
    texts: &dyn ScanTexts,
    lines: Range<usize>,
    around: usize,
    matcher: &Matcher,
    workers: usize,
    found: &Mutex<Found>,
    cancel: &AtomicBool,
    tx: &async_channel::Sender<Found>,
) {
    let end = scan_in_steps(texts, lines, around, matcher, workers, cancel, |f| {
        *found.lock().unwrap() = f.clone();
        let _ = tx.try_send(f);
    });
    *found.lock().unwrap() = end.clone();
    let _ = tx.try_send(end);
}

/// Read the blocks of `lines` from `texts` with `matcher`, those around
/// line `around` first, `workers` blocks at a time in parallel. After each
/// round, at most every 50 ms, `on_step` hears what was found so far; it
/// runs on the calling thread. Stops after the round in which `cancel` is
/// set. What was found in the end.
#[allow(clippy::too_many_arguments)]
pub fn scan_in_steps(
    texts: &dyn ScanTexts,
    lines: Range<usize>,
    around: usize,
    matcher: &Matcher,
    workers: usize,
    cancel: &AtomicBool,
    on_step: impl FnMut(Found),
) -> Found {
    scan_blocks(lines, around, workers, cancel, |block| read_block(texts, block, matcher), on_step)
}

/// `scan_in_steps` with `look` finding what matches in a block of `lines`
/// (positions in `lines`, ascending), however it reads them.
pub fn scan_blocks(
    lines: Range<usize>,
    around: usize,
    workers: usize,
    cancel: &AtomicBool,
    look: impl Fn(&Range<usize>) -> Result<Vec<u32>, String> + Sync,
    mut on_step: impl FnMut(Found),
) -> Found {
    let order = block_order(lines.clone(), around);
    let mut parts: BTreeMap<usize, Vec<u32>> = BTreeMap::new();
    let mut found = Found { total: lines.len(), ..Found::default() };
    let mut published = None::<Instant>;
    let workers = workers.max(1);
    let look = &look;
    for round in order.chunks(workers) {
        if cancel.load(Ordering::Acquire) {
            break;
        }
        let results: Vec<Result<Vec<u32>, String>> = if round.len() == 1 {
            vec![look(&round[0])]
        } else {
            std::thread::scope(|s| {
                let handles: Vec<_> = round.iter().map(|block| s.spawn(move || look(block))).collect();
                handles.into_iter().map(|h| h.join().expect("a scan block panicked")).collect()
            })
        };
        for (block, result) in round.iter().zip(results) {
            match result {
                Ok(hits) => {
                    parts.insert(block.start, hits);
                    found.scanned += block.len();
                }
                Err(error) => {
                    found.error.get_or_insert(error);
                }
            }
        }
        if found.error.is_some() {
            break;
        }
        if found.scanned < found.total && published.is_none_or(|at: Instant| at.elapsed() >= PUBLISH_EVERY) {
            published = Some(Instant::now());
            found.lines = parts.values().flatten().copied().collect();
            on_step(found.clone());
        }
    }
    found.lines = parts.into_values().flatten().collect();
    found.done = found.scanned == found.total && found.error.is_none();
    found
}
