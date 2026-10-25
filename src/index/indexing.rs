//! Indexing (decision 7): a file's Line Index, `LineMeta`, Filter columns
//! and column widths built in the background, published as they grow.
//!
//! 1. A reader thread reads the source in blocks (a small first one so the
//!    first lines show early, then 4 MiB), each ending at a line break, at
//!    most four ahead of the workers.
//! 2. Workers read the lines of a block in parallel: their metadata and
//!    their column values (`ChunkColumns`).
//! 3. The blocks are added in file order, which settles the lines at the
//!    top of a block that continue a line of the block before.
//!
//! Each added block is published to `Indexed` at once; `Progress` tells a
//! listener, at most every 50 ms (and for the first and last block). A
//! cancelled Indexing keeps the lines added so far.

use super::fast;
use super::line_index::{line_text, split_lines};
use super::meta::{RawMeta, micros};
use super::{FileIndex, LineSource};
use crate::column::FieldMaxChars;
use crate::filter_index::{ChunkColumns, FilterIndex, LineFacts, Overflowed};
use crate::log_format::CompiledFormat;
use crate::model::LogEntry;
use crate::parser::text::{LineConfig, parse_line_with};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock, mpsc};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// The first block, small so the first screen comes early.
pub const FIRST_READ: usize = 256 << 10;
/// Every later block.
pub const READ_BLOCK: usize = super::READ_BLOCK;
/// Blocks read ahead of the workers.
const READ_AHEAD: usize = 4;
/// Lines that must read alike both ways before a file locks on the fast
/// reader.
pub const LOCK_LINES: usize = 64;
/// How often `Progress` is sent at most.
const PUBLISH_EVERY: Duration = Duration::from_millis(50);

/// How a file's lines are read.
#[derive(Clone, Default)]
pub struct IndexOptions {
    /// The Log Format of the file (`None`: Auto-detect).
    pub format: Option<Arc<CompiledFormat>>,
    /// The Detected Fields turned on.
    pub config: Arc<LineConfig>,
    /// Never lock on the fast reader (to check it against Auto-detect).
    pub no_lock: bool,
    /// Read blocks of this many bytes instead (tests cross block edges).
    pub block_bytes: Option<usize>,
}

/// What Indexing has built so far.
#[derive(Debug, Default)]
pub struct IndexedData {
    pub file: FileIndex,
    pub filter: FilterIndex,
    pub max_chars: FieldMaxChars,
}

/// How an Indexing ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Done,
    /// Cancelled: the lines read before stay.
    Cancelled,
    /// The source could not be read on; the lines read before stay.
    Failed(String),
}

/// Indexing's state, shared with whoever shows the file.
#[derive(Debug, Default)]
pub struct Indexed {
    pub data: RwLock<IndexedData>,
    /// Bytes of the source when Indexing started.
    pub total: std::sync::atomic::AtomicU64,
    /// Whether the file locked on the fast reader.
    pub locked: AtomicBool,
    outcome: Mutex<Option<Outcome>>,
}

impl Indexed {
    /// `None` while Indexing runs.
    pub fn outcome(&self) -> Option<Outcome> {
        self.outcome.lock().unwrap().clone()
    }

    /// Bytes indexed, and of the source.
    pub fn progress(&self) -> (u64, u64) {
        let done = self.data.read().unwrap().file.lines.end();
        (done, self.total.load(Ordering::Acquire))
    }
}

/// What a listener hears: the lines indexed so far, and how far through the
/// source; `outcome` once it ended.
#[derive(Debug, Clone, PartialEq)]
pub struct Progress {
    pub lines: usize,
    pub bytes: u64,
    pub total: u64,
    pub outcome: Option<Outcome>,
}

/// A running Indexing.
pub struct Indexing {
    pub indexed: Arc<Indexed>,
    cancel: Arc<AtomicBool>,
    pub updates: async_channel::Receiver<Progress>,
    thread: Option<JoinHandle<()>>,
}

