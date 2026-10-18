//! Downloading Remote Files to a folder over SFTP.

use russh_sftp::client::SftpSession;
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// The names the downloaded files get, one per `(server name, remote path)`.
/// A file keeps the name it has on the Server; when two share a name, or the
/// folder already has one, the Server's name goes in front: `web1-app.log`.
pub fn download_names(files: &[(String, String)], existing: &HashSet<String>) -> Vec<String> {
    let base = |path: &str| path.rsplit('/').next().unwrap_or(path).to_string();
    let bases: Vec<String> = files.iter().map(|(_, p)| base(p)).collect();
    let mut used: HashSet<String> = existing.clone();
    files
        .iter()
        .zip(&bases)
        .map(|((server, _), name)| {
            let clashes = bases.iter().filter(|b| *b == name).count() > 1 || existing.contains(name);
            let mut candidate = if clashes { format!("{server}-{name}") } else { name.clone() };
            let mut n = 2;
            while used.contains(&candidate) {
                candidate = format!("{server}-{name}-{n}");
                n += 1;
            }
            used.insert(candidate.clone());
            candidate
        })
        .collect()
}

/// How far a download got, shared with the app.
#[derive(Debug, Default)]
pub struct Progress {
    pub bytes: AtomicU64,
    pub total: AtomicU64,
    /// Files finished so far.
    pub files_done: std::sync::atomic::AtomicUsize,
    pub cancel: AtomicBool,
}

/// What stopped a download.
#[derive(Debug, PartialEq, Eq)]
pub enum DownloadError {
    Cancelled,
    Failed(String),
}

/// Copy a remote file to `dest`. A cancelled or failed download leaves no
/// partial file.
pub async fn download_file(
    sftp: &SftpSession,
    remote: &str,
    dest: &Path,
    progress: &Arc<Progress>,
) -> Result<u64, DownloadError> {
    let fail = |e: String| DownloadError::Failed(format!("{remote}: {e}"));
    let mut source = sftp.open(remote.to_string()).await.map_err(|e| fail(e.to_string()))?;
    let mut out = tokio::fs::File::create(dest).await.map_err(|e| fail(e.to_string()))?;
    let mut buf = vec![0u8; 256 * 1024];
    let mut done = 0u64;
    let result = loop {
        if progress.cancel.load(Ordering::Relaxed) {
            break Err(DownloadError::Cancelled);
        }
        match source.read(&mut buf).await {
            Ok(0) => break Ok(done),
            Ok(n) => {
                if let Err(e) = out.write_all(&buf[..n]).await {
                    break Err(fail(e.to_string()));
                }
                done += n as u64;
                progress.bytes.fetch_add(n as u64, Ordering::Relaxed);
            }
            Err(e) => break Err(fail(e.to_string())),
        }
    };
    if result.is_err() {
        drop(out);
        let _ = tokio::fs::remove_file(dest).await;
    } else {
        let _ = out.flush().await;
    }
    result
}
