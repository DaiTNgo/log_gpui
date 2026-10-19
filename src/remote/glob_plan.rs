//! Deciding what a Live Glob does after its folder was listed: which files
//! to add, which of its files are gone, and how many matching files wait
//! because of the file limit. Pure, so it needs no Server to be tested.

use super::browse::{DirEntry, glob_match};
use super::rotated::is_rotated_file;
use crate::workspace::log_space::LiveGlob;
use std::collections::{HashMap, HashSet};

/// The inodes a Live Glob has read, with the path each was read at.
pub type ReadInodes = HashMap<u64, String>;

/// A Remote File of the space on the Live Glob's Server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpaceFile {
    pub path: String,
    /// Brought by this Live Glob.
    pub from_glob: bool,
    /// Its source says the file is gone.
    pub gone: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GlobPlan {
    /// Paths to add as Remote Files, newest modified first.
    pub add: Vec<String>,
    /// Paths of its files that are no longer in the folder.
    pub gone: Vec<String>,
    /// Matching files not read because of the limit.
    pub pending: usize,
}

/// What the watcher learned about the folder.
#[derive(Debug, Clone)]
pub struct Listing<'a> {
    pub entries: &'a [DirEntry],
    /// Inode by file name, for the names the watcher could ask about.
    pub inodes: &'a HashMap<String, u64>,
    /// Names that are compressed files or Archives by their first bytes.
    pub archives: &'a HashSet<String>,
}

/// Names of `listing` that the glob matches and that are real candidates:
/// files, not excluded, not already in the space, not a Rotated File, not an
/// Archive, and not a file this glob already read under another name.
pub fn candidates<'a>(
    glob: &LiveGlob,
    files: &[SpaceFile],
    listing: &Listing<'a>,
    read_inodes: &ReadInodes,
) -> Vec<&'a DirEntry> {
    let matching: HashSet<&str> = listing
        .entries
        .iter()
        .filter(|e| !e.is_dir && glob_match(&glob.pattern, &e.name))
        .map(|e| e.name.as_str())
        .collect();
    let in_space = |name: &str| {
        let path = glob.path_of(name);
        files.iter().any(|f| f.path == path)
    };
    listing
        .entries
        .iter()
        .filter(|e| matching.contains(e.name.as_str()))
        .filter(|e| !glob.excluded.contains(&glob.path_of(&e.name)))
        .filter(|e| !in_space(&e.name))
        .filter(|e| !is_rotated_file(&e.name, |base| matching.contains(base) || in_space(base)))
        .filter(|e| !listing.archives.contains(&e.name))
        .filter(|e| {
            // Read before, at another path: the same file under a new name.
            let path = glob.path_of(&e.name);
            !listing.inodes.get(&e.name).is_some_and(|i| read_inodes.get(i).is_some_and(|at| *at != path))
        })
        .collect()
}

