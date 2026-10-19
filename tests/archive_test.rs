use log_gpui::archive::{
    ArchiveError, ArchiveKind, FileSource, detect, list, read_entries, read_source,
};
use log_gpui::workspace::log_space::{ImportSource, LogFile, LogSpace, LogSpaceKind};
use log_gpui::workspace::new_space::{NewLogSpaceForm, PendingStatus};
use std::io::Write;
use std::path::{Path, PathBuf};

fn zip_with(path: &Path, files: &[(&str, &[u8])]) {
    let mut writer = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, content) in files {
        if name.ends_with('/') {
            writer.add_directory(*name, options).unwrap();
        } else {
            writer.start_file(*name, options).unwrap();
            writer.write_all(content).unwrap();
        }
    }
    writer.finish().unwrap();
}

fn tar_bytes(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());
    for (name, content) in files {
        let mut header = tar::Header::new_gnu();
        header.set_size(content.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append_data(&mut header, name, *content).unwrap();
    }
    builder.into_inner().unwrap()
}

fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap()
}

fn dir() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

fn path_str(p: &Path) -> String {
    p.to_string_lossy().to_string()
}

#[test]
fn detection_goes_by_content_not_by_extension() {
    let d = dir();
    let zip = d.path().join("bundle.dat");
    zip_with(&zip, &[("a.log", b"x\n")]);
    assert_eq!(detect(&zip), Some(ArchiveKind::Zip));

    let tar = d.path().join("t.bin");
    std::fs::write(&tar, tar_bytes(&[("a.log", b"x\n")])).unwrap();
    assert_eq!(detect(&tar), Some(ArchiveKind::Tar));

    let tgz = d.path().join("t.tgz");
    std::fs::write(&tgz, gzip(&tar_bytes(&[("a.log", b"x\n")]))).unwrap();
    assert_eq!(detect(&tgz), Some(ArchiveKind::TarGz));

    let gz = d.path().join("app.log.1.gz");
    std::fs::write(&gz, gzip(b"line\n")).unwrap();
    assert_eq!(detect(&gz), Some(ArchiveKind::Gz));

    let seven = d.path().join("logs.7z");
    std::fs::write(&seven, b"7z\xBC\xAF\x27\x1C rest").unwrap();
    assert_eq!(detect(&seven), None);
    let text = d.path().join("a.log");
    std::fs::write(&text, "hello\n").unwrap();
    assert_eq!(detect(&text), None);
    let empty = d.path().join("empty");
    std::fs::write(&empty, "").unwrap();
    assert_eq!(detect(&empty), None);
}

#[test]
fn zip_lists_only_regular_files() {
    let d = dir();
    let zip = d.path().join("bundle.zip");
    zip_with(
        &zip,
        &[
            ("app.log", b"one\n"),
            ("nginx/", b""),
            ("nginx/access.log", b"two\n"),
            ("__MACOSX/._app.log", b"junk"),
            (".DS_Store", b"junk"),
            ("nginx/.DS_Store", b"junk"),
        ],
    );
    let entries = list(&zip, ArchiveKind::Zip).unwrap();
    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["app.log", "nginx/access.log"]);
    assert_eq!(entries[0].size, 4);
    assert_eq!(entries[0].head, b"one\n");
}

#[test]
fn tar_and_tar_gz_list_and_read() {
    let d = dir();
    let files: &[(&str, &[u8])] = &[("a.log", b"alpha\n"), ("sub/b.log", b"beta\n")];
    let tar = d.path().join("t.tar");
    std::fs::write(&tar, tar_bytes(files)).unwrap();
    let tgz = d.path().join("t.tar.gz");
    std::fs::write(&tgz, gzip(&tar_bytes(files))).unwrap();

    for (path, kind) in [(&tar, ArchiveKind::Tar), (&tgz, ArchiveKind::TarGz)] {
        let names: Vec<String> = list(path, kind).unwrap().into_iter().map(|e| e.name).collect();
        assert_eq!(names, ["a.log", "sub/b.log"]);
        let mut read = read_entries(path, kind, &["sub/b.log", "a.log", "gone.log"], 1 << 20).unwrap();
        assert_eq!(read.remove("a.log").unwrap().unwrap(), b"alpha\n");
        assert_eq!(read.remove("sub/b.log").unwrap().unwrap(), b"beta\n");
        assert!(matches!(read.remove("gone.log").unwrap(), Err(ArchiveError::NotFound)));
    }
}

