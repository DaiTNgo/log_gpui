//! Remote Cache: the lines a Remote File or a Remote Command received,
//! kept on disk so they are read through a Line Index like a local file
//! and shown again at once when the tab is chosen again (openspec change
//! `remote-local-cache`, decisions 1 and 2).
//!
//! ```text
//! remote-cache/<server id>/<fnv64 of the path>/   (cmd-<fnv64> for a command)
//!   meta.json          the Generations, their segments and checkpoints
//!   lock               the pid of the app using it
//!   gen-<id>/seg-<k>.log
//!   gen-<id>/seg-<k>.lens     length of each line on the Server (u32 LE)
//!   gen-<id>/seg-<k>.stream   a command's output of each line (one byte)
//! ```
//!
//! A Generation is a list of segments read top to bottom. New lines go to
//! the end of its last segment, which is rolled over every `segment_lines`
//! lines; older lines read above go into a new segment at its start; the
//! Live Window cut drops whole segments from the top. No file is rewritten.

use serde::{Deserialize, Serialize};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

pub const META_FILE: &str = "meta.json";
const LOCK_FILE: &str = "lock";

/// The folder of Remote Caches in the data folder `base`.
pub fn root_in(base: &Path) -> PathBuf {
    base.join("remote-cache")
}

/// What a cache holds the lines of.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CacheTarget {
    File { path: String },
    Command { command: String },
}

/// FNV-1a: the same across runs and platforms.
pub fn fnv64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |hash, &b| (hash ^ u64::from(b)).wrapping_mul(0x100000001b3))
}

/// The folder of the cache of `target` on Server `server_id`.
pub fn dir_for(root: &Path, server_id: &str, target: &CacheTarget) -> PathBuf {
    let name = match target {
        CacheTarget::File { path } => format!("{:016x}", fnv64(path.as_bytes())),
        CacheTarget::Command { command } => format!("cmd-{:016x}", fnv64(command.as_bytes())),
    };
    root.join(sanitize(server_id)).join(name)
}

/// A Server id as a folder name.
fn sanitize(id: &str) -> String {
    id.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect()
}

/// One segment of a Generation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SegmentMeta {
    pub id: u32,
    /// Lines held: the lines of its file after the skipped ones.
    pub lines: usize,
    /// Bytes of its file.
    pub bytes: u64,
    /// Lines at the start of its file that left the Live Window, and the
    /// bytes they take: the segment is read from after them.
    #[serde(default)]
    pub skip_lines: usize,
    #[serde(default)]
    pub skip_bytes: u64,
}

/// Where the Server's file stood when the lines of a Generation were last
/// read: to tell, when the tab is chosen again, whether it is the same file
/// (decision 5).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checkpoint {
    /// The Generation as the source counts them.
    #[serde(default)]
    pub generation: u32,
    pub inode: Option<u64>,
    /// Offset in the Server's file after the last line cached.
    pub server_end: u64,
    /// FNV-1a of its first 4 KiB, and of the 4 KiB before `server_end`.
    pub head_hash: Option<u64>,
    pub tail_hash: Option<u64>,
}

/// One Generation: the stretch of the file between two Breaks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerationMeta {
    pub id: u32,
    /// Number of its first line cached, in its file on the Server.
    pub first_line_no: u64,
    pub segments: Vec<SegmentMeta>,
    #[serde(default)]
    pub checkpoint: Checkpoint,
    /// The Rotated File it was read from, for an earlier Generation.
    #[serde(default)]
    pub rotated_name: Option<String>,
}

