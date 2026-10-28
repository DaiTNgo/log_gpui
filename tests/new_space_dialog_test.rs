//! The New Log Space dialog in the real `AppView`: what it makes from the
//! remote picker, and how it lays out many members.

mod common;

use log_gpui::workspace::new_space::{LiveCounts, NewSpaceKind};
use common::Harness;
use gpui::TestAppContext;
use log_gpui::app::remote_picker::{PickerTarget, Staged};
use log_gpui::remote::{AuthMethod, Server};

/// Open the dialog and switch it to `kind`.
fn open_on(h: &mut Harness, kind: NewSpaceKind) {
    h.view.update(&mut h.cx, |app, cx| {
        app.open_new_space_dialog(cx);
        app.new_space_set_kind(kind, cx);
    });
    h.cx.run_until_parked();
}

fn kind_of(h: &mut Harness) -> Option<NewSpaceKind> {
    h.view.read_with(&h.cx, |app, _| app.new_space.as_ref().map(|d| d.kind))
}

/// A Server nothing listens on: the picker's folder load fails at once.
/// Its connection runs on the app's own threads, so the test lets them be.
fn add_server(h: &mut Harness) -> String {
    h.cx.executor().allow_parking();
    h.view.update(&mut h.cx, |app, _| {
        let mut server = Server::new("web", "127.0.0.1", "u", AuthMethod::Password);
        server.port = 1;
        app.remote.servers.save(server).unwrap()
    })
}

/// One Add in the picker: `paths` on `server_id`, with `start` typed.
fn add_in_picker(h: &mut Harness, server_id: &str, paths: &[&str], start: &str) {
    h.view.update(&mut h.cx, |app, cx| {
        app.open_remote_picker(PickerTarget::NewSpace, cx);
        let picker = app.remote.picker.as_mut().unwrap();
        picker.staged = paths
            .iter()
            .map(|p| Staged::File { server_id: server_id.to_string(), path: p.to_string() })
            .collect();
        picker.start_time = start.to_string();
        app.picker_confirm(cx);
    });
    h.cx.run_until_parked();
}

#[gpui::test]
fn a_starting_time_goes_only_to_the_files_added_with_it(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    let server = add_server(&mut h);
    open_on(&mut h, NewSpaceKind::Live);
    add_in_picker(&mut h, &server, &["/var/log/api.log"], "1h");
    add_in_picker(&mut h, &server, &["/var/log/db.log"], "");

    h.view.update(&mut h.cx, |app, cx| {
        app.new_space.as_mut().unwrap().form.set_name("Checkout");
        app.new_space_create(cx);
    });
    h.cx.run_until_parked();

    let starts: Vec<(String, Option<String>)> = h.view.read_with(&h.cx, |app, _| {
        let project = app.workspace_state.active_project().unwrap();
        let space = project.log_spaces.iter().find(|s| s.name == "Checkout").expect("the Remote Log Space was made");
        space
            .files
            .iter()
            .map(|f| (f.display_name.clone(), f.remote.as_ref().and_then(|r| r.start_time.clone())))
            .collect()
    });
    assert_eq!(
        starts,
        vec![
            ("web:/var/log/api.log".to_string(), Some("1h".to_string())),
            ("web:/var/log/db.log".to_string(), None),
        ]
    );
}

#[gpui::test]
fn removing_a_chosen_file_forgets_its_starting_time(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    let server = add_server(&mut h);
    open_on(&mut h, NewSpaceKind::Live);
    add_in_picker(&mut h, &server, &["/var/log/api.log"], "1h");
    h.view.update(&mut h.cx, |app, cx| app.new_space_remove_remote(0, cx));
    add_in_picker(&mut h, &server, &["/var/log/api.log"], "");
    h.view.read_with(&h.cx, |app, _| assert!(app.remote.new_space_starts.is_empty()));
}

#[gpui::test]
fn many_remote_files_scroll_and_create_stays_on_screen(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    let server = add_server(&mut h);
    open_on(&mut h, NewSpaceKind::Live);
    let paths: Vec<String> = (1..=40).map(|i| format!("/var/log/app-{i}.log")).collect();
    let paths: Vec<&str> = paths.iter().map(String::as_str).collect();
    add_in_picker(&mut h, &server, &paths, "");
    h.cx.update(|window, _| window.refresh());
    h.cx.run_until_parked();

    let list = h.cx.debug_bounds("new-space-remote").expect("the remote list is drawn");
    assert!(list.size.height <= gpui::px(180.0), "the list scrolls: {:?}", list.size.height);
    let footer = h.cx.debug_bounds("new-space-footer").expect("the buttons are drawn");
    let window = h.cx.update(|window, _| window.viewport_size());
    assert!(footer.bottom() <= window.height, "Create is on screen: {footer:?} in {window:?}");
}