#[test]
fn a_lone_gz_is_one_entry_without_the_gz_suffix() {
    let d = dir();
    let gz = d.path().join("app.log.1.gz");
    std::fs::write(&gz, gzip(b"line one\nline two\n")).unwrap();
    let entries = list(&gz, ArchiveKind::Gz).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "app.log.1");
    assert_eq!(entries[0].size, 18);

    let file = LogFile::from_archive_entry(&path_str(&gz), "app.log.1");
    assert_eq!(file.display_name, "app.log.1");
    assert_eq!(read_source(file.source().unwrap()).unwrap(), b"line one\nline two\n");
}

#[test]
fn a_password_protected_zip_is_reported() {
    let d = dir();
    let zip = d.path().join("secret.zip");
    zip_with(&zip, &[("a.log", b"x\n")]);
    let mut bytes = std::fs::read(&zip).unwrap();
    // Set the "encrypted" bit in the local and the central header.
    bytes[6] |= 1;
    let central = bytes.windows(4).position(|w| w == b"PK\x01\x02").unwrap();
    bytes[central + 8] |= 1;
    std::fs::write(&zip, bytes).unwrap();
    assert!(matches!(list(&zip, ArchiveKind::Zip), Err(ArchiveError::Encrypted)));
}

#[test]
fn a_cut_tar_gz_is_damaged() {
    let d = dir();
    let content: Vec<u8> = (0..200_000u32).flat_map(|n| format!("line {n}\n").into_bytes()).collect();
    let mut bytes = gzip(&tar_bytes(&[("big.log", &content)]));
    bytes.truncate(bytes.len() / 2);
    let path = d.path().join("logs.tar.gz");
    std::fs::write(&path, bytes).unwrap();
    assert!(matches!(list(&path, ArchiveKind::TarGz), Err(ArchiveError::Damaged(_))));
}

#[test]
fn an_entry_over_the_limit_is_too_large() {
    let d = dir();
    let zip = d.path().join("bundle.zip");
    zip_with(&zip, &[("a.log", &[b'x'; 4096])]);
    let mut read = read_entries(&zip, ArchiveKind::Zip, &["a.log"], 1024).unwrap();
    assert!(matches!(read.remove("a.log").unwrap(), Err(ArchiveError::TooLarge)));
}

#[test]
fn a_missing_archive_or_entry_is_not_found() {
    let d = dir();
    let missing = d.path().join("gone.zip");
    let err = read_source(FileSource::ArchiveEntry { archive: &missing, entry: "a.log" }).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);

    let zip = d.path().join("bundle.zip");
    zip_with(&zip, &[("a.log", b"x\n")]);
    let err = read_source(FileSource::ArchiveEntry { archive: &zip, entry: "b.log" }).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    assert_eq!(
        read_source(FileSource::ArchiveEntry { archive: &zip, entry: "a.log" }).unwrap(),
        b"x\n"
    );
}

// ─── Log files that are Archive Entries ──────────────────────────────────────

#[test]
fn an_archive_entry_has_its_own_identity_and_name() {
    let file = LogFile::from_archive_entry("/tmp/x/bundle.zip", "nginx/access.log");
    assert_eq!(file.file_id, "/tmp/x/bundle.zip!/nginx/access.log");
    assert_eq!(file.display_name, "bundle.zip › nginx/access.log");
    assert_eq!(file.folder().as_deref(), Some("/tmp/x"));
}

