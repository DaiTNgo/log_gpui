//! Local paths: names and folders come from path-aware splitting, so a
//! Windows path (`C:\logs\app.log`) works the same as `/var/log/app.log`.
//! The Windows rules are testable on any platform through `PathStyle`.

use log_gpui::local_path::{PathStyle, file_name, folder, is_absolute, split_last};
use log_gpui::workspace::log_space::LogFile;

#[test]
fn a_windows_path_has_its_file_name_and_folder() {
    let p = r"C:\logs\app.log";
    assert_eq!(file_name(p, PathStyle::Windows), Some("app.log"));
    assert_eq!(folder(p, PathStyle::Windows).as_deref(), Some(r"C:\logs"));
}

#[test]
fn a_windows_path_may_mix_both_separators() {
    let p = r"C:\logs/app.log";
    assert_eq!(file_name(p, PathStyle::Windows), Some("app.log"));
    assert_eq!(folder(p, PathStyle::Windows).as_deref(), Some(r"C:\logs"));
}

#[test]
fn a_file_at_a_drive_root_has_the_root_as_folder() {
    assert_eq!(file_name(r"C:\app.log", PathStyle::Windows), Some("app.log"));
    assert_eq!(folder(r"C:\app.log", PathStyle::Windows).as_deref(), Some(r"C:\"));
}

#[test]
fn a_unc_path_is_split_on_backslashes() {
    let p = r"\\srv\share\logs\app.log";
    assert_eq!(file_name(p, PathStyle::Windows), Some("app.log"));
    assert_eq!(folder(p, PathStyle::Windows).as_deref(), Some(r"\\srv\share\logs"));
}

#[test]
fn a_unix_path_keeps_backslashes_in_the_name() {
    assert_eq!(file_name(r"/tmp/a\b.log", PathStyle::Unix), Some(r"a\b.log"));
    assert_eq!(folder("/tmp/x/app.log", PathStyle::Unix).as_deref(), Some("/tmp/x"));
    assert_eq!(folder("/app.log", PathStyle::Unix).as_deref(), Some("/"));
}

#[test]
fn a_bare_name_has_no_folder() {
    assert_eq!(folder("app.log", PathStyle::Unix), None);
    assert_eq!(folder("app.log", PathStyle::Windows), None);
    assert_eq!(file_name("app.log", PathStyle::Windows), Some("app.log"));
}

#[test]
fn absolute_paths_by_style() {
    assert!(is_absolute(r"C:\x\y", PathStyle::Windows));
    assert!(is_absolute("C:/x/y", PathStyle::Windows));
    assert!(is_absolute(r"\\srv\share", PathStyle::Windows));
    assert!(is_absolute("/x", PathStyle::Windows));
    assert!(!is_absolute(r"conf.d\x", PathStyle::Windows));
    assert!(!is_absolute("C:x", PathStyle::Windows));
    assert!(is_absolute("/x", PathStyle::Unix));
    assert!(!is_absolute(r"C:\x", PathStyle::Unix));
}

#[test]
fn split_last_keeps_the_separator_that_was_used() {
    assert_eq!(split_last(r"C:\a\b*", PathStyle::Windows), Some((r"C:\a", "b*", '\\')));
    assert_eq!(split_last("/a/b", PathStyle::Unix), Some(("/a", "b", '/')));
    assert_eq!(split_last("b", PathStyle::Unix), None);
}

#[test]
fn a_log_file_shows_its_name_and_folder() {
    // A path of the running platform's own style.
    let (path, dir) = if cfg!(windows) { (r"C:\logs\app.log", r"C:\logs") } else { ("/logs/app.log", "/logs") };
    let file = LogFile::from_path(path);
    assert_eq!(file.display_name, "app.log");
    assert_eq!(file.folder().as_deref(), Some(dir));
}
