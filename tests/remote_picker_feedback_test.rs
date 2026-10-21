mod common;

use common::Harness;
use gpui::TestAppContext;
use log_gpui::app::remote_picker::{PickerBusy, PickerTarget, Staged, added_message};
use log_gpui::remote::browse::{DirEntry, Resolved};
use log_gpui::remote::{AuthMethod, Server};

#[test]
fn busy_labels_name_what_is_loading() {
    assert_eq!(PickerBusy::Listing(Some("/var/log".into())).label(), "Listing /var/log\u{2026}");
    assert_eq!(PickerBusy::Listing(None).label(), "Listing the home folder\u{2026}");
    assert_eq!(PickerBusy::Resolving("/var/log/*.log".into()).label(), "Matching /var/log/*.log\u{2026}");
    assert_eq!(PickerBusy::Resolving("/var/log/app.log".into()).label(), "Checking /var/log/app.log\u{2026}");
}

#[test]
fn added_message_counts_what_was_new() {
    assert_eq!(added_message(0, 0, 0, 0), None);
    assert_eq!(added_message(1, 0, 0, 0).as_deref(), Some("Added 1 file"));
    assert_eq!(added_message(3, 1, 0, 0).as_deref(), Some("Added 3 files (1 already chosen)"));
    assert_eq!(added_message(0, 0, 1, 0).as_deref(), Some("Added 1 Live Glob"));
    assert_eq!(added_message(0, 2, 0, 0).as_deref(), Some("Nothing new: 2 already chosen"));
}

/// A picker on a Server nothing listens on (nothing comes back from it on
/// its own); returns the Server id.
fn open_picker(cx: &mut TestAppContext) -> (Harness, String) {
    let mut h = common::open_app(cx);
    let id = h.view.update(&mut h.cx, |app, cx| {
        let mut server = Server::new("box", "127.0.0.1", "me", AuthMethod::Password);
        server.port = 1;
        let id = app.remote.servers.save(server).unwrap();
        *app.remote.shared_servers.lock().unwrap() = app.remote.servers.clone();
        app.open_remote_picker(PickerTarget::NewSpace, cx);
        app.remote.picker.as_mut().unwrap().server_id = Some(id.clone());
        id
    });
    (h, id)
}

fn entry(name: &str, is_dir: bool) -> DirEntry {
    DirEntry { name: name.into(), is_dir, size: 1, modified: None }
}

#[gpui::test]
fn the_list_says_what_it_lists_and_hides_the_old_rows(cx: &mut TestAppContext) {
    let (mut h, _) = open_picker(cx);
    h.view.update(&mut h.cx, |app, _| {
        let picker = app.remote.picker.as_mut().unwrap();
        picker.entries = vec![entry("old.log", false)];
        picker.busy = Some(PickerBusy::Listing(Some("/var/log".into())));
        assert_eq!(picker.list_notice().as_deref(), Some("Listing /var/log\u{2026}"));
        assert!(!picker.shows_rows(), "the folder being left is not clickable");

        picker.busy = None;
        assert!(picker.shows_rows());
        assert_eq!(picker.list_notice(), None);

        picker.entries.clear();
        assert_eq!(picker.list_notice().as_deref(), Some("This folder is empty"));

        picker.entries = vec![entry("app.log", false)];
        picker.filter = "zzz".into();
        assert_eq!(picker.list_notice().as_deref(), Some("Nothing in this folder matches the filter"));
    });
}

#[gpui::test]
fn adding_reports_what_was_added_and_keeps_the_typed_text(cx: &mut TestAppContext) {
    let (mut h, id) = open_picker(cx);
    let files = |paths: &[&str]| Ok(Ok(Resolved::Files(paths.iter().map(|p| p.to_string()).collect())));
    h.view.update(&mut h.cx, |app, cx| {
        app.picker_resolved(&id, vec![(id.clone(), files(&["/var/log/a.log", "/var/log/b.log"]))], cx);
        let picker = app.remote.picker.as_ref().unwrap();
        assert_eq!(picker.info.as_deref(), Some("Added 2 files"));
        assert!(!picker.clear_input, "the path field keeps its text");
        assert_eq!(picker.staged.len(), 2);

        app.picker_resolved(&id, vec![(id.clone(), files(&["/var/log/b.log", "/var/log/c.log"]))], cx);
        let picker = app.remote.picker.as_ref().unwrap();
        assert_eq!(picker.info.as_deref(), Some("Added 1 file (1 already chosen)"));
        assert_eq!(picker.staged.len(), 3);

        let glob = Ok(Ok(Resolved::Glob { dir: "/var/log".into(), pattern: "*.log".into() }));
        app.picker_resolved(&id, vec![(id.clone(), glob)], cx);
        let picker = app.remote.picker.as_ref().unwrap();
        assert_eq!(picker.info.as_deref(), Some("Added 1 Live Glob"));
        assert!(matches!(picker.staged.last(), Some(Staged::Glob { .. })));
    });
}

#[gpui::test]
fn typing_a_folder_opens_it_and_empties_the_field(cx: &mut TestAppContext) {
    let (mut h, id) = open_picker(cx);
    h.view.update(&mut h.cx, |app, cx| {
        app.picker_resolved(&id, vec![(id.clone(), Ok(Ok(Resolved::Dir("/var/log".into()))))], cx);
        let picker = app.remote.picker.as_ref().unwrap();
        assert!(picker.clear_input);
        assert_eq!(picker.busy, Some(PickerBusy::Listing(Some("/var/log".into()))));
    });
}