impl GenerationMeta {
    pub fn lines(&self) -> usize {
        self.segments.iter().map(|s| s.lines).sum()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheMeta {
    pub server_id: String,
    pub target: CacheTarget,
    /// Oldest first.
    pub generations: Vec<GenerationMeta>,
    /// Seconds since the epoch the cache was last opened.
    pub last_opened: u64,
    /// More lines than the Live Window are held (Load more).
    #[serde(default)]
    pub extended: bool,
    /// A Remote Command: when the Run kept here was left (`HH:MM`).
    #[serde(default)]
    pub ended: Option<String>,
}

/// The cache of one Remote File or Remote Command, open for writing.
#[derive(Debug)]
pub struct RemoteCache {
    dir: PathBuf,
    meta: CacheMeta,
    /// Lines a segment takes before the next one starts.
    segment_lines: usize,
}

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn locked() -> io::Error {
    io::Error::new(io::ErrorKind::WouldBlock, "the Remote Cache is used by another app window")
}

/// Whether process `pid` runs.
fn alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        // SAFETY: signal 0 only checks that the process exists; EPERM
        // means it does, run by another user.
        let sent = unsafe { libc::kill(pid as libc::pid_t, 0) } == 0;
        sent || io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        true
    }
}

impl RemoteCache {
    /// Open the cache in `dir`, making it when there is none. A missing or
    /// broken `meta.json` is a cache with no lines. Fails when another app
    /// process holds it.
    pub fn open(dir: &Path, server_id: &str, target: CacheTarget, segment_lines: usize) -> io::Result<Self> {
        std::fs::create_dir_all(dir)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            // Logs of a Server may be private: only the user reads them.
            for d in [dir, dir.parent().unwrap_or(dir)] {
                let _ = std::fs::set_permissions(d, std::fs::Permissions::from_mode(0o700));
            }
        }
        let lock = dir.join(LOCK_FILE);
        if let Ok(text) = std::fs::read_to_string(&lock)
            && let Ok(pid) = text.trim().parse::<u32>()
            && pid != std::process::id()
            && alive(pid)
        {
            return Err(locked());
        }
        std::fs::write(&lock, std::process::id().to_string())?;
        let meta = match read_meta(dir) {
            Ok(meta) if meta.target == target && meta.server_id == server_id => meta,
            _ => CacheMeta {
                server_id: server_id.to_string(),
                target,
                generations: Vec::new(),
                last_opened: 0,
                extended: false,
                ended: None,
            },
        };
        let mut cache = Self { dir: dir.to_path_buf(), meta, segment_lines: segment_lines.max(1) };
        cache.recover()?;
        cache.meta.last_opened = now_secs();
        cache.save()?;
        Ok(cache)
    }

    /// Make `meta.json` agree with the segment files: a segment written to
    /// after the last save is counted again, a missing one dropped.
    fn recover(&mut self) -> io::Result<()> {
        let dir = self.dir.clone();
        for generation in &mut self.meta.generations {
            let gen_dir = dir.join(gen_name(generation.id));
            generation.segments.retain_mut(|seg| {
                let path = gen_dir.join(seg_name(seg.id));
                let Ok(len) = std::fs::metadata(&path).map(|m| m.len()) else { return false };
                if len != seg.bytes {
                    match std::fs::read(&path) {
                        Ok(bytes) => {
                            seg.lines = memchr::memchr_iter(b'\n', &bytes).count().saturating_sub(seg.skip_lines);
                            seg.bytes = len;
                        }
                        Err(_) => return false,
                    }
                }
                true
            });
        }
        Ok(())
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn meta(&self) -> &CacheMeta {
        &self.meta
    }

    /// Lines held, in every Generation.
    pub fn lines(&self) -> usize {
        self.meta.generations.iter().map(GenerationMeta::lines).sum()
    }

    /// Every segment file, top to bottom, with the Generation (its index in
    /// `meta().generations`) it belongs to.
    pub fn segment_paths(&self) -> Vec<(usize, PathBuf)> {
        self.meta
            .generations
            .iter()
            .enumerate()
            .flat_map(|(g, generation)| {
                let gen_dir = self.dir.join(gen_name(generation.id));
                generation.segments.iter().map(move |s| (g, gen_dir.join(seg_name(s.id))))
            })
            .collect()
    }

    /// The lines held as one source, top to bottom, for a Line Index: the
    /// segments end to end, each read through `budget`, and where each
    /// Generation starts in it (its first line's position and number).
    pub fn chain(
        &self,
        budget: std::sync::Arc<crate::index::HandleBudget>,
    ) -> io::Result<(crate::index::ChainSource, Vec<(usize, u64)>)> {
        use crate::index::{ChainSource, DiskSource, LineSource};
        use std::sync::Arc;
        let mut starts = Vec::new();
        let mut at = 0;
        let mut chain: Option<ChainSource> = None;
        for generation in &self.meta.generations {
            starts.push((at, generation.first_line_no));
            at += generation.lines();
            let gen_dir = self.dir.join(gen_name(generation.id));
            for seg in &generation.segments {
                let disk = DiskSource::on_demand(gen_dir.join(seg_name(seg.id)), budget.clone());
                let source: Arc<dyn LineSource> = match seg.skip_bytes {
                    0 => Arc::new(disk),
                    skip => Arc::new(crate::index::source::OffsetSource::new(Arc::new(disk), skip)),
                };
                match &chain {
                    Some(c) => {
                        c.push(source)?;
                    }
                    None => chain = Some(ChainSource::new(source)),
                }
            }
        }
        let chain = chain.unwrap_or_else(|| ChainSource::new(Arc::new(crate::index::BytesSource::new(Vec::new()))));
        Ok((chain, starts))
    }

    /// Write `meta.json`, whole and renamed, so a crash leaves the old one.
    pub fn save(&self) -> io::Result<()> {
        let json = serde_json::to_vec_pretty(&self.meta).map_err(io::Error::other)?;
        let tmp = self.dir.join("meta.json.tmp");
        std::fs::write(&tmp, json)?;
        std::fs::rename(tmp, self.dir.join(META_FILE))
    }

    /// A new Generation below the others, its first line numbered
    /// `first_line_no` on the Server: the file was rotated, cut or written
    /// over, or a command ran again.
    pub fn begin_generation(&mut self, first_line_no: u64) -> io::Result<()> {
        let id = self.meta.generations.iter().map(|g| g.id + 1).max().unwrap_or(0);
        std::fs::create_dir_all(self.dir.join(gen_name(id)))?;
        self.meta.generations.push(GenerationMeta {
            id,
            first_line_no,
            segments: Vec::new(),
            checkpoint: Checkpoint::default(),
            rotated_name: None,
        });
        self.save()
    }

    /// An earlier Generation above all the others, read from the Rotated
    /// File `name`, its first line numbered `first_line_no`.
    pub fn add_earlier_generation(&mut self, name: &str, first_line_no: u64, lines: &[&str]) -> io::Result<()> {
        self.add_earlier_generation_with(name, first_line_no, lines, &[])
    }

    /// `add_earlier_generation`, with the length of each line on the Server.
    pub fn add_earlier_generation_with(&mut self, name: &str, first_line_no: u64, lines: &[&str], lens: &[u32]) -> io::Result<()> {
        let id = self.meta.generations.iter().map(|g| g.id + 1).max().unwrap_or(0);
        std::fs::create_dir_all(self.dir.join(gen_name(id)))?;
        self.meta.generations.insert(
            0,
            GenerationMeta {
                id,
                first_line_no,
                segments: Vec::new(),
                checkpoint: Checkpoint::default(),
                rotated_name: Some(name.to_string()),
            },
        );
        // Its lines' numbers are given: not moved by the prepend.
        self.prepend_with(0, lines, lens)?;
        self.meta.generations[0].first_line_no = first_line_no;
        self.save()
    }

    /// Add `lines` after the last line of the newest Generation.
    pub fn append(&mut self, lines: &[&str]) -> io::Result<()> {
        self.append_with(lines, &[])
    }

    /// `append`, with the length of each line on the Server (kept to find
    /// where lines are in the file there; empty: not known).
    pub fn append_with(&mut self, lines: &[&str], lens: &[u32]) -> io::Result<()> {
        self.append_streams(lines, lens, &[])
    }

    /// `append_with`, with the output of a Remote Command each line came
    /// from (`stream_code`; empty: a file).
    pub fn append_streams(&mut self, lines: &[&str], lens: &[u32], streams: &[u8]) -> io::Result<()> {
        if lines.is_empty() {
            return Ok(());
        }
        if self.meta.generations.is_empty() {
            self.begin_generation(1)?;
        }
        let mut rest = lines;
        while !rest.is_empty() {
            let generation = self.meta.generations.last_mut().expect("a Generation");
            let full = generation.segments.last().is_none_or(|s| s.lines + s.skip_lines >= self.segment_lines);
            if full {
                let id = generation.segments.iter().map(|s| s.id + 1).max().unwrap_or(0);
                generation.segments.push(SegmentMeta { id, lines: 0, bytes: 0, skip_lines: 0, skip_bytes: 0 });
            }
            let generation = self.meta.generations.last_mut().expect("a Generation");
            let seg = generation.segments.last_mut().expect("a segment");
            let used = seg.lines + seg.skip_lines;
            let take = (self.segment_lines - used.min(self.segment_lines)).max(1).min(rest.len());
            let gen_dir = self.dir.join(gen_name(generation.id));
            seg.bytes += write_lines(&gen_dir.join(seg_name(seg.id)), &rest[..take], true)?;
            let done = lines.len() - rest.len();
            write_lens(&gen_dir.join(lens_name(seg.id)), lens.get(done..done + take).unwrap_or(&[]), take, true)?;
            if !streams.is_empty() {
                write_streams(&gen_dir.join(streams_name(seg.id)), streams.get(done..done + take).unwrap_or(&[]), seg.lines + seg.skip_lines, take)?;
            }
            seg.lines += take;
            rest = &rest[take..];
        }
        Ok(())
    }

    /// Add `lines` above the first line of Generation `generation` (its
    /// index): older lines read from the Server. Its first line number
    /// moves up by as many.
    pub fn prepend(&mut self, generation: usize, lines: &[&str]) -> io::Result<()> {
        self.prepend_with(generation, lines, &[])
    }

    /// `prepend`, with the length of each line on the Server.
    pub fn prepend_with(&mut self, generation: usize, lines: &[&str], lens: &[u32]) -> io::Result<()> {
        if lines.is_empty() {
            return Ok(());
        }
        let Some(g) = self.meta.generations.get_mut(generation) else {
            return Err(io::Error::new(io::ErrorKind::NotFound, "no such Generation"));
        };
        let id = g.segments.iter().map(|s| s.id + 1).max().unwrap_or(0);
        let gen_dir = self.dir.join(gen_name(g.id));
        let bytes = write_lines(&gen_dir.join(seg_name(id)), lines, false)?;
        write_lens(&gen_dir.join(lens_name(id)), lens, lines.len(), false)?;
        g.segments.insert(0, SegmentMeta { id, lines: lines.len(), bytes, skip_lines: 0, skip_bytes: 0 });
        g.first_line_no = g.first_line_no.saturating_sub(lines.len() as u64).max(1);
        self.save()
    }

    /// The length on the Server of each line held of Generation
    /// `generation` (its index), top to bottom (0 where not known).
    pub fn lens_of(&self, generation: usize) -> Vec<u32> {
        let Some(g) = self.meta.generations.get(generation) else { return Vec::new() };
        let gen_dir = self.dir.join(gen_name(g.id));
        let mut out = Vec::with_capacity(g.lines());
        for seg in &g.segments {
            let raw = std::fs::read(gen_dir.join(lens_name(seg.id))).unwrap_or_default();
            let mut lens: Vec<u32> = raw.chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
            lens.resize(seg.lines + seg.skip_lines, 0);
            out.extend_from_slice(&lens[seg.skip_lines..]);
        }
        out
    }

    /// The output each line held came from, top to bottom across every
    /// Generation (`stream_code`; 0 where not kept).
    pub fn streams(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.lines());
        for g in &self.meta.generations {
            let gen_dir = self.dir.join(gen_name(g.id));
            for seg in &g.segments {
                let mut codes = std::fs::read(gen_dir.join(streams_name(seg.id))).unwrap_or_default();
                codes.resize(seg.lines + seg.skip_lines, 0);
                out.extend_from_slice(&codes[seg.skip_lines..]);
            }
        }
        out
    }

    /// A Remote Command's Run kept here was left at `time`.
    pub fn set_ended(&mut self, time: Option<String>) -> io::Result<()> {
        self.meta.ended = time;
        self.save()
    }

    /// Drop the oldest `count` lines held: the Live Window. A segment whose
    /// lines all left goes; one with some left is read from after them. An
    /// emptied earlier Generation goes too.
    pub fn drop_top(&mut self, mut count: usize) -> io::Result<()> {
        while count > 0 {
            let Some(g) = self.meta.generations.first_mut() else { break };
            let Some(seg) = g.segments.first_mut() else {
                if self.meta.generations.len() == 1 {
                    break;
                }
                let g = self.meta.generations.remove(0);
                let _ = std::fs::remove_dir_all(self.dir.join(gen_name(g.id)));
                continue;
            };
            let gen_dir = self.dir.join(gen_name(g.id));
            if seg.lines <= count {
                count -= seg.lines;
                g.first_line_no += seg.lines as u64;
                let id = seg.id;
                g.segments.remove(0);
                let _ = std::fs::remove_file(gen_dir.join(seg_name(id)));
                let _ = std::fs::remove_file(gen_dir.join(lens_name(id)));
                let _ = std::fs::remove_file(gen_dir.join(streams_name(id)));
                continue;
            }
            // Some of its lines stay: find where the first of them starts.
            let text = std::fs::read(gen_dir.join(seg_name(seg.id)))?;
            let mut at = seg.skip_bytes as usize;
            for _ in 0..count {
                at = memchr::memchr(b'\n', &text[at..]).map_or(text.len(), |i| at + i + 1);
            }
            seg.skip_bytes = at as u64;
            seg.skip_lines += count;
            seg.lines -= count;
            g.first_line_no += count as u64;
            count = 0;
        }
        // An earlier Generation left empty goes.
        while self.meta.generations.len() > 1 && self.meta.generations[0].lines() == 0 {
            let g = self.meta.generations.remove(0);
            let _ = std::fs::remove_dir_all(self.dir.join(gen_name(g.id)));
        }
        self.save()
    }

    /// Drop the newest `count` lines: lines received after the last
    /// Checkpoint, which the source sends again when it reads on from it.
    pub fn drop_bottom(&mut self, mut count: usize) -> io::Result<()> {
        while count > 0 {
            let Some(g) = self.meta.generations.last_mut() else { break };
            let gen_dir = self.dir.join(gen_name(g.id));
            let Some(seg) = g.segments.last_mut() else { break };
            if seg.lines <= count {
                count -= seg.lines;
                let id = seg.id;
                g.segments.pop();
                let _ = std::fs::remove_file(gen_dir.join(seg_name(id)));
                let _ = std::fs::remove_file(gen_dir.join(lens_name(id)));
                let _ = std::fs::remove_file(gen_dir.join(streams_name(id)));
                continue;
            }
            let path = gen_dir.join(seg_name(seg.id));
            let text = std::fs::read(&path)?;
            // The start of the first of the last `count` lines.
            let mut end = text.len();
            for _ in 0..count {
                end = memchr::memrchr(b'\n', &text[..end.saturating_sub(1)]).map_or(0, |i| i + 1);
            }
            std::fs::OpenOptions::new().write(true).open(&path)?.set_len(end as u64)?;
            let lens = gen_dir.join(lens_name(seg.id));
            let kept = (seg.skip_lines + seg.lines - count) as u64 * 4;
            if let Ok(file) = std::fs::OpenOptions::new().write(true).open(&lens) {
                file.set_len(kept)?;
            }
            if let Ok(file) = std::fs::OpenOptions::new().write(true).open(gen_dir.join(streams_name(seg.id))) {
                file.set_len(kept / 4)?;
            }
            seg.lines -= count;
            seg.bytes = end as u64;
            count = 0;
        }
        self.save()
    }

    /// Keep at most `keep` lines: the oldest go (`drop_top`). Returns the
    /// lines dropped.
    pub fn trim_to(&mut self, keep: usize) -> io::Result<usize> {
        let dropped = self.lines().saturating_sub(keep);
        if dropped > 0 {
            self.drop_top(dropped)?;
        }
        Ok(dropped)
    }

    /// Drop every line: Clear. The newest Generation stays, empty, its next
    /// line numbered after the last one dropped.
    pub fn clear(&mut self) -> io::Result<()> {
        let next = self.meta.generations.last().map(|g| (g.first_line_no + g.lines() as u64, g.checkpoint.clone()));
        for g in std::mem::take(&mut self.meta.generations) {
            let _ = std::fs::remove_dir_all(self.dir.join(gen_name(g.id)));
        }
        if let Some((first_line_no, checkpoint)) = next {
            self.begin_generation(first_line_no)?;
            if let Some(g) = self.meta.generations.last_mut() {
                g.checkpoint = checkpoint;
            }
        }
        self.meta.extended = false;
        self.save()
    }

    /// The number of the first line of Generation `generation` (its
    /// index): known now, or the first line of an empty one.
    pub fn set_first_line_no(&mut self, generation: usize, first_line_no: u64) -> io::Result<()> {
        if let Some(g) = self.meta.generations.get_mut(generation) {
            g.first_line_no = first_line_no;
        }
        self.save()
    }

    /// Where the newest Generation's file stood on the Server.
    pub fn set_checkpoint(&mut self, checkpoint: Checkpoint) -> io::Result<()> {
        if let Some(g) = self.meta.generations.last_mut() {
            g.checkpoint = checkpoint;
        }
        self.save()
    }

    /// Replace what the cache holds with `generations`, oldest first: what
    /// the app holds of the file when its tab is left or the app quits
    /// (the first stage of `remote-local-cache`: the lines stay in memory
    /// while the tab is open, and are kept here for the next time).
    pub fn write_snapshot(&mut self, generations: &[Snapshot<'_>], extended: bool) -> io::Result<()> {
        for g in std::mem::take(&mut self.meta.generations) {
            let _ = std::fs::remove_dir_all(self.dir.join(gen_name(g.id)));
        }
        for (id, snap) in generations.iter().enumerate() {
            let id = id as u32;
            let gen_dir = self.dir.join(gen_name(id));
            std::fs::create_dir_all(&gen_dir)?;
            let bytes = write_lines(&gen_dir.join(seg_name(0)), &snap.lines, false)?;
            let lens: Vec<u8> = snap.lens.iter().flat_map(|l| l.to_le_bytes()).collect();
            std::fs::write(gen_dir.join(lens_name(0)), lens)?;
            self.meta.generations.push(GenerationMeta {
                id,
                first_line_no: snap.first_line_no,
                segments: vec![SegmentMeta { id: 0, lines: snap.lines.len(), bytes, skip_lines: 0, skip_bytes: 0 }],
                checkpoint: snap.checkpoint.clone(),
                rotated_name: None,
            });
        }
        self.meta.extended = extended;
        self.save()
    }

    /// The lines of each Generation, oldest first, with the length each one
    /// had in the file on the Server (empty when not kept).
    pub fn read_lines(&self) -> io::Result<Vec<(GenerationMeta, Vec<String>, Vec<u32>)>> {
        let mut out = Vec::new();
        for generation in &self.meta.generations {
            let gen_dir = self.dir.join(gen_name(generation.id));
            let (mut lines, mut lens) = (Vec::new(), Vec::new());
            for seg in &generation.segments {
                let text = std::fs::read(gen_dir.join(seg_name(seg.id)))?;
                let held = &text[(seg.skip_bytes as usize).min(text.len())..];
                lines.extend(String::from_utf8_lossy(held).lines().map(str::to_string));
                if let Ok(raw) = std::fs::read(gen_dir.join(lens_name(seg.id))) {
                    lens.extend(raw.chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).skip(seg.skip_lines));
                }
            }
            if lens.len() != lines.len() {
                lens.clear();
            }
            out.push((generation.clone(), lines, lens));
        }
        Ok(out)
    }

    pub fn set_extended(&mut self, extended: bool) -> io::Result<()> {
        self.meta.extended = extended;
        self.save()
    }
}

