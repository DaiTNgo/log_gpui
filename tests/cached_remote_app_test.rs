//! A Remote File read through its Remote Cache in the real `AppView`
//! (openspec change `remote-local-cache`, stage 2): the same events give the
//! view they give with the lines held in memory, and the text is not in
//! memory.

mod common;

use gpui::TestAppContext;
use log_gpui::app::AppView;
use log_gpui::app::live::LiveSourceState;
use log_gpui::live::source::EarlierFile;
use log_gpui::live::{BreakKind, SourceEvent, SourceHandle, SourceStatus};
use log_gpui::workspace::log_space::LogSpaceKind;

const FILE_ID: &str = "remote:srv-1:/app/app.log";

struct Remote {
    h: common::Harness,
    tx: async_channel::Sender<SourceEvent>,
}

/// A Live Log Space of one Remote File, its source a fake the test feeds.
fn open(cx: &mut TestAppContext, cached: bool, window: usize) -> Remote {
    let mut h = common::open_app(cx);
    h.view.update(&mut h.cx, |app, cx| {
        app.remote_cache_index = cached;
        app.live.window = window;
        let project = app.workspace_state.active_project_mut().unwrap();
        let space = project.create_log_space("Remote", LogSpaceKind::Live).unwrap();
        space.add_remote_files(&[("srv-1".into(), "web1".into(), "/app/app.log".into())]).unwrap();
        let id = space.id.clone();
        app.open_log_space_tab(&id, cx);
    });
    h.cx.run_until_parked();
    let (tx, events) = async_channel::unbounded();
    let (commands, _rx) = async_channel::unbounded();
    h.view.update(&mut h.cx, |app, _| {
        app.live.sources.insert(
            FILE_ID.to_string(),
            LiveSourceState {
                handle: SourceHandle { events, commands },
                seq: 1_000,
                status: SourceStatus::Live,
                name: "web1:/app/app.log".into(),
                remote: true,
                command: false,
                numbers_unavailable: false,
                health: Default::default(),
            },
        );
    });
    Remote { h, tx }
}

fn lines(range: std::ops::Range<usize>) -> Vec<String> {
    range
        .flat_map(|i| {
            let line = format!("2026-09-05 {:02}:{:02}:{:02} [{}] [api] request {i} user={}", 10 + i / 3600, i / 60 % 60, i % 60, ["INFO", "WARN", "ERROR"][i % 3], i % 7);
            if i % 10 == 0 { vec![line, format!("    at Handler.run(Handler.java:{i})")] } else { vec![line] }
        })
        .collect()
}

fn bytes(lines: &[String]) -> Vec<u32> {
    lines.iter().map(|l| l.len() as u32 + 1).collect()
}

impl Remote {
    fn send(&mut self, event: SourceEvent) {
        self.tx.try_send(event).unwrap();
        self.h.view.update(&mut self.h.cx, |app, cx| {
            app.pump_live(cx);
        });
        self.h.cx.run_until_parked();
    }

    fn tail(&mut self, l: Vec<String>) {
        let end = bytes(&l).iter().map(|b| u64::from(*b)).sum();
        self.send(SourceEvent::Tail { bytes: bytes(&l), lines: l, generation: 0, end_offset: end });
    }

    fn append(&mut self, l: Vec<String>) {
        self.send(SourceEvent::Append { bytes: bytes(&l), lines: l, streams: None });
    }

    fn view(&self) -> (Vec<(usize, u32, String, Option<String>)>, Vec<usize>, Vec<usize>, Option<(usize, bool)>) {
        self.h.view.read_with(&self.h.cx, |app: &AppView, _| {
            let rows = app
                .rows_snapshot()
                .iter()
                .map(|e| (e.index, e.generation, e.raw_line.to_string(), e.break_before.as_ref().map(|m| m.label.clone())))
                .collect();
            let hold = app.hold_info(FILE_ID).map(|i| (i.lines, i.extended));
            (rows, app.filter_pass.to_vec(), app.focus.search().matched_visible_indices.to_vec(), hold)
        })
    }
}