#[gpui::test]
fn a_folder_can_be_chosen_as_well_as_files(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    open_on(&mut h, NewSpaceKind::Static);
    // The native picker takes files or folders, never both: a second button
    // opens it on folders.
    assert!(h.cx.debug_bounds("new-space-browse-folder").is_some(), "Choose folder… is drawn");

    // What the folder picker hands back is listed like a dropped folder.
    let folder = h.dir.path().join("logs");
    std::fs::create_dir(&folder).unwrap();
    std::fs::write(folder.join("a.log"), "one\n").unwrap();
    std::fs::write(folder.join("b.log"), "two\n").unwrap();
    h.view.update(&mut h.cx, |app, cx| {
        app.new_space_add_paths(vec![folder.to_string_lossy().into_owned()], cx);
    });
    let (names, label) = h.view.read_with(&h.cx, |app, _| {
        let form = &app.new_space.as_ref().unwrap().form;
        (form.files.iter().map(|f| f.display_name()).collect::<Vec<_>>(), form.create_label(NewSpaceKind::Static, LiveCounts::default()))
    });
    assert_eq!(names, ["a.log", "b.log"]);
    assert_eq!(label, "Create with 2 files");
}

#[gpui::test]
fn local_and_remote_lists_do_not_block_each_other(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    let server = add_server(&mut h);
    open_on(&mut h, NewSpaceKind::Static);
    let log = h.dir.path().join("a.log");
    std::fs::write(&log, "one\n").unwrap();
    h.view.update(&mut h.cx, |app, cx| app.new_space_add_paths(vec![log.to_string_lossy().into_owned()], cx));
    h.cx.run_until_parked();
    assert!(h.cx.debug_bounds("new-space-drop").is_some());
    assert!(h.cx.debug_bounds("new-space-remote").is_none(), "the Remote list is hidden");

    // Switching does not open the picker, and the Remote buttons are on.
    h.view.update(&mut h.cx, |app, cx| app.new_space_set_kind(NewSpaceKind::Live, cx));
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| assert!(app.remote.picker.is_none()));
    assert!(h.cx.debug_bounds("new-space-remote").is_some());
    assert!(h.cx.debug_bounds("new-space-drop").is_none());
    assert!(h.cx.debug_bounds("new-space-empty-hint").is_some(), "Add files to follow, remote files or a command");
    h.cx.update(|window, _| window.refresh());
    let text = h.cx.debug_bounds("new-space-already-listed");
    assert!(text.is_none(), "no count of the hidden list");
    add_in_picker(&mut h, &server, &["/var/log/api.log"], "");

    // Back and forth keeps both lists.
    h.view.update(&mut h.cx, |app, cx| app.new_space_set_kind(NewSpaceKind::Static, cx));
    h.view.update(&mut h.cx, |app, cx| app.new_space_set_kind(NewSpaceKind::Live, cx));
    h.view.read_with(&h.cx, |app, _| {
        assert_eq!(app.new_space.as_ref().unwrap().form.files.len(), 1);
        assert_eq!(app.remote.new_space_files.len(), 1);
    });
}

#[gpui::test]
fn create_makes_only_what_the_kind_showing_lists(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    let server = add_server(&mut h);
    open_on(&mut h, NewSpaceKind::Static);
    let log = h.dir.path().join("a.log");
    std::fs::write(&log, "one\n").unwrap();
    h.view.update(&mut h.cx, |app, cx| app.new_space_add_paths(vec![log.to_string_lossy().into_owned()], cx));
    h.view.update(&mut h.cx, |app, cx| app.new_space_set_kind(NewSpaceKind::Live, cx));
    add_in_picker(&mut h, &server, &["/var/log/web.log"], "");
    h.view.update(&mut h.cx, |app, cx| {
        app.new_space.as_mut().unwrap().form.set_name("Checkout");
        app.new_space_create(cx);
    });
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| {
        let space = app
            .workspace_state
            .active_project()
            .unwrap()
            .log_spaces
            .iter()
            .find(|s| s.name == "Checkout")
            .expect("made");
        assert_eq!(space.files.len(), 1);
        assert_eq!(space.files[0].display_name, "web:/var/log/web.log");
    });
}

