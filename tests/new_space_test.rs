use log_gpui::workspace::log_space::{LogSpaceKind, SpaceNameError};
use log_gpui::workspace::new_space::{
    NewLogSpaceForm, PendingStatus, expand_paths, is_binary, path_is_binary,
};
use log_gpui::workspace::project::{Project, ProjectColorAccent};

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("log_gpui_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn project_with(names: &[&str]) -> Project {
    let mut p = Project::new("prod".into(), "Prod".into(), ProjectColorAccent::Blue);
    for n in names {
        p.create_log_space(n, LogSpaceKind::Scratch).unwrap();
    }
    p
}

#[test]
fn an_empty_form_cannot_be_created() {
    let form = NewLogSpaceForm::new();
    assert!(!form.can_create(&project_with(&[])));
    assert!(form.files.is_empty());
}

#[test]
fn a_name_of_only_spaces_cannot_be_created() {
    let mut form = NewLogSpaceForm::new();
    form.set_name("   ");
    let p = project_with(&[]);
    assert!(!form.can_create(&p));
    assert_eq!(form.name_error(&p), Some(SpaceNameError::Blank));
    // An empty field is not shown as an error yet.
    assert_eq!(form.name_message(&p), None);
}

#[test]
fn a_duplicate_name_in_the_project_is_refused_with_a_message() {
    let p = project_with(&["Payment"]);
    let mut form = NewLogSpaceForm::new();
    form.set_name(" Payment ");
    assert!(!form.can_create(&p));
    assert_eq!(form.name_message(&p), Some("A Log Space with this name already exists"));
}

#[test]
fn the_same_name_in_another_project_does_not_block() {
    let staging = project_with(&["Payment"]);
    let prod = project_with(&[]);
    let mut form = NewLogSpaceForm::new();
    form.set_name("Payment");
    assert!(!form.can_create(&staging));
    assert!(form.can_create(&prod));
}

#[test]
fn the_kind_follows_the_files_and_the_label_says_so() {
    let mut form = NewLogSpaceForm::new();
    form.set_name("Payment");
    assert_eq!(form.kind(), LogSpaceKind::Scratch);
    assert_eq!(form.create_label(), "Create empty");

    form.push("/a.log".into(), PendingStatus::Ok);
    assert_eq!(form.kind(), LogSpaceKind::Imported);
    assert_eq!(form.create_label(), "Create with 1 file");

    form.push("/b.log".into(), PendingStatus::Ok);
    assert_eq!(form.create_label(), "Create with 2 files");
}

#[test]
fn binary_files_are_listed_but_not_counted() {
    let mut form = NewLogSpaceForm::new();
    form.push("/a.log".into(), PendingStatus::Ok);
    form.push("/core.dump".into(), PendingStatus::Binary);

    assert_eq!(form.files.len(), 2);
    assert_eq!(form.files[1].warning(), Some("Not a text file"));
    assert_eq!(form.importable_paths(), vec!["/a.log".to_string()]);
    assert_eq!(form.create_label(), "Create with 1 file");
}

#[test]
fn removing_a_file_updates_the_label() {
    let mut form = NewLogSpaceForm::new();
    form.push("/a.log".into(), PendingStatus::Ok);
    form.push("/b.log".into(), PendingStatus::Ok);
    form.remove_file(1);
    assert_eq!(form.files.len(), 1);
    assert_eq!(form.create_label(), "Create with 1 file");
    form.remove_file(0);
    assert_eq!(form.kind(), LogSpaceKind::Scratch);
}

#[test]
fn the_same_path_is_listed_once() {
    let mut form = NewLogSpaceForm::new();
    form.push("/a.log".into(), PendingStatus::Ok);
    form.push("/a.log".into(), PendingStatus::Ok);
    assert_eq!(form.files.len(), 1);
}

// ─── Dropping files and folders ──────────────────────────────────────────────

#[test]
fn a_dropped_folder_gives_its_files_one_level_deep() {
    let dir = temp_dir("drop_folder");
    std::fs::write(dir.join("a.log"), "one\n").unwrap();
    std::fs::write(dir.join("b.log"), "two\n").unwrap();
    std::fs::create_dir_all(dir.join("old")).unwrap();
    std::fs::write(dir.join("old").join("c.log"), "three\n").unwrap();

    let mut form = NewLogSpaceForm::new();
    form.add_paths(&[dir.to_string_lossy().to_string()]);

    let names: Vec<String> = form
        .files
        .iter()
        .map(|f| std::path::Path::new(&f.path).file_name().unwrap().to_string_lossy().to_string())
        .collect();
    assert_eq!(names, ["a.log", "b.log"]);
    assert!(form.files.iter().all(|f| f.status == PendingStatus::Ok));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_file_with_nul_bytes_is_binary() {
    let dir = temp_dir("binary");
    let dump = dir.join("core.dump");
    std::fs::write(&dump, b"ELF\0\0\x01\x02binary").unwrap();
    let text = dir.join("api.log");
    std::fs::write(&text, "2026-09-05 10:00:00 [INFO] hello\n").unwrap();

    assert!(path_is_binary(&dump));
    assert!(!path_is_binary(&text));

    let mut form = NewLogSpaceForm::new();
    form.add_paths(&[dump.to_string_lossy().to_string(), text.to_string_lossy().to_string()]);
    assert_eq!(form.files[0].status, PendingStatus::Binary);
    assert_eq!(form.files[1].status, PendingStatus::Ok);
    assert_eq!(form.importable_paths().len(), 1);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn invalid_utf8_is_binary_but_a_cut_multibyte_character_is_not() {
    assert!(is_binary(&[0xff, 0xfe, b'a']));
    assert!(!is_binary("lỗi hệ thống".as_bytes()));
    // The 8 KB window can end in the middle of a character.
    let cut = &"ỗ".as_bytes()[..2];
    assert!(!is_binary(cut));
}

#[test]
fn expand_paths_keeps_plain_files_as_they_are() {
    let out = expand_paths(&["/does/not/exist.log".to_string()]);
    assert_eq!(out, vec!["/does/not/exist.log".to_string()]);
}