fn entry(archive: &str, name: &str) -> ImportSource {
    ImportSource::Entry { archive: archive.into(), entry: name.into() }
}

#[test]
fn adding_an_archive_again_only_adds_new_entries() {
    let mut space = LogSpace::new("s".into(), "S".into(), LogSpaceKind::Imported);
    space.add_imported_sources(&[entry("/b.zip", "app.log")]).unwrap();
    let outcome = space
        .add_imported_sources(&[entry("/b.zip", "app.log"), entry("/b.zip", "db.log")])
        .unwrap();
    assert_eq!(outcome.added.len(), 1);
    assert_eq!(outcome.skipped, ["b.zip › app.log"]);
    assert_eq!(space.files.len(), 2);
}

#[test]
fn the_same_entry_name_in_another_archive_is_added() {
    let mut space = LogSpace::new("s".into(), "S".into(), LogSpaceKind::Imported);
    space.add_imported_sources(&[entry("/a.zip", "app.log")]).unwrap();
    let outcome = space.add_imported_sources(&[entry("/b.zip", "app.log")]).unwrap();
    assert_eq!(outcome.added.len(), 1);
}

#[test]
fn an_entry_survives_a_session_round_trip_and_old_files_stay_plain() {
    let mut file = LogFile::from_archive_entry("/b.zip", "app.log");
    file.paint_marker(12, "text", 0..2, log_gpui::palette::HighlightColor::Custom("#ff0000".into()));
    let back: LogFile = serde_json::from_str(&serde_json::to_string(&file).unwrap()).unwrap();
    assert_eq!(back.entry.as_deref(), Some("app.log"));
    assert_eq!(back.markers.len(), 1);

    let old = r#"{"file_id":"/a.log","display_name":"a.log","path":"/a.log"}"#;
    let plain: LogFile = serde_json::from_str(old).unwrap();
    assert_eq!(plain.entry, None);
}

// ─── The New Log Space dialog ────────────────────────────────────────────────

#[test]
fn dropping_an_archive_lists_its_entries() {
    let d = dir();
    let zip = d.path().join("bundle.zip");
    zip_with(
        &zip,
        &[("app.log", b"one\n"), ("heap.hprof", b"JAVA\0PROFILE"), ("old.zip", b"PK\x03\x04\0\0")],
    );
    let mut form = NewLogSpaceForm::new();
    form.add_paths(&[path_str(&zip)]);

    let names: Vec<String> = form.files.iter().map(|f| f.display_name()).collect();
    assert_eq!(names, ["bundle.zip › app.log", "bundle.zip › heap.hprof", "bundle.zip › old.zip"]);
    assert_eq!(form.files[0].status, PendingStatus::Ok);
    assert_eq!(form.files[1].warning(), Some("Not a text file"));
    assert_eq!(form.files[2].warning(), Some("Not a text file"));
    assert_eq!(form.create_label(), "Create with 1 file");

    form.remove_file(0);
    assert_eq!(form.create_label(), "Create empty");
}

#[test]
fn an_archive_inside_a_dropped_folder_is_opened() {
    let d = dir();
    std::fs::write(d.path().join("a.log"), "one\n").unwrap();
    zip_with(&d.path().join("bundle.zip"), &[("app.log", b"two\n")]);
    let mut form = NewLogSpaceForm::new();
    form.add_paths(&[path_str(d.path())]);
    let names: Vec<String> = form.files.iter().map(|f| f.display_name()).collect();
    assert_eq!(names, ["a.log", "bundle.zip › app.log"]);
}