impl Drop for RemoteCache {
    fn drop(&mut self) {
        let _ = self.save();
        let _ = std::fs::remove_file(self.dir.join(LOCK_FILE));
    }
}

/// One Generation of what the app holds, for `RemoteCache::write_snapshot`.
pub struct Snapshot<'a> {
    pub first_line_no: u64,
    pub lines: Vec<&'a str>,
    /// Length of each line in the file on the Server, line end included.
    pub lens: Vec<u32>,
    pub checkpoint: Checkpoint,
}

/// Whether a cache with lines to show is kept in `dir`.
pub fn has_lines(dir: &Path) -> bool {
    read_meta(dir).is_ok_and(|m| m.generations.iter().any(|g| g.lines() > 0))
}

fn lens_name(id: u32) -> String {
    format!("seg-{id}.lens")
}

fn streams_name(id: u32) -> String {
    format!("seg-{id}.stream")
}

/// The byte a line's output is kept as: 0 for none (a file).
pub fn stream_code(stream: Option<crate::model::Stream>) -> u8 {
    match stream {
        None => 0,
        Some(crate::model::Stream::Stdout) => 1,
        Some(crate::model::Stream::Stderr) => 2,
    }
}

pub fn stream_of(code: u8) -> Option<crate::model::Stream> {
    match code {
        1 => Some(crate::model::Stream::Stdout),
        2 => Some(crate::model::Stream::Stderr),
        _ => None,
    }
}

