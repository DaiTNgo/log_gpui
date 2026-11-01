//! Archive Entries unpacked to disk (openspec change `line-offset-index`,
//! group 10): written once to the archive cache, read like local files,
//! removed with their Log Space and swept at start.

mod common;

use gpui::TestAppContext;
use log_gpui::archive::{ArchiveError, ArchiveKind, detect};
use log_gpui::archive_cache::{dir_of, entry_path, remove_entry, sweep, unpack};
use log_gpui::workspace::log_space::{ImportSource, LogSpaceKind};
use std::collections::HashSet;
use std::io::Write;
use std::path::Path;

fn tar_gz(path: &Path, files: &[(&str, &[u8])]) {
    let mut builder = tar::Builder::new(Vec::new());
    for (name, content) in files {
        let mut header = tar::Header::new_gnu();
        header.set_size(content.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append_data(&mut header, name, *content).unwrap();
    }
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(&builder.into_inner().unwrap()).unwrap();
    std::fs::write(path, encoder.finish().unwrap()).unwrap();
}

fn zip(path: &Path, files: &[(&str, &[u8])]) {
    let mut writer = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
    for (name, content) in files {
        writer.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
        writer.write_all(content).unwrap();
    }
    writer.finish().unwrap();
}

#[test]
fn entries_unpack_in_one_pass_and_once() {
    let d = tempfile::tempdir().unwrap();
    let (root, archive) = (d.path().join("cache"), d.path().join("logs.tar.gz"));
    tar_gz(&archive, &[("a.log", b"one\n"), ("dir/b.log", b"two\n"), ("c.log", b"three\n")]);
    let kind = detect(&archive).unwrap();
    let out = unpack(&root, &archive, kind, &["a.log", "dir/b.log", "missing.log"], 1 << 20).unwrap();
    let a = out["a.log"].as_ref().unwrap();
    assert_eq!(std::fs::read(a).unwrap(), b"one\n");
    assert_eq!(std::fs::read(out["dir/b.log"].as_ref().unwrap()).unwrap(), b"two\n");
    assert!(matches!(out["missing.log"], Err(ArchiveError::NotFound)));
    assert!(!dir_of(&root, &archive).unwrap().join("c.log").exists(), "only what was asked for");

    // Asked again, the file on disk is the answer: nothing is unpacked.
    std::fs::write(a, b"kept\n").unwrap();
    let again = unpack(&root, &archive, kind, &["a.log"], 1 << 20).unwrap();
    assert_eq!(std::fs::read(again["a.log"].as_ref().unwrap()).unwrap(), b"kept\n");
}

#[test]
fn zip_and_a_lone_gz_unpack_too() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("cache");
    let z = d.path().join("b.zip");
    zip(&z, &[("x.log", b"zipped\n")]);
    let out = unpack(&root, &z, ArchiveKind::Zip, &["x.log"], 1 << 20).unwrap();
    assert_eq!(std::fs::read(out["x.log"].as_ref().unwrap()).unwrap(), b"zipped\n");

    let gz = d.path().join("app.log.gz");
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(b"gzipped\n").unwrap();
    std::fs::write(&gz, encoder.finish().unwrap()).unwrap();
    let out = unpack(&root, &gz, ArchiveKind::Gz, &["app.log"], 1 << 20).unwrap();
    assert_eq!(std::fs::read(out["app.log"].as_ref().unwrap()).unwrap(), b"gzipped\n");
}

#[test]
fn an_entry_over_the_limit_leaves_nothing() {
    let d = tempfile::tempdir().unwrap();
    let (root, archive) = (d.path().join("cache"), d.path().join("big.tar.gz"));
    tar_gz(&archive, &[("big.log", &[b'x'; 4096])]);
    let out = unpack(&root, &archive, ArchiveKind::TarGz, &["big.log"], 1024).unwrap();
    assert!(matches!(out["big.log"], Err(ArchiveError::TooLarge)));
    let dir = dir_of(&root, &archive).unwrap();
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0, "no file, not even a part");
}