/// Run `steps` with the lines held in memory and through the cache, and
/// compare the views after each.
fn alike(cx: &mut TestAppContext, window: usize, steps: &[&dyn Fn(&mut Remote)]) {
    let mut ram = open(cx, false, window);
    let mut cached = open(&mut cx.clone(), true, window);
    for (n, step) in steps.iter().enumerate() {
        step(&mut ram);
        step(&mut cached);
        let (a, b) = (ram.view(), cached.view());
        let first = a.0.iter().zip(&b.0).position(|(x, y)| x != y);
        assert!(a.0.len() == b.0.len() && first.is_none(), "step {n}: rows {} / {}, first difference {:?} / {:?}", a.0.len(), b.0.len(), first.map(|i| &a.0[i]), first.map(|i| &b.0[i]));
        assert_eq!(a.1, b.1, "step {n}: Filter");
        assert_eq!(a.2, b.2, "step {n}: Matches");
        assert_eq!(a.3, b.3, "step {n}: what the file holds");
    }
    cached.h.view.read_with(&cached.h.cx, |app, _| {
        let parsed = &app.parsed_files[FILE_ID];
        assert!(parsed.indexed.is_some() && parsed.entries.is_empty() && parsed.lines.is_empty(), "no text in memory");
    });
}

#[gpui::test]
fn a_remote_file_through_its_cache_shows_what_it_shows_in_memory(cx: &mut TestAppContext) {
    alike(
        cx,
        200,
        &[
            &|r| r.tail(lines(400..460)),
            &|r| r.send(SourceEvent::TotalLines(Some(500))),
            &|r| r.send(SourceEvent::Backfill { lines: lines(350..400), generation: 0, bytes: bytes(&lines(350..400)) }),
            &|r| {
                r.h.view.update(&mut r.h.cx, |app, cx| {
                    app.filter_toggle_value("level", "error", cx);
                    app.trigger_search_query("user=3", cx);
                });
                r.h.cx.run_until_parked();
            },
            &|r| r.append(lines(460..480)),
            &|r| r.send(SourceEvent::Break { kind: BreakKind::Disconnected }),
            &|r| r.send(SourceEvent::Break { kind: BreakKind::Reconnected }),
            &|r| r.append(lines(480..500)),
            // Past the window: the oldest lines leave.
            &|r| r.append(lines(500..600)),
            &|r| r.send(SourceEvent::Break { kind: BreakKind::Rotated }),
            &|r| r.append(lines(0..30)),
        ],
    );
}

#[gpui::test]
fn load_more_clear_and_back_to_the_window_are_alike(cx: &mut TestAppContext) {
    alike(
        cx,
        100,
        &[
            &|r| r.tail(lines(300..360)),
            &|r| r.send(SourceEvent::TotalLines(Some(400))),
            &|r| {
                let older = lines(200..300);
                r.send(SourceEvent::Older { generation: 0, bytes: bytes(&older), lines: older, reached_start: false, before_start_time: false })
            },
            &|r| r.append(lines(360..380)),
            &|r| {
                r.h.view.update(&mut r.h.cx, |app, cx| app.back_to_live_window(FILE_ID, cx));
                r.h.cx.run_until_parked();
            },
            &|r| {
                r.send(SourceEvent::EarlierAvailable(Some(EarlierFile { name: "app.log.1".into(), size: 2048, compressed: false })))
            },
            &|r| {
                r.h.view.update(&mut r.h.cx, |app, cx| app.request_clear_file(FILE_ID, cx));
                r.h.cx.run_until_parked();
            },
            &|r| r.append(lines(380..390)),
        ],
    );
}

// ── Cleanup (task 8) ────────────────────────────────────────────────────────

impl Remote {
    fn space_id(&self) -> String {
        self.h.view.read_with(&self.h.cx, |app, _| app.workspace_state.active_project().unwrap().log_spaces.iter().find(|s| s.name == "Remote").unwrap().id.clone())
    }

