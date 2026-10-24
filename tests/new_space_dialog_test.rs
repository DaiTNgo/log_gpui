//! The New Log Space dialog in the real `AppView`: what it makes from the
//! remote picker, and how it lays out many members.

mod common;

use common::Harness;
use gpui::TestAppContext;
use log_gpui::app::remote_picker::{PickerTarget, Staged};
use log_gpui::remote::{AuthMethod, Server};

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
    h.view.update(&mut h.cx, |app, cx| app.open_new_space_dialog(cx));
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
    h.view.update(&mut h.cx, |app, cx| app.open_new_space_dialog(cx));
    add_in_picker(&mut h, &server, &["/var/log/api.log"], "1h");
    h.view.update(&mut h.cx, |app, cx| app.new_space_remove_remote(0, cx));
    add_in_picker(&mut h, &server, &["/var/log/api.log"], "");
    h.view.read_with(&h.cx, |app, _| assert!(app.remote.new_space_starts.is_empty()));
}

#[gpui::test]
fn many_remote_files_scroll_and_create_stays_on_screen(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    let server = add_server(&mut h);
    h.view.update(&mut h.cx, |app, cx| app.open_new_space_dialog(cx));
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
    h.view.update(&mut h.cx, |app, cx| app.open_new_space_dialog(cx));
    h.cx.run_until_parked();
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
        (form.files.iter().map(|f| f.display_name()).collect::<Vec<_>>(), form.create_label())
    });
    assert_eq!(names, ["a.log", "b.log"]);
    assert_eq!(label, "Create with 2 files");
}

#[gpui::test]
fn the_dialog_says_why_local_and_remote_files_do_not_mix(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    let server = add_server(&mut h);
    h.view.update(&mut h.cx, |app, cx| app.open_new_space_dialog(cx));
    h.cx.run_until_parked();
    assert!(h.cx.debug_bounds("new-space-local-hint").is_none());
    assert!(h.cx.debug_bounds("new-space-remote-hint").is_none());

    // A local file turns the remote buttons off, and the dialog says why.
    let log = h.dir.path().join("a.log");
    std::fs::write(&log, "one\n").unwrap();
    h.view.update(&mut h.cx, |app, cx| app.new_space_add_paths(vec![log.to_string_lossy().into_owned()], cx));
    h.cx.run_until_parked();
    assert!(h.cx.debug_bounds("new-space-remote-hint").is_some());

    // A remote file hides the drop zone, and the dialog says why.
    h.view.update(&mut h.cx, |app, cx| app.new_space_remove_file(0, cx));
    add_in_picker(&mut h, &server, &["/var/log/api.log"], "");
    h.cx.run_until_parked();
    assert!(h.cx.debug_bounds("new-space-local-hint").is_some());
    assert!(h.cx.debug_bounds("new-space-remote-hint").is_none());
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
    h.view.update(&mut h.cx, |app, cx| app.open_new_space_dialog(cx));
    add_in_picker(&mut h, &server, &["/var/log/nginx/access.log"], "");
    assert_eq!(name(&mut h), "access");
}

#[gpui::test]
fn create_empty_says_what_it_makes(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    h.view.update(&mut h.cx, |app, cx| app.open_new_space_dialog(cx));
    h.cx.run_until_parked();
    assert!(h.cx.debug_bounds("new-space-scratch-hint").is_some());

    let log = h.dir.path().join("a.log");
    std::fs::write(&log, "one\n").unwrap();
    h.view.update(&mut h.cx, |app, cx| app.new_space_add_paths(vec![log.to_string_lossy().into_owned()], cx));
    h.cx.run_until_parked();
    assert!(h.cx.debug_bounds("new-space-scratch-hint").is_none(), "gone once a file is listed");
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

#[gpui::test]
fn files_dropped_anywhere_open_a_new_log_space_with_them(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    let a = h.dir.path().join("api.log");
    let b = h.dir.path().join("db.log");
    std::fs::write(&a, "one\n").unwrap();
    std::fs::write(&b, "two\n").unwrap();
    h.cx.run_until_parked();

    drop_files(&mut h, vec![a.clone()], 500.0, 300.0);
    let names = |h: &mut Harness| {
        h.view.read_with(&h.cx, |app, _| {
            app.new_space.as_ref().map(|d| d.form.files.iter().map(|f| f.display_name()).collect::<Vec<_>>())
        })
    };
    assert_eq!(names(&mut h), Some(vec!["api.log".to_string()]), "the dialog opened with the file");

    // A second drop, on the open dialog's backdrop, adds to it.
    drop_files(&mut h, vec![a, b], 40.0, 40.0);
    assert_eq!(names(&mut h), Some(vec!["api.log".to_string(), "db.log".to_string()]));
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
    h.view.update(&mut h.cx, |app, cx| app.open_new_space_dialog(cx));
    h.cx.run_until_parked();

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
        assert_eq!(form.create_label(), "Create empty");
    });
}

#[gpui::test]
fn remove_all_empties_a_long_list(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    let folder = h.dir.path().join("many");
    std::fs::create_dir(&folder).unwrap();
    for i in 1..=30 {
        std::fs::write(folder.join(format!("app-{i}.log")), "x\n").unwrap();
    }
    h.view.update(&mut h.cx, |app, cx| app.open_new_space_dialog(cx));
    h.cx.run_until_parked();
    assert!(h.cx.debug_bounds("new-space-clear").is_none(), "nothing to remove");

    h.view.update(&mut h.cx, |app, cx| app.new_space_add_paths(vec![folder.to_string_lossy().into_owned()], cx));
    h.cx.run_until_parked();
    assert!(h.cx.debug_bounds("new-space-clear").is_some(), "Remove all is offered");
    h.view.update(&mut h.cx, |app, cx| app.new_space_clear_files(cx));
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| {
        let form = &app.new_space.as_ref().unwrap().form;
        assert!(form.files.is_empty());
        assert_eq!(form.name, "many", "the name it suggested stays");
    });
    assert!(h.cx.debug_bounds("new-space-clear").is_none());
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
        app.remote.new_space_commands.push((server.clone(), "web".into(), written, Some(docker.clone())));
        app.remote.new_space_commands.push((server.clone(), "web".into(), edited, Some(docker)));
        cx.notify();
    });
    h.cx.run_until_parked();
    assert!(h.cx.debug_bounds("new-space-command-tag-0").is_some());
    assert!(h.cx.debug_bounds("new-space-command-tag-1").is_none());
}