/// The plan for one Live Glob.
pub fn plan_glob(
    glob: &LiveGlob,
    files: &[SpaceFile],
    listing: &Listing<'_>,
    read_inodes: &ReadInodes,
    limit: usize,
) -> GlobPlan {
    let present: HashSet<String> = listing
        .entries
        .iter()
        .filter(|e| !e.is_dir)
        .map(|e| glob.path_of(&e.name))
        .collect();
    let gone: Vec<String> = files
        .iter()
        .filter(|f| f.from_glob && !f.gone && !present.contains(&f.path))
        .map(|f| f.path.clone())
        .collect();
    if glob.stopped {
        return GlobPlan { add: Vec::new(), gone, pending: 0 };
    }
    let active = files
        .iter()
        .filter(|f| f.from_glob && !f.gone && !gone.contains(&f.path))
        .count();
    let mut found = candidates(glob, files, listing, read_inodes);
    // Newest first; a file with no time comes last.
    found.sort_by(|a, b| b.modified.cmp(&a.modified).then_with(|| a.name.cmp(&b.name)));
    let room = limit.saturating_sub(active);
    let pending = found.len().saturating_sub(room);
    found.truncate(room);
    GlobPlan { add: found.iter().map(|e| glob.path_of(&e.name)).collect(), gone, pending }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn glob(pattern: &str) -> LiveGlob {
        LiveGlob {
            glob_id: "glob-1".into(),
            server_id: "s".into(),
            dir: "/var/log/app".into(),
            pattern: pattern.into(),
            excluded: Vec::new(),
            stopped: false,
        }
    }

    fn file(name: &str, modified: u64) -> DirEntry {
        DirEntry { name: name.into(), is_dir: false, size: 10, modified: Some(modified) }
    }

    fn dir(name: &str) -> DirEntry {
        DirEntry { name: name.into(), is_dir: true, size: 0, modified: Some(1) }
    }

    fn path(name: &str) -> String {
        format!("/var/log/app/{name}")
    }

    fn space_file(name: &str, from_glob: bool, gone: bool) -> SpaceFile {
        SpaceFile { path: path(name), from_glob, gone }
    }

    struct Env {
        inodes: HashMap<String, u64>,
        archives: HashSet<String>,
        read: ReadInodes,
    }

    impl Env {
        fn new() -> Self {
            Self { inodes: HashMap::new(), archives: HashSet::new(), read: ReadInodes::new() }
        }

        fn plan(&self, glob: &LiveGlob, files: &[SpaceFile], entries: &[DirEntry], limit: usize) -> GlobPlan {
            let listing = Listing { entries, inodes: &self.inodes, archives: &self.archives };
            plan_glob(glob, files, &listing, &self.read, limit)
        }
    }

    #[test]
    fn adds_matching_files_not_folders() {
        let entries = [file("a.log", 1), file("b.log", 2), file("c.txt", 3), dir("d.log")];
        let plan = Env::new().plan(&glob("*.log"), &[], &entries, 50);
        assert_eq!(plan.add, [path("b.log"), path("a.log")]);
        assert_eq!(plan.pending, 0);
    }

    #[test]
    fn skips_files_already_in_the_space() {
        let entries = [file("a.log", 1), file("b.log", 2)];
        let files = [space_file("a.log", false, false)];
        let plan = Env::new().plan(&glob("*.log"), &files, &entries, 50);
        assert_eq!(plan.add, [path("b.log")]);
    }

    #[test]
    fn skips_excluded_paths() {
        let mut g = glob("*.log");
        g.excluded.push(path("debug.log"));
        let entries = [file("debug.log", 5), file("a.log", 1)];
        let plan = Env::new().plan(&g, &[], &entries, 50);
        assert_eq!(plan.add, [path("a.log")]);
    }

    #[test]
    fn empty_match_adds_nothing() {
        let entries = [file("other.txt", 1)];
        let plan = Env::new().plan(&glob("worker-*.log"), &[], &entries, 50);
        assert_eq!(plan, GlobPlan::default());
    }

    #[test]
    fn rotated_files_are_not_added() {
        let entries = [file("app.log", 5), file("app.log.1", 4), file("app.log.2.gz", 3)];
        let plan = Env::new().plan(&glob("app.log*"), &[], &entries, 50);
        assert_eq!(plan.add, [path("app.log")]);
    }

    #[test]
    fn rotated_file_of_an_existing_remote_file_is_not_added() {
        let entries = [file("app.log.1", 4)];
        let files = [space_file("app.log", false, false)];
        let plan = Env::new().plan(&glob("app.log*"), &files, &entries, 50);
        assert!(plan.add.is_empty());
    }

    #[test]
    fn dated_names_that_are_not_rotations_are_added() {
        let entries = [file("app-2026-10-19.log", 1), file("app-2026-10-20.log", 2)];
        let plan = Env::new().plan(&glob("app-*.log"), &[], &entries, 50);
        assert_eq!(plan.add.len(), 2);
    }

    #[test]
    fn a_file_read_under_another_name_is_not_added() {
        let mut env = Env::new();
        env.inodes.insert("old-batch.log".into(), 77);
        env.read.insert(77, path("current.log"));
        let entries = [file("old-batch.log", 9), file("current.log", 10)];
        let plan = env.plan(&glob("*.log"), &[space_file("current.log", true, false)], &entries, 50);
        assert!(plan.add.is_empty());
    }

    #[test]
    fn unknown_inode_counts_as_new() {
        let mut env = Env::new();
        env.read.insert(77, path("y.log"));
        let entries = [file("x.log", 1)];
        assert_eq!(env.plan(&glob("*.log"), &[], &entries, 50).add, [path("x.log")]);
    }

    #[test]
    fn a_file_read_at_its_own_path_is_not_a_copy() {
        let mut env = Env::new();
        env.inodes.insert("x.log".into(), 5);
        env.read.insert(5, path("x.log"));
        let entries = [file("x.log", 1)];
        assert_eq!(env.plan(&glob("*.log"), &[], &entries, 50).add, [path("x.log")]);
    }

    #[test]
    fn archives_are_skipped() {
        let mut env = Env::new();
        env.archives.insert("dump.tar.gz".into());
        let entries = [file("app.log", 1), file("dump.tar.gz", 2)];
        assert_eq!(env.plan(&glob("*"), &[], &entries, 50).add, [path("app.log")]);
    }

    #[test]
    fn limit_keeps_the_newest_and_counts_the_rest() {
        let entries: Vec<DirEntry> = (0..80).map(|i| file(&format!("f{i:02}.log"), i)).collect();
        let plan = Env::new().plan(&glob("*.log"), &[], &entries, 50);
        assert_eq!(plan.add.len(), 50);
        assert_eq!(plan.add[0], path("f79.log"));
        assert_eq!(plan.pending, 30);
    }

    #[test]
    fn raised_limit_fills_the_room() {
        let entries: Vec<DirEntry> = (0..80).map(|i| file(&format!("f{i:02}.log"), i)).collect();
        let files: Vec<SpaceFile> = (30..80).map(|i| space_file(&format!("f{i:02}.log"), true, false)).collect();
        let plan = Env::new().plan(&glob("*.log"), &files, &entries, 60);
        assert_eq!(plan.add.len(), 10);
        assert_eq!(plan.add[0], path("f29.log"));
        assert_eq!(plan.pending, 20);
    }

    #[test]
    fn lowered_limit_keeps_files_and_adds_none() {
        let entries: Vec<DirEntry> = (0..60).map(|i| file(&format!("f{i:02}.log"), i)).collect();
        let files: Vec<SpaceFile> = (10..60).map(|i| space_file(&format!("f{i:02}.log"), true, false)).collect();
        let plan = Env::new().plan(&glob("*.log"), &files, &entries, 20);
        assert!(plan.add.is_empty());
        assert_eq!(plan.pending, 10);
        assert!(plan.gone.is_empty());
    }

    #[test]
    fn gone_files_do_not_take_room() {
        let entries = [file("new.log", 9)];
        let files = [space_file("a.log", true, true), space_file("b.log", true, false)];
        let plan = Env::new().plan(&glob("*.log"), &files, &entries, 2);
        // b.log vanished from the folder: gone, so there is room for two.
        assert_eq!(plan.gone, [path("b.log")]);
        assert_eq!(plan.add, [path("new.log")]);
    }

    #[test]
    fn files_missing_from_the_folder_become_gone() {
        let entries = [file("a.log", 1)];
        let files = [space_file("a.log", true, false), space_file("b.log", true, false), space_file("c.log", false, false)];
        let plan = Env::new().plan(&glob("*.log"), &files, &entries, 50);
        assert_eq!(plan.gone, [path("b.log")]);
    }

    #[test]
    fn a_stopped_glob_adds_nothing_and_reports_nothing_pending() {
        let mut g = glob("*.log");
        g.stopped = true;
        let entries = [file("a.log", 1), file("b.log", 2)];
        let files = [space_file("old.log", true, false)];
        let plan = Env::new().plan(&g, &files, &entries, 50);
        assert!(plan.add.is_empty());
        assert_eq!(plan.pending, 0);
        // It still tells which files are gone.
        assert_eq!(plan.gone, [path("old.log")]);
    }

    #[test]
    fn empty_files_are_added() {
        let entries = [DirEntry { name: "new.log".into(), is_dir: false, size: 0, modified: Some(1) }];
        assert_eq!(Env::new().plan(&glob("*.log"), &[], &entries, 50).add, [path("new.log")]);
    }
}