#[gpui::test]
fn the_first_file_names_the_log_space_until_a_name_is_typed(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    let server = add_server(&mut h);
    let name = |h: &mut Harness| h.view.read_with(&h.cx, |app, _| app.new_space.as_ref().unwrap().form.name.clone());
    let field = |h: &mut Harness| {
        h.view.update(&mut h.cx, |app, cx| {
            app.new_space.as_ref().unwrap().name_input.as_ref().unwrap().read(cx).value().to_string()
        })
    };

    h.view.update(&mut h.cx, |app, cx| app.open_new_space_dialog(cx));
    h.cx.run_until_parked();
    let folder = h.dir.path().join("checkout");
    std::fs::create_dir(&folder).unwrap();
    std::fs::write(folder.join("a.log"), "one\n").unwrap();
    h.view.update(&mut h.cx, |app, cx| app.new_space_add_paths(vec![folder.to_string_lossy().into_owned()], cx));
    h.cx.run_until_parked();
    assert_eq!(name(&mut h), "checkout");
    assert_eq!(field(&mut h), "checkout", "the field shows it");

    // A name already there is kept.
    let other = h.dir.path().join("b.log");
    std::fs::write(&other, "two\n").unwrap();
    h.view.update(&mut h.cx, |app, cx| app.new_space_add_paths(vec![other.to_string_lossy().into_owned()], cx));
    assert_eq!(name(&mut h), "checkout");

    // Emptied by hand, the dialog says why Create is off.
    h.view.update(&mut h.cx, |app, cx| {
        app.new_space.as_mut().unwrap().form.set_name("");
        cx.notify();
    });
    h.cx.run_until_parked();
    assert!(h.cx.debug_bounds("new-space-name-hint").is_some());

    // A remote file names a fresh dialog too.
    open_on(&mut h, NewSpaceKind::Live);
    add_in_picker(&mut h, &server, &["/var/log/nginx/access.log"], "");
    assert_eq!(name(&mut h), "access");
}

#[gpui::test]
fn the_dialog_always_opens_on_scratch(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    open_on(&mut h, NewSpaceKind::Live);
    h.view.update(&mut h.cx, |app, cx| app.new_space_cancel(cx));
    h.view.update(&mut h.cx, |app, cx| app.open_new_space_dialog(cx));
    h.cx.run_until_parked();
    assert_eq!(kind_of(&mut h), Some(NewSpaceKind::Scratch), "it does not remember the last kind");
    assert!(h.cx.debug_bounds("new-space-kind-description").is_some());
    assert!(h.cx.debug_bounds("new-space-drop").is_none());
    assert!(h.cx.debug_bounds("new-space-remote").is_none());
    h.view.read_with(&h.cx, |app, _| {
        let form = &app.new_space.as_ref().unwrap().form;
        let project = app.workspace_state.active_project().unwrap();
        assert_eq!(form.name, "Scratch");
        assert!(form.can_create(NewSpaceKind::Scratch, 0, project));
        assert_eq!(form.create_label(NewSpaceKind::Scratch, LiveCounts::default()), "Create");
    });
}

#[gpui::test]
fn the_name_follows_the_kind_until_one_is_typed(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    let name = |h: &mut Harness| h.view.read_with(&h.cx, |app, _| app.new_space.as_ref().unwrap().form.name.clone());
    h.view.update(&mut h.cx, |app, cx| app.open_new_space_dialog(cx));
    let log = h.dir.path().join("api.log");
    std::fs::write(&log, "one\n").unwrap();
    h.view.update(&mut h.cx, |app, cx| app.new_space_add_paths(vec![log.to_string_lossy().into_owned()], cx));
    assert_eq!(name(&mut h), "api");
    h.view.update(&mut h.cx, |app, cx| app.new_space_set_kind(NewSpaceKind::Scratch, cx));
    assert_eq!(name(&mut h), "Scratch");
    h.view.update(&mut h.cx, |app, cx| app.new_space_set_kind(NewSpaceKind::Static, cx));
    assert_eq!(name(&mut h), "api");

    h.view.update(&mut h.cx, |app, cx| {
        app.new_space.as_mut().unwrap().form.set_name("Checkout");
        app.new_space_set_kind(NewSpaceKind::Scratch, cx);
    });
    assert_eq!(name(&mut h), "Checkout", "a typed name stays");
}

#[gpui::test]
fn keys_pick_the_kind(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    h.view.update(&mut h.cx, |app, cx| app.open_new_space_dialog(cx));
    h.cx.run_until_parked();
    h.cx.simulate_keystrokes("cmd-2");
    assert_eq!(kind_of(&mut h), Some(NewSpaceKind::Live));
    h.cx.simulate_keystrokes("cmd-1");
    assert_eq!(kind_of(&mut h), Some(NewSpaceKind::Static));
    h.cx.simulate_keystrokes("cmd-3");
    assert_eq!(kind_of(&mut h), Some(NewSpaceKind::Scratch));
    let tabs_before = h.view.read_with(&h.cx, |app, _| app.workspace_state.active_project().unwrap().working_set.active_tab_index());
    h.view.update(&mut h.cx, |app, cx| app.new_space_step_kind(-1, cx));
    assert_eq!(kind_of(&mut h), Some(NewSpaceKind::Live));
    let tabs_after = h.view.read_with(&h.cx, |app, _| app.workspace_state.active_project().unwrap().working_set.active_tab_index());
    assert_eq!(tabs_before, tabs_after);

    // The arrow keys act while the switch has the cursor.
    h.cx.update(|window, cx| {
        let focus = {
            let app = h.view.read(cx);
            app.new_space.as_ref().unwrap().kind_focus.clone()
        };
        window.focus(&focus, cx);
    });
    h.cx.simulate_keystrokes("right");
    assert_eq!(kind_of(&mut h), Some(NewSpaceKind::Scratch));
    h.cx.simulate_keystrokes("left left");
    assert_eq!(kind_of(&mut h), Some(NewSpaceKind::Static));
    h.view.update(&mut h.cx, |app, cx| app.new_space_step_kind(-5, cx));
    assert_eq!(kind_of(&mut h), Some(NewSpaceKind::Static), "the switch stops at its ends");
}

