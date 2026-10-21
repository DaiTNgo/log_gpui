//! Quick Connect in the file picker, against a local sshd (see
//! `remote_hub_test.rs`): `SSH_TEST_DIR`, `SSH_TEST_PORT`. Skipped when unset.

mod common;

use common::Harness;
use gpui::TestAppContext;
use log_gpui::app::remote_picker::{PickerTarget, Staged};
use log_gpui::remote::{AuthMethod, Server};
#[cfg(unix)]
use std::process::Command;
use std::time::{Duration, Instant};

struct Ssh {
    dir: String,
    key: String,
    port: u16,
    user: String,
}

fn ssh() -> Option<Ssh> {
    let dir = std::env::var("SSH_TEST_DIR").ok()?;
    Some(Ssh {
        key: format!("{dir}/userkey"),
        dir,
        port: std::env::var("SSH_TEST_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(2222),
        user: std::env::var("USER").ok()?,
    })
}

#[cfg(unix)]
/// A throwaway ssh-agent that the app's "System" agent socket points at.
struct Agent {
    pid: String,
}

#[cfg(unix)]
fn start_agent(dir: &std::path::Path, keys: &[&str]) -> Agent {
    let socket = dir.join("agent.sock").display().to_string();
    let out = Command::new("ssh-agent").args(["-a", &socket]).output().expect("ssh-agent");
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let pid = text.split("SSH_AGENT_PID=").nth(1).and_then(|r| r.split(';').next()).unwrap_or_default().to_string();
    for key in keys {
        let out = Command::new("ssh-add").arg(key).env("SSH_AUTH_SOCK", &socket).output().unwrap();
        assert!(out.status.success());
    }
    // SAFETY: tests of this file run one after another (see `serial`).
    unsafe { std::env::set_var("SSH_AUTH_SOCK", &socket) };
    Agent { pid }
}

#[cfg(unix)]
impl Drop for Agent {
    fn drop(&mut self) {
        let _ = Command::new("kill").arg(&self.pid).status();
    }
}

/// The tests change the process environment: one at a time.
fn serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn wait_for(h: &mut Harness, mut done: impl FnMut(&log_gpui::app::AppView) -> bool) {
    h.cx.executor().allow_parking();
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        h.cx.run_until_parked();
        let finished = h.view.update(&mut h.cx, |app, _| {
            if let Some(request) = app.remote.host_prompts.pop_front() {
                let _ = request.reply.send(true);
            }
            done(app)
        });
        if finished {
            return;
        }
        if Instant::now() >= deadline {
            let state = h.view.read_with(&h.cx, |app, _| {
                app.remote.picker.as_ref().map(|p| (p.error.clone(), p.busy.clone(), p.quick_pending.clone(), p.server_id.clone()))
            });
            panic!("timed out: {state:?}");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn idle(app: &log_gpui::app::AppView) -> bool {
    app.remote.picker.as_ref().is_some_and(|p| p.busy.is_none() && p.quick_pending.is_none())
}

fn open_picker(h: &mut Harness, ssh: &Ssh) {
    // One saved Server so the picker opens.
    let mut server = Server::new("saved", "127.0.0.1", &ssh.user, AuthMethod::Key { path: ssh.key.clone() });
    server.port = ssh.port;
    h.view.update(&mut h.cx, |app, _| {
        app.remote.servers.save(server).unwrap();
        *app.remote.shared_servers.lock().unwrap() = app.remote.servers.clone();
    });
    h.view.update_in(&mut h.cx, |app, window, cx| {
        app.open_remote_picker(PickerTarget::NewSpace, cx);
        app.sync_remote_picker(window, cx);
    });
    wait_for(h, idle);
}

fn quick(h: &mut Harness, text: &str) {
    let text = text.to_string();
    h.view.update(&mut h.cx, |app, cx| app.picker_quick_connect_text(&text, cx));
}

fn staged(h: &mut Harness) -> Vec<Staged> {
    h.view.read_with(&h.cx, |app, _| app.remote.picker.as_ref().map(|p| p.staged.clone()).unwrap_or_default())
}

#[gpui::test]
fn a_server_name_adds_the_file_without_a_new_server(cx: &mut TestAppContext) {
    let _serial = serial();
    let Some(ssh) = ssh() else { return };
    let mut h = common::open_app(cx);
    open_picker(&mut h, &ssh);
    let root = tempfile::tempdir().unwrap();
    let path = format!("{}/app.log", root.path().display());
    quick(&mut h, &format!("saved:{path}"));
    wait_for(&mut h, |app| idle(app) && !app.remote.picker.as_ref().unwrap().staged.is_empty());
    let staged = staged(&mut h);
    assert!(matches!(&staged[..], [Staged::File { path: p, .. }] if *p == path), "{staged:?}");
    h.view.update(&mut h.cx, |app, cx| app.picker_confirm(cx));
    assert_eq!(h.view.read_with(&h.cx, |app, _| app.remote.servers.servers.len()), 1);
}

#[gpui::test]
fn the_same_address_reuses_the_saved_server(cx: &mut TestAppContext) {
    let _serial = serial();
    let Some(ssh) = ssh() else { return };
    let mut h = common::open_app(cx);
    open_picker(&mut h, &ssh);
    let root = tempfile::tempdir().unwrap();
    let path = format!("{}/app.log", root.path().display());
    quick(&mut h, &format!("{}@127.0.0.1:{}:{path}", ssh.user, ssh.port));
    wait_for(&mut h, |app| idle(app) && !app.remote.picker.as_ref().unwrap().staged.is_empty());
    let saved_id = h.view.read_with(&h.cx, |app, _| app.remote.servers.servers[0].id.clone());
    assert_eq!(staged(&mut h)[0].server_id(), saved_id);
    h.view.read_with(&h.cx, |app, _| assert!(app.remote.picker.as_ref().unwrap().quick_servers.is_empty()));
}

#[cfg(unix)]
#[gpui::test]
fn a_new_host_connects_with_the_agent_and_is_saved_only_with_a_file(cx: &mut TestAppContext) {
    let _serial = serial();
    let Some(ssh) = ssh() else { return };
    let agent_dir = tempfile::tempdir().unwrap();
    let _agent = start_agent(agent_dir.path(), &[&ssh.key]);
    let mut h = common::open_app(cx);
    open_picker(&mut h, &ssh);
    let root = tempfile::tempdir().unwrap();
    let path = format!("{}/syslog", root.path().display());
    // `localhost` is another address than the saved `127.0.0.1`.
    quick(&mut h, &format!("{}@localhost:{}:{path}", ssh.user, ssh.port));
    wait_for(&mut h, |app| idle(app) && !app.remote.picker.as_ref().unwrap().staged.is_empty());
    // Not saved while the picker is open.
    assert_eq!(h.view.read_with(&h.cx, |app, _| app.remote.servers.servers.len()), 1);
    h.view.update(&mut h.cx, |app, cx| app.picker_confirm(cx));
    h.view.read_with(&h.cx, |app, _| {
        let name = format!("{}@localhost:{}", ssh.user, ssh.port);
        let saved = app.remote.servers.by_name(&name).expect("saved");
        assert!(matches!(saved.auth, AuthMethod::Agent { .. }));
        assert_eq!(saved.origin, Some(log_gpui::remote::Origin::QuickConnect));
        assert_eq!(app.remote.new_space_files[0].0, saved.id);
    });
}

#[cfg(unix)]
#[gpui::test]
fn closing_the_picker_without_a_file_saves_nothing(cx: &mut TestAppContext) {
    let _serial = serial();
    let Some(ssh) = ssh() else { return };
    let agent_dir = tempfile::tempdir().unwrap();
    let _agent = start_agent(agent_dir.path(), &[&ssh.key]);
    let mut h = common::open_app(cx);
    open_picker(&mut h, &ssh);
    let root = tempfile::tempdir().unwrap();
    quick(&mut h, &format!("{}@localhost:{}:{}/x.log", ssh.user, ssh.port, root.path().display()));
    wait_for(&mut h, |app| idle(app) && !app.remote.picker.as_ref().unwrap().staged.is_empty());
    h.view.update(&mut h.cx, |app, cx| app.close_remote_picker(cx));
    assert_eq!(h.view.read_with(&h.cx, |app, _| app.remote.servers.servers.len()), 1);
}

#[cfg(unix)]
#[gpui::test]
fn a_refused_agent_asks_for_a_key_file_and_saves_that_sign_in(cx: &mut TestAppContext) {
    let _serial = serial();
    let Some(ssh) = ssh() else { return };
    // An agent with no keys: it is refused.
    let agent_dir = tempfile::tempdir().unwrap();
    let _agent = start_agent(agent_dir.path(), &[]);
    let mut h = common::open_app(cx);
    open_picker(&mut h, &ssh);
    quick(&mut h, &format!("{}@localhost:{}:/tmp/quick.log", ssh.user, ssh.port));
    wait_for(&mut h, |app| app.remote.picker.as_ref().is_some_and(|p| p.quick_login.is_some()));
    let asked = h.view.read_with(&h.cx, |app, _| {
        let login = app.remote.picker.as_ref().unwrap().quick_login.as_ref().unwrap();
        login.server_name.clone()
    });
    assert_eq!(asked, format!("{}@localhost:{}", ssh.user, ssh.port));
    // Choose a key file and sign in.
    h.view.update_in(&mut h.cx, |app, window, cx| {
        app.quick_login_set_key(true, cx);
        app.sync_remote_picker(window, cx);
        let login = app.remote.picker.as_ref().unwrap().quick_login.as_ref().unwrap();
        login.key_path.clone().unwrap().update(cx, |state, cx| state.set_value(ssh.key.clone(), window, cx));
        app.quick_login_submit(cx);
    });
    wait_for(&mut h, |app| idle(app) && !app.remote.picker.as_ref().unwrap().staged.is_empty());
    h.view.update(&mut h.cx, |app, cx| app.picker_confirm(cx));
    h.view.read_with(&h.cx, |app, _| {
        let saved = app.remote.servers.by_name(&format!("{}@localhost:{}", ssh.user, ssh.port)).expect("saved");
        assert_eq!(saved.auth, AuthMethod::Key { path: ssh.key.clone() });
    });
    let _ = ssh.dir;
}

#[gpui::test]
fn a_string_of_the_wrong_shape_says_what_is_expected(cx: &mut TestAppContext) {
    let _serial = serial();
    let Some(ssh) = ssh() else { return };
    let mut h = common::open_app(cx);
    open_picker(&mut h, &ssh);
    quick(&mut h, "10.0.2.7/var/log");
    let (error, staged) = h.view.read_with(&h.cx, |app, _| {
        let p = app.remote.picker.as_ref().unwrap();
        (p.error.clone(), p.staged.len())
    });
    assert_eq!(error.as_deref(), Some("Expected user@host[:port]:/path"));
    assert_eq!(staged, 0);
}

#[gpui::test]
fn a_home_relative_path_means_the_home_folder_on_the_server(cx: &mut TestAppContext) {
    let _serial = serial();
    let Some(ssh) = ssh() else { return };
    let mut h = common::open_app(cx);
    open_picker(&mut h, &ssh);
    quick(&mut h, "saved:~/not-there-yet.log");
    wait_for(&mut h, |app| idle(app) && !app.remote.picker.as_ref().unwrap().staged.is_empty());
    let home = std::env::var("HOME").unwrap();
    let path = staged(&mut h)[0].path();
    assert!(path.ends_with("/not-there-yet.log") && !path.contains('~'), "{path}");
    // The sshd runs as this user, so its home is ours.
    assert_eq!(std::fs::canonicalize(&home).unwrap(), std::fs::canonicalize(path.trim_end_matches("/not-there-yet.log")).unwrap());
}
