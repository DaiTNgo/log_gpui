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

/// What editing a Live Glob does to its files: see `plan_glob_edit`.
pub use crate::workspace::log_space::GlobEditPlan;

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

/// What editing the Live Glob `old` into `new` does to its files.
///
/// `own` are the paths of the files `old` brought. `files` are the Remote
/// Files of the space on `new`'s Server (their `from_glob` is not read: a
/// file is the glob's when it is kept). Files that still match stay; files
/// that stop matching, or are on another Server or in another folder than
/// `new` looks at, go; what `new` matches and the space does not have comes,
/// within `limit`, unless the glob is stopped.
pub fn plan_glob_edit(
    old: &LiveGlob,
    new: &LiveGlob,
    listing: &Listing<'_>,
    own: &[String],
    files: &[SpaceFile],
    read_inodes: &ReadInodes,
    limit: usize,
) -> GlobEditPlan {
    let same_place = old.server_id == new.server_id && old.dir == new.dir;
    let (keep, remove): (Vec<String>, Vec<String>) = own.iter().cloned().partition(|path| {
        same_place
            && !new.excluded.contains(path)
            && glob_match(&new.pattern, path.rsplit('/').next().unwrap_or(""))
    });
    if new.stopped {
        return GlobEditPlan { add: Vec::new(), remove, keep };
    }
    let after: Vec<SpaceFile> = files
        .iter()
        .filter(|f| !remove.contains(&f.path) || old.server_id != new.server_id)
        .map(|f| SpaceFile { from_glob: keep.contains(&f.path), ..f.clone() })
        .collect();
    let add = plan_glob(new, &after, listing, read_inodes, limit).add;
    GlobEditPlan { add, remove, keep }
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

    // ── Editing a Live Glob ────────────────────────────────────────────────

    fn edit_plan(old: &LiveGlob, new: &LiveGlob, own: &[&str], files: &[SpaceFile], entries: &[DirEntry], limit: usize) -> GlobEditPlan {
        let env = Env::new();
        let listing = Listing { entries, inodes: &env.inodes, archives: &env.archives };
        let own: Vec<String> = own.iter().map(|n| path(n)).collect();
        plan_glob_edit(old, new, &listing, &own, files, &env.read, limit)
    }

    fn names(paths: &[String]) -> Vec<String> {
        paths.iter().map(|p| p.rsplit('/').next().unwrap().to_string()).collect()
    }

    #[test]
    fn narrowing_the_pattern_removes_what_stops_matching() {
        let entries = [file("app-1.log", 1), file("app-2.log", 2), file("worker.log", 3)];
        let files = [space_file("app-1.log", true, false), space_file("app-2.log", true, false), space_file("worker.log", true, false)];
        let plan = edit_plan(&glob("*.log"), &glob("app-*.log"), &["app-1.log", "app-2.log", "worker.log"], &files, &entries, 50);
        assert_eq!(names(&plan.remove), ["worker.log"]);
        assert_eq!(names(&plan.keep), ["app-1.log", "app-2.log"]);
        assert!(plan.add.is_empty());
    }

    #[test]
    fn widening_the_pattern_adds_the_new_matches_and_keeps_the_rest() {
        let entries = [file("app-1.log", 1), file("worker.log", 3), file("notes.txt", 4)];
        let files = [space_file("app-1.log", true, false)];
        let plan = edit_plan(&glob("app-*.log"), &glob("*.log"), &["app-1.log"], &files, &entries, 50);
        assert_eq!(names(&plan.keep), ["app-1.log"]);
        assert_eq!(names(&plan.add), ["worker.log"]);
        assert!(plan.remove.is_empty());
    }

    #[test]
    fn another_folder_removes_every_file_and_adds_the_new_folders_matches() {
        let mut new = glob("*.log");
        new.dir = "/var/log/app2".into();
        let entries = [DirEntry { name: "x.log".into(), is_dir: false, size: 1, modified: Some(1) }];
        let files = [space_file("a.log", true, false)];
        let plan = edit_plan(&glob("*.log"), &new, &["a.log"], &files, &entries, 50);
        assert_eq!(names(&plan.remove), ["a.log"]);
        assert!(plan.keep.is_empty());
        assert_eq!(plan.add, ["/var/log/app2/x.log"]);
    }

    #[test]
    fn another_server_keeps_nothing_and_does_not_mistake_a_same_path_file_for_its_own() {
        let mut new = glob("*.log");
        new.server_id = "t".into();
        let entries = [file("a.log", 1), file("b.log", 2)];
        // `files` are the files of the new Server: it already has b.log by path.
        let files = [space_file("b.log", false, false)];
        let plan = edit_plan(&glob("*.log"), &new, &["a.log"], &files, &entries, 50);
        assert_eq!(names(&plan.remove), ["a.log"]);
        assert!(plan.keep.is_empty());
        assert_eq!(names(&plan.add), ["a.log"]);
    }

    #[test]
    fn a_stopped_glob_removes_but_adds_nothing() {
        let mut new = glob("api-*.log");
        new.stopped = true;
        let entries = [file("api-1.log", 1), file("a.log", 2)];
        let files = [space_file("a.log", true, false)];
        let plan = edit_plan(&glob("*.log"), &new, &["a.log"], &files, &entries, 50);
        assert_eq!(names(&plan.remove), ["a.log"]);
        assert!(plan.add.is_empty());
    }

    #[test]
    fn the_limit_counts_the_files_that_are_kept() {
        let entries: Vec<DirEntry> = (0..10).map(|i| file(&format!("f{i}.log"), i)).collect();
        let files: Vec<SpaceFile> = (0..3).map(|i| space_file(&format!("f{i}.log"), true, false)).collect();
        let plan = edit_plan(&glob("f[0-2].log"), &glob("*.log"), &["f0.log", "f1.log", "f2.log"], &files, &entries, 5);
        assert_eq!(plan.keep.len(), 3);
        assert_eq!(plan.add.len(), 2);
    }

    #[test]
    fn a_file_added_by_path_is_not_added_again_by_the_new_glob() {
        let entries = [file("a.log", 1), file("b.log", 2)];
        let files = [space_file("b.log", false, false)];
        let plan = edit_plan(&glob("a*.log"), &glob("*.log"), &[], &files, &entries, 50);
        assert_eq!(names(&plan.add), ["a.log"]);
    }

    #[test]
    fn a_path_the_user_excluded_is_not_kept_or_added() {
        let mut new = glob("*.log");
        new.excluded.push(path("debug.log"));
        let entries = [file("debug.log", 1), file("a.log", 2)];
        let plan = edit_plan(&glob("*.log"), &new, &["a.log"], &[space_file("a.log", true, false)], &entries, 50);
        assert_eq!(names(&plan.keep), ["a.log"]);
        assert!(plan.add.is_empty());
    }
}
