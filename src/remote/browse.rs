//! Looking at a Server's files: listing a folder, and turning what the user
//! typed (a path or a glob) into the files to add.

use russh_sftp::client::SftpSession;
use russh_sftp::client::error::Error as SftpError;
use russh_sftp::protocol::StatusCode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    /// Seconds since the epoch.
    pub modified: Option<u64>,
}

/// Whether the text has glob characters.
pub fn is_glob(text: &str) -> bool {
    text.contains(['*', '?', '['])
}

/// Match a file name against a pattern with `*`, `?` and `[abc]`/`[a-z]`.
pub fn glob_match(pattern: &str, name: &str) -> bool {
    fn go(p: &[char], n: &[char]) -> bool {
        match p.first() {
            None => n.is_empty(),
            Some('*') => (0..=n.len()).any(|i| go(&p[1..], &n[i..])),
            Some('?') => !n.is_empty() && go(&p[1..], &n[1..]),
            Some('[') => {
                let Some(close) = p.iter().position(|c| *c == ']').filter(|c| *c > 1) else {
                    return !n.is_empty() && n[0] == '[' && go(&p[1..], &n[1..]);
                };
                if n.is_empty() {
                    return false;
                }
                let (set, negate) = match p[1] {
                    '!' | '^' => (&p[2..close], true),
                    _ => (&p[1..close], false),
                };
                let mut hit = false;
                let mut i = 0;
                while i < set.len() {
                    if i + 2 < set.len() && set[i + 1] == '-' {
                        hit |= set[i] <= n[0] && n[0] <= set[i + 2];
                        i += 3;
                    } else {
                        hit |= set[i] == n[0];
                        i += 1;
                    }
                }
                hit != negate && go(&p[close + 1..], &n[1..])
            }
            Some(c) => !n.is_empty() && n[0] == *c && go(&p[1..], &n[1..]),
        }
    }
    let p: Vec<char> = pattern.chars().collect();
    let n: Vec<char> = name.chars().collect();
    go(&p, &n)
}

/// The folder part of an absolute path (`/` for a file at the root).
pub fn dirname(path: &str) -> String {
    match path.trim_end_matches('/').rfind('/') {
        Some(0) | None => "/".to_string(),
        Some(i) => path[..i].to_string(),
    }
}

/// The path `name` inside `dir`.
pub fn join(dir: &str, name: &str) -> String {
    if dir.ends_with('/') { format!("{dir}{name}") } else { format!("{dir}/{name}") }
}

fn describe(e: &SftpError) -> String {
    match e {
        SftpError::Status(s) if s.status_code == StatusCode::NoSuchFile => "does not exist".into(),
        SftpError::Status(s) if s.status_code == StatusCode::PermissionDenied => "Permission denied".into(),
        other => other.to_string(),
    }
}

/// The entries of a folder: folders first, then files, each by name.
pub async fn list_dir(sftp: &SftpSession, dir: &str) -> Result<Vec<DirEntry>, String> {
    let entries = sftp
        .read_dir(dir.to_string())
        .await
        .map_err(|e| format!("{dir}: {}", describe(&e)))?;
    let mut out = Vec::new();
    for entry in entries {
        let name = entry.file_name();
        if name == "." || name == ".." {
            continue;
        }
        let mut meta = entry.metadata();
        let mut is_dir = entry.file_type().is_dir();
        // A link shows what it points at.
        if entry.file_type().is_symlink() {
            if let Ok(target) = sftp.metadata(join(dir, &name)).await {
                is_dir = target.is_dir();
                meta = target;
            }
        }
        out.push(DirEntry {
            name,
            is_dir,
            size: meta.len(),
            modified: meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs()),
        });
    }
    out.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.cmp(&b.name)));
    Ok(out)
}

/// The files that what the user typed stands for. A glob is matched once,
/// now. A path that does not exist yet is accepted when its folder exists.
pub async fn resolve_input(sftp: &SftpSession, input: &str) -> Result<Vec<String>, String> {
    let input = input.trim();
    if !input.starts_with('/') {
        return Err("Enter an absolute path, starting with /".into());
    }
    if is_glob(input) {
        let dir = dirname(input);
        if is_glob(&dir) {
            return Err("A glob can only be in the file name".into());
        }
        let pattern = input.rsplit('/').next().unwrap_or("");
        let entries = list_dir(sftp, &dir).await?;
        let found: Vec<String> = entries
            .iter()
            .filter(|e| !e.is_dir && glob_match(pattern, &e.name))
            .map(|e| join(&dir, &e.name))
            .collect();
        if found.is_empty() {
            return Err(format!("No file matches {input}"));
        }
        return Ok(found);
    }
    match sftp.metadata(input.to_string()).await {
        Ok(meta) if meta.is_dir() => Err(format!("{input} is a folder: open it and pick its files")),
        Ok(_) => Ok(vec![input.to_string()]),
        Err(SftpError::Status(s)) if s.status_code == StatusCode::NoSuchFile => {
            let parent = dirname(input);
            match sftp.metadata(parent.clone()).await {
                Ok(meta) if meta.is_dir() => Ok(vec![input.to_string()]),
                _ => Err(format!("The folder {parent} does not exist")),
            }
        }
        Err(e) => Err(format!("{input}: {}", describe(&e))),
    }
}
