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
        app.create_log_space("Bundle", LogSpaceKind::Imported, &[source], cx);
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