#[gpui::test]
fn a_files_error_is_shown_like_a_field_error(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    h.view.update(&mut h.cx, |app, cx| app.open_new_space_dialog(cx));
    let dump = h.dir.path().join("core.dump");
    std::fs::write(&dump, b"ELF\0\0binary").unwrap();
    h.view.update(&mut h.cx, |app, cx| app.new_space_add_paths(vec![dump.to_string_lossy().into_owned()], cx));
    h.cx.run_until_parked();
    let row = h.cx.debug_bounds("new-space-files-error").expect("the error row is drawn");
    // The row holds the icon before the text, so it is wider than the icon.
    assert!(row.size.width > gpui::px(12.0));
}

/// Open the Add remote command dialog and type `command` in it.
fn type_command(h: &mut Harness, command: &str) {
    h.view.update(&mut h.cx, |app, cx| app.open_remote_command_picker(PickerTarget::NewSpace, cx));
    h.cx.run_until_parked();
    h.cx.update(|window, cx| {
        let view = h.view.clone();
        let input = view.read(cx).remote.picker.as_ref().unwrap().command.inputs.as_ref().unwrap().command.clone();
        input.update(cx, |state, cx| state.set_value(command.to_string(), window, cx));
    });
}

#[gpui::test]
fn a_command_already_chosen_is_refused_with_a_reason(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    add_server(&mut h);
    h.view.update(&mut h.cx, |app, cx| app.open_new_space_dialog(cx));
    type_command(&mut h, "journalctl -f -u nginx");
    h.view.update(&mut h.cx, |app, cx| {
        assert_eq!(app.command_form_error(cx), None);
        app.command_add(cx);
    });

    type_command(&mut h, "journalctl -f -u nginx");
    h.view.update(&mut h.cx, |app, cx| {
        assert_eq!(app.command_form_error(cx).as_deref(), Some("Already chosen for this Log Space"));
        // Add is off: nothing changes and the dialog stays open.
        app.command_add(cx);
        assert_eq!(app.remote.new_space_commands.len(), 1);
        assert!(app.remote.picker.is_some());
    });
}

/// Type `text` in the picker's path field.
fn type_path(h: &mut Harness, text: &str) {
    h.cx.run_until_parked();
    h.cx.update(|window, cx| {
        let view = h.view.clone();
        let input = view.read(cx).remote.picker.as_ref().unwrap().input.clone().unwrap();
        input.update(cx, |state, cx| state.set_value(text.to_string(), window, cx));
    });
}

#[gpui::test]
fn done_adds_a_typed_path_without_add_to_list_first(cx: &mut TestAppContext) {
    use log_gpui::remote::browse::Resolved;
    let mut h = common::open_app(cx);
    let server = add_server(&mut h);
    h.view.update(&mut h.cx, |app, cx| {
        app.open_new_space_dialog(cx);
        app.open_remote_picker(PickerTarget::NewSpace, cx);
    });
    type_path(&mut h, "/var/log/api.log");
    h.view.update(&mut h.cx, |app, cx| {
        assert_eq!(app.picker_typed_pending(cx).as_deref(), Some("/var/log/api.log"), "Done is on");
        app.picker_confirm(cx);
        assert!(app.remote.picker.is_some(), "Done waits for the path to be checked");
        // The Server answers: the path is a file.
        let found = Ok(Ok(Resolved::Files(vec!["/var/log/api.log".to_string()])));
        app.picker_resolved(&server, vec![(server.clone(), found)], cx);
        assert!(app.remote.picker.is_none(), "then Done goes on");
        assert_eq!(app.remote.new_space_files.len(), 1);
        assert_eq!(app.remote.new_space_files[0].2, "/var/log/api.log");
    });
}

#[gpui::test]
fn done_stays_open_when_the_typed_path_fails_or_is_a_folder(cx: &mut TestAppContext) {
    use log_gpui::remote::browse::Resolved;
    let mut h = common::open_app(cx);
    let server = add_server(&mut h);
    h.view.update(&mut h.cx, |app, cx| {
        app.open_new_space_dialog(cx);
        app.open_remote_picker(PickerTarget::NewSpace, cx);
    });
    type_path(&mut h, "/var/log/missing.log");
    h.view.update(&mut h.cx, |app, cx| {
        app.picker_confirm(cx);
        let failed = Ok(Err("No such file".to_string()));
        app.picker_resolved(&server, vec![(server.clone(), failed)], cx);
        assert!(app.remote.picker.is_some(), "the error is shown in the open picker");
        assert!(app.remote.new_space_files.is_empty());
    });
    type_path(&mut h, "/var/log");
    h.view.update(&mut h.cx, |app, cx| {
        app.picker_confirm(cx);
        let folder = Ok(Ok(Resolved::Dir("/var/log".to_string())));
        app.picker_resolved(&server, vec![(server.clone(), folder)], cx);
        assert!(app.remote.picker.is_some(), "a folder opens instead");
    });
}

