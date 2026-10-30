//! Logic: the Remote Cache of each Remote File (openspec change
//! `remote-local-cache`, first stage). While a tab is open its Remote Files
//! hold their lines in memory as before; when the tab is left or the app
//! quits, what they hold up to the last Checkpoint is written to the cache.
//! Choosing the tab again, or opening the app, shows those lines at once
//! and reads on from the Server only what was written since (`Resume`).

use super::AppView;
use crate::live::LiveBuffer;
use crate::live::remote::ResumeFrom;
use crate::remote::cache::{self, CacheTarget, Checkpoint, RemoteCache, Snapshot};
use crate::space_view::ParsedFile;
use crate::workspace::log_space::RemoteTarget;
use std::path::PathBuf;

/// Lines a segment of the cache takes (the first stage writes a
/// Generation as one segment).
const SEGMENT_LINES: usize = 100_000;

impl AppView {
    /// The folder of Remote Caches, next to the Remote Copies.
    pub(crate) fn remote_cache_root(&self) -> PathBuf {
        let base = self.remote.picker_state_path.parent().map(std::path::Path::to_path_buf).unwrap_or_default();
        cache::root_in(&base)
    }

    #[doc(hidden)]
    pub fn remote_cache_root_for_test(&self) -> PathBuf {
        self.remote_cache_root()
    }

    /// The cache folder of the Remote File `file_id`: `None` for anything
    /// else (a Remote Command, a local file).
    fn remote_cache_dir(&self, file_id: &str) -> Option<(PathBuf, String, CacheTarget)> {
        let file = self.workspace_state.projects.iter().flat_map(|p| p.log_spaces.iter()).find_map(|s| s.file(file_id))?;
        let remote = file.remote.as_ref()?;
        let RemoteTarget::File { path } = &remote.target else { return None };
        let target = CacheTarget::File { path: path.clone() };
        Some((cache::dir_for(&self.remote_cache_root(), &remote.server_id, &target), remote.server_id.clone(), target))
    }

    /// Write what the Remote File `file_id` holds, up to its last
    /// Checkpoint, to its cache. Nothing is written when the lines do not
    /// line up with the Checkpoint (it is then read anew next time).
    pub(crate) fn snapshot_remote_file(&mut self, file_id: &str) {
        if !self.remote_cache_enabled {
            return;
        }
        let Some((dir, server_id, target)) = self.remote_cache_dir(file_id) else { return };
        let Some(from) = self.live.checkpoints.get(file_id).cloned() else { return };
        let Some(parsed) = self.parsed_files.get(file_id) else { return };
        let Some(buffer) = parsed.live.as_ref() else { return };
        let Some(after) = buffer.lines_after(parsed, from.generation, from.server_end) else { return };
        let keep = parsed.entries.len() - after;
        let mut generations: Vec<Snapshot<'_>> = Vec::new();
        let mut last_generation = None;
        for (i, entry) in parsed.entries[..keep].iter().enumerate() {
            if last_generation != Some(entry.generation) {
                last_generation = Some(entry.generation);
                generations.push(Snapshot {
                    first_line_no: entry.index as u64,
                    lines: Vec::new(),
                    lens: Vec::new(),
                    checkpoint: Checkpoint::default(),
                });
            }
            let snap = generations.last_mut().expect("pushed above");
            snap.lines.push(entry.raw_line.as_ref());
            snap.lens.push(buffer.byte_len(i));
        }
        // Lines with no number yet (the total was not counted) are not kept.
        if generations.iter().any(|g| !crate::live::buffer::is_real_line_number(g.first_line_no as usize)) {
            return;
        }
        if let Some(newest) = generations.last_mut() {
            newest.checkpoint = Checkpoint {
                generation: from.generation,
                inode: from.inode,
                server_end: from.server_end,
                head_hash: Some(from.head_hash),
                tail_hash: Some(from.tail_hash),
            };
        }
        let extended = buffer.hold == crate::live::HoldMode::Extended;
        let written = RemoteCache::open(&dir, &server_id, target, SEGMENT_LINES)
            .and_then(|mut cache| cache.write_snapshot(&generations, extended));
        if let Err(e) = written
            && e.kind() != std::io::ErrorKind::WouldBlock
        {
            self.notice = Some(format!("Could not keep the lines of {file_id}: {e}"));
        }
    }

    /// Write the cache of every Remote File held: the app quits.
    pub(crate) fn snapshot_all_remote_files(&mut self) {
        let ids: Vec<String> = self.parsed_files.keys().filter(|id| self.live.checkpoints.contains_key(*id)).cloned().collect();
        for id in ids {
            self.snapshot_remote_file(&id);
        }
    }

    /// The Remote File `file_id` as its cache kept it, with where its
    /// source reads on from; `None` when it has no cache to show.
    pub(crate) fn remote_file_from_cache(&mut self, file_id: &str, name: &str) -> Option<ParsedFile> {
        if !self.remote_cache_enabled {
            return None;
        }
        let (dir, server_id, target) = self.remote_cache_dir(file_id)?;
        if !cache::has_lines(&dir) {
            return None;
        }
        let cache = RemoteCache::open(&dir, &server_id, target, SEGMENT_LINES).ok()?;
        let extended = cache.meta().extended;
        let read = cache.read_lines().ok()?;
        let newest = read.last()?.0.checkpoint.clone();
        let (Some(head_hash), Some(tail_hash)) = (newest.head_hash, newest.tail_hash) else { return None };
        let generations: Vec<(usize, Vec<String>, Vec<u32>)> =
            read.into_iter().map(|(g, lines, lens)| (g.first_line_no as usize, lines, lens)).collect();
        let mut parsed = ParsedFile::empty();
        let buffer =
            LiveBuffer::restore(name, &mut parsed, &generations, newest.generation, newest.server_end, newest.inode, extended);
        parsed.live = Some(buffer);
        self.live.resume_from.insert(
            file_id.to_string(),
            ResumeFrom {
                generation: newest.generation,
                inode: newest.inode,
                server_end: newest.server_end,
                head_hash,
                tail_hash,
            },
        );
        // Its Markers count Generations from the newest one kept.
        if let Some(project) = self.active_project_mut() {
            for space in &mut project.log_spaces {
                if let Some(f) = space.file_mut(file_id) {
                    f.generation = newest.generation;
                }
            }
        }
        Some(parsed)
    }
}