#[test]
fn a_changed_archive_unpacks_again() {
    let d = tempfile::tempdir().unwrap();
    let (root, archive) = (d.path().join("cache"), d.path().join("logs.tar.gz"));
    tar_gz(&archive, &[("a.log", b"old\n")]);
    let before = dir_of(&root, &archive).unwrap();
    unpack(&root, &archive, ArchiveKind::TarGz, &["a.log"], 1 << 20).unwrap();
    tar_gz(&archive, &[("a.log", b"newer content\n")]);
    assert_ne!(dir_of(&root, &archive).unwrap(), before);
    let out = unpack(&root, &archive, ArchiveKind::TarGz, &["a.log"], 1 << 20).unwrap();
    assert_eq!(std::fs::read(out["a.log"].as_ref().unwrap()).unwrap(), b"newer content\n");
}

#[test]
fn the_sweep_clears_cut_off_entries_and_unused_folders() {
    let d = tempfile::tempdir().unwrap();
    let (root, archive) = (d.path().join("cache"), d.path().join("logs.tar.gz"));
    tar_gz(&archive, &[("a.log", b"one\n")]);
    unpack(&root, &archive, ArchiveKind::TarGz, &["a.log"], 1 << 20).unwrap();
    let dir = dir_of(&root, &archive).unwrap();
    // The app closed while an entry was being written, and an old folder
    // no Log Space uses.
    std::fs::write(dir.join("0000000000000000.part"), b"cut").unwrap();
    std::fs::create_dir_all(root.join("ffffffffffffffff")).unwrap();
    let in_use: HashSet<_> = [dir.clone()].into();
    assert_eq!(sweep(&root, &in_use), 2);
    assert!(entry_path(&dir, "a.log").exists(), "a whole entry in use stays");
    assert!(!root.join("ffffffffffffffff").exists());
    remove_entry(&root, &archive, "a.log");
    assert!(!dir.exists(), "the folder goes with its last entry");
}

#[gpui::test]
fn an_archive_entry_is_read_from_its_unpacked_file_and_goes_with_its_space(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    let archive = h.dir.path().join("logs.tar.gz");
    tar_gz(&archive, &[("app.log", b"2026-09-05 10:00:00 [INFO] [a] one\n2026-09-05 10:00:01 [WARN] [a] two\n")]);
    let source = ImportSource::Entry { archive: archive.to_string_lossy().into_owned(), entry: "app.log".into() };
    h.view.update(&mut h.cx, |app, cx| {
        app.create_log_space("Bundle", LogSpaceKind::Static, &[source], cx);
    });
    h.cx.run_until_parked();
    let (root, space_id) = h.view.read_with(&h.cx, |app, _| {
        assert_eq!(app.view.len(), 2);
        let project = app.workspace_state.active_project().unwrap();
        let space = project.working_set.active_tab().unwrap().target.space_id().unwrap().to_string();
        (app.archive_cache_root_for_test(), space)
    });
    let unpacked = entry_path(&dir_of(&root, &archive).unwrap(), "app.log");
    assert!(unpacked.is_file(), "the entry was unpacked to disk");

    h.view.update(&mut h.cx, |app, cx| {
        app.request_delete_log_space(&space_id, cx);
        app.accept_confirm(cx);
    });
    assert!(!unpacked.exists(), "deleting the Log Space removes the unpacked file");
}

// memory-budget, group 5: unpacked in the background, counted, cancellable.

#[test]
fn an_unpacking_counts_the_archive_read_and_stops_when_cancelled() {
    use log_gpui::archive_cache::{Unpacking, unpack_with};
    use std::sync::atomic::Ordering;
    let d = tempfile::tempdir().unwrap();
    let (root, archive) = (d.path().join("cache"), d.path().join("logs.tar.gz"));
    let big: Vec<u8> = (0..200_000).flat_map(|i| format!("2026-09-05 10:00:00 [INFO] line {i}\n").into_bytes()).collect();
    tar_gz(&archive, &[("a.log", &big), ("b.log", b"two\n")]);

    let how = Unpacking::default();
    unpack_with(&root, &archive, ArchiveKind::TarGz, &["a.log"], 1 << 30, &how).unwrap();
    let size = std::fs::metadata(&archive).unwrap().len();
    let read = how.read.load(Ordering::Relaxed);
    assert!(read > 0 && read <= size + 64 * 1024, "{read} of {size} bytes read");

    // Cancelled: nothing of it is kept, no `.part` left.
    let how = Unpacking::default();
    how.cancel.store(true, Ordering::Release);
    let out = unpack_with(&root, &archive, ArchiveKind::TarGz, &["b.log"], 1 << 30, &how);
    assert!(matches!(out, Err(ArchiveError::Cancelled)));
    let dir = dir_of(&root, &archive).unwrap();
    assert_eq!(parts_in(&dir), 0);
    assert!(!entry_path(&dir, "b.log").exists());

    // An entry unpacked before needs no read of the Archive at all.
    let how = Unpacking::default();
    unpack_with(&root, &archive, ArchiveKind::TarGz, &["a.log"], 1 << 30, &how).unwrap();
    assert_eq!(how.read.load(Ordering::Relaxed), 0);
}