#[gpui::test]
fn the_starting_time_field_has_a_label(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    add_server(&mut h);
    h.view.update(&mut h.cx, |app, cx| {
        app.open_new_space_dialog(cx);
        app.open_remote_picker(PickerTarget::NewSpace, cx);
    });
    h.cx.run_until_parked();
    assert!(h.cx.debug_bounds("picker-start-label").is_some());
}

/// Drop `paths` from the Finder on the window at `(x, y)`.
fn drop_files(h: &mut Harness, paths: Vec<std::path::PathBuf>, x: f32, y: f32) {
    let position = gpui::point(gpui::px(x), gpui::px(y));
    h.cx.simulate_event(gpui::FileDropEvent::Entered { position, paths: gpui::ExternalPaths(paths.into()) });
    h.cx.simulate_event(gpui::FileDropEvent::Submit { position });
    h.cx.run_until_parked();
}

fn space_files(h: &mut Harness, name: &str) -> Option<Vec<String>> {
    h.view.read_with(&h.cx, |app, _| {
        let project = app.workspace_state.active_project().unwrap();
        project
            .log_spaces
            .iter()
            .find(|s| s.name == name)
            .map(|s| s.files.iter().map(|f| f.display_name.clone()).collect())
    })
}

#[gpui::test]
fn files_dropped_with_no_dialog_make_an_imported_log_space(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    let a = h.dir.path().join("api.log");
    let b = h.dir.path().join("worker.log");
    std::fs::write(&a, "one\n").unwrap();
    std::fs::write(&b, "two\n").unwrap();
    h.cx.run_until_parked();

    drop_files(&mut h, vec![a.clone()], 500.0, 300.0);
    h.view.read_with(&h.cx, |app, _| assert!(app.new_space.is_none(), "no dialog"));
    assert_eq!(space_files(&mut h, "api"), Some(vec!["api.log".to_string()]));

    drop_files(&mut h, vec![a, b], 500.0, 300.0);
    assert_eq!(space_files(&mut h, "api 2"), Some(vec!["api.log".to_string(), "worker.log".to_string()]));
}

#[gpui::test]
fn a_dropped_folder_and_archive_make_a_log_space(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    let folder = h.dir.path().join("logs");
    std::fs::create_dir(&folder).unwrap();
    std::fs::write(folder.join("a.log"), "one\n").unwrap();
    std::fs::write(folder.join("b.log"), "two\n").unwrap();
    drop_files(&mut h, vec![folder], 500.0, 300.0);
    assert_eq!(space_files(&mut h, "logs"), Some(vec!["a.log".to_string(), "b.log".to_string()]));

    let gz = h.dir.path().join("web.log.gz");
    let mut encoder = flate2::write::GzEncoder::new(std::fs::File::create(&gz).unwrap(), flate2::Compression::default());
    std::io::Write::write_all(&mut encoder, b"one\ntwo\n").unwrap();
    encoder.finish().unwrap();
    drop_files(&mut h, vec![gz], 500.0, 300.0);
    h.cx.executor().allow_parking();
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| assert_eq!(app.dropped_archives_reading, 0));
    assert!(space_files(&mut h, "web").is_some(), "made once the Archive was read");
}

#[gpui::test]
fn a_drop_with_an_unusable_file_opens_the_dialog_on_local_files(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    let a = h.dir.path().join("api.log");
    let dump = h.dir.path().join("core.dump");
    std::fs::write(&a, "one\n").unwrap();
    std::fs::write(&dump, b"ELF\0\0binary").unwrap();
    drop_files(&mut h, vec![a, dump], 500.0, 300.0);
    assert_eq!(space_files(&mut h, "api"), None, "nothing was made");
    assert_eq!(kind_of(&mut h), Some(NewSpaceKind::Static));
    h.view.read_with(&h.cx, |app, _| {
        let files = &app.new_space.as_ref().unwrap().form.files;
        assert_eq!(files.len(), 2);
        assert_eq!(files[1].warning().as_deref(), Some("Not a text file"));
    });
}

#[gpui::test]
fn a_drop_on_the_dialog_switches_to_local_and_keeps_the_remote_list(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    let server = add_server(&mut h);
    open_on(&mut h, NewSpaceKind::Live);
    add_in_picker(&mut h, &server, &["/var/log/web.log"], "");
    let a = h.dir.path().join("api.log");
    std::fs::write(&a, "one\n").unwrap();
    drop_files(&mut h, vec![a], 40.0, 40.0);
    assert_eq!(kind_of(&mut h), Some(NewSpaceKind::Static));
    h.view.read_with(&h.cx, |app, _| {
        assert_eq!(app.new_space.as_ref().unwrap().form.files.len(), 1);
        assert_eq!(app.remote.new_space_files.len(), 1);
    });
}

