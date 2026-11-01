//! Warm Tabs (memory-budget, groups 6 and 7): a tab left keeps its files'
//! Line Indexes while they fit in the Warm Tab Budget, reads nothing
//! meanwhile, and shows again without Indexing.

mod common;

use common::{Harness, open_app};
use gpui::TestAppContext;
use log_gpui::app::warm::WarmFiles;
use log_gpui::workspace::log_space::{ImportSource, LogSpaceKind};

fn log_file(h: &Harness, name: &str, lines: usize) -> ImportSource {
    let path = h.dir.path().join(name);
    let text: String = (0..lines).map(|i| format!("2026-09-05 10:00:{:02} [INFO] [{name}] line {i}\n", i % 60)).collect();
    std::fs::write(&path, text).unwrap();
    ImportSource::Disk(path.to_string_lossy().into_owned())
}

fn new_space(h: &mut Harness, name: &str, kind: LogSpaceKind, files: &[ImportSource]) -> String {
    h.view.update(&mut h.cx, |app, cx| app.create_log_space(name, kind, files, cx));
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| {
        app.workspace_state.active_project().unwrap().working_set.active_tab().unwrap().target.space_id().unwrap().to_string()
    })
}

fn tab_of(h: &Harness, space_id: &str) -> usize {
    h.view.read_with(&h.cx, |app, _| {
        app.workspace_state.active_project().unwrap().working_set.tabs().iter().position(|t| t.target.space_id() == Some(space_id)).unwrap()
    })
}

fn file_ids(h: &Harness, space_id: &str) -> Vec<String> {
    h.view.read_with(&h.cx, |app, _| {
        app.workspace_state.active_project().unwrap().log_space(space_id).unwrap().files.iter().map(|f| f.file_id.clone()).collect()
    })
}

fn switch_to(h: &mut Harness, space_id: &str) {
    let at = tab_of(h, space_id);
    h.view.update(&mut h.cx, |app, cx| app.switch_to_tab(at, cx));
}

fn pump_until(h: &mut Harness, what: &str, done: impl Fn(&log_gpui::app::AppView) -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        h.view.update(&mut h.cx, |app, cx| app.pump_live(cx));
        h.cx.run_until_parked();
        if h.view.read_with(&h.cx, |app, _| done(app)) {
            return;
        }
        assert!(std::time::Instant::now() < deadline, "{what}");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

#[test]
fn warm_files_go_oldest_first_to_fit_the_budget() {
    let mut warm = WarmFiles::default();
    warm.leave([("a".to_string(), 100)]);
    warm.leave([("b".to_string(), 150), ("c".to_string(), 10)]);
    assert_eq!(warm.total(), 260);
    assert!(warm.fit(1000).is_empty());
    assert_eq!(warm.fit(200), ["a"], "the one left longest ago");
    // Left again: it is the newest now.
    warm.leave([("b".to_string(), 150)]);
    warm.leave([("d".to_string(), 50)]);
    assert_eq!(warm.fit(100), ["c", "b"], "c and b, oldest first");
    assert!(warm.take("d"));
    assert!(!warm.take("d"));
    assert!(warm.fit(0).is_empty());
    warm.leave([("e".to_string(), 1)]);
    assert_eq!(warm.fit(0), ["e"], "budget 0 keeps nothing");
}

#[gpui::test]
fn a_tab_left_stays_warm_and_shows_again_without_indexing(cx: &mut TestAppContext) {
    let mut h = open_app(cx);
    let (a, b) = (log_file(&h, "a.log", 3000), log_file(&h, "b.log", 10));
    let first = new_space(&mut h, "First", LogSpaceKind::Static, &[a]);
    let a_id = file_ids(&h, &first)[0].clone();
    h.view.update(&mut h.cx, |app, _| app.selected_row_pos = Some(1200));
    new_space(&mut h, "Second", LogSpaceKind::Static, &[b]);
    h.view.read_with(&h.cx, |app, _| {
        assert!(app.warm.contains(&a_id), "kept for its tab");
        assert!(app.parsed_files.contains_key(&a_id));
        assert!(!app.is_dormant_tab(tab_of(&h, &first)), "a Warm Tab shows as read");
    });
    switch_to(&mut h, &first);
    h.view.read_with(&h.cx, |app, _| {
        assert!(!app.is_reading(), "no Indexing");
        assert_eq!(app.view.len(), 3000, "its rows at once");
        assert!(!app.warm.contains(&a_id), "shown, so no longer warm");
    });
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| assert_eq!(app.selected_row_pos, Some(1200)));
}

