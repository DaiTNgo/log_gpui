//! Import from `~/.ssh/config` and Refresh in the Servers dialog, and the
//! Server Group list with its search.

mod common;

use common::Harness;
use gpui::TestAppContext;
use log_gpui::remote::ssh_config::{ConfigFs, Env};
use log_gpui::remote::{AuthMethod, Server};
use std::collections::HashMap;

struct Files(HashMap<String, String>);

impl ConfigFs for Files {
    fn read(&self, path: &str) -> Option<String> {
        self.0.get(path).cloned()
    }
    fn list(&self, _dir: &str) -> Vec<String> {
        Vec::new()
    }
    fn exists(&self, path: &str) -> bool {
        self.0.contains_key(path)
    }
}

fn config(text: &str) -> (Files, Env) {
    let env = Env { home: "/home/me".into(), local_user: "me".into() };
    (Files(HashMap::from([("/home/me/.ssh/config".to_string(), text.to_string())])), env)
}

fn open_dialog(cx: &mut TestAppContext) -> Harness {
    let mut h = common::open_app(cx);
    h.view.update_in(&mut h.cx, |app, window, cx| {
        app.open_servers_dialog(cx);
        app.sync_server_dialog(window, cx);
    });
    h
}

fn open_import(h: &mut Harness, text: &str) {
    let (fs, env) = config(text);
    h.view.update_in(&mut h.cx, |app, window, cx| {
        app.open_ssh_import_with(&fs, &env, cx);
        app.sync_server_dialog(window, cx);
    });
    h.cx.run_until_parked();
}

const CONFIG: &str = "Host web1\n  HostName 10.0.0.5\n  User deploy\nHost db\n  HostName 10.0.1.9\n  Port 2222\n  ProxyJump web1\nHost legacy\n  ProxyCommand ssh -W %h:%p gw\nMatch host x\n  User y\n";

#[gpui::test]
fn the_ticked_hosts_are_imported_into_the_group_typed(cx: &mut TestAppContext) {
    let mut h = open_dialog(cx);
    open_import(&mut h, CONFIG);
    h.view.update_in(&mut h.cx, |app, window, cx| {
        let import = app.server_dialog.as_ref().unwrap().import.as_ref().unwrap();
        assert!(import.config_found);
        let aliases: Vec<_> = import.new.iter().map(|c| c.alias.clone()).collect();
        assert_eq!(aliases, vec!["web1", "db", "legacy"]);
        // ProxyCommand is shown but not ticked; the Match block is skipped.
        assert!(import.ticked.contains("web1") && import.ticked.contains("db") && !import.ticked.contains("legacy"));
        assert_eq!(import.skipped.len(), 1);
        let group = import.group_input.clone().unwrap();
        group.update(cx, |state, cx| state.set_value("prod".to_string(), window, cx));
        app.apply_ssh_import(cx);
    });
    h.view.read_with(&h.cx, |app, _| {
        assert!(app.server_dialog.as_ref().unwrap().import.is_none());
        let names: Vec<_> = app.remote.servers.servers.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["web1", "db"]);
        assert!(app.remote.servers.servers.iter().all(|s| s.group.as_deref() == Some("prod")));
        let db = app.remote.servers.by_name("db").unwrap();
        assert_eq!(db.port, 2222);
        assert_eq!(db.jump_server_id.as_deref(), Some(app.remote.servers.by_name("web1").unwrap().id.as_str()));
    });
}

#[gpui::test]
fn a_missing_config_file_creates_nothing(cx: &mut TestAppContext) {
    let mut h = open_dialog(cx);
    let fs = Files(HashMap::new());
    let env = Env { home: "/home/me".into(), local_user: "me".into() };
    h.view.update_in(&mut h.cx, |app, _, cx| app.open_ssh_import_with(&fs, &env, cx));
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| {
        assert!(!app.server_dialog.as_ref().unwrap().import.as_ref().unwrap().config_found);
        assert!(app.remote.servers.servers.is_empty());
    });
}