#[gpui::test]
fn a_drop_is_ignored_while_another_dialog_is_open(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    let a = h.dir.path().join("api.log");
    std::fs::write(&a, "one\n").unwrap();
    h.view.update(&mut h.cx, |app, cx| app.open_servers_dialog(cx));
    h.cx.run_until_parked();
    drop_files(&mut h, vec![a], 500.0, 300.0);
    h.view.read_with(&h.cx, |app, _| assert!(app.new_space.is_none()));
    assert_eq!(space_files(&mut h, "api"), None);
}

#[gpui::test]
fn a_folder_added_to_a_log_space_brings_its_files(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &common::sample_lines(3));
    let folder = h.dir.path().join("more");
    std::fs::create_dir(&folder).unwrap();
    std::fs::write(folder.join("b.log"), "two\n").unwrap();
    std::fs::write(folder.join(".DS_Store"), b"\0\0").unwrap();
    // What Add folder… hands back.
    h.view.update(&mut h.cx, |app, cx| {
        let space_id = app.workspace_state.active_project().unwrap().log_spaces[0].id.clone();
        app.add_files_to_space(&space_id, vec![folder.to_string_lossy().into_owned()], cx);
    });
    h.cx.run_until_parked();
    let names: Vec<String> = h.view.read_with(&h.cx, |app, _| {
        app.workspace_state.active_project().unwrap().log_spaces[0].files.iter().map(|f| f.display_name.clone()).collect()
    });
    assert_eq!(names, ["app.log", "b.log"]);
}

#[gpui::test]
fn the_cursor_returns_to_the_name_after_the_picker_closes(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    add_server(&mut h);
    let name_focused = |h: &mut Harness| {
        h.view.update_in(&mut h.cx, |app, window, cx| {
            let input = app.new_space.as_ref().unwrap().name_input.clone().unwrap();
            gpui::Focusable::focus_handle(input.read(cx), cx).is_focused(window)
        })
    };
    h.view.update(&mut h.cx, |app, cx| app.open_new_space_dialog(cx));
    h.cx.run_until_parked();
    assert!(name_focused(&mut h), "the name has the cursor when the dialog opens");

    h.view.update(&mut h.cx, |app, cx| app.open_remote_picker(PickerTarget::NewSpace, cx));
    type_path(&mut h, "/var/log/api.log");
    // The user clicks into the path field.
    h.cx.update(|window, cx| {
        let view = h.view.clone();
        let input = view.read(cx).remote.picker.as_ref().unwrap().input.clone().unwrap();
        input.update(cx, |state, cx| state.focus(window, cx));
    });
    h.cx.run_until_parked();
    assert!(!name_focused(&mut h), "the picker's path field has it");

    h.view.update(&mut h.cx, |app, cx| app.close_remote_picker(cx));
    h.cx.run_until_parked();
    assert!(name_focused(&mut h), "back on the name: Enter creates the Log Space");
}

#[gpui::test]
fn dropping_files_already_listed_says_so(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    let a = h.dir.path().join("api.log");
    let b = h.dir.path().join("db.log");
    std::fs::write(&a, "one\n").unwrap();
    std::fs::write(&b, "two\n").unwrap();
    open_on(&mut h, NewSpaceKind::Static);

    // Dropped on the drop zone: listed once, nothing said.
    let zone = h.cx.debug_bounds("new-space-drop").expect("the drop zone is drawn").center();
    drop_files(&mut h, vec![a.clone()], f32::from(zone.x), f32::from(zone.y));
    let listed = |h: &mut Harness| h.view.read_with(&h.cx, |app, _| app.new_space.as_ref().unwrap().form.files.len());
    assert_eq!(listed(&mut h), 1);
    assert!(h.cx.debug_bounds("new-space-already-listed").is_none());

    // The same file again, with a new one.
    let zone = h.cx.debug_bounds("new-space-drop").unwrap().center();
    drop_files(&mut h, vec![a, b], f32::from(zone.x), f32::from(zone.y));
    assert_eq!(listed(&mut h), 2);
    let message = h.view.read_with(&h.cx, |app, _| app.new_space.as_ref().unwrap().form.already_listed_message());
    assert_eq!(message.as_deref(), Some("1 file was already in the list"));
    assert!(h.cx.debug_bounds("new-space-already-listed").is_some());

    // Removing a file clears it.
    h.view.update(&mut h.cx, |app, cx| app.new_space_remove_file(1, cx));
    h.cx.run_until_parked();
    assert!(h.cx.debug_bounds("new-space-already-listed").is_none());
}

