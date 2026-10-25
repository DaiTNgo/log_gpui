//! Spike of the Line Index (openspec change `line-offset-index`, tasks
//! 1.1–1.5). Run it in release:
//!
//! - `index_probe --gen <file> [lines]`: write the 3,000,000-line log the
//!   baseline was measured on (`mem_probe`), ~376MB.
//! - `index_probe <file> [--cold]`: Indexing with and without the format
//!   lock and with `row_ids` for every column (a) or only for the columns
//!   every line has (b); then reading a screen on demand, and Scans from
//!   disk. `--cold` runs `purge` (needs sudo) before the cold Scans.
//! - `index_probe --merge [lines]`: k-way merge of 3 indexed files against
//!   `timeline::merge` on the same lines.
//!
//! Nothing here is app code: it answers whether the numbers of design.md
//! (decision 14) are within reach.
use log_gpui::filter_index::{MAX_DISTINCT, MISSING, value_of};
use log_gpui::model::{LogEntry, LogLevel};
use log_gpui::parser::text::{LineConfig, LineParser, parse_line_with, parse_text_bytes_with_config};
use log_gpui::search_expr::{self, Matcher};
use roaring::RoaringBitmap;
use std::borrow::Cow;
use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap, HashMap};
use std::fmt::Write as _;
use std::fs::File;
use std::io::{Read, Write as _};
use std::os::unix::fs::FileExt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

/// Lines per block of the Line Index.
const BLOCK_LINES: usize = 128;
/// Bytes the reader hands the workers at a time.
const READ_BLOCK: usize = 4 << 20;
/// The first read is small, so the first lines show early.
const FIRST_READ: usize = 256 << 10;
/// Lines a Scan reads at a time.
const SCAN_CHUNK: usize = 10_000;
/// Lines that must read alike with the fast reader before the file locks.
const LOCK_LINES: usize = 64;
/// The columns: the two every line of the file has, then the Detected
/// Fields of its messages.
const COLUMNS: [&str; 6] = ["level", "module", "user", "took", "trace_id", "path"];
const KV_KEYS: [&str; 4] = ["user", "took", "trace_id", "path"];

fn rss_mb() -> f64 {
    let out = std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim().parse::<f64>().unwrap_or(0.0) / 1024.0
}

fn cores() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get())
}

// ---------------------------------------------------------------- data (1.1)

const LEVELS: [&str; 4] = ["INFO", "DEBUG", "WARN", "ERROR"];
const TAGS: [&str; 5] = ["auth", "api", "db", "cache", "worker"];

/// Line `i` of the baseline log.
fn gen_line(i: usize, out: &mut String) {
    let s = i % 86_400;
    let _ = writeln!(
        out,
        "2026-09-05 {:02}:{:02}:{:02}.{:03} [{}] [{}] request {} handled user={} took {}ms trace_id=ab{:08x} path=/api/v1/items/{}",
        s / 3600, (s / 60) % 60, s % 60, i % 1000, LEVELS[i % 4], TAGS[i % 5], i, i % 977, i % 300, i, i % 5000
    );
}

fn gen_file(path: &str, lines: usize) {
    let mut out = std::io::BufWriter::new(File::create(path).unwrap());
    let mut line = String::new();
    for i in 0..lines {
        line.clear();
        gen_line(i, &mut line);
        out.write_all(line.as_bytes()).unwrap();
    }
    out.flush().unwrap();
    println!("wrote {lines} lines to {path}");
}

// ------------------------------------------------------------- Line Index

/// Where each line starts: blocks of 128 lines, each an absolute start and
/// the varint length of every line in it.
#[derive(Default)]
struct BlockIndex {
    bases: Vec<u64>,
    /// Position in `lens` of each block's first length.
    starts: Vec<u32>,
    lens: Vec<u8>,
    lines: usize,
    end: u64,
}

impl BlockIndex {
    fn push(&mut self, len: u32) {
        if self.lines % BLOCK_LINES == 0 {
            self.bases.push(self.end);
            self.starts.push(self.lens.len() as u32);
        }
        let mut v = len;
        while v >= 0x80 {
            self.lens.push((v as u8) | 0x80);
            v >>= 7;
        }
        self.lens.push(v as u8);
        self.lines += 1;
        self.end += u64::from(len);
    }

    /// Start and length (with its line break) of line `n` (0-based).
    fn line(&self, n: usize) -> (u64, u32) {
        let block = n / BLOCK_LINES;
        let mut at = self.starts[block] as usize;
        let mut start = self.bases[block];
        let mut len = 0;
        for i in 0..=n % BLOCK_LINES {
            let mut v = 0u32;
            let mut shift = 0;
            loop {
                let b = self.lens[at];
                at += 1;
                v |= u32::from(b & 0x7f) << shift;
                if b < 0x80 {
                    break;
                }
                shift += 7;
            }
            if i < n % BLOCK_LINES {
                start += u64::from(v);
            }
            len = v;
        }
        (start, len)
    }