    fn cache_dir(&self) -> std::path::PathBuf {
        self.h.view.read_with(&self.h.cx, |app, _| {
            let root = app.remote_cache_root_for_test();
            let target = log_gpui::remote::cache::CacheTarget::File { path: "/app/app.log".into() };
            log_gpui::remote::cache::dir_for(&root, "srv-1", &target)
        })
    }
}

#[gpui::test]
fn a_remote_file_removed_from_its_log_space_takes_its_cache_along(cx: &mut TestAppContext) {
    let mut r = open(cx, true, 1_000);
    r.tail(lines(0..50));
    let dir = r.cache_dir();
    assert!(dir.is_dir());
    let space = r.space_id();
    r.h.view.update(&mut r.h.cx, |app, cx| app.remove_file_from_space(&space, FILE_ID, cx));
    r.h.cx.run_until_parked();
    assert!(!dir.exists(), "its cache is gone");
    r.h.view.read_with(&r.h.cx, |app, _| assert!(app.live.cached.is_empty(), "nothing reads it"));
}

#[gpui::test]
fn a_deleted_log_space_takes_the_caches_of_its_files_along(cx: &mut TestAppContext) {
    let mut r = open(cx, true, 1_000);
    r.tail(lines(0..50));
    let dir = r.cache_dir();
    let space = r.space_id();
    r.h.view.update(&mut r.h.cx, |app, cx| {
        app.request_delete_log_space(&space, cx);
        app.accept_confirm(cx);
    });
    r.h.cx.run_until_parked();
    assert!(!dir.exists());
}

#[gpui::test]
fn a_cache_another_log_space_still_refers_to_stays(cx: &mut TestAppContext) {
    let mut r = open(cx, true, 1_000);
    r.tail(lines(0..50));
    let dir = r.cache_dir();
    let first = r.space_id();
    r.h.view.update(&mut r.h.cx, |app, _| {
        let project = app.workspace_state.active_project_mut().unwrap();
        let space = project.create_log_space("Remote 2", LogSpaceKind::Live).unwrap();
        space.add_remote_files(&[("srv-1".into(), "web1".into(), "/app/app.log".into())]).unwrap();
    });
    r.h.view.update(&mut r.h.cx, |app, cx| app.remove_file_from_space(&first, FILE_ID, cx));
    r.h.cx.run_until_parked();
    assert!(dir.is_dir(), "the other Log Space reads it next time");
}

#[gpui::test]
fn over_the_limit_the_caches_of_other_files_go_and_the_open_one_stays(cx: &mut TestAppContext) {
    let mut r = open(cx, true, 100_000);
    r.tail(lines(0..2_000));
    let open_dir = r.cache_dir();
    // A cache of a file no tab shows, opened earlier.
    let root = r.h.view.read_with(&r.h.cx, |app, _| app.remote_cache_root_for_test());
    let target = log_gpui::remote::cache::CacheTarget::File { path: "/app/other.log".into() };
    let other = log_gpui::remote::cache::dir_for(&root, "srv-1", &target);
    {
        let mut c = log_gpui::remote::cache::RemoteCache::open(&other, "srv-1", target, 1_000).unwrap();
        let l = lines(0..2_000);
        c.append(&l.iter().map(String::as_str).collect::<Vec<_>>()).unwrap();
    }
    r.h.view.update(&mut r.h.cx, |app, _| {
        // The other file is in a Log Space too, so only the limit removes it.
        let project = app.workspace_state.active_project_mut().unwrap();
        let space = project.create_log_space("Other", LogSpaceKind::Live).unwrap();
        space.add_remote_files(&[("srv-1".into(), "web1".into(), "/app/other.log".into())]).unwrap();
        app.live.cache_limit = 10_000;
        app.trim_remote_caches();
        assert!(app.notice.as_deref().is_some_and(|n| n.contains("open tabs")), "{:?}", app.notice);
    });
    assert!(!other.exists(), "the cache no tab shows went");
    assert!(open_dir.is_dir(), "the open tab's stayed");
    r.append(lines(2_000..2_010));
    r.h.view.read_with(&r.h.cx, |app, _| assert_eq!(app.rows_snapshot().len(), lines(0..2_010).len()));
}