/// Write the outputs of `count` lines to `path` after its first `at`
/// (zeros past `codes`): the file is padded to `at` first, as a segment
/// may have lines from before streams were kept.
fn write_streams(path: &Path, codes: &[u8], at: usize, count: usize) -> io::Result<()> {
    let mut file = std::fs::OpenOptions::new().create(true).write(true).truncate(false).open(path)?;
    let len = file.metadata()?.len() as usize;
    if len != at {
        file.set_len(at as u64)?;
    }
    use std::io::Seek as _;
    file.seek(io::SeekFrom::Start(at as u64))?;
    let mut bytes = codes.to_vec();
    bytes.resize(count, 0);
    file.write_all(&bytes)
}

fn gen_name(id: u32) -> String {
    format!("gen-{id}")
}

fn seg_name(id: u32) -> String {
    format!("seg-{id}.log")
}

/// Write `lines`, each with its line break, to `path`: added at its end, or
/// as a new file. The bytes written.
fn write_lines(path: &Path, lines: &[&str], append: bool) -> io::Result<u64> {
    let mut out = String::with_capacity(lines.iter().map(|l| l.len() + 1).sum());
    for line in lines {
        out.push_str(line);
        out.push('\n');
    }
    let mut file = std::fs::OpenOptions::new().create(true).append(append).write(true).truncate(!append).open(path)?;
    file.write_all(out.as_bytes())?;
    Ok(out.len() as u64)
}