/// The `.part` files in an Archive's folder: entries cut off.
fn parts_in(dir: &Path) -> usize {
    std::fs::read_dir(dir).map_or(0, |d| d.filter(|e| e.as_ref().unwrap().path().extension().is_some_and(|x| x == "part")).count())
}

fn bundle(h: &mut common::Harness, archive: &Path, entries: &[&str]) {
    let sources: Vec<ImportSource> = entries
        .iter()
        .map(|e| ImportSource::Entry { archive: archive.to_string_lossy().into_owned(), entry: (*e).into() })
        .collect();
    h.view.update(&mut h.cx, |app, cx| {
        app.create_log_space("Bundle", LogSpaceKind::Static, &sources, cx);
    });
}

#[gpui::test]
fn archive_entries_are_unpacked_and_read_in_the_background(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    let archive = h.dir.path().join("logs.tar.gz");
    tar_gz(&archive, &[
        ("a.log", b"2026-09-05 10:00:00 [INFO] [a] one\n"),
        ("b.log", b"2026-09-05 10:00:01 [INFO] [b] two\n"),
        ("c.log", b"2026-09-05 10:00:02 [INFO] [c] three\n"),
    ]);
    bundle(&mut h, &archive, &["a.log", "b.log", "c.log"]);
    h.view.read_with(&h.cx, |app, _| {
        assert!(app.is_reading(), "unpacked in the background");
        assert!(app.parsed_files.is_empty(), "nothing read on the UI thread");
        assert!(app.loads.values().all(|l| l.is_unpacking()));
    });
    // Asked again meanwhile: no second unpacking.
    h.view.update(&mut h.cx, |app, cx| app.rebuild_open_view_keeping_position(cx));
    h.view.read_with(&h.cx, |app, _| assert!(app.archive_queue.is_empty()));
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| {
        assert_eq!(app.view.len(), 3);
        assert!(!app.is_reading());
        assert!(app.notice.is_none(), "{:?}", app.notice);
    });
}

#[gpui::test]
fn a_damaged_archive_reads_as_empty_with_a_notice(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    let archive = h.dir.path().join("logs.tar.gz");
    std::fs::write(&archive, b"\x1f\x8bnot really gzip").unwrap();
    bundle(&mut h, &archive, &["a.log"]);
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| {
        assert_eq!(app.view.len(), 0);
        assert!(app.notice.is_some(), "the failure is told");
        assert!(!app.is_reading());
    });
}

#[gpui::test]
fn leaving_the_tab_stops_the_unpacking(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    let archive = h.dir.path().join("logs.tar.gz");
    tar_gz(&archive, &[("a.log", b"2026-09-05 10:00:00 [INFO] [a] one\n")]);
    bundle(&mut h, &archive, &["a.log"]);
    let other = h.dir.path().join("other.log");
    std::fs::write(&other, "2026-09-05 10:00:00 [INFO] other\n").unwrap();
    let source = ImportSource::Disk(other.to_string_lossy().into_owned());
    h.view.update(&mut h.cx, |app, cx| app.create_log_space("Other", LogSpaceKind::Static, &[source], cx));
    h.view.read_with(&h.cx, |app, _| {
        assert!(app.loads.values().all(|l| !l.is_unpacking()), "the Archive's read went with its tab");
    });
    h.cx.run_until_parked();
    let root = h.view.read_with(&h.cx, |app, _| app.archive_cache_root_for_test());
    assert_eq!(parts_in(&dir_of(&root, &archive).unwrap()), 0, "no part of an entry is left");
}