    fn bytes(&self) -> usize {
        self.bases.capacity() * 8 + self.starts.capacity() * 4 + self.lens.capacity()
    }
}

/// What a line keeps besides where it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
struct LineMeta {
    /// Effective timestamp, µs: the line's own, else the one above.
    ts_us: i64,
    /// Lines up to the Parent Line; 0 when it is not a Continuation Line.
    parent_dist: u32,
    level: u8,
    /// Bit 0: the line has a time of its own.
    flags: u8,
}

/// A column's value per line, as ids into its values.
#[derive(Default, PartialEq, Eq, Debug)]
struct Column {
    values: Vec<String>,
    counts: Vec<usize>,
    row_ids: Vec<u16>,
    overflow: bool,
    #[allow(dead_code)]
    map: HashMap<String, u16>,
    /// Id of the latest Parent Line, for its Continuation Lines.
    last: u16,
}

impl Column {
    fn bytes(&self) -> usize {
        self.row_ids.capacity() * 2
    }
}

#[derive(Default)]
struct Index {
    blocks: BlockIndex,
    /// The same starts uncompressed, to compare (empty unless asked for).
    offsets: Vec<u64>,
    meta: Vec<LineMeta>,
    cols: Vec<Column>,
    last_timed: Option<usize>,
    cur_ts: Option<i64>,
    /// Lines before the first timestamp: they take that timestamp.
    untimed_head: usize,
}

impl Index {
    fn len(&self) -> usize {
        self.meta.len()
    }
}

// -------------------------------------------------- reading a line's meta

#[derive(Clone, Copy, PartialEq, Debug, Default)]
struct RawMeta {
    ts_us: i64,
    has_time: bool,
    level: u8,
}

/// One block's column values, as ids of its own.
#[derive(Default)]
struct LocalCol {
    ids: Vec<u16>,
    dict: Vec<String>,
    map: HashMap<String, u16>,
    overflow: bool,
}

impl LocalCol {
    fn record(&mut self, v: Option<&str>) {
        if self.overflow {
            return;
        }
        let id = match v.filter(|v| !v.is_empty()) {
            None => MISSING,
            Some(v) => match self.map.get(v) {
                Some(&id) => id,
                None => {
                    if self.dict.len() >= MAX_DISTINCT {
                        self.overflow = true;
                        self.ids = Vec::new();
                        return;
                    }
                    let id = self.dict.len() as u16;
                    self.dict.push(v.to_string());
                    self.map.insert(v.to_string(), id);
                    id
                }
            },
        };
        self.ids.push(id);
    }
}

/// The value of `key=` in `message`, up to the next space.
fn kv_value<'a>(message: &'a str, key: &str) -> Option<&'a str> {
    let mut from = 0;
    while let Some(at) = message[from..].find(key) {
        let start = from + at;
        let after = start + key.len();
        let word_start = start == 0 || message.as_bytes()[start - 1] == b' ';
        if word_start && message.as_bytes().get(after) == Some(&b'=') {
            let rest = &message[after + 1..];
            return Some(rest.split(' ').next().unwrap_or(""));
        }
        from = after;
    }
    None
}

