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
    pub lines: usize,
    pub bytes: u64,
}

/// Where the Server's file stood when the lines of a Generation were last
/// read: to tell, when the tab is chosen again, whether it is the same file
/// (decision 5).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checkpoint {
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
                            seg.lines = memchr::memchr_iter(b'\n', &bytes).count();
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
        self.prepend(0, lines)
    }

    /// Add `lines` after the last line of the newest Generation.
    pub fn append(&mut self, lines: &[&str]) -> io::Result<()> {
        if lines.is_empty() {
            return Ok(());
        }
        if self.meta.generations.is_empty() {
            self.begin_generation(1)?;
        }
        let mut rest = lines;
        while !rest.is_empty() {
            let generation = self.meta.generations.last_mut().expect("a Generation");
            let full = generation.segments.last().is_none_or(|s| s.lines >= self.segment_lines);
            if full {
                let id = generation.segments.iter().map(|s| s.id + 1).max().unwrap_or(0);
                generation.segments.push(SegmentMeta { id, lines: 0, bytes: 0 });
            }
            let generation = self.meta.generations.last_mut().expect("a Generation");
            let seg = generation.segments.last_mut().expect("a segment");
            let take = (self.segment_lines - seg.lines.min(self.segment_lines)).max(1).min(rest.len());
            let path = self.dir.join(gen_name(generation.id)).join(seg_name(seg.id));
            seg.bytes += write_lines(&path, &rest[..take], true)?;
            seg.lines += take;
            rest = &rest[take..];
        }
        Ok(())
    }

    /// Add `lines` above the first line of Generation `generation` (its
    /// index): older lines read from the Server. Its first line number
    /// moves up by as many.
    pub fn prepend(&mut self, generation: usize, lines: &[&str]) -> io::Result<()> {
        if lines.is_empty() {
            return Ok(());
        }
        let Some(g) = self.meta.generations.get_mut(generation) else {
            return Err(io::Error::new(io::ErrorKind::NotFound, "no such Generation"));
        };
        let id = g.segments.iter().map(|s| s.id + 1).max().unwrap_or(0);
        let path = self.dir.join(gen_name(g.id)).join(seg_name(id));
        let bytes = write_lines(&path, lines, false)?;
        g.segments.insert(0, SegmentMeta { id, lines: lines.len(), bytes });
        g.first_line_no = g.first_line_no.saturating_sub(lines.len() as u64).max(1);
        self.save()
    }

    /// Drop the oldest segments while `keep` lines or more stay: the Live
    /// Window. An emptied earlier Generation goes too. Returns the lines
    /// dropped.
    pub fn trim_to(&mut self, keep: usize) -> io::Result<usize> {
        let mut total = self.lines();
        let mut dropped = 0;
        while let Some(first) = self.meta.generations.first() {
            let Some(seg) = first.segments.first() else {
                if self.meta.generations.len() == 1 {
                    break;
                }
                let g = self.meta.generations.remove(0);
                let _ = std::fs::remove_dir_all(self.dir.join(gen_name(g.id)));
                continue;
            };
            if total - seg.lines < keep {
                break;
            }
            let (gen_id, seg) = (first.id, seg.clone());
            let _ = std::fs::remove_file(self.dir.join(gen_name(gen_id)).join(seg_name(seg.id)));
            let g = &mut self.meta.generations[0];
            g.segments.remove(0);
            g.first_line_no += seg.lines as u64;
            total -= seg.lines;
            dropped += seg.lines;
        }
        if dropped > 0 {
            self.save()?;
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

    /// Where the newest Generation's file stood on the Server.
    pub fn set_checkpoint(&mut self, checkpoint: Checkpoint) -> io::Result<()> {
        if let Some(g) = self.meta.generations.last_mut() {
            g.checkpoint = checkpoint;
        }
        self.save()
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

pub fn read_meta(dir: &Path) -> io::Result<CacheMeta> {
    let bytes = std::fs::read(dir.join(META_FILE))?;
    serde_json::from_slice(&bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}
