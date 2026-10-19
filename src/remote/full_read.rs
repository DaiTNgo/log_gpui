//! Open in full: finding where a time starts and ends in a file without
//! reading all of it, then reading forward.
//!
//! The search reads a few places in the file (about 15–20 reads for 1 GB),
//! each a block of 64 KB, and takes the time of the first Parent Line in it.
//! Logs are mostly in time order; where writers disagree by a few seconds the
//! final forward scan still lands on the first line at or after the time.

use super::compress::{AsyncBridge, Compression, decoder};
use super::copy::{self, CopyMeta, GenerationMeta, StopReason};
use super::{ConnectError, RemoteHub, RemoteSession, ServerStore, shell_quote};
use crate::live::backward::{ReadAt, START_SLACK_SECS};
use crate::live::line_time::line_timestamp;
use crate::live::splitter::{LineSplitter, decode_line};
use async_channel::Sender;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncSeekExt};

/// `ReadAt` over an SFTP file, for use on a blocking thread (it waits for each
/// read on the runtime).
pub struct SftpReadAt {
    pub handle: tokio::runtime::Handle,
    pub file: russh_sftp::client::fs::File,
}

impl ReadAt for SftpReadAt {
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> io::Result<usize> {
        let file = &mut self.file;
        self.handle.block_on(async {
            file.seek(io::SeekFrom::Start(offset)).await?;
            let mut done = 0;
            while done < buf.len() {
                let n = file.read(&mut buf[done..]).await?;
                if n == 0 {
                    break;
                }
                done += n;
            }
            Ok(done)
        })
    }
}

/// Size of one read while searching.
pub const PROBE_BLOCK: usize = 64 * 1024;
/// A block with no time in it is widened up to this much.
pub const PROBE_MAX: usize = 1024 * 1024;

/// The first Parent Line that begins at or after `from`, as `(offset, time)`,
/// looking at no more than `limit` bytes. `from` that is not 0 is taken as
/// the middle of a line: the line it is in is skipped.
fn first_parent_after<R: ReadAt>(src: &mut R, from: u64, size: u64, limit: usize) -> io::Result<Option<(u64, f64)>> {
    let len = (limit as u64).min(size.saturating_sub(from)) as usize;
    let mut block = vec![0u8; len];
    let n = src.read_at(from, &mut block)?;
    block.truncate(n);
    let mut pos = 0usize;
    if from > 0 {
        match block.iter().position(|b| *b == b'\n') {
            Some(i) => pos = i + 1,
            None => return Ok(None),
        }
    }
    while pos < block.len() {
        let Some(end) = block[pos..].iter().position(|b| *b == b'\n') else { break };
        let line = decode_line(&block[pos..pos + end]);
        if let Some(t) = line_timestamp(&line) {
            return Ok(Some((from + pos as u64, t)));
        }
        pos += end + 1;
    }
    Ok(None)
}

/// The offset of the first Parent Line whose time satisfies `past` (that is
/// `false` for every earlier line and `true` from the boundary on), or `size`
/// when none does.
fn find_boundary<R: ReadAt>(src: &mut R, size: u64, past: impl Fn(f64) -> bool) -> io::Result<u64> {
    // Invariant: the boundary is at `lo` or after it.
    let (mut lo, mut hi) = (0u64, size);
    while hi - lo > PROBE_BLOCK as u64 {
        let mid = lo + (hi - lo) / 2;
        let mut found = first_parent_after(src, mid, size, PROBE_BLOCK)?;
        if found.is_none() {
            found = first_parent_after(src, mid, size, PROBE_MAX)?;
        }
        match found {
            Some((at, t)) if !past(t) => lo = at,
            Some(_) => hi = mid,
            // No time anywhere in the widened block: nothing is known here.
            None => lo = (mid + PROBE_MAX as u64).min(hi),
        }
    }
    scan_forward(src, lo, size, past)
}