#[gpui::test]
fn a_tab_bigger_than_the_budget_goes_dormant(cx: &mut TestAppContext) {
    let mut h = open_app(cx);
    h.view.update(&mut h.cx, |app, _| app.workspace_state.warm_tab_budget_mb = 1);
    let (a, b) = (log_file(&h, "a.log", 60_000), log_file(&h, "b.log", 10));
    let first = new_space(&mut h, "First", LogSpaceKind::Static, &[a]);
    let a_id = file_ids(&h, &first)[0].clone();
    let bytes = h.view.read_with(&h.cx, |app, _| app.parsed_files[&a_id].indexed.as_ref().unwrap().memory_bytes());
    assert!(bytes > 1 << 20, "the file takes {bytes} bytes");
    new_space(&mut h, "Second", LogSpaceKind::Static, &[b]);
    h.view.read_with(&h.cx, |app, _| {
        assert!(!app.parsed_files.contains_key(&a_id), "let go");
        assert!(app.warm.is_empty());
        assert!(app.is_dormant_tab(tab_of(&h, &first)));
    });
}

#[gpui::test]
fn lowering_the_budget_lets_the_warm_tabs_go(cx: &mut TestAppContext) {
    let mut h = open_app(cx);
    let (a, b, c) = (log_file(&h, "a.log", 100), log_file(&h, "b.log", 100), log_file(&h, "c.log", 10));
    let first = new_space(&mut h, "First", LogSpaceKind::Static, &[a]);
    let second = new_space(&mut h, "Second", LogSpaceKind::Static, &[b]);
    new_space(&mut h, "Third", LogSpaceKind::Static, &[c]);
    let (a_id, b_id) = (file_ids(&h, &first)[0].clone(), file_ids(&h, &second)[0].clone());
    h.view.read_with(&h.cx, |app, _| assert_eq!(app.warm.len(), 2));
    h.view.update(&mut h.cx, |app, cx| app.set_warm_tab_budget(0, cx));
    h.view.read_with(&h.cx, |app, _| {
        assert!(app.warm.is_empty());
        assert!(!app.parsed_files.contains_key(&a_id) && !app.parsed_files.contains_key(&b_id));
        assert_eq!(app.workspace_state.warm_tab_budget_mb, 0);
    });
}

#[gpui::test]
fn a_live_file_written_on_while_warm_reads_only_what_is_new(cx: &mut TestAppContext) {
    let mut h = open_app(cx);
    let budget = log_gpui::index::HandleBudget::new(16);
    h.view.update(&mut h.cx, |app, _| app.live.handle_budget = budget.clone());
    let a = log_file(&h, "a.log", 20);
    let first = new_space(&mut h, "First", LogSpaceKind::Live, &[a]);
    pump_until(&mut h, "a.log shows", |app| app.view.len() == 20);
    let b = log_file(&h, "b.log", 3);
    new_space(&mut h, "Second", LogSpaceKind::Live, &[b]);
    pump_until(&mut h, "b.log shows", |app| app.view.len() == 3);
    assert_eq!(budget.open(), 1, "the Warm Tab holds no file open");

    let path = h.dir.path().join("a.log");
    let more: String = (0..5).map(|i| format!("2026-09-05 11:00:0{i} [INFO] [a.log] more {i}\n")).collect();
    std::fs::OpenOptions::new().append(true).open(&path).unwrap();
    std::fs::write(&path, std::fs::read_to_string(&path).unwrap() + &more).unwrap();
    switch_to(&mut h, &first);
    h.view.read_with(&h.cx, |app, _| assert!(!app.is_reading(), "not indexed again"));
    pump_until(&mut h, "the new lines show", |app| app.view.len() == 25);
}

#[gpui::test]
fn a_live_file_replaced_while_warm_is_read_again_below_a_break(cx: &mut TestAppContext) {
    let mut h = open_app(cx);
    let a = log_file(&h, "a.log", 20);
    let first = new_space(&mut h, "First", LogSpaceKind::Live, &[a]);
    pump_until(&mut h, "a.log shows", |app| app.view.len() == 20);
    let b = log_file(&h, "b.log", 3);
    new_space(&mut h, "Second", LogSpaceKind::Live, &[b]);

    let other = h.dir.path().join("other.log");
    let lines: String = (0..4).map(|i| format!("2026-09-05 12:00:0{i} [WARN] [a.log] other {i}\n")).collect();
    std::fs::write(&other, lines).unwrap();
    std::fs::rename(&other, h.dir.path().join("a.log")).unwrap();
    switch_to(&mut h, &first);
    pump_until(&mut h, "the other file's lines show", |app| {
        app.rows_snapshot().iter().any(|e| e.break_before.as_ref().is_some_and(|b| b.kind == log_gpui::live::BreakKind::Replaced))
            && app.rows_snapshot().last().is_some_and(|e| e.raw_line.ends_with("other 3"))
    });
}

#[gpui::test]
fn a_static_file_rewritten_while_warm_is_read_again(cx: &mut TestAppContext) {
    let mut h = open_app(cx);
    let a = log_file(&h, "a.log", 20);
    let first = new_space(&mut h, "First", LogSpaceKind::Static, &[a]);
    let b = log_file(&h, "b.log", 3);
    new_space(&mut h, "Second", LogSpaceKind::Static, &[b]);
    log_file(&h, "a.log", 7);
    switch_to(&mut h, &first);
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| assert_eq!(app.view.len(), 7, "read again"));
}

