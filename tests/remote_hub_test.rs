//! Needs a local sshd: set `SSH_TEST_DIR` to a folder with `userkey` and
//! `userkey_pp` (passphrase `secretpass`) and `SSH_TEST_PORT` (default 2222).
//! The tests are skipped when it is not set.

use log_gpui::remote::*;
use std::sync::{Arc, Mutex};

fn setup() -> Option<(String, u16, String)> {
    let dir = std::env::var("SSH_TEST_DIR").ok()?;
    let port = std::env::var("SSH_TEST_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(2222);
    let user = std::env::var("USER").ok()?;
    Some((dir, port, user))
}

fn key_server(name: &str, user: &str, port: u16, key: String) -> Server {
    let mut s = Server::new(name, "127.0.0.1", user, AuthMethod::Key { path: key });
    s.port = port;
    s
}

/// Answers every host key question with `trust`.
fn answer_prompts(requests: async_channel::Receiver<HostKeyRequest>, trust: bool) {
    std::thread::spawn(move || {
        while let Ok(req) = requests.recv_blocking() {
            let _ = req.reply.send(trust);
        }
    });
}

#[test]
fn connects_with_a_passphrase_key_through_a_jump_server_and_reads_a_file() {
    let Some((dir, port, user)) = setup() else { return };
    let mut store = ServerStore::default();
    let hop = store.save(key_server("hop", &user, port, format!("{dir}/userkey_pp"))).unwrap();
    let mut db = key_server("db", &user, port, format!("{dir}/userkey_pp"));
    db.jump_server_id = Some(hop);
    let db = store.save(db).unwrap();

    let secrets = Arc::new(MemorySecrets::default());
    let known = Arc::new(Mutex::new(KnownHosts::default()));
    let (hub, requests) = RemoteHub::new(secrets, known.clone());
    answer_prompts(requests, true);

    // No passphrase yet: the connection asks for it.
    let h = hub.clone();
    let st = store.clone();
    let dbc = db.clone();
    let err = hub.handle().block_on(async move { h.session(&st, &dbc).await.err() });
    assert!(matches!(err, Some(ConnectError::NeedsSecret { .. })), "{err:?}");

    // A wrong one fails; the right one connects.
    let server = store.get(&store.chain(&db).unwrap()[0].id).unwrap().clone();
    hub.provide_secret(&server, "wrong");
    let (h, st, dbc) = (hub.clone(), store.clone(), db.clone());
    let err = hub.handle().block_on(async move { h.session(&st, &dbc).await.err() });
    assert!(matches!(err, Some(ConnectError::AuthFailed { .. })), "{err:?}");
    for s in store.chain(&db).unwrap() {
        hub.provide_secret(s, "secretpass");
    }
    let (h, st, dbc) = (hub.clone(), store.clone(), db.clone());
    let out = hub.handle().block_on(async move {
        let session = h.session(&st, &dbc).await.map_err(|e| e.message())?;
        session.exec("echo hello").await
    });
    assert_eq!(String::from_utf8_lossy(&out.unwrap().stdout).trim(), "hello");
    assert!(known.lock().unwrap().check(&format!("127.0.0.1:{port}"), "x") != HostKeyStatus::Unknown);
}

#[test]
fn an_untrusted_new_key_refuses_and_a_changed_key_is_blocked() {
    let Some((dir, port, user)) = setup() else { return };
    let mut store = ServerStore::default();
    let id = store.save(key_server("web", &user, port, format!("{dir}/userkey"))).unwrap();

    // The user says no to a new key.
    let known = Arc::new(Mutex::new(KnownHosts::default()));
    let (hub, requests) = RemoteHub::new(Arc::new(MemorySecrets::default()), known.clone());
    answer_prompts(requests, false);
    let (h, st, i) = (hub.clone(), store.clone(), id.clone());
    let err = hub.handle().block_on(async move { h.session(&st, &i).await.err() });
    assert!(matches!(err, Some(ConnectError::HostKeyRejected { .. })), "{err:?}");

    // A different key is trusted for this address: blocked, no prompt.
    known.lock().unwrap().trust(&format!("127.0.0.1:{port}"), "SHA256:other");
    let (h, st, i) = (hub.clone(), store.clone(), id.clone());
    let err = hub.handle().block_on(async move { h.session(&st, &i).await.err() });
    assert!(matches!(err, Some(ConnectError::HostKeyChanged { .. })), "{err:?}");
}

#[test]
fn a_session_is_shared_between_files_on_one_server() {
    let Some((dir, port, user)) = setup() else { return };
    let mut store = ServerStore::default();
    let id = store.save(key_server("web", &user, port, format!("{dir}/userkey"))).unwrap();
    let (hub, requests) = RemoteHub::new(Arc::new(MemorySecrets::default()), Arc::new(Mutex::new(KnownHosts::default())));
    answer_prompts(requests, true);
    let (h, st) = (hub.clone(), store.clone());
    let same = hub.handle().block_on(async move {
        let a = h.session(&st, &id).await.map_err(|e| e.message())?;
        let b = h.session(&st, &id).await.map_err(|e| e.message())?;
        Ok::<_, String>(Arc::ptr_eq(&a, &b))
    });
    assert_eq!(same, Ok(true));
}

#[test]
fn browsing_and_resolving_what_the_user_typed() {
    use log_gpui::remote::browse::*;
    let Some((dir, port, user)) = setup() else { return };
    let mut store = ServerStore::default();
    let id = store.save(key_server("web", &user, port, format!("{dir}/userkey"))).unwrap();
    let (hub, requests) = RemoteHub::new(Arc::new(MemorySecrets::default()), Arc::new(Mutex::new(KnownHosts::default())));
    answer_prompts(requests, true);
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().to_string_lossy().to_string();
    std::fs::write(tmp.path().join("a.log"), "x\n").unwrap();
    std::fs::write(tmp.path().join("b.log"), "x\n").unwrap();
    std::fs::write(tmp.path().join("c.txt"), "x\n").unwrap();
    std::fs::create_dir(tmp.path().join("sub")).unwrap();
    let h = hub.clone();
    let result = hub.handle().block_on(async move {
        let session = h.session(&store, &id).await.map_err(|e| e.message())?;
        let sftp = session.open_sftp().await?;
        let list = list_dir(&sftp, &root).await?;
        let names: Vec<(String, bool)> = list.iter().map(|e| (e.name.clone(), e.is_dir)).collect();
        assert_eq!(names, vec![("sub".into(), true), ("a.log".into(), false), ("b.log".into(), false), ("c.txt".into(), false)]);
        let globbed = resolve_input(&sftp, &format!("{root}/*.log")).await?;
        assert_eq!(globbed, vec![format!("{root}/a.log"), format!("{root}/b.log")]);
        // A file made after the glob is not added by it: matching is once.
        assert_eq!(resolve_input(&sftp, &format!("{root}/a.log")).await?, vec![format!("{root}/a.log")]);
        // Not there yet, folder there: accepted.
        assert_eq!(resolve_input(&sftp, &format!("{root}/today.log")).await?, vec![format!("{root}/today.log")]);
        // Folder missing: refused.
        let err = resolve_input(&sftp, &format!("{root}/no/such/app.log")).await.unwrap_err();
        assert!(err.contains("does not exist"), "{err}");
        assert!(resolve_input(&sftp, &format!("{root}/sub")).await.is_err());
        assert!(resolve_input(&sftp, &format!("{root}/*.nothing")).await.is_err());
        assert!(resolve_input(&sftp, "relative.log").await.is_err());
        Ok::<_, String>(())
    });
    assert_eq!(result, Ok(()));
}

#[test]
fn downloads_a_file_with_progress_and_cancel_leaves_nothing() {
    use log_gpui::remote::download::*;
    use std::sync::atomic::Ordering;
    let Some((dir, port, user)) = setup() else { return };
    let mut store = ServerStore::default();
    let id = store.save(key_server("web", &user, port, format!("{dir}/userkey"))).unwrap();
    let (hub, requests) = RemoteHub::new(Arc::new(MemorySecrets::default()), Arc::new(Mutex::new(KnownHosts::default())));
    answer_prompts(requests, true);
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("big.log");
    let body: Vec<u8> = (0..3_000_000u32).map(|i| (i % 251) as u8).collect();
    std::fs::write(&src, &body).unwrap();
    let dest = tmp.path().join("copy.log");
    let cancelled = tmp.path().join("cancelled.log");
    let (src_s, dest_c, cancelled_c) = (src.to_string_lossy().to_string(), dest.clone(), cancelled.clone());
    let h = hub.clone();
    let result = hub.handle().block_on(async move {
        let session = h.session(&store, &id).await.map_err(|e| e.message())?;
        let sftp = session.open_sftp().await?;
        let progress = Arc::new(Progress::default());
        let n = download_file(&sftp, &src_s, &dest_c, &progress).await.map_err(|e| format!("{e:?}"))?;
        assert_eq!(progress.bytes.load(Ordering::Relaxed), n);
        let stopped = Arc::new(Progress::default());
        stopped.cancel.store(true, Ordering::Relaxed);
        let r = download_file(&sftp, &src_s, &cancelled_c, &stopped).await;
        assert_eq!(r, Err(DownloadError::Cancelled));
        Ok::<_, String>(())
    });
    assert_eq!(result, Ok(()));
    assert_eq!(std::fs::read(&dest).unwrap(), body);
    assert!(!cancelled.exists(), "a cancelled download leaves no file");
}
