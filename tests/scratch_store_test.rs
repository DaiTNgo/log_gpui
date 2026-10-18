use log_gpui::workspace::log_space::{LogSpace, LogSpaceKind, scratch_dir_for, spaces_root};
use log_gpui::workspace::scratch_store;

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("log_gpui_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn creating_a_scratch_file_makes_its_folder_and_an_empty_file() {
    let dir = temp_dir("scratch_create");
    let path = dir.join("space").join("untitled.log");

    scratch_store::create_file(&path).unwrap();

    assert!(path.exists());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "");
    // Creating again keeps what the user typed.
    scratch_store::save_text(&path, "hello\n").unwrap();
    scratch_store::create_file(&path).unwrap();
    assert_eq!(scratch_store::read_text(&path).unwrap(), "hello\n");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn saving_writes_the_text_and_a_missing_file_reads_as_empty() {
    let dir = temp_dir("scratch_save");
    let path = dir.join("a.log");
    assert_eq!(scratch_store::read_text(&path).unwrap(), "");

    scratch_store::save_text(&path, "line one\nline two\n").unwrap();
    assert_eq!(scratch_store::read_text(&path).unwrap(), "line one\nline two\n");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn renaming_and_deleting_a_scratch_file() {
    let dir = temp_dir("scratch_rename");
    let a = dir.join("untitled.log");
    let b = dir.join("gateway.log");
    scratch_store::save_text(&a, "x").unwrap();

    scratch_store::rename_file(&a, &b).unwrap();
    assert!(!a.exists());
    assert_eq!(scratch_store::read_text(&b).unwrap(), "x");

    // A name in use is refused and nothing is overwritten.
    scratch_store::save_text(&a, "y").unwrap();
    assert!(scratch_store::rename_file(&a, &b).is_err());
    assert_eq!(scratch_store::read_text(&b).unwrap(), "x");

    scratch_store::delete_file(&b).unwrap();
    assert!(!b.exists());
    // Deleting a file that is gone is fine.
    scratch_store::delete_file(&b).unwrap();
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn deleting_a_space_removes_its_whole_folder() {
    let dir = temp_dir("scratch_space_dir");
    let space = dir.join("prod--payment");
    scratch_store::save_text(&space.join("untitled.log"), "a").unwrap();
    scratch_store::save_text(&space.join("untitled-2.log"), "b").unwrap();

    scratch_store::delete_space_dir(&space).unwrap();

    assert!(!space.exists());
    scratch_store::delete_space_dir(&space).unwrap();
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn save_as_writes_a_copy_and_leaves_the_scratch_file_alone() {
    let dir = temp_dir("scratch_save_as");
    let scratch = dir.join("untitled.log");
    let copy = dir.join("exported.log");
    scratch_store::save_text(&scratch, "keep me\n").unwrap();

    let text = scratch_store::read_text(&scratch).unwrap();
    scratch_store::save_as(&text, &copy).unwrap();

    assert_eq!(scratch_store::read_text(&copy).unwrap(), "keep me\n");
    assert_eq!(scratch_store::read_text(&scratch).unwrap(), "keep me\n");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn scratch_files_live_next_to_the_session_file() {
    let session = log_gpui::workspace::session::default_session_path();
    let space = LogSpace::new("prod--payment".into(), "Payment".into(), LogSpaceKind::Scratch);
    let dir = space.scratch_dir();
    assert_eq!(dir, scratch_dir_for("prod--payment"));
    assert_eq!(dir.parent().unwrap(), spaces_root());
    assert_eq!(spaces_root().parent(), session.parent());
    assert!(dir.ends_with("spaces/prod--payment"));
}