/// From `from` (the middle of a line unless 0) on, the first Parent Line
/// whose time satisfies `past`.
fn scan_forward<R: ReadAt>(src: &mut R, from: u64, size: u64, past: impl Fn(f64) -> bool) -> io::Result<u64> {
    let mut offset = from;
    let mut carry: Vec<u8> = Vec::new();
    let mut carry_at = from;
    let mut skipping = from > 0;
    let mut block = vec![0u8; PROBE_BLOCK];
    while offset < size {
        let n = src.read_at(offset, &mut block)?;
        if n == 0 {
            break;
        }
        offset += n as u64;
        carry.extend_from_slice(&block[..n]);
        let mut pos = 0;
        while let Some(end) = carry[pos..].iter().position(|b| *b == b'\n') {
            let at = carry_at + pos as u64;
            if skipping {
                skipping = false;
            } else if let Some(t) = line_timestamp(&decode_line(&carry[pos..pos + end])) {
                if past(t) {
                    return Ok(at);
                }
            }
            pos += end + 1;
        }
        carry.drain(..pos);
        carry_at += pos as u64;
    }
    // A last line without a newline.
    if !skipping && !carry.is_empty() {
        if let Some(t) = line_timestamp(&decode_line(&carry)) {
            if past(t) {
                return Ok(carry_at);
            }
        }
    }
    Ok(size)
}

/// Where reading starts for the Starting Time `start`: the offset of the
/// first Parent Line not older than it (less a second of slack).
pub fn find_start_offset<R: ReadAt>(src: &mut R, size: u64, start: f64) -> io::Result<u64> {
    find_boundary(src, size, |t| t >= start - START_SLACK_SECS)
}

/// Where reading ends for the time `to`: the offset of the first Parent Line
/// later than it, so the last line kept keeps its Continuation Lines.
pub fn find_end_offset<R: ReadAt>(src: &mut R, size: u64, to: f64) -> io::Result<u64> {
    find_boundary(src, size, |t| t > to)
}

// ── Reading ──────────────────────────────────────────────────────────────────

/// One file to read: the Remote File itself or one of its Rotated Files.
#[derive(Debug, Clone, PartialEq)]
pub struct GenerationPlan {
    /// 0 for the file itself, 1 for the Rotated File just before it, …
    pub distance: u32,
    /// File name on the Server.
    pub name: String,
    pub path: String,
    /// Size on the Server when the plan was made (the read ends there).
    pub size: u64,
    pub modified: Option<u64>,
    pub compression: Compression,
}

/// What Open in full reads.
#[derive(Debug, Clone)]
pub struct ReadPlan {
    pub server_id: String,
    pub server_name: String,
    /// The Remote File the copy is of.
    pub path: String,
    /// Oldest Generation first.
    pub generations: Vec<GenerationPlan>,
    /// Start and end of the time to read, on the clock of the file.
    pub start: Option<f64>,
    pub to: Option<f64>,
    pub start_text: Option<String>,
    pub to_text: Option<String>,
    pub limit_lines: usize,
    /// The folder of the Remote Copy (made, empty).
    pub dir: PathBuf,
}

/// What the read tells the app, in order.
#[derive(Debug, Clone, PartialEq)]
pub enum ReadEvent {
    /// The next Generation begins. `first_line_no` is its first line's number
    /// in its file when already known (`FirstLineNo` follows when it is not).
    GenerationStart { index: usize, distance: u32, name: String, first_line_no: Option<usize> },
    /// The number of the first line of the Generation, once counted (`None`:
    /// it could not be).
    FirstLineNo { index: usize, first_line_no: Option<usize> },
    Lines { lines: Vec<String>, bytes: Vec<u32> },
    /// How far the file being read has come (bytes of the file on the Server).
    Progress { what: String, done: u64, total: u64 },
    /// Reading ended early; the Break goes after the last line.
    Stopped(StopReason),
    /// The read ended; the copy is on disk.
    Done(CopyMeta),
    /// The read failed; what was read is kept on disk.
    Failed { message: String, meta: Option<CopyMeta> },
}