/// Write the lengths of `count` lines to `path` (zeros past `lens`):
/// added at its end, or as a new file.
fn write_lens(path: &Path, lens: &[u32], count: usize, append: bool) -> io::Result<()> {
    let bytes: Vec<u8> = (0..count).flat_map(|i| lens.get(i).copied().unwrap_or(0).to_le_bytes()).collect();
    let mut file = std::fs::OpenOptions::new().create(true).append(append).write(true).truncate(!append).open(path)?;
    file.write_all(&bytes)
}

pub fn read_meta(dir: &Path) -> io::Result<CacheMeta> {
    let bytes = std::fs::read(dir.join(META_FILE))?;
    serde_json::from_slice(&bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

// ── Cleanup (decision 7) ────────────────────────────────────────────────────

/// One cache folder found on disk.
#[derive(Debug, Clone)]
pub struct CacheInfo {
    pub dir: PathBuf,
    pub bytes: u64,
    /// 0 when its `meta.json` is missing or broken.
    pub last_opened: u64,
}

/// Whether another app process uses the cache in `dir` now.
fn used_by_another(dir: &Path) -> bool {
    std::fs::read_to_string(dir.join(LOCK_FILE))
        .ok()
        .and_then(|text| text.trim().parse::<u32>().ok())
        .is_some_and(|pid| pid != std::process::id() && alive(pid))
}

fn size_of(dir: &Path) -> u64 {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| match e.metadata() {
            Ok(m) if m.is_dir() => size_of(&e.path()),
            Ok(m) => m.len(),
            Err(_) => 0,
        })
        .sum()
}

/// Remove the cache in `dir`. Only a folder holding nothing but what a cache
/// writes is removed, so a wrong path never deletes more; a cache another
/// app window uses is left.
pub fn remove(dir: &Path) -> io::Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    if used_by_another(dir) {
        return Err(locked());
    }
    for entry in std::fs::read_dir(dir)? {
        let name = entry?.file_name().to_string_lossy().to_string();
        let ours = name == META_FILE || name == "meta.json.tmp" || name == LOCK_FILE || name.starts_with("gen-");
        if !ours {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, format!("not a Remote Cache folder: {}", dir.display())));
        }
    }
    std::fs::remove_dir_all(dir)
}

