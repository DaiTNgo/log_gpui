use log_gpui::remote::*;

fn server(name: &str) -> Server {
    Server::new(name, "10.0.0.5", "deploy", AuthMethod::Password)
}

#[test]
fn names_must_be_unique_and_not_blank() {
    let mut store = ServerStore::default();
    store.save(server("prod-web1")).unwrap();
    assert_eq!(store.save(server("prod-web1")), Err(ServerError::DuplicateName));
    assert_eq!(store.save(server("  ")), Err(ServerError::BlankName));
}

#[test]
fn port_defaults_to_22_and_servers_round_trip() {
    let mut store = ServerStore::default();
    let id = store.save(server("a")).unwrap();
    assert_eq!(store.get(&id).unwrap().port, 22);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("servers.json");
    store.save_to(&path).unwrap();
    assert_eq!(ServerStore::load(&path), store);
}

fn chained(store: &mut ServerStore, name: &str, jump: Option<&str>) -> String {
    let mut s = server(name);
    s.jump_server_id = jump.map(str::to_string);
    store.save(s).unwrap()
}

#[test]
fn a_chain_lists_jump_servers_outermost_first() {
    let mut store = ServerStore::default();
    let bastion = chained(&mut store, "bastion", None);
    let db = chained(&mut store, "db", Some(&bastion));
    let names: Vec<&str> = store.chain(&db).unwrap().iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, vec!["bastion", "db"]);
}

#[test]
fn a_loop_is_refused() {
    let mut store = ServerStore::default();
    let a = chained(&mut store, "a", None);
    let b = chained(&mut store, "b", Some(&a));
    let mut a2 = store.get(&a).unwrap().clone();
    a2.jump_server_id = Some(b);
    assert_eq!(store.save(a2), Err(ServerError::JumpLoop));
    let mut self_jump = store.get(&a).unwrap().clone();
    self_jump.jump_server_id = Some(a);
    assert_eq!(store.save(self_jump), Err(ServerError::JumpLoop));
}

#[test]
fn more_than_four_jump_servers_is_refused() {
    let mut store = ServerStore::default();
    let mut prev: Option<String> = None;
    // a -> b -> c -> d -> e: e has four Jump Servers.
    for name in ["a", "b", "c", "d", "e"] {
        prev = Some(chained(&mut store, name, prev.as_deref()));
    }
    let mut f = server("f");
    f.jump_server_id = prev;
    assert_eq!(store.save(f), Err(ServerError::TooManyJumps));
}

#[test]
fn editing_a_middle_server_cannot_make_a_chain_too_long() {
    let mut store = ServerStore::default();
    let a = chained(&mut store, "a", None);
    let b = chained(&mut store, "b", None);
    let c = chained(&mut store, "c", Some(&b));
    let d = chained(&mut store, "d", Some(&c));
    let e = chained(&mut store, "e", Some(&d));
    // b -> a would make e go through four Jump Servers: ok (a,b,c,d).
    let mut b2 = store.get(&b).unwrap().clone();
    b2.jump_server_id = Some(a.clone());
    assert!(store.save(b2).is_ok());
    let _ = e;
    // One more level above is too many.
    let z = chained(&mut store, "z", None);
    let mut a2 = store.get(&a).unwrap().clone();
    a2.jump_server_id = Some(z);
    assert_eq!(store.save(a2), Err(ServerError::TooManyJumps));
}

#[test]
fn a_server_in_use_is_not_deleted_and_says_by_what() {
    let mut store = ServerStore::default();
    let web = chained(&mut store, "prod-web1", None);
    let err = store.delete(&web, vec!["Payment (Project A)".into()]).unwrap_err();
    assert_eq!(err.message(), "Used by: Payment (Project A)");
    assert!(store.get(&web).is_some());
    let bastion = chained(&mut store, "bastion", None);
    chained(&mut store, "db", Some(&bastion));
    assert!(matches!(store.delete(&bastion, vec![]), Err(ServerError::InUse(_))));
    let staging = chained(&mut store, "staging", None);
    assert!(store.delete(&staging, vec![]).is_ok());
    assert!(store.get(&staging).is_none());
}

#[test]
fn secrets_use_per_server_accounts() {
    let secrets = MemorySecrets::default();
    let mut s = server("x");
    s.id = "srv-9".into();
    assert_eq!(s.secret_account(), "srv-9:password");
    s.auth = AuthMethod::Key { path: "/k".into() };
    assert_eq!(s.secret_account(), "srv-9:passphrase");
    secrets.set(&s.secret_account(), "pw").unwrap();
    assert_eq!(secrets.get("srv-9:passphrase").as_deref(), Some("pw"));
    secrets.delete("srv-9:passphrase");
    assert!(secrets.get("srv-9:passphrase").is_none());
}

#[test]
fn host_keys_are_unknown_trusted_or_changed() {
    let mut hosts = KnownHosts::default();
    assert_eq!(hosts.check("10.0.0.5:22", "SHA256:aaa"), HostKeyStatus::Unknown);
    hosts.trust("10.0.0.5:22", "SHA256:aaa");
    assert_eq!(hosts.check("10.0.0.5:22", "SHA256:aaa"), HostKeyStatus::Trusted);
    assert_eq!(
        hosts.check("10.0.0.5:22", "SHA256:bbb"),
        HostKeyStatus::Changed { trusted: "SHA256:aaa".into() }
    );
    // Another port is another address.
    assert_eq!(hosts.check("10.0.0.5:2222", "SHA256:aaa"), HostKeyStatus::Unknown);
    hosts.trust("10.0.0.5:22", "SHA256:bbb");
    assert_eq!(hosts.check("10.0.0.5:22", "SHA256:bbb"), HostKeyStatus::Trusted);
}

/// Touches the real Keychain with a throwaway item; run with `--ignored`.
#[test]
#[ignore]
fn keychain_round_trip() {
    let k = Keychain;
    let account = "test:roundtrip";
    k.set(account, "s3cret").unwrap();
    assert_eq!(k.get(account).as_deref(), Some("s3cret"));
    k.delete(account);
    assert!(k.get(account).is_none());
}

#[test]
fn glob_matching_and_path_helpers() {
    use log_gpui::remote::browse::*;
    assert!(glob_match("*.log", "a.log"));
    assert!(!glob_match("*.log", "a.log.1"));
    assert!(glob_match("app-?.log", "app-1.log"));
    assert!(glob_match("app-[0-9].log", "app-7.log"));
    assert!(!glob_match("app-[0-9].log", "app-x.log"));
    assert!(glob_match("[!a]*", "bcd"));
    assert!(is_glob("/var/log/*.log") && !is_glob("/var/log/app.log"));
    assert_eq!(dirname("/var/log/app.log"), "/var/log");
    assert_eq!(dirname("/app.log"), "/");
    assert_eq!(join("/var/log", "a.log"), "/var/log/a.log");
    assert_eq!(join("/", "a.log"), "/a.log");
}

#[test]
fn downloaded_files_are_named_apart_by_server_when_they_clash() {
    use log_gpui::remote::download::download_names;
    use std::collections::HashSet;
    let files = vec![
        ("web1".to_string(), "/app/app.log".to_string()),
        ("web2".to_string(), "/app/app.log".to_string()),
        ("db".to_string(), "/var/log/pg.log".to_string()),
    ];
    assert_eq!(
        download_names(&files, &HashSet::new()),
        vec!["web1-app.log", "web2-app.log", "pg.log"]
    );
    let existing = HashSet::from(["pg.log".to_string()]);
    assert_eq!(download_names(&files, &existing)[2], "db-pg.log");
}
