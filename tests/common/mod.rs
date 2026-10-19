//! GPUI test harness: the real `AppView` in a test window.
//!
//! Everything the app would write or read from the user's machine is pointed
//! at a temp dir (session, `servers.json`, `known_hosts.json`) and remembered
//! secrets live in memory instead of the OS keychain. This goes through
//! `AppView::with_services`; nothing else in the app knows about tests.
//!
//! Some app code still writes to the config dir directly (saving Servers,
//! known hosts, scratch files), so before any app opens the whole test
//! process gets its own `XDG_CONFIG_HOME` in a temp dir, and the harness
//! refuses to run if the config dir is not inside it.
#![allow(dead_code)]

use gpui::{AppContext as _, Entity, TestAppContext, VisualTestContext};
use gpui_component::Root;
use log_gpui::app::AppView;
use log_gpui::app::remote::RemoteUi;
use log_gpui::remote::MemorySecrets;
use log_gpui::workspace::SessionPersistence;
use log_gpui::workspace::log_space::{ImportSource, LogSpaceKind};
use std::sync::Arc;

pub struct Harness {
    pub view: Entity<AppView>,
    pub cx: VisualTestContext,
    /// Holds config and the log files; removed when the harness drops.
    pub dir: tempfile::TempDir,
}

/// Point the config dir of this test process at a temp dir, once.
fn isolate_config_dir() {
    static CONFIG: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    let dir = CONFIG.get_or_init(|| {
        let dir = tempfile::tempdir().unwrap();
        // SAFETY: set once, before any app (or thread of one) reads it.
        unsafe { std::env::set_var("XDG_CONFIG_HOME", dir.path()) };
        dir
    });
    let config = log_gpui::workspace::session::app_data_dir();
    assert!(config.starts_with(dir.path()), "config dir {config:?} is not isolated");
}

/// Open `AppView` in a test window with config in a fresh temp dir.
pub fn open_app(cx: &mut TestAppContext) -> Harness {
    isolate_config_dir();
    cx.update(|cx| {
        gpui_component::init(cx);
    });
    let dir = tempfile::tempdir().unwrap();
    let persistence = SessionPersistence::new(dir.path().join("session.json"));
    let remote = RemoteUi::with_paths(
        &dir.path().join("servers.json"),
        &dir.path().join("known_hosts.json"),
        Arc::new(MemorySecrets::default()),
    );
    let mut view = None;
    let window = cx.add_window(|window, cx| {
        let app = cx.new(|cx| AppView::with_services(Some(persistence), remote, window, cx));
        view = Some(app.clone());
        Root::new(app, window, cx)
    });
    let vcx = VisualTestContext::from_window(window.into(), cx);
    Harness { view: view.unwrap(), cx: vcx, dir }
}

/// Open `AppView` on an Imported Log Space holding one file of `lines`.
pub fn open_app_with_lines(cx: &mut TestAppContext, lines: &[String]) -> Harness {
    let mut h = open_app(cx);
    let path = h.dir.path().join("app.log");
    std::fs::write(&path, lines.join("\n")).unwrap();
    let source = ImportSource::Disk(path.to_string_lossy().into_owned());
    h.view.update(&mut h.cx, |app, cx| {
        app.create_log_space("Test", LogSpaceKind::Imported, &[source], cx);
    });
    h.cx.run_until_parked();
    h
}

pub fn sample_lines(n: usize) -> Vec<String> {
    (1..=n)
        .map(|i| format!("2026-10-19 10:00:{:02} INFO line {i}", i % 60))
        .collect()
}
