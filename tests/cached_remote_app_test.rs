//! A Remote File read through its Remote Cache in the real `AppView`
//! (openspec change `remote-local-cache`): the events give the view they
//! gave when the lines were held in memory, and the text is not in memory.
//! What the view showed then is kept in `tests/golden/cached_remote_app/`
//! (recorded from the cache path once it was checked equal to the memory
//! path, before that path was removed).

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
    id: String,
}

/// A Live Log Space of one Remote File, its source a fake the test feeds.
fn open(cx: &mut TestAppContext, window: usize) -> Remote {
    open_one(cx, window, false)
}

/// The same with a Remote Command in place of the file.
fn open_one(cx: &mut TestAppContext, window: usize, command: bool) -> Remote {
    let mut h = common::open_app(cx);
    let file_id = h.view.update(&mut h.cx, |app, cx| {
        app.live.window = window;
        let project = app.workspace_state.active_project_mut().unwrap();
        let space = project.create_log_space("Remote", LogSpaceKind::Live).unwrap();
        let file_id = if command {
            space.add_remote_commands(&[("srv-1".into(), "web1".into(), "tail -f app.log".into(), None)]).unwrap().added[0].clone()
        } else {
            space.add_remote_files(&[("srv-1".into(), "web1".into(), "/app/app.log".into())]).unwrap();
            FILE_ID.to_string()
        };
        let id = space.id.clone();
        app.open_log_space_tab(&id, cx);
        file_id
    });
    h.cx.run_until_parked();
    let (tx, events) = async_channel::unbounded();
    let (commands, _rx) = async_channel::unbounded();
    h.view.update(&mut h.cx, |app, _| {
        app.live.sources.insert(
            file_id.clone(),
            LiveSourceState {
                handle: SourceHandle { events, commands },
                seq: 1_000,
                status: SourceStatus::Live,
                name: "web1:/app/app.log".into(),
                remote: true,
                command,
                numbers_unavailable: false,
                health: Default::default(),
            },
        );
    });
    Remote { h, tx, id: file_id }
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

/// Rows (number, Generation, text, Break above, stream), the Filter, the
/// Matches, and what the file holds.
type View = (Vec<(usize, u32, String, Option<String>, Option<log_gpui::model::Stream>)>, Vec<usize>, Vec<usize>, Option<usize>);

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

    fn view(&self) -> View {
        self.h.view.read_with(&self.h.cx, |app: &AppView, _| {
            let rows = app
                .rows_snapshot()
                .iter()
                .map(|e| (e.index, e.generation, e.raw_line.to_string(), e.break_before.as_ref().map(|m| m.label.clone()), e.stream))
                .collect();
            let hold = app.hold_info(&self.id).map(|i| i.lines);
            (rows, app.filter_pass.to_vec(), app.focus.search().matched_visible_indices.to_vec(), hold)
        })
    }
}

/// The view as text: what the file holds, the Filter, the Matches, then
/// each row (number, Generation, stream, text) with the Break above it. The
/// time of day in a Break's label is left out.
fn render(view: &View) -> String {
    let clock = regex::Regex::new(r"\b\d\d:\d\d:\d\d\b").unwrap();
    let (rows, filter, matches, hold) = view;
    let mut out = format!("hold {hold:?}\nfilter {filter:?}\nmatches {matches:?}\n");
    for (index, generation, text, mark, stream) in rows {
        if let Some(mark) = mark {
            out.push_str(&format!("  -- {}\n", clock.replace_all(mark, "hh:mm:ss")));
        }
        let stream = stream.map_or("", |s| s.as_str());
        out.push_str(&format!("{index} g{generation} {stream} {text}\n"));
    }
    out
}

