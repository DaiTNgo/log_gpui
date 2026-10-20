//! `servers.json` with the new sign-in methods, groups and tags.

use log_gpui::remote::*;

#[test]
fn a_servers_file_from_before_still_loads() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("servers.json");
    std::fs::write(
        &path,
        r#"{"servers":[{"id":"srv-1","name":"web","host":"10.0.0.1","user":"deploy","auth":{"Key":{"path":"/k"}}},
             {"id":"srv-2","name":"db","host":"10.0.0.2","user":"deploy","auth":"Password"}],"next_id":2}"#,
    )
    .unwrap();
    let store = ServerStore::load(&path);
    assert_eq!(store.servers.len(), 2);
    let web = store.get("srv-1").unwrap();
    assert_eq!(web.port, 22);
    assert_eq!(web.group, None);
    assert!(web.tags.is_empty());
    assert_eq!(web.origin, None);
    assert!(!web.needs_sign_in_prompt);
    assert_eq!(web.auth, AuthMethod::Key { path: "/k".into() });
}

#[test]
fn the_new_fields_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("servers.json");
    let mut store = ServerStore::default();
    let mut server = Server::new(
        "web1",
        "10.0.0.5",
        "deploy",
        AuthMethod::Agent { socket: AgentSocket::Path("/tmp/a.sock".into()), preferred_key: Some("SHA256:abc".into()) },
    );
    server.group = Some("prod".into());
    server.tags = vec!["eu".into(), "web".into()];
    server.origin = Some(Origin::SshConfig { alias: "web1".into() });
    server.needs_sign_in_prompt = true;
    store.save(server.clone()).unwrap();
    store.save_to(&path).unwrap();
    let loaded = ServerStore::load(&path);
    server.id = "srv-1".into();
    assert_eq!(loaded.servers, vec![server]);
}

#[test]
fn an_agent_without_a_socket_uses_the_system_one() {
    let json = r#"{"id":"s","name":"a","host":"h","user":"u","auth":{"Agent":{}}}"#;
    let server: Server = serde_json::from_str(json).unwrap();
    assert_eq!(server.auth, AuthMethod::Agent { socket: AgentSocket::System, preferred_key: None });
}

#[test]
fn the_old_file_is_kept_once_before_the_first_new_sign_in_is_written() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("servers.json");
    let backup = dir.path().join("servers.v1.json");
    let mut store = ServerStore::default();
    store.save(Server::new("web", "10.0.0.1", "deploy", AuthMethod::Password)).unwrap();
    store.save_to(&path).unwrap();
    // Nothing new yet: no backup.
    assert!(!backup.exists());
    let before = std::fs::read_to_string(&path).unwrap();

    store.save(Server::new("ag", "10.0.0.2", "deploy", AuthMethod::Agent { socket: AgentSocket::System, preferred_key: None })).unwrap();
    store.save_to(&path).unwrap();
    assert_eq!(std::fs::read_to_string(&backup).unwrap(), before);

    // Later writes keep the first backup.
    store.save(Server::new("ki", "10.0.0.3", "deploy", AuthMethod::KeyboardInteractive)).unwrap();
    store.save_to(&path).unwrap();
    assert_eq!(std::fs::read_to_string(&backup).unwrap(), before);
}
