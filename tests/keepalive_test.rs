use log_gpui::remote::*;

fn server(name: &str) -> Server {
    Server::new(name, "10.0.0.5", "deploy", AuthMethod::Password)
}

#[test]
fn keepalive_defaults_for_servers_saved_before_it_existed() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("servers.json");
    std::fs::write(
        &path,
        r#"{"servers":[{"id":"srv-1","name":"old","host":"h","user":"u","auth":"Password"}],"next_id":1}"#,
    )
    .unwrap();
    let store = ServerStore::load(&path);
    let old = store.get("srv-1").expect("old server is read");
    assert_eq!((old.keepalive_interval_secs, old.keepalive_max_missed), (30, 3));
}

#[test]
fn keepalive_values_are_kept_within_their_ranges() {
    let mut store = ServerStore::default();
    let mut s = server("a");
    s.keepalive_interval_secs = 4;
    assert_eq!(store.save(s.clone()), Err(ServerError::KeepaliveInterval));
    s.keepalive_interval_secs = 301;
    assert_eq!(store.save(s.clone()), Err(ServerError::KeepaliveInterval));
    s.keepalive_interval_secs = 10;
    s.keepalive_max_missed = 0;
    assert_eq!(store.save(s.clone()), Err(ServerError::KeepaliveMissed));
    s.keepalive_max_missed = 11;
    assert_eq!(store.save(s.clone()), Err(ServerError::KeepaliveMissed));
    s.keepalive_max_missed = 3;
    let id = store.save(s).unwrap();
    assert_eq!(store.get(&id).unwrap().keepalive_interval_secs, 10);
}
