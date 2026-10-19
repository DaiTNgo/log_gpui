//! Looking at a Server's files: listing a folder, and turning what the user
//! typed (a path or a glob) into the files to add.

use russh_sftp::client::SftpSession;
use russh_sftp::client::error::Error as SftpError;
use russh_sftp::protocol::StatusCode;

pub use super::picker_state::{SortKey, SortOrder};

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

/// Narrow a listing to what the filter field holds: names containing the
/// text (case-insensitive), or matching it when it has glob characters. An
/// empty filter keeps everything.
pub fn filter_entries<'a>(entries: &'a [DirEntry], filter: &str) -> Vec<&'a DirEntry> {
    let filter = filter.trim();
    if filter.is_empty() {
        return entries.iter().collect();
    }
    if is_glob(filter) {
        return entries.iter().filter(|e| glob_match(filter, &e.name)).collect();
    }
    let needle = filter.to_lowercase();
    entries.iter().filter(|e| e.name.to_lowercase().contains(&needle)).collect()
}

/// Sort a listing: folders first, then by `order`. Folders are sorted by name
/// when sorting by size (the size SFTP gives for a folder means nothing).
pub fn sort_entries(entries: &mut [&DirEntry], order: SortOrder) {
    let by_name = |a: &DirEntry, b: &DirEntry| {
        a.name.to_lowercase().cmp(&b.name.to_lowercase()).then_with(|| a.name.cmp(&b.name))
    };
    entries.sort_by(|a, b| {
        b.is_dir.cmp(&a.is_dir).then_with(|| {
            let ord = match order.key {
                SortKey::Name => by_name(a, b),
                SortKey::Modified => a.modified.cmp(&b.modified).then_with(|| by_name(a, b)),
                SortKey::Size if a.is_dir => by_name(a, b),
                SortKey::Size => a.size.cmp(&b.size).then_with(|| by_name(a, b)),
            };
            // Folders sorted by size keep their name order whatever the direction.
            let flip = order.descending && !(a.is_dir && order.key == SortKey::Size);
            if flip { ord.reverse() } else { ord }
        })
    });
}

/// What the path field holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolved {
    /// A folder: the picker opens it.
    Dir(String),
    /// One file path (which may not exist yet).
    Files(Vec<String>),
    /// A glob in the file name of an existing folder: a Live Glob.
    Glob { dir: String, pattern: String },
}

/// What the user typed stands for. A glob only needs its folder to exist; a
/// path that does not exist yet is accepted when its folder exists.
pub async fn resolve_input(sftp: &SftpSession, input: &str) -> Result<Resolved, String> {
    let input = input.trim();
    if !input.starts_with('/') {
        return Err("Enter an absolute path, starting with /".into());
    }
    if is_glob(input) {
        let dir = dirname(input);
        if is_glob(&dir) {
            return Err("A glob can only be in the file name".into());
        }
        if input.ends_with('/') {
            return Err("A glob can only be in the file name".into());
        }
        let pattern = input.rsplit('/').next().unwrap_or("").to_string();
        return match sftp.metadata(dir.clone()).await {
            Ok(meta) if meta.is_dir() => Ok(Resolved::Glob { dir, pattern }),
            Ok(_) => Err(format!("{dir} is not a folder")),
            Err(SftpError::Status(s)) if s.status_code == StatusCode::NoSuchFile => {
                Err(format!("The folder {dir} does not exist"))
            }
            Err(e) => Err(format!("{dir}: {}", describe(&e))),
        };
    }
    match sftp.metadata(input.to_string()).await {
        Ok(meta) if meta.is_dir() => {
            let trimmed = input.trim_end_matches('/');
            Ok(Resolved::Dir(if trimmed.is_empty() { "/".into() } else { trimmed.to_string() }))
        },
        Ok(_) => Ok(Resolved::Files(vec![input.to_string()])),
        Err(SftpError::Status(s)) if s.status_code == StatusCode::NoSuchFile => {
            let parent = dirname(input);
            match sftp.metadata(parent.clone()).await {
                Ok(meta) if meta.is_dir() => Ok(Resolved::Files(vec![input.to_string()])),
                _ => Err(format!("The folder {parent} does not exist")),
            }
        }
        Err(e) => Err(format!("{input}: {}", describe(&e))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, is_dir: bool, size: u64, modified: u64) -> DirEntry {
        DirEntry { name: name.into(), is_dir, size, modified: Some(modified) }
    }

    fn names(entries: &[&DirEntry]) -> Vec<String> {
        entries.iter().map(|e| e.name.clone()).collect()
    }

    fn sample() -> Vec<DirEntry> {
        vec![
            entry("syslog", false, 500, 30),
            entry("auth.log", false, 900, 10),
            entry("nginx", true, 4096, 20),
            entry("Apps", true, 64, 5),
            entry("kern.log", false, 100, 40),
        ]
    }

    #[test]
    fn filter_by_substring_ignores_case() {
        let entries = sample();
        assert_eq!(names(&filter_entries(&entries, "LOG")), ["syslog", "auth.log", "kern.log"]);
        assert_eq!(filter_entries(&entries, "").len(), 5);
        assert_eq!(filter_entries(&entries, "  ").len(), 5);
    }

    #[test]
    fn filter_by_glob() {
        let entries = sample();
        assert_eq!(names(&filter_entries(&entries, "*.log")), ["auth.log", "kern.log"]);
        assert_eq!(names(&filter_entries(&entries, "s?slog")), ["syslog"]);
    }

    #[test]
    fn sort_by_name_puts_folders_first() {
        let entries = sample();
        let mut view: Vec<&DirEntry> = entries.iter().collect();
        sort_entries(&mut view, SortOrder::default());
        assert_eq!(names(&view), ["Apps", "nginx", "auth.log", "kern.log", "syslog"]);
        sort_entries(&mut view, SortOrder { key: SortKey::Name, descending: true });
        assert_eq!(names(&view), ["nginx", "Apps", "syslog", "kern.log", "auth.log"]);
    }

    #[test]
    fn sort_by_modified() {
        let entries = sample();
        let mut view: Vec<&DirEntry> = entries.iter().collect();
        sort_entries(&mut view, SortOrder { key: SortKey::Modified, descending: true });
        assert_eq!(names(&view), ["nginx", "Apps", "kern.log", "syslog", "auth.log"]);
    }

    #[test]
    fn sort_by_size_orders_folders_by_name() {
        let entries = sample();
        let mut view: Vec<&DirEntry> = entries.iter().collect();
        sort_entries(&mut view, SortOrder { key: SortKey::Size, descending: false });
        assert_eq!(names(&view), ["Apps", "nginx", "kern.log", "syslog", "auth.log"]);
        sort_entries(&mut view, SortOrder { key: SortKey::Size, descending: true });
        assert_eq!(names(&view), ["Apps", "nginx", "auth.log", "syslog", "kern.log"]);
    }
}