/// Cancel and Pause of a read, from the app.
#[derive(Debug, Default)]
pub struct ReadControl {
    pub cancel: AtomicBool,
    pub paused: AtomicBool,
}

impl ReadControl {
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    pub fn set_paused(&self, paused: bool) {
        self.paused.store(paused, Ordering::Relaxed);
    }

    fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    /// Wait while paused; `false` when cancelled meanwhile.
    async fn checkpoint(&self) -> bool {
        while self.paused.load(Ordering::Relaxed) {
            if self.is_cancelled() {
                return false;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        !self.is_cancelled()
    }

    fn checkpoint_blocking(&self) -> bool {
        while self.paused.load(Ordering::Relaxed) {
            if self.is_cancelled() {
                return false;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        !self.is_cancelled()
    }
}

/// Reading goes on for this long without a connection before it gives up.
const GIVE_UP_AFTER: Duration = Duration::from_secs(300);
const COUNT_TIMEOUT: Duration = Duration::from_secs(30);
const BLOCK: usize = 1024 * 1024;

/// How one Generation ended.
enum Outcome {
    Complete,
    Stopped(StopReason),
}

struct Reader {
    hub: Arc<RemoteHub>,
    servers: Arc<Mutex<ServerStore>>,
    plan: ReadPlan,
    tx: Sender<ReadEvent>,
    ctl: Arc<ReadControl>,
    /// Lines read so far, over every Generation.
    lines: usize,
    /// The count of the lines before the start of the Generation being read.
    pending_count: Option<tokio::task::JoinHandle<Option<usize>>>,
}

/// Read the plan into its folder, telling the app as lines arrive. The last
/// event is `Done` or `Failed`.
pub async fn run(
    hub: Arc<RemoteHub>,
    servers: Arc<Mutex<ServerStore>>,
    plan: ReadPlan,
    tx: Sender<ReadEvent>,
    ctl: Arc<ReadControl>,
) {
    let mut reader = Reader { hub, servers, plan, tx, ctl, lines: 0, pending_count: None };
    let mut generations: Vec<GenerationMeta> = Vec::new();
    let mut failure: Option<String> = None;
    let mut stopped: Option<StopReason> = None;
    let planned = reader.plan.generations.clone();
    for (index, gp) in planned.iter().enumerate() {
        let mut meta = GenerationMeta {
            distance: gp.distance,
            source_name: gp.name.clone(),
            compressed: gp.compression.is_compressed(),
            start_offset: 0,
            end_offset: None,
            first_line_no: None,
            from: reader.plan.start,
            to: reader.plan.to,
            complete: false,
            stop: None,
            lines: 0,
            file: GenerationMeta::file_name(gp.distance),
        };
        let result = reader.read_generation(index, gp, &mut meta).await;
        if let Some(count) = reader.pending_count.take() {
            meta.first_line_no = count.await.ok().flatten();
        }
        match result {
            Ok(Outcome::Complete) => meta.complete = true,
            Ok(Outcome::Stopped(reason)) => {
                meta.stop = Some(reason);
                stopped = Some(reason);
            }
            Err(message) => failure = Some(message),
        }
        generations.push(meta);
        if stopped.is_some() || failure.is_some() {
            break;
        }
    }
    let meta = CopyMeta {
        server_id: reader.plan.server_id.clone(),
        server_name: reader.plan.server_name.clone(),
        path: reader.plan.path.clone(),
        read_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        start_text: reader.plan.start_text.clone(),
        to_text: reader.plan.to_text.clone(),
        limit_lines: reader.plan.limit_lines,
        generations,
    };
    let written = copy::write_meta(&reader.plan.dir, &meta);
    if let Some(reason) = stopped {
        let _ = reader.tx.try_send(ReadEvent::Stopped(reason));
    }
    let event = match (failure, written) {
        (Some(message), _) => ReadEvent::Failed { message, meta: Some(meta) },
        (None, Err(e)) => ReadEvent::Failed { message: format!("could not save the copy: {e}"), meta: None },
        (None, Ok(())) => ReadEvent::Done(meta),
    };
    let _ = reader.tx.try_send(event);
}

impl Reader {
    fn send(&self, event: ReadEvent) {
        let _ = self.tx.try_send(event);
    }

    /// A session to the Server, trying again for a while when it is down.
    async fn session(&self) -> Result<Arc<RemoteSession>, String> {
        let started = Instant::now();
        let mut backoff = Duration::from_secs(1);
        loop {
            if self.ctl.is_cancelled() {
                return Err("cancelled".into());
            }
            let store = self.servers.lock().unwrap().clone();
            match self.hub.session(&store, &self.plan.server_id).await {
                Ok(session) => return Ok(session),
                Err(e @ (ConnectError::NeedsSecret { .. } | ConnectError::AuthFailed { .. })) => return Err(e.message()),
                Err(e) => {
                    if started.elapsed() > GIVE_UP_AFTER {
                        return Err(format!("lost the connection: {}", e.message()));
                    }
                }
            }
            tokio::time::sleep(backoff).await;
            backoff = (backoff * 2).min(Duration::from_secs(30));
        }
    }

    async fn read_generation(
        &mut self,
        index: usize,
        gp: &GenerationPlan,
        meta: &mut GenerationMeta,
    ) -> Result<Outcome, String> {
        let file_path = self.plan.dir.join(&meta.file);
        if gp.compression.is_compressed() {
            self.read_compressed(index, gp, meta, &file_path).await
        } else {
            self.read_plain(index, gp, meta, &file_path).await
        }
    }

    async fn read_plain(
        &mut self,
        index: usize,
        gp: &GenerationPlan,
        meta: &mut GenerationMeta,
        out_path: &Path,
    ) -> Result<Outcome, String> {
        use tokio::io::AsyncWriteExt;
        let mut session = self.session().await?;
        let mut sftp = session.open_sftp().await?;
        let (start, end) = {
            let (start, to) = (self.plan.start, self.plan.to);
            let (size, path) = (gp.size, gp.path.clone());
            let handle = tokio::runtime::Handle::current();
            let file = sftp.open(path.clone()).await.map_err(|e| format!("{}: {e}", gp.name))?;
            tokio::task::spawn_blocking(move || -> io::Result<(u64, u64)> {
                let mut src = SftpReadAt { handle, file };
                let start = match start {
                    Some(t) => find_start_offset(&mut src, size, t)?,
                    None => 0,
                };
                let end = match to {
                    Some(t) => find_end_offset(&mut src, size, t)?.max(start),
                    None => size,
                };
                Ok((start, end))
            })
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| format!("{}: {e}", gp.name))?
        };
        meta.start_offset = start;
        meta.end_offset = Some(end);
        let known = (start == 0).then_some(1);
        meta.first_line_no = known;
        self.send(ReadEvent::GenerationStart {
            index,
            distance: gp.distance,
            name: gp.name.clone(),
            first_line_no: known,
        });
        if start > 0 {
            // The real number of the first line, counted on the Server while
            // the lines are read.
            let (tx, path, session2) = (self.tx.clone(), gp.path.clone(), session.clone());
            self.pending_count = Some(tokio::spawn(async move {
                let command = format!("head -c {start} -- {} | wc -l", shell_quote(&path));
                let counted = tokio::time::timeout(COUNT_TIMEOUT, session2.exec(&command)).await;
                let n = counted
                    .ok()
                    .and_then(|r| r.ok())
                    .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse::<usize>().ok())
                    .map(|n| n + 1);
                let _ = tx.try_send(ReadEvent::FirstLineNo { index, first_line_no: n });
                n
            }));
        }
        let mut out = tokio::fs::File::create(out_path).await.map_err(|e| e.to_string())?;
        let mut file = sftp.open(gp.path.clone()).await.map_err(|e| format!("{}: {e}", gp.name))?;
        let mut splitter = LineSplitter::new();
        let mut offset = start;
        let mut buf = vec![0u8; BLOCK];
        let mut last_progress = Instant::now() - Duration::from_secs(1);
        while offset < end {
            if !self.ctl.checkpoint().await {
                return Ok(Outcome::Stopped(StopReason::Cancelled));
            }
            let want = BLOCK.min((end - offset) as usize);
            let read = async {
                file.seek(io::SeekFrom::Start(offset)).await?;
                file.read(&mut buf[..want]).await
            }
            .await;
            let n = match read {
                Ok(0) => break,
                Ok(n) => n,
                Err(_) => {
                    // Connection lost: reconnect and go on from the offset.
                    self.hub.drop_session(&self.plan.server_id).await;
                    session = self.session().await?;
                    sftp = session.open_sftp().await?;
                    file = match sftp.open(gp.path.clone()).await {
                        Ok(f) => f,
                        Err(_) => return Ok(Outcome::Stopped(StopReason::RotatedWhileReading)),
                    };
                    continue;
                }
            };
            offset += n as u64;
            let (mut lines, mut bytes) = splitter.push_with_lens(&buf[..n]);
            if offset >= end {
                if let Some(last) = splitter.finish() {
                    bytes.push(last.len() as u32);
                    lines.push(last);
                }
            }
            let room = self.plan.limit_lines.saturating_sub(self.lines);
            let hit_limit = lines.len() > room;
            lines.truncate(room);
            bytes.truncate(room);
            if !lines.is_empty() {
                let mut text = lines.join("\n");
                text.push('\n');
                out.write_all(text.as_bytes()).await.map_err(|e| e.to_string())?;
                self.lines += lines.len();
                meta.lines += lines.len();
                self.send(ReadEvent::Lines { lines, bytes });
            }
            if last_progress.elapsed() > Duration::from_millis(250) || offset >= end {
                last_progress = Instant::now();
                self.send(ReadEvent::Progress { what: gp.name.clone(), done: offset - start, total: end - start });
            }
            if hit_limit || self.lines >= self.plan.limit_lines && offset < end {
                out.flush().await.map_err(|e| e.to_string())?;
                meta.end_offset = Some(offset);
                return Ok(Outcome::Stopped(StopReason::FullReadLimit));
            }
        }
        out.flush().await.map_err(|e| e.to_string())?;
        Ok(Outcome::Complete)
    }

    async fn read_compressed(
        &mut self,
        index: usize,
        gp: &GenerationPlan,
        meta: &mut GenerationMeta,
        out_path: &Path,
    ) -> Result<Outcome, String> {
        self.send(ReadEvent::GenerationStart {
            index,
            distance: gp.distance,
            name: gp.name.clone(),
            first_line_no: None,
        });
        let (start, to) = (self.plan.start.map(|t| t - START_SLACK_SECS), self.plan.to);
        let room = self.plan.limit_lines.saturating_sub(self.lines);
        // Lines already written to the copy when a decode has to start over.
        let mut written = 0usize;
        let mut first_line_no: Option<usize> = None;
        let started = Instant::now();
        loop {
            let session = self.session().await?;
            let sftp = session.open_sftp().await?;
            let file = sftp.open(gp.path.clone()).await.map_err(|e| format!("{}: {e}", gp.name))?;
            let job = DecodeJob {
                file,
                kind: gp.compression,
                size: gp.size,
                name: gp.name.clone(),
                out_path: out_path.to_path_buf(),
                start,
                to,
                room,
                skip_written: written,
                tx: self.tx.clone(),
                ctl: self.ctl.clone(),
                index,
            };
            let handle = tokio::runtime::Handle::current();
            let result = tokio::task::spawn_blocking(move || job.run(handle)).await.map_err(|e| e.to_string())?;
            written = result.written;
            if first_line_no.is_none() {
                first_line_no = result.first_line_no;
            }
            meta.lines = result.written;
            meta.first_line_no = first_line_no;
            match result.end {
                DecodeEnd::Complete => {
                    self.lines += result.written;
                    return Ok(Outcome::Complete);
                }
                DecodeEnd::Cancelled => {
                    self.lines += result.written;
                    return Ok(Outcome::Stopped(StopReason::Cancelled));
                }
                DecodeEnd::Limit => {
                    self.lines += result.written;
                    return Ok(Outcome::Stopped(StopReason::FullReadLimit));
                }
                DecodeEnd::Lost => {
                    if started.elapsed() > GIVE_UP_AFTER {
                        return Err(format!("{}: lost the connection", gp.name));
                    }
                    self.hub.drop_session(&self.plan.server_id).await;
                    // Decode again from the start, skipping what was kept.
                    continue;
                }
                DecodeEnd::Failed(message) => return Err(format!("{}: {message}", gp.name)),
            }
        }
    }
}

enum DecodeEnd {
    Complete,
    Cancelled,
    Limit,
    /// The connection dropped; the caller decodes again.
    Lost,
    Failed(String),
}

struct DecodeResult {
    end: DecodeEnd,
    /// Lines in the copy for this Generation so far (over every attempt).
    written: usize,
    first_line_no: Option<usize>,
}

/// The decoding of one compressed Rotated File, on a blocking thread.
struct DecodeJob {
    file: russh_sftp::client::fs::File,
    kind: Compression,
    size: u64,
    name: String,
    out_path: PathBuf,
    start: Option<f64>,
    to: Option<f64>,
    room: usize,
    /// Lines the copy already holds, which a second attempt does not repeat.
    skip_written: usize,
    tx: Sender<ReadEvent>,
    ctl: Arc<ReadControl>,
    index: usize,
}

impl DecodeJob {
    fn run(self, handle: tokio::runtime::Handle) -> DecodeResult {
        use std::io::{BufRead, Write};
        let counter = Arc::new(AtomicU64::new(0));
        let mut result = DecodeResult { end: DecodeEnd::Complete, written: self.skip_written, first_line_no: None };
        let bridge = CountingRead { inner: AsyncBridge::buffered(handle, self.file), count: counter.clone() };
        let reader = match decoder(self.kind, bridge) {
            Ok(r) => r,
            Err(e) => {
                result.end = DecodeEnd::Failed(e.to_string());
                return result;
            }
        };
        let mut reader = std::io::BufReader::with_capacity(256 * 1024, reader);
        let out = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .append(self.skip_written > 0)
            .truncate(self.skip_written == 0)
            .open(&self.out_path);
        let mut out = match out {
            Ok(f) => std::io::BufWriter::new(f),
            Err(e) => {
                result.end = DecodeEnd::Failed(e.to_string());
                return result;
            }
        };
        let (mut seen_kept, mut line_no) = (0usize, 0usize);
        let mut started = self.start.is_none();
        let mut batch_lines: Vec<String> = Vec::new();
        let mut batch_bytes: Vec<u32> = Vec::new();
        let mut buf = Vec::new();
        let mut last_progress = Instant::now() - Duration::from_secs(1);
        let flush = |lines: &mut Vec<String>, bytes: &mut Vec<u32>, tx: &Sender<ReadEvent>| {
            if !lines.is_empty() {
                let _ = tx.try_send(ReadEvent::Lines { lines: std::mem::take(lines), bytes: std::mem::take(bytes) });
            }
        };
        loop {
            buf.clear();
            let n = match reader.read_until(b'\n', &mut buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(e) => {
                    let _ = out.flush();
                    flush(&mut batch_lines, &mut batch_bytes, &self.tx);
                    // A decoder cannot tell a cut connection from a damaged
                    // file; the file is tried again once.
                    result.end = if e.kind() == io::ErrorKind::InvalidData { DecodeEnd::Failed(e.to_string()) } else { DecodeEnd::Lost };
                    return result;
                }
            };
            line_no += 1;
            let text = decode_line(buf.strip_suffix(b"\n").unwrap_or(&buf));
            let time = line_timestamp(&text);
            if !started {
                match time {
                    Some(t) if t >= self.start.unwrap_or(f64::MIN) => started = true,
                    _ => continue,
                }
            }
            if let (Some(to), Some(t)) = (self.to, time) {
                if t > to {
                    break;
                }
            }
            if result.first_line_no.is_none() {
                result.first_line_no = Some(line_no);
                let _ = self.tx.try_send(ReadEvent::FirstLineNo { index: self.index, first_line_no: Some(line_no) });
            }
            seen_kept += 1;
            if seen_kept <= self.skip_written {
                continue;
            }
            if result.written >= self.room {
                result.end = DecodeEnd::Limit;
                break;
            }
            if out.write_all(text.as_bytes()).and_then(|_| out.write_all(b"\n")).is_err() {
                result.end = DecodeEnd::Failed("could not write the copy".into());
                return result;
            }
            result.written += 1;
            batch_bytes.push(n as u32);
            batch_lines.push(text);
            if batch_lines.len() >= 5_000 {
                flush(&mut batch_lines, &mut batch_bytes, &self.tx);
                if !self.ctl.checkpoint_blocking() {
                    result.end = DecodeEnd::Cancelled;
                    break;
                }
            }
            if last_progress.elapsed() > Duration::from_millis(250) {
                last_progress = Instant::now();
                let _ = self.tx.try_send(ReadEvent::Progress {
                    what: self.name.clone(),
                    done: counter.load(Ordering::Relaxed).min(self.size),
                    total: self.size,
                });
            }
        }
        let _ = out.flush();
        flush(&mut batch_lines, &mut batch_bytes, &self.tx);
        let _ = self.tx.try_send(ReadEvent::Progress { what: self.name.clone(), done: self.size, total: self.size });
        result
    }
}

struct CountingRead<R> {
    inner: R,
    count: Arc<AtomicU64>,
}

impl<R: io::Read> io::Read for CountingRead<R> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(out)?;
        self.count.fetch_add(n as u64, Ordering::Relaxed);
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Counting<R> {
        inner: R,
        reads: usize,
    }

    impl<R: ReadAt> ReadAt for Counting<R> {
        fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> io::Result<usize> {
            self.reads += 1;
            self.inner.read_at(offset, buf)
        }
    }

    fn stamp(secs: u64) -> String {
        let (h, m, s) = (secs / 3600 % 24, secs / 60 % 60, secs % 60);
        format!("2026-10-19 {h:02}:{m:02}:{s:02}")
    }

    /// One line per second, `n` lines, a Continuation Line after every 7th.
    fn steady_log(n: u64) -> (Vec<u8>, Vec<(u64, u64)>) {
        let mut data = Vec::new();
        let mut starts = Vec::new();
        for i in 0..n {
            starts.push((data.len() as u64, i));
            data.extend_from_slice(format!("{} INFO request {i} handled by worker\n", stamp(i)).as_bytes());
            if i % 7 == 0 {
                data.extend_from_slice(b"    at some.frame(Frame.java:1)\n");
            }
        }
        (data, starts)
    }

    fn secs_of(text: &str) -> f64 {
        line_timestamp(&format!("{text} INFO x")).unwrap()
    }

    #[test]
    fn finds_the_exact_line_in_a_large_steady_log() {
        let (data, starts) = steady_log(60_000);
        let size = data.len() as u64;
        for wanted in [0, 1, 17, 30_000, 41_234, 59_999] {
            let mut src = Counting { inner: data.clone(), reads: 0 };
            let at = find_start_offset(&mut src, size, secs_of(&stamp(wanted))).unwrap();
            // One second of slack: the line a second earlier is included.
            let first = wanted.saturating_sub(1);
            let expected = starts.iter().find(|(_, i)| *i == first).unwrap().0;
            assert_eq!(at, expected, "start {wanted}");
            assert!(src.reads < 40, "{} reads", src.reads);
        }
    }

    #[test]
    fn start_past_the_end_is_the_size() {
        let (data, _) = steady_log(1000);
        let size = data.len() as u64;
        let at = find_start_offset(&mut data.clone(), size, secs_of("2026-10-20 00:00:00")).unwrap();
        assert_eq!(at, size);
    }

    #[test]
    fn start_before_the_first_line_is_zero() {
        let (data, _) = steady_log(1000);
        let size = data.len() as u64;
        assert_eq!(find_start_offset(&mut data.clone(), size, secs_of("2026-10-18 00:00:00")).unwrap(), 0);
    }

    #[test]
    fn end_offset_keeps_continuation_lines_of_the_last_line() {
        let (data, starts) = steady_log(20_000);
        let size = data.len() as u64;
        let at = find_end_offset(&mut data.clone(), size, secs_of(&stamp(7_000))).unwrap();
        let next = starts.iter().find(|(_, i)| *i == 7_001).unwrap().0;
        assert_eq!(at, next);
        // Line 7000 is the 1001st multiple of 7: its stack frame is before `at`.
        assert!(String::from_utf8_lossy(&data[..at as usize]).ends_with("(Frame.java:1)\n"));
    }

    #[test]
    fn a_log_that_is_a_few_seconds_out_of_order() {
        // Two writers: every other line is up to 3 seconds behind.
        let mut data = Vec::new();
        let mut starts = Vec::new();
        for i in 0..30_000u64 {
            let skew = if i % 2 == 1 { 3 } else { 0 };
            starts.push((data.len() as u64, i));
            data.extend_from_slice(format!("{} INFO line {i} from a writer\n", stamp(i + 10 - skew.min(i + 10))).as_bytes());
        }
        let size = data.len() as u64;
        let wanted = 12_000;
        let at = find_start_offset(&mut data.clone(), size, secs_of(&stamp(wanted + 10))).unwrap();
        // Lands within a few lines of the exact one, never after the first line at the time.
        let exact = starts.iter().find(|(_, i)| *i == wanted).unwrap().0;
        let a_few = starts.iter().find(|(_, i)| *i == wanted - 8).unwrap().0;
        assert!(at <= exact && at >= a_few, "at {at}, between {a_few} and {exact}");
    }

    #[test]
    fn a_long_stretch_without_times_does_not_stop_the_search() {
        let mut data = Vec::new();
        let mut starts = Vec::new();
        for i in 0..2000u64 {
            starts.push((data.len() as u64, i));
            data.extend_from_slice(format!("{} INFO early {i}\n", stamp(i)).as_bytes());
        }
        // 3 MB of stack trace with no time in it.
        let filler = "    at com.example.Deep.frame(Deep.java:99)\n".repeat(75_000);
        data.extend_from_slice(filler.as_bytes());
        for i in 2000..4000u64 {
            starts.push((data.len() as u64, i));
            data.extend_from_slice(format!("{} INFO late {i}\n", stamp(i)).as_bytes());
        }
        let size = data.len() as u64;
        let at = find_start_offset(&mut data.clone(), size, secs_of(&stamp(3500))).unwrap();
        let exact = starts.iter().find(|(_, i)| *i == 3499).unwrap().0;
        assert_eq!(at, exact);
    }

    #[test]
    fn a_file_with_no_times_gives_the_size() {
        let data = b"just text\nmore text\n".repeat(10_000);
        let size = data.len() as u64;
        assert_eq!(find_start_offset(&mut data.clone(), size, 1e9).unwrap(), size);
    }

    #[test]
    fn a_small_file_and_an_empty_one() {
        let (data, _) = steady_log(10);
        let size = data.len() as u64;
        assert_eq!(find_start_offset(&mut data.clone(), size, secs_of(&stamp(0))).unwrap(), 0);
        assert_eq!(find_start_offset(&mut Vec::<u8>::new(), 0, 5.0).unwrap(), 0);
    }
}