/// A line read the fast way: `YYYY-MM-DD HH:MM:SS[.f] [LEVEL] [tag] message`.
/// `None` when the line is not written so; the full reader reads it then.
fn fast_line(line: &str) -> Option<(RawMeta, [Option<&str>; 6])> {
    let b = line.as_bytes();
    let num = |r: std::ops::Range<usize>| -> Option<u32> {
        let mut v = 0u32;
        for &c in b.get(r)? {
            if !c.is_ascii_digit() {
                return None;
            }
            v = v * 10 + u32::from(c - b'0');
        }
        Some(v)
    };
    if b.len() < 20 || b[4] != b'-' || b[7] != b'-' || b[10] != b' ' || b[13] != b':' || b[16] != b':' {
        return None;
    }
    let (y, mo, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let (h, mi, s) = (num(11..13)?, num(14..16)?, num(17..19)?);
    let mut at = 19;
    let mut nanos = 0u32;
    if b[at] == b'.' {
        at += 1;
        let start = at;
        while at < b.len() && b[at].is_ascii_digit() {
            at += 1;
        }
        let digits = at - start;
        if digits == 0 || digits > 9 {
            return None;
        }
        nanos = num(start..at)? * 10u32.pow((9 - digits) as u32);
    }
    let ts_us = chrono::NaiveDate::from_ymd_opt(y as i32, mo, d)?
        .and_hms_nano_opt(h, mi, s, nanos)?
        .and_utc()
        .timestamp_micros();
    let rest = line.get(at..)?.strip_prefix(" [")?;
    let (level, rest) = rest.split_once(']')?;
    let lvl = LogLevel::from_token(level);
    if lvl == LogLevel::Unknown {
        return None;
    }
    let rest = rest.strip_prefix(" [")?;
    let (tag, message) = rest.split_once(']')?;
    let message = message.strip_prefix(' ')?;
    let mut vals = [Some(level), Some(tag), None, None, None, None];
    for (i, key) in KV_KEYS.iter().enumerate() {
        vals[2 + i] = kv_value(message, key);
    }
    Some((RawMeta { ts_us, has_time: true, level: lvl as u8 }, vals))
}

/// A line read by the app's own parser.
fn full_line(line: &str, kv: &LineConfig, ncols: usize, mut each: impl FnMut(usize, Option<&str>)) -> RawMeta {
    let event = parse_line_with(0, line, None);
    let mut entry = LogEntry::from_event(&event);
    if ncols > 2 {
        entry.extra = kv.extra_spans(&entry);
    }
    each(0, Some(entry.level_text()));
    each(1, Some(entry.tag()));
    for (c, key) in COLUMNS.iter().enumerate().take(ncols).skip(2) {
        each(c, entry.span_text(key));
    }
    let has_time = entry.has_time && entry.timestamp.is_finite() && entry.timestamp > 0.0;
    RawMeta {
        ts_us: if has_time { (entry.timestamp * 1e6).round() as i64 } else { 0 },
        has_time,
        level: entry.level as u8,
    }
}

// ------------------------------------------------------ Indexing (1.2)

struct Chunk {
    seq: usize,
    data: Vec<u8>,
}

struct ChunkOut {
    seq: usize,
    lens: Vec<u32>,
    meta: Vec<RawMeta>,
    cols: Vec<LocalCol>,
    fast: usize,
}

#[derive(Clone, Copy)]
struct Opts {
    locked: bool,
    ncols: usize,
    keep_offsets: bool,
}

struct Stats {
    first_batch: Duration,
    total: Duration,
    /// Lines the fast reader read (the rest went to the full one).
    fast: usize,
    locked: bool,
}

/// The lines of `data`, without their line breaks, with their lengths.
fn split(data: &[u8]) -> impl Iterator<Item = (&[u8], u32)> {
    let mut start = 0;
    let mut ends = memchr::memchr_iter(b'\n', data);
    std::iter::from_fn(move || {
        if start >= data.len() {
            return None;
        }
        let end = ends.next().map_or(data.len(), |e| e + 1);
        let line = &data[start..end];
        start = end;
        let text = line.strip_suffix(b"\n").unwrap_or(line);
        Some((text.strip_suffix(b"\r").unwrap_or(text), line.len() as u32))
    })
}

fn index_chunk(chunk: Chunk, opts: Opts, kv: &LineConfig) -> ChunkOut {
    let mut out = ChunkOut {
        seq: chunk.seq,
        lens: Vec::new(),
        meta: Vec::new(),
        cols: (0..opts.ncols).map(|_| LocalCol::default()).collect(),
        fast: 0,
    };
    for (bytes, len) in split(&chunk.data) {
        out.lens.push(len);
        let text = String::from_utf8_lossy(bytes);
        if opts.locked {
            if let Some((meta, vals)) = fast_line(&text) {
                out.meta.push(meta);
                for (c, col) in out.cols.iter_mut().enumerate() {
                    col.record(vals[c]);
                }
                out.fast += 1;
                continue;
            }
        }
        let cols = &mut out.cols;
        let meta = full_line(&text, kv, opts.ncols, |c, v| cols[c].record(v));
        out.meta.push(meta);
    }
    out
}

/// Whether the first lines of the file read alike both ways: the file then
/// locks on the fast reader.
fn decide_lock(path: &str, ncols: usize, kv: &LineConfig) -> bool {
    let mut head = vec![0; 64 << 10];
    let n = File::open(path).unwrap().read(&mut head).unwrap();
    let lines: Vec<_> = split(&head[..n]).take(LOCK_LINES).collect();
    lines.len() == LOCK_LINES
        && lines.iter().all(|(bytes, _)| {
            let text = String::from_utf8_lossy(bytes);
            let Some((fast, vals)) = fast_line(&text) else { return false };
            let mut full_vals: Vec<Option<String>> = vec![None; ncols];
            let full = full_line(&text, kv, ncols, |c, v| full_vals[c] = v.filter(|v| !v.is_empty()).map(str::to_string));
            fast == full && (0..ncols).all(|c| vals[c] == full_vals[c].as_deref())
        })
}

fn stitch(index: &mut Index, out: ChunkOut, opts: Opts) {
    let first = index.len();
    let mut cont = Vec::with_capacity(out.meta.len());
    for (j, (raw, &len)) in out.meta.iter().zip(&out.lens).enumerate() {
        let line = first + j;
        index.blocks.push(len);
        if opts.keep_offsets {
            index.offsets.push(index.blocks.end - u64::from(len));
        }
        let (ts_us, parent_dist) = if raw.has_time {
            index.last_timed = Some(line);
            index.cur_ts = Some(raw.ts_us);
            (raw.ts_us, 0)
        } else {
            match index.last_timed {
                Some(p) => (index.cur_ts.unwrap(), (line - p) as u32),
                None => {
                    index.untimed_head += 1;
                    (i64::MIN, 0)
                }
            }
        };
        cont.push(parent_dist > 0);
        index.meta.push(LineMeta { ts_us, parent_dist, level: raw.level, flags: u8::from(raw.has_time) });
    }
    for (col, local) in index.cols.iter_mut().zip(out.cols) {
        if col.overflow {
            continue;
        }
        if local.overflow {
            col.overflow = true;
            col.row_ids = Vec::new();
            continue;
        }
        let mut remap = Vec::with_capacity(local.dict.len());
        for v in local.dict {
            let id = match col.map.get(&v) {
                Some(&id) => id,
                None => {
                    let id = col.values.len() as u16;
                    col.values.push(v.clone());
                    col.counts.push(0);
                    col.map.insert(v, id);
                    id
                }
            };
            remap.push(id);
        }
        if col.values.len() > MAX_DISTINCT {
            col.overflow = true;
            col.row_ids = Vec::new();
            continue;
        }
        for (j, &id) in local.ids.iter().enumerate() {
            let id = if cont[j] {
                col.last
            } else {
                let id = if id == MISSING { MISSING } else { remap[id as usize] };
                col.last = id;
                id
            };
            if id != MISSING {
                col.counts[id as usize] += 1;
            }
            col.row_ids.push(id);
        }
    }
}

/// Index the file at `path`. `on_batch` sees how many lines are indexed
/// after each stitched chunk.
fn index_file(path: &str, opts: Opts) -> (Index, Stats) {
    let started = Instant::now();
    let kv = Arc::new(LineConfig::new(&KV_KEYS.map(String::from)));
    let opts = Opts { locked: opts.locked && decide_lock(path, opts.ncols, &kv), ..opts };
    let (chunk_tx, chunk_rx) = mpsc::sync_channel::<Chunk>(4);
    let chunk_rx = Arc::new(Mutex::new(chunk_rx));
    let (out_tx, out_rx) = mpsc::channel::<ChunkOut>();

    let path_owned = path.to_string();
    let reader = std::thread::spawn(move || {
        let mut file = File::open(path_owned).unwrap();
        let mut carry: Vec<u8> = Vec::new();
        let mut seq = 0;
        loop {
            let want = if seq == 0 { FIRST_READ } else { READ_BLOCK };
            let mut buf = std::mem::take(&mut carry);
            let have = buf.len();
            buf.resize(have + want, 0);
            let mut got = 0;
            while got < want {
                let n = file.read(&mut buf[have + got..]).unwrap();
                if n == 0 {
                    break;
                }
                got += n;
            }
            buf.truncate(have + got);
            let eof = got < want;
            if !eof {
                match memchr::memrchr(b'\n', &buf) {
                    Some(at) => carry = buf.split_off(at + 1),
                    None => {
                        carry = buf;
                        continue;
                    }
                }
            }
            if !buf.is_empty() {
                chunk_tx.send(Chunk { seq, data: buf }).unwrap();
                seq += 1;
            }
            if eof {
                break;
            }
        }
    });
    let workers: Vec<_> = (0..cores().saturating_sub(1).max(1))
        .map(|_| {
            let rx = chunk_rx.clone();
            let tx = out_tx.clone();
            let kv = kv.clone();
            std::thread::spawn(move || {
                loop {
                    let chunk = rx.lock().unwrap().recv();
                    let Ok(chunk) = chunk else { break };
                    tx.send(index_chunk(chunk, opts, &kv)).unwrap();
                }
            })
        })
        .collect();
    drop(out_tx);

    let mut index = Index { cols: (0..opts.ncols).map(|_| Column::default()).collect(), ..Index::default() };
    let mut waiting = BTreeMap::new();
    let mut next = 0;
    let mut first_batch = None;
    let mut fast = 0;
    for out in out_rx {
        waiting.insert(out.seq, out);
        while let Some(out) = waiting.remove(&next) {
            fast += out.fast;
            stitch(&mut index, out, opts);
            next += 1;
            if first_batch.is_none() && index.len() >= 1_000 {
                first_batch = Some(started.elapsed());
            }
        }
    }
    reader.join().unwrap();
    for w in workers {
        w.join().unwrap();
    }
    let first_ts = index.meta.iter().find(|m| m.flags & 1 != 0).map(|m| m.ts_us);
    if let Some(ts) = first_ts {
        for m in &mut index.meta[..index.untimed_head] {
            m.ts_us = ts;
        }
    }
    // Grown by doubling while indexing: give back the slack.
    index.meta.shrink_to_fit();
    index.blocks.lens.shrink_to_fit();
    index.offsets.shrink_to_fit();
    for col in &mut index.cols {
        col.row_ids.shrink_to_fit();
    }
    let total = started.elapsed();
    (index, Stats { first_batch: first_batch.unwrap_or(total), total, fast, locked: opts.locked })
}

/// The meta and values of the first `n` lines checked against the app's
/// parser on the whole text (`parse_text_bytes_with_config`).
fn verify_head(path: &str, index: &Index, n: usize) -> usize {
    let n = n.min(index.len());
    let (start, len) = index.blocks.line(n - 1);
    let mut bytes = vec![0; (start + u64::from(len)) as usize];
    File::open(path).unwrap().read_exact_at(&mut bytes, 0).unwrap();
    let config = LineConfig::new(&KV_KEYS.map(String::from));
    let entries = parse_text_bytes_with_config(&bytes, &config).unwrap().entries;
    let ts = log_gpui::timeline::effective_timestamps(&entries).unwrap_or_default();
    let mut bad = 0;
    for (row, e) in entries.iter().enumerate() {
        let m = index.meta[row];
        let ok_meta = m.flags & 1 == u8::from(e.has_time)
            && m.level == e.level as u8
            && m.parent_dist as usize == e.parent.map_or(0, |p| row + 1 - p)
            && ts.get(row).is_none_or(|&t| m.ts_us == (t * 1e6).round() as i64);
        let ok_cols = index.cols.iter().zip(COLUMNS).all(|(col, key)| {
            if col.overflow {
                return true;
            }
            let want = value_of(&entries, row, key);
            let got = col.row_ids[row];
            want == (got != MISSING).then(|| col.values[got as usize].as_str())
        });
        if !(ok_meta && ok_cols) {
            if bad < 3 {
                println!("  mismatch at line {}: {:?} {:?}", row + 1, m, e.raw_line);
            }
            bad += 1;
        }
    }
    bad
}

// ------------------------------------------------ reading on demand (1.3)

/// The entries of `rows` lines from line `first`: one read, then the app's
/// parser.
fn read_screen(file: &File, index: &Index, first: usize, rows: usize) -> Vec<LogEntry> {
    let last = (first + rows).min(index.len()) - 1;
    let (start, _) = index.blocks.line(first);
    let (end_start, end_len) = index.blocks.line(last);
    let mut buf = vec![0; (end_start + u64::from(end_len) - start) as usize];
    file.read_exact_at(&mut buf, start).unwrap();
    let mut parser = LineParser::new();
    let m = index.meta[first];
    if m.parent_dist > 0 {
        parser.set_parent(Some(first + 1 - m.parent_dist as usize));
    }
    split(&buf)
        .enumerate()
        .map(|(i, (bytes, _))| parser.parse(first + i + 1, &String::from_utf8_lossy(bytes)))
        .collect()
}

fn percentile(times: &mut [Duration], p: f64) -> Duration {
    times.sort();
    times[((times.len() - 1) as f64 * p) as usize]
}

// ------------------------------------------------------------ Scan (1.4)

/// `f(first line, lines)` for each chunk of `SCAN_CHUNK` lines, read from
/// disk in parallel; the results in line order.
fn scan<R: Send>(path: &str, index: &Index, f: impl Fn(usize, &[&str]) -> R + Sync) -> Vec<R> {
    let file = File::open(path).unwrap();
    let chunks = index.len().div_ceil(SCAN_CHUNK);
    let next = AtomicUsize::new(0);
    let results = Mutex::new(Vec::with_capacity(chunks));
    std::thread::scope(|s| {
        for _ in 0..cores() {
            s.spawn(|| {
                let mut buf = Vec::new();
                loop {
                    let c = next.fetch_add(1, Ordering::Relaxed);
                    if c >= chunks {
                        break;
                    }
                    let first = c * SCAN_CHUNK;
                    let last = (first + SCAN_CHUNK).min(index.len()) - 1;
                    let (start, _) = index.blocks.line(first);
                    let (end_start, end_len) = index.blocks.line(last);
                    buf.resize((end_start + u64::from(end_len) - start) as usize, 0);
                    file.read_exact_at(&mut buf, start).unwrap();
                    let text: Cow<str> = String::from_utf8_lossy(&buf);
                    let lines: Vec<&str> = text.lines().collect();
                    let r = f(first, &lines);
                    results.lock().unwrap().push((c, r));
                }
            });
        }
    });
    let mut results = results.into_inner().unwrap();
    results.sort_by_key(|(c, _)| *c);
    results.into_iter().map(|(_, r)| r).collect()
}

fn search(path: &str, index: &Index, query: &str) -> Vec<u32> {
    let expr = search_expr::parse(query).unwrap().unwrap();
    let matcher = Matcher::compile(&expr);
    scan(path, index, |first, lines| {
        lines
            .iter()
            .enumerate()
            .filter(|(_, l)| matcher.is_match(l))
            .map(|(i, _)| (first + i) as u32)
            .collect::<Vec<u32>>()
    })
    .concat()
}

fn purge() -> bool {
    std::process::Command::new("purge").status().is_ok_and(|s| s.success())
}

// ----------------------------------------------------------- merge (1.5)

/// Line `i` of file `k` of the merge test: times rise within a file and
/// interleave between files; every 500th line has a stack trace line.
fn merge_line(k: usize, i: usize, out: &mut String) {
    let base = chrono::NaiveDate::from_ymd_opt(2026, 9, 5).unwrap().and_hms_opt(0, 0, 0).unwrap();
    let ms = (i * 3000 + (i * 7919 + k * 104_729) % 3000) as i64;
    let t = base + chrono::Duration::milliseconds(ms);
    let _ = writeln!(out, "{} [{}] [{}] request {i} of file {k} user={}", t.format("%Y-%m-%d %H:%M:%S%.3f"), LEVELS[i % 4], TAGS[k], i % 977);
    if i % 500 == 0 {
        let _ = writeln!(out, "    at com.example.Worker.run(Worker.java:{})", i % 97);
    }
}

fn pack(file: usize, line: usize) -> u64 {
    ((file as u64) << 48) | line as u64
}

/// The merge so far: what is merged, and how far into each file.
#[derive(Default)]
struct Merger {
    next: Vec<usize>,
    in_heap: Vec<bool>,
    heap: BinaryHeap<Reverse<(i64, usize, usize)>>,
    rows: Vec<u64>,
}

impl Merger {
    fn new(files: usize) -> Self {
        Self { next: vec![0; files], in_heap: vec![false; files], ..Self::default() }
    }

    /// Merge what is there. A file not `done` may still get lines, so the
    /// merge stops when it runs out of that file's lines.
    fn advance(&mut self, lanes: &[&[LineMeta]], done: &[bool]) {
        for (k, lane) in lanes.iter().enumerate() {
            if !self.in_heap[k] && self.next[k] < lane.len() {
                self.heap.push(Reverse((lane[self.next[k]].ts_us, k, self.next[k])));
                self.in_heap[k] = true;
            }
        }
        loop {
            if (0..lanes.len()).any(|k| !done[k] && !self.in_heap[k]) {
                break;
            }
            let Some(Reverse((_, k, line))) = self.heap.pop() else { break };
            self.in_heap[k] = false;
            let lane = lanes[k];
            self.rows.push(pack(k, line));
            let mut n = line + 1;
            // Continuation Lines stay right below their Parent Line.
            while n < lane.len() && lane[n].parent_dist > 0 {
                self.rows.push(pack(k, n));
                n += 1;
            }
            self.next[k] = n;
            if n < lane.len() {
                self.heap.push(Reverse((lane[n].ts_us, k, n)));
                self.in_heap[k] = true;
            }
        }
    }
}

fn merge_probe(lines: usize) {
    let dir = tempfile::tempdir().unwrap();
    let paths: Vec<String> = (0..3)
        .map(|k| {
            let path = dir.path().join(format!("f{k}.log")).to_string_lossy().into_owned();
            let mut out = std::io::BufWriter::new(File::create(&path).unwrap());
            let mut line = String::new();
            for i in 0..lines {
                line.clear();
                merge_line(k, i, &mut line);
                out.write_all(line.as_bytes()).unwrap();
            }
            out.flush().unwrap();
            path
        })
        .collect();
    let opts = Opts { locked: true, ncols: 2, keep_offsets: false };
    let indexes: Vec<Index> = paths.iter().map(|p| index_file(p, opts).0).collect();
    let lanes: Vec<&[LineMeta]> = indexes.iter().map(|i| i.meta.as_slice()).collect();
    let total: usize = lanes.iter().map(|l| l.len()).sum();
    println!("merge: 3 files, {total} lines");

    let t = Instant::now();
    let mut merger = Merger::new(3);
    merger.advance(&lanes, &[true; 3]);
    let whole = t.elapsed();
    println!("  k-way merge, whole                     {whole:>10.2?}  ({} rows, {} B/row)", merger.rows.len(), 8);

    // The same files 1,000 lines short, still growing; then the rest.
    let short: Vec<&[LineMeta]> = lanes.iter().map(|l| &l[..l.len() - 1_000]).collect();
    let mut inc = Merger::new(3);
    inc.advance(&short, &[false; 3]);
    let t = Instant::now();
    inc.advance(&lanes, &[true; 3]);
    println!("  k-way merge, +1,000 lines per file     {:>10.2?}", t.elapsed());
    println!("  incremental == whole: {}", inc.rows == merger.rows);

    let entries: Vec<Vec<LogEntry>> = paths
        .iter()
        .map(|p| log_gpui::parser::text::parse_entries(&String::from_utf8_lossy(&std::fs::read(p).unwrap())))
        .collect();
    let slices: Vec<&[LogEntry]> = entries.iter().map(Vec::as_slice).collect();
    let t = Instant::now();
    let reference = log_gpui::timeline::merge(&slices);
    println!("  timeline::merge (entries in RAM)       {:>10.2?}", t.elapsed());
    let same = reference.rows.len() == merger.rows.len()
        && reference.rows.iter().zip(&merger.rows).all(|(r, &p)| pack(r.file as usize, r.line as usize) == p);
    println!("  same order as timeline::merge: {same}");
}

// ------------------------------------------------------------------ main

fn report_index(label: &str, index: &Index, stats: &Stats, rss: Option<f64>) {
    let n = index.len() as f64;
    let cols: usize = index.cols.iter().map(Column::bytes).sum();
    let meta = index.meta.capacity() * std::mem::size_of::<LineMeta>();
    let total = index.blocks.bytes() + meta + cols;
    println!(
        "{label:<28} first {:>8.2?}  total {:>8.2?}  {:.1} B/line (index {:.2}, meta {:.1}, row_ids {:.1}){}{}",
        stats.first_batch,
        stats.total,
        total as f64 / n,
        index.blocks.bytes() as f64 / n,
        meta as f64 / n,
        cols as f64 / n,
        if stats.locked { format!("  fast {:.1}%", stats.fast as f64 * 100.0 / n) } else { String::new() },
        rss.map_or(String::new(), |r| format!("  RSS +{r:.0} MB")),
    );
}

/// The app's own Indexing (`log_gpui::index::indexing`) on `path`: with and
/// without the lock, with the Detected Fields of the spike's columns.
fn app_probe(path: &str) {
    use log_gpui::index::indexing::{IndexOptions, Indexing};
    use log_gpui::index::{DiskSource, HandleBudget, LineSource};
    let _ = std::fs::read(path).unwrap();
    let config = Arc::new(LineConfig::new(&KV_KEYS.map(String::from)));
    for no_lock in [false, true] {
        let before = rss_mb();
        let started = Instant::now();
        let source: Arc<dyn LineSource> = Arc::new(DiskSource::on_demand(path, HandleBudget::for_process()));
        let indexing = Indexing::start(source, IndexOptions { config: config.clone(), no_lock, ..Default::default() });
        let first = indexing.updates.recv_blocking().unwrap();
        let first_at = started.elapsed();
        let indexed = indexing.wait();
        let total = started.elapsed();
        let data = indexed.data.read().unwrap();
        let lines = data.file.len();
        let index_bytes = data.file.heap_bytes();
        let row_ids: usize = data.filter.columns.iter().filter(|c| c.has_row_ids()).count() * 2 * lines;
        println!(
            "app Indexing lock={} : first {} lines in {:?}, all {lines} in {:?}; index+meta {:.1} B/line, row_ids ~{:.1} B/line ({} columns), RSS +{:.0} MB",
            indexed.locked.load(std::sync::atomic::Ordering::Acquire),
            first.lines,
            first_at,
            total,
            index_bytes as f64 / lines as f64,
            row_ids as f64 / lines as f64,
            data.filter.columns.iter().filter(|c| c.has_row_ids()).count(),
            rss_mb() - before
        );
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--gen") {
        let lines = args.get(2).and_then(|a| a.parse().ok()).unwrap_or(3_000_000);
        gen_file(&args[1], lines);
        return;
    }
    if args.first().map(String::as_str) == Some("--app") {
        app_probe(&args[1]);
        return;
    }
    if args.first().map(String::as_str) == Some("--merge") {
        merge_probe(args.get(1).and_then(|a| a.parse().ok()).unwrap_or(1_000_000));
        return;
    }
    let path = args.first().expect("index_probe <file> [--cold] | --gen <file> [lines] | --merge [lines]").clone();
    let cold = args.iter().any(|a| a == "--cold");
    let size = std::fs::metadata(&path).unwrap().len();
    println!("{path}: {:.1} MB, {} cores", size as f64 / 1e6, cores());

    // Warm the page cache, so the runs compare the work and not the disk.
    let _ = std::fs::read(&path).unwrap();

    // 1.2: Indexing four ways. Only one Index is alive at a time.
    println!("\n-- Indexing (1.2)");
    let runs = [
        ("no lock, (a) every column", Opts { locked: false, ncols: 6, keep_offsets: false }),
        ("no lock, (b) fixed columns", Opts { locked: false, ncols: 2, keep_offsets: false }),
        ("lock, (a) every column", Opts { locked: true, ncols: 6, keep_offsets: false }),
        ("lock, (b) fixed columns", Opts { locked: true, ncols: 2, keep_offsets: false }),
    ];
    let mut unlocked_a = None;
    for (label, opts) in runs {
        let before = rss_mb();
        let (index, stats) = index_file(&path, opts);
        report_index(label, &index, &stats, Some(rss_mb() - before));
        if opts.ncols == 6 && !opts.locked {
            unlocked_a = Some(index);
        } else if opts.ncols == 6 && opts.locked {
            let reference = unlocked_a.take().unwrap();
            let same = reference.meta == index.meta
                && reference.cols.iter().zip(&index.cols).all(|(a, b)| {
                    a.overflow == b.overflow && a.values == b.values && a.row_ids == b.row_ids
                });
            println!("  lock reads every line as without it: {same}");
            drop(reference);
            println!("  first 300,000 lines against the app's parser: {} mismatches", verify_head(&path, &index, 300_000));
            for (col, key) in index.cols.iter().zip(COLUMNS) {
                println!("  column {key:<9} {}", if col.overflow { "overflow".to_string() } else { format!("{} values", col.values.len()) });
            }
        }
    }

    // Line Index encodings side by side.
    let (index, _) = index_file(&path, Opts { locked: true, ncols: 2, keep_offsets: true });
    println!(
        "  Line Index: blocks of 128 + varint {:.2} B/line, Vec<u64> {:.2} B/line",
        index.blocks.bytes() as f64 / index.len() as f64,
        index.offsets.capacity() as f64 * 8.0 / index.len() as f64
    );
    let ok = (0..index.len()).step_by(997).all(|n| index.blocks.line(n).0 == index.offsets[n]);
    println!("  block index agrees with Vec<u64>: {ok}");

    // 1.3: a screen of 60 rows read on demand.
    println!("\n-- Reading a screen on demand (1.3)");
    let file = File::open(&path).unwrap();
    let n = index.len();
    for (label, at) in [("start", 0), ("middle", n / 2), ("end", n - 60)] {
        let mut times: Vec<Duration> = (0..200)
            .map(|_| {
                let t = Instant::now();
                std::hint::black_box(read_screen(&file, &index, at, 60));
                t.elapsed()
            })
            .collect();
        println!("  60 rows at {label:<8} p50 {:>8.2?}  p99 {:>8.2?}", percentile(&mut times, 0.5), percentile(&mut times, 0.99));
    }
    let mut seed = 0x9e37_79b9_7f4a_7c15u64;
    let mut times: Vec<Duration> = (0..2_000)
        .map(|_| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let at = (seed % (n as u64 - 60)) as usize;
            let t = Instant::now();
            std::hint::black_box(read_screen(&file, &index, at, 60));
            t.elapsed()
        })
        .collect();
    println!("  60 rows, random jumps  p50 {:>8.2?}  p99 {:>8.2?}", percentile(&mut times, 0.5), percentile(&mut times, 0.99));

    // 1.4: Scans from disk.
    println!("\n-- Scan (1.4), {} lines a chunk", SCAN_CHUNK);
    for query in ["timeout", "user=42"] {
        for pass in ["warm", "cold"] {
            if pass == "cold" && !(cold && purge()) {
                println!("  Search {query:<9} cold: skipped (run with --cold under sudo, for purge)");
                continue;
            }
            let t = Instant::now();
            let hits = search(&path, &index, query);
            let took = t.elapsed();
            let t = Instant::now();
            let bitmap = RoaringBitmap::from_sorted_iter(hits.iter().copied()).unwrap();
            let roaring_time = t.elapsed();
            println!(
                "  Search {query:<9} {pass}: {took:>9.2?}  {} matches  Vec<u32> {} KB, Roaring {} KB (built in {roaring_time:.2?})",
                hits.len(),
                hits.len() * 4 / 1024,
                bitmap.serialized_size() / 1024
            );
        }
    }
    // (b): a Detected Field turned on later is a Scan of its own. `user`
    // has more values than MAX_DISTINCT, so every value is counted here,
    // without the cap, to time the whole read.
    let t = Instant::now();
    let parts = scan(&path, &index, |_, lines| {
        let mut ids = Vec::with_capacity(lines.len());
        let mut dict: HashMap<&str, u16> = HashMap::new();
        for line in lines {
            let v = line.split_once("] ").and_then(|(_, rest)| kv_value(rest, "user"));
            ids.push(v.map_or(MISSING, |v| {
                let id = dict.len() as u16;
                *dict.entry(v).or_insert(id)
            }));
        }
        let mut dict: Vec<(String, u16)> = dict.into_iter().map(|(v, id)| (v.to_string(), id)).collect();
        dict.sort_by_key(|(_, id)| *id);
        (ids, dict)
    });
    let mut values: HashMap<String, u16> = HashMap::new();
    let mut row_ids: Vec<u16> = Vec::with_capacity(index.len());
    for (ids, dict) in parts {
        let remap: Vec<u16> = dict
            .into_iter()
            .map(|(v, _)| {
                let id = values.len() as u16;
                *values.entry(v).or_insert(id)
            })
            .collect();
        row_ids.extend(ids.iter().map(|&id| if id == MISSING { id } else { remap[id as usize] }));
    }
    println!("  Scan building Detected Field `user` (b): {:>9.2?}  {} values, {} rows", t.elapsed(), values.len(), row_ids.len());

    println!("\nRSS now {:.0} MB", rss_mb());
}
