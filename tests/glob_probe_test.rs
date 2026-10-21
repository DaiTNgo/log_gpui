//! Archive detection of a Live Glob against a local sshd (see
//! `remote_hub_test.rs`): `SSH_TEST_DIR`, `SSH_TEST_PORT`.

use log_gpui::remote::glob_watcher::HeadProbe;
use log_gpui::remote::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

/// Counts the bytes that come back over a stream.
struct Counted<S> {
    inner: S,
    read: Arc<AtomicUsize>,
}

impl<S: AsyncRead + Unpin> AsyncRead for Counted<S> {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let before = buf.filled().len();
        let poll = std::pin::Pin::new(&mut self.inner).poll_read(cx, buf);
        self.read.fetch_add(buf.filled().len() - before, Ordering::SeqCst);
        poll
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for Counted<S> {
    fn poll_write(mut self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>, b: &[u8]) -> Poll<std::io::Result<usize>> {
        std::pin::Pin::new(&mut self.inner).poll_write(cx, b)
    }
    fn poll_flush(mut self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.inner).poll_flush(cx)
    }
    fn poll_shutdown(mut self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

#[test]
fn probing_many_files_transfers_only_their_headers() {
    let Ok(key_dir) = std::env::var("SSH_TEST_DIR") else { return };
    let port: u16 = std::env::var("SSH_TEST_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(2222);
    let Ok(user) = std::env::var("USER") else { return };
    let mut server = Server::new("web1", "127.0.0.1", &user, AuthMethod::Key { path: format!("{key_dir}/userkey") });
    server.port = port;
    let mut store = ServerStore::default();
    let server_id = store.save(server).unwrap();
    let (hub, requests) = RemoteHub::new(Arc::new(MemorySecrets::default()), Arc::new(Mutex::new(KnownHosts::default())));
    std::thread::spawn(move || {
        while let Ok(req) = requests.recv_blocking() {
            let _ = req.reply.send(true);
        }
    });
    let dir = tempfile::tempdir().unwrap();
    let mut paths = Vec::new();
    for i in 0..60 {
        let path = dir.path().join(format!("f{i}.log"));
        // About 300 KB of text: more than one SFTP read chunk.
        std::fs::write(&path, "2024-01-01 INFO hello world\n".repeat(11_000)).unwrap();
        paths.push(path.to_string_lossy().to_string());
    }
    let gz = dir.path().join("packed.log");
    std::fs::write(&gz, [0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 3, 1, 2, 3, 4]).unwrap();
    paths.push(gz.to_string_lossy().to_string());
    let empty = dir.path().join("empty.log");
    std::fs::write(&empty, "").unwrap();
    paths.push(empty.to_string_lossy().to_string());

    let read = Arc::new(AtomicUsize::new(0));
    let results = hub.handle().block_on(async {
        let session = hub.session(&store, &server_id).await.unwrap();
        let stream = Counted { inner: session.open_sftp_stream().await.unwrap(), read: read.clone() };
        let probe = Arc::new(HeadProbe::new(stream).await.unwrap());
        probe.are_archives(&paths).await
    });
    assert_eq!(results.len(), paths.len());
    let archives: Vec<_> = results.iter().filter(|(_, archive)| *archive).map(|(p, _)| p.as_str()).collect();
    assert_eq!(archives, [gz.to_string_lossy().as_ref()]);
    let total = read.load(Ordering::SeqCst);
    assert!(total < 62 * 4096, "read {total} bytes");
}