/// Run `steps` and compare the view after each with
/// `tests/golden/cached_remote_app/<name>.txt`; with `UPDATE_GOLDEN` set,
/// the file is written instead.
fn like_golden(cx: &mut TestAppContext, name: &str, window: usize, command: bool, steps: &[&dyn Fn(&mut Remote)]) {
    let mut r = open_one(cx, window, command);
    let mut have = String::new();
    for (n, step) in steps.iter().enumerate() {
        step(&mut r);
        have.push_str(&format!("## step {n}\n{}", render(&r.view())));
    }
    r.h.view.read_with(&r.h.cx, |app, _| {
        let parsed = &app.parsed_files[&r.id];
        assert!(parsed.indexed.is_some(), "read through its Line Index");
    });
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/cached_remote_app").join(format!("{name}.txt"));
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &have).unwrap();
        return;
    }
    let want = std::fs::read_to_string(&path).unwrap();
    for (n, (a, b)) in want.split("## step ").zip(have.split("## step ")).enumerate() {
        if a != b {
            let line = a.lines().zip(b.lines()).position(|(x, y)| x != y);
            panic!("{name}, step {}: first difference at line {line:?}:\nwant {:?}\nhave {:?}", n.saturating_sub(1), line.and_then(|l| a.lines().nth(l)), line.and_then(|l| b.lines().nth(l)));
        }
    }
    assert_eq!(want.len(), have.len(), "{name}: as many steps");
}