impl Indexing {
    /// Index `source` in the background.
    pub fn start(source: Arc<dyn LineSource>, options: IndexOptions) -> Self {
        let indexed = Arc::new(Indexed::default());
        let cancel = Arc::new(AtomicBool::new(false));
        let (tx, updates) = async_channel::unbounded();
        let thread = {
            let indexed = indexed.clone();
            let cancel = cancel.clone();
            std::thread::Builder::new()
                .name("indexing".into())
                .spawn(move || {
                    let outcome = run(&*source, &options, &indexed, &cancel, &tx);
                    source.release();
                    *indexed.outcome.lock().unwrap() = Some(outcome.clone());
                    let (bytes, total) = indexed.progress();
                    let lines = indexed.data.read().unwrap().file.len();
                    let _ = tx.try_send(Progress { lines, bytes, total, outcome: Some(outcome) });
                })
                .expect("spawn the indexing thread")
        };
        Self { indexed, cancel, updates, thread: Some(thread) }
    }

    /// Stop soon; the lines indexed so far stay.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Release);
    }

    /// Wait for the end; what was built.
    pub fn wait(mut self) -> Arc<Indexed> {
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        self.indexed.clone()
    }
}

impl Drop for Indexing {
    fn drop(&mut self) {
        // Nobody listens any more: stop reading.
        if self.thread.is_some() {
            self.cancel();
        }
    }
}

/// Index all of `source` on the calling thread, the same way.
pub fn index_all(source: &dyn LineSource, options: &IndexOptions) -> (IndexedData, Outcome) {
    let indexed = Indexed::default();
    let (tx, _rx) = async_channel::unbounded();
    let outcome = run(source, options, &indexed, &AtomicBool::new(false), &tx);
    source.release();
    (indexed.data.into_inner().unwrap(), outcome)
}

/// A line's metadata and facts, read the fast way when the file locked on
/// it and the line allows, else by the app's parser.
fn read_line(
    text: &str,
    options: &IndexOptions,
    locked: bool,
    each: &mut dyn FnMut(RawMeta, &LineFacts),
) -> bool {
    if locked {
        if let Some(facts) = fast::read_line(text, &options.config) {
            let raw = RawMeta {
                has_time: true,
                ts_us: facts.timestamp.map(micros),
                level: facts.level,
                not_utf8: false,
            };
            each(raw, &facts);
            return true;
        }
    }
    let event = parse_line_with(0, text, options.format.as_deref());
    let mut entry = LogEntry::from_event(&event);
    entry.extra = options.config.extra_spans(&entry);
    let facts = LineFacts::of_entry(&entry);
    each(RawMeta::of_entry(&entry), &facts);
    false
}

/// Whether the lines of `head` read alike the fast way and by the app's
/// parser: `LOCK_LINES` of them do, and none differs. Lines the fast reader
/// leaves (a stack trace, a blank line) do not count either way. The file
/// then locks on the fast reader.
pub fn decide_lock(head: &[u8], options: &IndexOptions) -> bool {
    if options.no_lock || options.format.is_some() {
        return false;
    }
    let mut lines: Vec<&[u8]> = split_lines(head).collect();
    // The last line of the head may be cut off.
    if !head.ends_with(b"\n") {
        lines.pop();
    }
    let mut alike = 0;
    for bytes in lines {
        let text = String::from_utf8_lossy(line_text(bytes));
        let Some(fast) = fast::read_line(&text, &options.config) else { continue };
        let mut same = false;
        read_line(&text, options, false, &mut |_, slow| same = *slow == fast);
        if !same {
            return false;
        }
        alike += 1;
        if alike == LOCK_LINES {
            return true;
        }
    }
    false
}

struct Block {
    seq: usize,
    data: Vec<u8>,
}

struct BlockOut {
    seq: usize,
    lens: Vec<u64>,
    metas: Vec<RawMeta>,
    columns: ChunkColumns,
}

fn index_block(block: Block, options: &IndexOptions, locked: bool, overflowed: Overflowed) -> BlockOut {
    let mut out = BlockOut { seq: block.seq, lens: Vec::new(), metas: Vec::new(), columns: ChunkColumns::new(overflowed) };
    for bytes in split_lines(&block.data) {
        out.lens.push(bytes.len() as u64);
        let text = String::from_utf8_lossy(line_text(bytes));
        let not_utf8 = matches!(text, Cow::Owned(_));
        let (metas, columns) = (&mut out.metas, &mut out.columns);
        read_line(&text, options, locked, &mut |raw, facts| {
            metas.push(RawMeta { not_utf8, ..raw });
            columns.push(facts);
        });
    }
    out
}

fn add_block(data: &mut IndexedData, out: BlockOut) {
    let continues = data.file.meta.continues();
    data.max_chars.merge(&out.columns.settled_max_chars(continues));
    data.filter.append_chunk(out.columns, continues);
    for (len, raw) in out.lens.into_iter().zip(out.metas) {
        data.file.lines.push(len);
        data.file.meta.push(raw);
    }
}