#[gpui::test]
fn removing_an_archive_being_read_drops_its_entries(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    // A `.gz` of one log file: an Archive, read in the background.
    let gz = h.dir.path().join("api.log.gz");
    let mut encoder = flate2::write::GzEncoder::new(std::fs::File::create(&gz).unwrap(), flate2::Compression::default());
    std::io::Write::write_all(&mut encoder, b"one\ntwo\n").unwrap();
    encoder.finish().unwrap();

    h.view.update(&mut h.cx, |app, cx| {
        app.open_new_space_dialog(cx);
        app.new_space_add_paths(vec![gz.to_string_lossy().into_owned()], cx);
        let form = &app.new_space.as_ref().unwrap().form;
        assert!(form.is_reading(), "listed as Reading…");
        // Removed before the listing ends.
        app.new_space_remove_file(0, cx);
    });
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| {
        let form = &app.new_space.as_ref().unwrap().form;
        assert!(form.files.is_empty(), "the entries that came later were dropped");
        assert_eq!(form.create_label(NewSpaceKind::Static, LiveCounts::default()), "Create");
    });
}

#[gpui::test]
fn a_chosen_command_shows_the_preset_that_wrote_it(cx: &mut TestAppContext) {
    use log_gpui::app::remote_command::PresetKind;
    use log_gpui::remote::PresetFields;
    let mut h = common::open_app(cx);
    let server = add_server(&mut h);
    let docker = PresetFields::docker("api");
    let written = docker.command();
    let edited = format!("{written} | grep ERROR");
    assert_eq!(PresetKind::tag(&written, Some(&docker)), Some("Docker"));
    assert_eq!(PresetKind::tag(&edited, Some(&docker)), None, "changed by hand: no longer the preset's");

    h.view.update(&mut h.cx, |app, cx| {
        app.open_new_space_dialog(cx);
        app.new_space_set_kind(NewSpaceKind::Live, cx);
        app.remote.new_space_commands.push((server.clone(), "web".into(), written, Some(docker.clone())));
        app.remote.new_space_commands.push((server.clone(), "web".into(), edited, Some(docker)));
        cx.notify();
    });
    h.cx.run_until_parked();
    assert!(h.cx.debug_bounds("new-space-command-tag-0").is_some());
    assert!(h.cx.debug_bounds("new-space-command-tag-1").is_none());
}

#[gpui::test]
fn a_chosen_files_starting_time_is_changed_in_the_dialog(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    let server = add_server(&mut h);
    open_on(&mut h, NewSpaceKind::Live);
    add_in_picker(&mut h, &server, &["/var/log/api.log"], "1h");
    let key = (server.clone(), "/var/log/api.log".to_string());
    let typed = |h: &mut Harness, text: &str| {
        h.view.update(&mut h.cx, |app, cx| {
            app.new_space.as_mut().unwrap().start_edit.as_mut().unwrap().text = text.to_string();
            cx.notify();
        });
    };

    h.view.update(&mut h.cx, |app, cx| app.new_space_edit_start(0, cx));
    h.cx.run_until_parked();
    assert!(h.cx.debug_bounds("new-space-start-edit").is_some(), "the box opens");
    h.view.read_with(&h.cx, |app, _| {
        assert_eq!(app.new_space.as_ref().unwrap().start_edit.as_ref().unwrap().text, "1h", "with what the file has");
    });

    // Text that is not a Starting Time cannot be saved.
    typed(&mut h, "yesterday-ish");
    h.view.update(&mut h.cx, |app, cx| {
        app.new_space_save_start(cx);
        assert!(app.new_space.as_ref().unwrap().start_edit.is_some(), "still open");
        assert_eq!(app.remote.new_space_starts.get(&key).map(String::as_str), Some("1h"));
    });

    typed(&mut h, "30m");
    h.view.update(&mut h.cx, |app, cx| app.new_space_save_start(cx));
    h.view.read_with(&h.cx, |app, _| {
        assert!(app.new_space.as_ref().unwrap().start_edit.is_none());
        assert_eq!(app.remote.new_space_starts.get(&key).map(String::as_str), Some("30m"));
    });

    // Emptied, the file reads its last lines again.
    h.view.update(&mut h.cx, |app, cx| app.new_space_edit_start(0, cx));
    typed(&mut h, "");
    h.view.update(&mut h.cx, |app, cx| app.new_space_save_start(cx));
    h.view.read_with(&h.cx, |app, _| assert!(app.remote.new_space_starts.is_empty()));

    // Escape closes the box, not the dialog.
    h.view.update(&mut h.cx, |app, cx| {
        app.new_space_edit_start(0, cx);
        assert!(app.close_top_dialog(cx));
        assert!(app.new_space.as_ref().is_some_and(|d| d.start_edit.is_none()));
    });
}