#[gpui::test]
fn a_taken_name_is_renamed_on_its_row(cx: &mut TestAppContext) {
    let mut h = open_dialog(cx);
    h.view.update(&mut h.cx, |app, _| {
        app.remote.servers.save(Server::new("web1", "1.1.1.1", "me", AuthMethod::Password)).unwrap();
    });
    open_import(&mut h, "Host web1\n  HostName 10.0.0.5\n");
    h.view.update_in(&mut h.cx, |app, window, cx| {
        let import = app.server_dialog.as_ref().unwrap().import.as_ref().unwrap();
        let row = &import.new[0];
        assert!(row.name_taken && !import.ticked.contains("web1"));
        app.toggle_import_row("web1", cx);
        let input = app.server_dialog.as_ref().unwrap().import.as_ref().unwrap().name_inputs["web1"].clone();
        input.update(cx, |state, cx| state.set_value("web1-cfg".to_string(), window, cx));
        app.apply_ssh_import(cx);
    });
    h.view.read_with(&h.cx, |app, _| {
        assert_eq!(app.remote.servers.by_name("web1").unwrap().host, "1.1.1.1");
        assert_eq!(app.remote.servers.by_name("web1-cfg").unwrap().host, "10.0.0.5");
    });
}

#[gpui::test]
fn refresh_applies_only_the_ticked_changes(cx: &mut TestAppContext) {
    let mut h = open_dialog(cx);
    open_import(&mut h, "Host db\n  HostName 10.0.1.9\nHost old\n  HostName o\n");
    h.view.update(&mut h.cx, |app, cx| app.apply_ssh_import(cx));
    assert_eq!(h.view.read_with(&h.cx, |app, _| app.remote.servers.servers.len()), 2);

    open_import(&mut h, "Host db\n  HostName 10.0.1.9\n  Port 2222\n  User ops\n");
    let (db_id, kinds) = h.view.read_with(&h.cx, |app, _| {
        let import = app.server_dialog.as_ref().unwrap().import.as_ref().unwrap();
        let db = app.remote.servers.by_name("db").unwrap().id.clone();
        let kinds: Vec<String> = import
            .known
            .iter()
            .map(|r| match r {
                log_gpui::remote::ssh_config::RefreshRow::Changed { name, changes, .. } => {
                    format!("{name}:{}", changes.iter().map(|c| c.field).collect::<Vec<_>>().join("+"))
                }
                log_gpui::remote::ssh_config::RefreshRow::Gone { name, .. } => format!("{name}:gone"),
                log_gpui::remote::ssh_config::RefreshRow::Same { name, .. } => format!("{name}:same"),
            })
            .collect();
        (db, kinds)
    });
    assert_eq!(kinds, vec!["db:port+user", "old:gone"]);
    // Everything changed is ticked; untick the user.
    h.view.update(&mut h.cx, |app, cx| app.toggle_import_change(&db_id, "user", cx));
    h.view.update(&mut h.cx, |app, cx| app.apply_ssh_import(cx));
    h.view.read_with(&h.cx, |app, _| {
        let db = app.remote.servers.by_name("db").unwrap();
        assert_eq!((db.port, db.user.as_str()), (2222, "me"));
        // The Server that left the config stays.
        assert!(app.remote.servers.by_name("old").is_some());
    });
}

#[gpui::test]
fn folding_a_group_is_remembered(cx: &mut TestAppContext) {
    let mut h = open_dialog(cx);
    h.view.update(&mut h.cx, |app, cx| {
        let mut s = Server::new("web1", "10.0.0.5", "me", AuthMethod::Password);
        s.group = Some("prod".into());
        app.remote.servers.save(s).unwrap();
        app.toggle_server_group("prod", cx);
    });
    h.cx.run_until_parked();
    let folded = h.view.read_with(&h.cx, |app, _| app.remote.picker_state.is_group_collapsed("prod"));
    assert!(folded);
    h.view.update(&mut h.cx, |app, cx| app.toggle_server_group("prod", cx));
    assert!(!h.view.read_with(&h.cx, |app, _| app.remote.picker_state.is_group_collapsed("prod")));
}