#[gpui::test]
fn a_remote_file_through_its_cache_shows_what_it_showed_in_memory(cx: &mut TestAppContext) {
    like_golden(
        cx,
        "remote_file",
        200,
        false,
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
fn clear_and_an_earlier_file_show_what_they_should(cx: &mut TestAppContext) {
    like_golden(
        cx,
        "clear_and_earlier",
        100,
        false,
        &[
            &|r| r.tail(lines(300..360)),
            &|r| r.send(SourceEvent::TotalLines(Some(400))),
            &|r| r.append(lines(360..380)),
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

// ── Cleanup (tasks 8 and 10.1) ──────────────────────────────────────────────

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
    let mut r = open(cx, 1_000);
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
    let mut r = open(cx, 1_000);
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

impl Remote {
    /// Open a tab of a new Live Log Space named `name`, holding the same
    /// Remote File when `with_file`.
    fn open_other_space(&mut self, name: &str, with_file: bool) -> String {
        let id = self.h.view.update(&mut self.h.cx, |app, cx| {
            let project = app.workspace_state.active_project_mut().unwrap();
            let space = project.create_log_space(name, LogSpaceKind::Live).unwrap();
            if with_file {
                space.add_remote_files(&[("srv-1".into(), "web1".into(), "/app/app.log".into())]).unwrap();
            }
            let id = space.id.clone();
            app.open_log_space_tab(&id, cx);
            id
        });
        self.h.cx.run_until_parked();
        id
    }

    /// Close the tab of Log Space `space_id`.
    fn close_tab_of(&mut self, space_id: &str) {
        self.h.view.update(&mut self.h.cx, |app, cx| {
            let tabs = app.workspace_state.active_project().unwrap().working_set.tabs();
            let at = tabs.iter().position(|t| t.target.space_id() == Some(space_id)).unwrap();
            app.close_tab_at(at, cx);
        });
        self.h.cx.run_until_parked();
    }
}

/// Decision 10: another tab keeps the cache, closing the tab drops it.
#[gpui::test]
fn switching_tabs_keeps_the_cache_and_closing_the_tab_drops_it(cx: &mut TestAppContext) {
    let mut r = open(cx, 1_000);
    r.tail(lines(0..50));
    let dir = r.cache_dir();
    let remote = r.space_id();
    r.open_other_space("Other", false);
    assert!(dir.is_dir(), "a Dormant Tab keeps its cache");
    r.close_tab_of(&remote);
    assert!(!dir.exists(), "no open tab shows the file: its cache goes");
}

#[gpui::test]
fn a_cache_another_open_tab_shows_stays_until_that_tab_closes(cx: &mut TestAppContext) {
    let mut r = open(cx, 1_000);
    r.tail(lines(0..50));
    let dir = r.cache_dir();
    let first = r.space_id();
    let second = r.open_other_space("Remote 2", true);
    r.close_tab_of(&first);
    assert!(dir.is_dir(), "the tab of the other Log Space still shows it");
    r.close_tab_of(&second);
    assert!(!dir.exists());
}

#[gpui::test]
fn caches_left_by_a_run_that_stopped_short_go_when_the_app_opens(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let root = log_gpui::remote::cache::root_in(dir.path());
    let target = log_gpui::remote::cache::CacheTarget::File { path: "/app/app.log".into() };
    let left = log_gpui::remote::cache::dir_for(&root, "srv-1", &target);
    let mut cache = log_gpui::remote::cache::RemoteCache::open(&left, "srv-1", target, 100).unwrap();
    cache.append(&["2026-09-05 10:00:00 [INFO] left over"]).unwrap();
    drop(cache);
    let h = common::open_app_in(cx, dir);
    assert!(!left.exists(), "nothing is kept from a run before");
    drop(h);
}

/// Decision 10: a cache that cannot be written stops the file, with a
/// notice; the lines it has stay (task 10.5).
#[gpui::test]
fn a_cache_that_cannot_be_written_stops_the_file_and_keeps_its_lines(cx: &mut TestAppContext) {
    use std::os::unix::fs::PermissionsExt as _;
    let mut r = open(cx, 1_000);
    r.tail(lines(0..50));
    let rows = r.view().0.len();
    // Nothing in its folder can be written any more, as on a full disk.
    let set_mode = |dir: &std::path::Path, mode: u32| {
        for entry in walk(dir) {
            std::fs::set_permissions(&entry, std::fs::Permissions::from_mode(if entry.is_dir() { mode | 0o100 } else { mode })).unwrap();
        }
    };
    let dir = r.cache_dir();
    set_mode(&dir, 0o444);
    r.append(lines(50..60));
    set_mode(&dir, 0o644);
    r.h.view.read_with(&r.h.cx, |app, _| {
        let notice = app.notice.clone().unwrap_or_default();
        assert!(notice.contains("not read any further"), "{notice}");
        assert!(matches!(app.live.sources[FILE_ID].status, log_gpui::live::SourceStatus::Error(_)));
    });
    assert_eq!(r.view().0.len(), rows, "the lines it had stay");
    // What the source still sends is not read.
    r.append(lines(60..70));
    assert_eq!(r.view().0.len(), rows, "read no further");
}

/// Every folder and file under `dir`, folders after what they hold.
fn walk(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(walk(&path));
        }
        out.push(path);
    }
    out
}

// ── Remote Commands (task 6.1) ──────────────────────────────────────────────

impl Remote {
    /// Lines of a command's output, every third on stderr.
    fn output(&mut self, l: Vec<String>) {
        let streams: log_gpui::live::source::StreamMask = (0..l.len())
            .map(|i| if i % 3 == 0 { log_gpui::model::Stream::Stderr } else { log_gpui::model::Stream::Stdout })
            .collect();
        self.send(SourceEvent::Append { bytes: Vec::new(), lines: l, streams: Some(streams) });
    }
}

#[gpui::test]
fn a_remote_command_through_its_cache_shows_what_it_showed_in_memory(cx: &mut TestAppContext) {
    use log_gpui::live::source::RestartKind;
    like_golden(
        cx,
        "remote_command",
        100,
        true,
        &[
            &|r| r.output(lines(0..40)),
            &|r| {
                r.h.view.update(&mut r.h.cx, |app, cx| app.filter_toggle_value("stream", "stderr", cx));
                r.h.cx.run_until_parked();
            },
            &|r| r.output(lines(40..60)),
            &|r| r.send(SourceEvent::Break { kind: BreakKind::Exited(Some(1)) }),
            &|r| r.send(SourceEvent::Break { kind: BreakKind::Restarted(RestartKind::OnExit) }),
            &|r| r.output(lines(60..70)),
            &|r| r.send(SourceEvent::Break { kind: BreakKind::Disconnected }),
            &|r| r.send(SourceEvent::Break { kind: BreakKind::Resumed(36_000) }),
            &|r| r.output(lines(70..80)),
            &|r| r.send(SourceEvent::Break { kind: BreakKind::Skipped(5) }),
            &|r| r.output(lines(85..90)),
            &|r| r.send(SourceEvent::CommandChanged { from: "web1$ tail -f app.log".into(), to: "web1$ tail -f other.log".into() }),
            &|r| r.output(lines(90..105)),
            // Past the window: the oldest lines leave.
            &|r| r.output(lines(105..205)),
            &|r| {
                let id = r.id.clone();
                r.h.view.update(&mut r.h.cx, |app, cx| app.request_clear_file(&id, cx));
                r.h.cx.run_until_parked();
            },
            &|r| r.output(lines(205..210)),
        ],
    );
}

