use log_gpui::workspace::log_space::{ImportSource, LogSpaceKind, SpaceNameError};
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
    assert_eq!(form.kind(), LogSpaceKind::Static);
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
    assert_eq!(form.files[1].warning().as_deref(), Some("Not a text file"));
    assert_eq!(form.importable_sources(), vec![ImportSource::Disk("/a.log".to_string())]);
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
    // Hidden files of a folder are noise, not logs.
    std::fs::write(dir.join(".DS_Store"), b"\0\0\0\x01Bud1").unwrap();

    let mut form = NewLogSpaceForm::new();
    form.add_paths(&[dir.to_string_lossy().to_string()]);

    let names: Vec<String> = form
        .files
        .iter()
        .map(|f| f.display_name())
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
    assert_eq!(form.importable_sources().len(), 1);
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

#[test]
fn only_files_that_cannot_be_added_never_make_an_empty_log_space() {
    let p = project_with(&[]);
    let mut form = NewLogSpaceForm::new();
    form.set_name("Dumps");
    form.push("/core.dump".into(), PendingStatus::Binary);
    form.push("/a.zip".into(), PendingStatus::Damaged);

    assert!(!form.can_create(&p), "Create is off: nothing would be added");
    assert_eq!(form.create_label(), "Create");
    assert_eq!(
        form.files_message().as_deref(),
        Some("None of these files can be added. Remove them to create an empty Log Space")
    );

    // Removing them leaves an empty form, which makes a Scratch Log Space.
    form.remove_file(1);
    form.remove_file(0);
    assert!(form.can_create(&p));
    assert_eq!(form.files_message(), None);
    assert_eq!(form.create_label(), "Create empty");
}

#[test]
fn an_archive_being_read_is_not_called_unaddable_yet() {
    let mut form = NewLogSpaceForm::new();
    form.push("/logs.zip".into(), PendingStatus::Reading);
    assert_eq!(form.files_message(), None);
}

#[test]
fn a_name_is_suggested_from_a_file_or_folder() {
    use log_gpui::local_path::PathStyle;
    use log_gpui::workspace::new_space::{suggest_name, unique_name};
    assert_eq!(suggest_name("/var/log/api.log", PathStyle::Unix).as_deref(), Some("api"));
    assert_eq!(suggest_name("/tmp/api.log.gz", PathStyle::Unix).as_deref(), Some("api"));
    assert_eq!(suggest_name("/tmp/bundle.tar.gz", PathStyle::Unix).as_deref(), Some("bundle"));
    assert_eq!(suggest_name("/srv/checkout/", PathStyle::Unix).as_deref(), Some("checkout"));
    assert_eq!(suggest_name("C:\\logs\\db.txt", PathStyle::Windows).as_deref(), Some("db"));
    // A name that is only an extension is kept as it is.
    assert_eq!(suggest_name("/x/.log", PathStyle::Unix).as_deref(), Some(".log"));

    let p = project_with(&["api", "api 2"]);
    assert_eq!(unique_name("api", &p), "api 3");
    assert_eq!(unique_name("db", &p), "db");
}

#[test]
fn create_says_it_waits_for_an_archive() {
    let p = project_with(&[]);
    let mut form = NewLogSpaceForm::new();
    form.set_name("Bundle");
    form.push("/a.log".into(), PendingStatus::Ok);
    form.push("/logs.zip".into(), PendingStatus::Reading);
    assert!(!form.can_create(&p));
    assert_eq!(form.create_label(), "Reading archive…");
    form.finish_archive("/logs.zip", Vec::new());
    assert_eq!(form.create_label(), "Create with 1 file");
}

#[test]
fn the_remote_create_label_names_what_it_holds() {
    use log_gpui::workspace::new_space::remote_create_label;
    assert_eq!(remote_create_label(1, 0, 0), "Create with 1 file");
    assert_eq!(remote_create_label(3, 0, 0), "Create with 3 files");
    assert_eq!(remote_create_label(0, 0, 1), "Create with 1 command");
    assert_eq!(remote_create_label(2, 0, 1), "Create with 2 files and 1 command");
    assert_eq!(remote_create_label(2, 1, 2), "Create with 2 files, 1 Live Glob and 2 commands");
}

#[test]
fn a_hidden_file_dropped_on_its_own_is_kept() {
    let dir = temp_dir("hidden_file");
    let hidden = dir.join(".env.log");
    std::fs::write(&hidden, "one\n").unwrap();
    let out = expand_paths(&[hidden.to_string_lossy().to_string()]);
    assert_eq!(out.len(), 1);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_file_listed_again_gets_no_row_of_its_own() {
    let dir = temp_dir("listed_again");
    let log = dir.join("api.log");
    std::fs::write(&log, "one\n").unwrap();
    let path = log.to_string_lossy().to_string();
    let mut form = NewLogSpaceForm::new();
    form.add_paths(std::slice::from_ref(&path));
    form.add_paths(&[path]);
    // Counted for the message under the list, never shown as a row.
    assert_eq!(form.files.len(), 1);
    assert_eq!(form.files[0].warning(), None);
    assert_eq!(form.already_listed, 1);
    std::fs::remove_dir_all(&dir).ok();
}