fn run(
    source: &dyn LineSource,
    options: &IndexOptions,
    indexed: &Indexed,
    cancel: &AtomicBool,
    tx: &async_channel::Sender<Progress>,
) -> Outcome {
    let total = match source.len() {
        Ok(total) => total,
        Err(e) => return Outcome::Failed(e.to_string()),
    };
    indexed.total.store(total, Ordering::Release);
    let mut head = vec![0; FIRST_READ.min(total as usize)];
    let locked = match source.read_at(0, &mut head) {
        Ok(n) => decide_lock(&head[..n], options),
        Err(e) => return Outcome::Failed(e.to_string()),
    };
    drop(head);
    indexed.locked.store(locked, Ordering::Release);

    let overflowed = Arc::new(RwLock::new(Overflowed::default()));
    let failed: Mutex<Option<String>> = Mutex::new(None);
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get()).saturating_sub(1).max(1);
    let mut outcome = Outcome::Done;
    std::thread::scope(|s| {
        let (block_tx, block_rx) = mpsc::sync_channel::<Block>(READ_AHEAD);
        // Shared by the workers only: once they all stop, a reader waiting
        // to hand over a block stops too.
        let block_rx = Arc::new(Mutex::new(block_rx));
        let (out_tx, out_rx) = mpsc::channel::<BlockOut>();

        let failed = &failed;
        s.spawn(move || {
            let mut carry: Vec<u8> = Vec::new();
            let mut offset = 0u64;
            let mut seq = 0;
            loop {
                if cancel.load(Ordering::Acquire) {
                    return;
                }
                let want = options.block_bytes.unwrap_or(if seq == 0 { FIRST_READ } else { READ_BLOCK });
                let mut buf = std::mem::take(&mut carry);
                let have = buf.len();
                buf.resize(have + want, 0);
                let got = match source.read_at(offset, &mut buf[have..]) {
                    Ok(got) => got,
                    Err(e) => {
                        *failed.lock().unwrap() = Some(e.to_string());
                        return;
                    }
                };
                offset += got as u64;
                buf.truncate(have + got);
                let eof = got < want;
                if !eof {
                    match memchr::memrchr(b'\n', &buf) {
                        Some(at) => carry = buf.split_off(at + 1),
                        // One line longer than the block: read on.
                        None => {
                            carry = buf;
                            continue;
                        }
                    }
                }
                if !buf.is_empty() {
                    if block_tx.send(Block { seq, data: buf }).is_err() {
                        return;
                    }
                    seq += 1;
                }
                if eof {
                    return;
                }
            }
        });

        for _ in 0..workers {
            let (block_rx, out_tx, overflowed) = (block_rx.clone(), out_tx.clone(), overflowed.clone());
            s.spawn(move || {
                loop {
                    let block = block_rx.lock().unwrap().recv();
                    let Ok(block) = block else { return };
                    let seen = overflowed.read().unwrap().clone();
                    if out_tx.send(index_block(block, options, locked, seen)).is_err() {
                        return;
                    }
                }
            });
        }
        drop(out_tx);
        drop(block_rx);

        let mut waiting = BTreeMap::new();
        let mut next = 0;
        let mut published = None::<Instant>;
        for out in out_rx.iter() {
            if cancel.load(Ordering::Acquire) {
                outcome = Outcome::Cancelled;
                break;
            }
            waiting.insert(out.seq, out);
            while let Some(out) = waiting.remove(&next) {
                next += 1;
                let mut data = indexed.data.write().unwrap();
                add_block(&mut data, out);
                *overflowed.write().unwrap() = data.filter.overflowed();
                let lines = data.file.len();
                let bytes = data.file.lines.end();
                drop(data);
                if published.is_none_or(|at| at.elapsed() >= PUBLISH_EVERY) {
                    published = Some(Instant::now());
                    let _ = tx.try_send(Progress { lines, bytes, total, outcome: None });
                }
            }
        }
        // Dropping the receiver stops the workers, and then the reader.
        drop(out_rx);
    });
    if cancel.load(Ordering::Acquire) {
        outcome = Outcome::Cancelled;
    }
    if let Some(error) = failed.into_inner().unwrap() {
        outcome = Outcome::Failed(error);
    }
    let mut data = indexed.data.write().unwrap();
    data.file.shrink_to_fit();
    outcome
}