#[gpui::test]
fn tab_goes_round_the_dialog_and_never_behind_it(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &common::sample_lines(3));
    h.view.update(&mut h.cx, |app, cx| app.open_new_space_dialog(cx));
    h.cx.run_until_parked();
    let mut back_on_name = 0;
    for _ in 0..40 {
        h.cx.simulate_keystrokes("tab");
        h.cx.run_until_parked();
        let (name, search) = h.view.update_in(&mut h.cx, |app, window, cx| {
            let input = app.new_space.as_ref().unwrap().name_input.clone().unwrap();
            (
                gpui::Focusable::focus_handle(input.read(cx), cx).is_focused(window),
                gpui::Focusable::focus_handle(app.search_input.read(cx), cx).is_focused(window),
            )
        });
        assert!(!search, "the Search field behind the dialog never gets the cursor");
        back_on_name += usize::from(name);
    }
    assert!(back_on_name >= 2, "Tab comes round to the name again");
}

#[gpui::test]
fn the_cursor_reaches_the_kind_switch_and_stays_in_the_dialog(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    h.view.update(&mut h.cx, |app, cx| app.open_new_space_dialog(cx));
    h.cx.run_until_parked();
    let on_switch = |h: &mut Harness| {
        h.cx.update(|window, cx| h.view.read(cx).new_space.as_ref().unwrap().kind_focus.is_focused(window))
    };
    let inside = |h: &mut Harness| {
        h.cx.update(|window, cx| {
            h.view.read(cx).new_space.as_ref().unwrap().focus.as_ref().unwrap().contains_focused(window, cx)
        })
    };
    // From the name, Shift-Tab goes back to the switch.
    h.cx.simulate_keystrokes("shift-tab");
    assert!(on_switch(&mut h), "Tab stops at the switch");
    assert!(inside(&mut h));
    h.cx.simulate_keystrokes("shift-tab");
    h.cx.run_until_parked();
    assert!(inside(&mut h), "the cursor does not leave the dialog");
}

/// The Live step lists local files as well as remote ones, apart from the
/// Static step's list, and makes one Live Log Space of both.
#[gpui::test]
fn the_live_step_makes_one_live_space_of_local_and_remote_files(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    let server = add_server(&mut h);
    open_on(&mut h, NewSpaceKind::Live);
    let api = h.dir.path().join("api.log");
    std::fs::write(&api, "2026-09-05 10:00:00 [INFO] one\n").unwrap();
    let api = api.to_string_lossy().into_owned();
    h.view.update(&mut h.cx, |app, cx| app.new_space_add_live_paths(vec![api.clone()], cx));
    add_in_picker(&mut h, &server, &["/app/app.log"], "");
    h.cx.update(|window, _| window.refresh());
    h.cx.run_until_parked();
    assert!(h.cx.debug_bounds("new-space-live-files").is_some(), "the local files are listed");
    assert!(h.cx.debug_bounds("new-space-live-browse").is_some());
    assert!(h.cx.debug_bounds("new-space-remote").is_some());
    h.view.read_with(&h.cx, |app, _| {
        let dialog = app.new_space.as_ref().unwrap();
        assert_eq!(dialog.kind, NewSpaceKind::Live);
        assert!(dialog.form.files.is_empty(), "the Static step's list is untouched");
        let label = dialog.form.create_label(NewSpaceKind::Live, app.new_space_live_counts());
        assert_eq!(label, "Create with 1 file and 1 remote file");
        assert_eq!(dialog.form.name, "api", "named after the local file, listed first");
    });
    h.view.update(&mut h.cx, |app, cx| app.new_space_create(cx));
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| {
        let project = app.workspace_state.active_project().unwrap();
        let space = project.log_spaces.last().unwrap();
        assert_eq!(space.kind, log_gpui::workspace::log_space::LogSpaceKind::Live);
        assert_eq!(space.files.len(), 2);
        assert!(space.files.iter().any(|f| f.path.as_deref() == Some(api.as_str()) && f.remote.is_none()));
        assert!(space.files.iter().any(|f| f.remote.is_some()));
    });
}

#[gpui::test]
fn an_archive_on_the_live_step_is_listed_but_never_added(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    open_on(&mut h, NewSpaceKind::Live);
    let zip = h.dir.path().join("logs.zip");
    std::fs::write(&zip, b"PK\x03\x04 not read").unwrap();
    h.view.update(&mut h.cx, |app, cx| app.new_space_add_live_paths(vec![zip.to_string_lossy().into_owned()], cx));
    h.view.read_with(&h.cx, |app, _| {
        let dialog = app.new_space.as_ref().unwrap();
        let file = &dialog.live_form.files[0];
        assert_eq!(file.warning().as_deref(), Some("Archives can only be added to a Static Log Space"));
        let counts = app.new_space_live_counts();
        assert_eq!(counts, LiveCounts::default(), "not counted in the label");
        assert!(!dialog.form.can_create(NewSpaceKind::Live, counts.total(), app.workspace_state.active_project().unwrap()));
    });
    // Taken out again, the list is empty.
    h.view.update(&mut h.cx, |app, cx| app.new_space_remove_live_file(0, cx));
    h.view.read_with(&h.cx, |app, _| assert!(app.new_space.as_ref().unwrap().live_form.files.is_empty()));
}