#[gpui::test]
fn a_file_with_an_older_generation_is_let_go(cx: &mut TestAppContext) {
    let mut h = open_app(cx);
    let a = log_file(&h, "a.log", 20);
    let first = new_space(&mut h, "First", LogSpaceKind::Live, &[a]);
    pump_until(&mut h, "a.log shows", |app| app.view.len() == 20);
    let path = h.dir.path().join("a.log");
    std::fs::rename(&path, h.dir.path().join("a.log.1")).unwrap();
    std::fs::write(&path, "2026-09-05 12:00:00 [WARN] [a.log] after\n").unwrap();
    pump_until(&mut h, "rotated", |app| app.view.len() == 21);
    let a_id = file_ids(&h, &first)[0].clone();
    let b = log_file(&h, "b.log", 3);
    new_space(&mut h, "Second", LogSpaceKind::Live, &[b]);
    h.view.read_with(&h.cx, |app, _| {
        assert!(!app.warm.contains(&a_id), "its old lines need the old handle: Dormant");
        assert!(!app.parsed_files.contains_key(&a_id));
    });
}

#[gpui::test]
fn a_remote_file_is_let_go_while_the_local_one_stays_warm(cx: &mut TestAppContext) {
    let mut h = open_app(cx);
    let a = log_file(&h, "a.log", 20);
    let first = new_space(&mut h, "First", LogSpaceKind::Live, &[a]);
    let remote_id = h.view.update(&mut h.cx, |app, cx| {
        let space = app.workspace_state.active_project_mut().unwrap().log_space_mut(&first).unwrap();
        space.add_remote_files(&[("srv-1".into(), "web1".into(), "/app/app.log".into())]).unwrap();
        app.rebuild_open_view_keeping_position(cx);
        "remote:srv-1:/app/app.log".to_string()
    });
    h.cx.run_until_parked();
    let a_id = file_ids(&h, &first).into_iter().find(|id| *id != remote_id).unwrap();
    h.view.read_with(&h.cx, |app, _| assert!(app.parsed_files.contains_key(&remote_id), "read from its Remote Cache"));
    let b = log_file(&h, "b.log", 3);
    new_space(&mut h, "Second", LogSpaceKind::Static, &[b]);
    h.view.read_with(&h.cx, |app, _| {
        assert!(app.warm.contains(&a_id), "the local file stays warm");
        assert!(!app.warm.contains(&remote_id) && !app.parsed_files.contains_key(&remote_id), "the Remote File opens from its cache again");
    });
}

#[gpui::test]
fn a_warm_scratch_tab_keeps_its_editor_text(cx: &mut TestAppContext) {
    let mut h = open_app(cx);
    let notes = new_space(&mut h, "Warm notes", LogSpaceKind::Scratch, &[]);
    h.view.update(&mut h.cx, |app, cx| app.new_scratch_file_in(&notes, cx));
    h.cx.run_until_parked();
    let id = file_ids(&h, &notes)[0].clone();
    h.view.update(&mut h.cx, |app, _| {
        app.raw.texts.insert(id.clone(), "2026-09-05 10:00:00 [INFO] typed\n".into());
        app.raw.unsaved.insert(id.clone());
    });
    let b = log_file(&h, "b.log", 3);
    new_space(&mut h, "Second", LogSpaceKind::Static, &[b]);
    h.view.read_with(&h.cx, |app, _| {
        assert!(app.warm.contains(&id));
        assert!(app.raw.texts.contains_key(&id), "the editor's text stays for a Warm Tab");
        assert!(app.raw.unsaved.is_empty(), "saved all the same");
    });
    switch_to(&mut h, &notes);
    h.view.read_with(&h.cx, |app, _| assert!(app.raw.texts.contains_key(&id)));
}

#[gpui::test]
fn a_file_let_go_is_freed(cx: &mut TestAppContext) {
    let mut h = open_app(cx);
    common::no_warm_tabs(&mut h);
    let (a, b) = (log_file(&h, "a.log", 3000), log_file(&h, "b.log", 10));
    let first = new_space(&mut h, "First", LogSpaceKind::Static, &[a]);
    let a_id = file_ids(&h, &first)[0].clone();
    let weak = h.view.read_with(&h.cx, |app, _| std::sync::Arc::downgrade(app.parsed_files[&a_id].indexed.as_ref().unwrap()));
    let indexed = h.view.read_with(&h.cx, |app, _| std::sync::Arc::downgrade(&app.parsed_files[&a_id].indexed.as_ref().unwrap().indexed));
    new_space(&mut h, "Second", LogSpaceKind::Static, &[b]);
    h.cx.run_until_parked();
    assert_eq!(weak.strong_count(), 0, "the file is dropped");
    assert_eq!(indexed.strong_count(), 0, "its Line Index too");
}

#[gpui::test]
fn a_session_without_the_budget_takes_the_default(cx: &mut TestAppContext) {
    let h = common::open_app_with_session(cx, tempfile::tempdir().unwrap(), "{}");
    h.view.read_with(&h.cx, |app, _| {
        assert_eq!(app.workspace_state.warm_tab_budget_mb, log_gpui::app::warm::DEFAULT_WARM_TAB_BUDGET_MB);
    });
}