#[test]
fn broken_archives_are_listed_with_the_reason_and_not_counted() {
    let d = dir();
    let secret = d.path().join("secret.zip");
    zip_with(&secret, &[("a.log", b"x\n")]);
    let mut bytes = std::fs::read(&secret).unwrap();
    bytes[6] |= 1;
    let central = bytes.windows(4).position(|w| w == b"PK\x01\x02").unwrap();
    bytes[central + 8] |= 1;
    std::fs::write(&secret, bytes).unwrap();

    let cut = d.path().join("logs.tar.gz");
    let content: Vec<u8> = (0..100_000u32).flat_map(|n| format!("l {n}\n").into_bytes()).collect();
    let mut gz = gzip(&tar_bytes(&[("big.log", &content)]));
    gz.truncate(gz.len() / 2);
    std::fs::write(&cut, gz).unwrap();

    let mut form = NewLogSpaceForm::new();
    form.add_paths(&[path_str(&secret), path_str(&cut)]);
    assert_eq!(form.files[0].warning(), Some("Password-protected archive"));
    assert_eq!(form.files[1].warning(), Some("Damaged archive"));
    assert_eq!(form.create_label(), "Create empty");
}

#[test]
fn large_entries_warn_but_stay_and_the_total_is_capped() {
    use log_gpui::workspace::new_space::PendingSource;
    let entry = |name: &str, size: u64, status| {
        (PendingSource::Entry { archive: "/b.zip".into(), entry: name.into(), size }, status)
    };
    let mut form = NewLogSpaceForm::new();
    form.set_name("S");
    let project = log_gpui::workspace::project::Project::new(
        "p".into(),
        "P".into(),
        log_gpui::workspace::project::ProjectColorAccent::Blue,
    );
    let (source, status) = entry("big.log", 500 * 1024 * 1024, PendingStatus::LargeFile);
    form.push_source(source, status);
    assert_eq!(form.files[0].warning(), Some("Large file"));
    assert_eq!(form.create_label(), "Create with 1 file");
    assert!(form.can_create(&project));

    for i in 0..5 {
        let (source, status) = entry(&format!("b{i}.log"), 500 * 1024 * 1024, PendingStatus::LargeFile);
        form.push_source(source, status);
    }
    assert!(form.total_uncompressed() > 2 * 1024 * 1024 * 1024);
    assert!(!form.can_create(&project));
    assert!(form.size_limit_message().is_some());
}

#[test]
fn an_archive_reads_as_reading_until_it_is_listed() {
    let d = dir();
    let zip: PathBuf = d.path().join("bundle.zip");
    zip_with(&zip, &[("app.log", b"x\n")]);
    let mut form = NewLogSpaceForm::new();
    let pending = form.begin_paths(&[path_str(&zip)]);
    assert_eq!(form.files[0].display_name(), "Reading bundle.zip…");
    assert!(form.is_reading());
    let (path, kind) = pending.into_iter().next().unwrap();
    form.finish_archive(&path, log_gpui::workspace::new_space::list_archive(&path, kind));
    assert!(!form.is_reading());
    assert_eq!(form.files[0].display_name(), "bundle.zip › app.log");
}

#[test]
fn zstd_xz_and_bzip2_files_are_one_entry_each() {
    let d = dir();
    for (name, kind, bytes) in [
        ("app.log.1.zst", ArchiveKind::Zstd, include_bytes!("../src/remote/testdata/t.zst").as_slice()),
        ("app.log.2.xz", ArchiveKind::Xz, include_bytes!("../src/remote/testdata/t.xz").as_slice()),
        ("app.log.3.bz2", ArchiveKind::Bzip2, include_bytes!("../src/remote/testdata/t.bz2").as_slice()),
    ] {
        let path = d.path().join(name);
        std::fs::write(&path, bytes).unwrap();
        assert_eq!(detect(&path), Some(kind), "{name}");
        let entries = list(&path, kind).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, name.rsplit_once('.').unwrap().0);
        let file = LogFile::from_archive_entry(&path_str(&path), &entries[0].name);
        assert_eq!(file.display_name, entries[0].name);
        let content = read_source(file.source().unwrap()).unwrap();
        assert_eq!(content, b"first line\nsecond line\nthird line\n", "{name}");
    }
}