/// Every cache folder under `root` (`<server>/<target>`).
pub fn list(root: &Path) -> Vec<CacheInfo> {
    let mut found = Vec::new();
    for server in std::fs::read_dir(root).into_iter().flatten().flatten() {
        let server = server.path();
        if !server.is_dir() {
            continue;
        }
        for entry in std::fs::read_dir(&server).into_iter().flatten().flatten() {
            let dir = entry.path();
            if dir.is_dir() {
                let last_opened = read_meta(&dir).map_or(0, |m| m.last_opened);
                found.push(CacheInfo { bytes: size_of(&dir), dir, last_opened });
            }
        }
    }
    found
}

/// Bytes the caches under `root` take.
pub fn total_size(root: &Path) -> u64 {
    size_of(root)
}

/// Remove the Server folders left empty.
fn remove_empty_servers(root: &Path) {
    for server in std::fs::read_dir(root).into_iter().flatten().flatten() {
        let _ = std::fs::remove_dir(server.path());
    }
}

/// Remove the caches under `root` no file refers to (`in_use`), and those
/// last opened before `opened_before` (seconds since the epoch). Returns
/// how many.
pub fn sweep(root: &Path, in_use: &std::collections::HashSet<PathBuf>, opened_before: Option<u64>) -> usize {
    let mut removed = 0;
    for info in list(root) {
        let stale = opened_before.is_some_and(|t| info.last_opened < t);
        if (!in_use.contains(&info.dir) || stale) && remove(&info.dir).is_ok() {
            removed += 1;
        }
    }
    remove_empty_servers(root);
    removed
}

/// What `trim_to_size` did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Trimmed {
    pub removed: usize,
    /// Bytes the caches take after.
    pub bytes: u64,
}

/// Remove caches under `root`, the one opened longest ago first, until
/// they take at most `limit` bytes. The caches in `keep` (files of open
/// tabs) and those another app window uses are never removed.
pub fn trim_to_size(root: &Path, limit: u64, keep: &std::collections::HashSet<PathBuf>) -> Trimmed {
    let mut caches = list(root);
    let mut bytes: u64 = caches.iter().map(|c| c.bytes).sum();
    let mut removed = 0;
    caches.sort_by_key(|c| c.last_opened);
    for info in caches {
        if bytes <= limit {
            break;
        }
        if keep.contains(&info.dir) {
            continue;
        }
        if remove(&info.dir).is_ok() {
            bytes = bytes.saturating_sub(info.bytes);
            removed += 1;
        }
    }
    if removed > 0 {
        remove_empty_servers(root);
    }
    Trimmed { removed, bytes }
}

