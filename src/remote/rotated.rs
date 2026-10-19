//! Telling a Rotated File by its name: `<name>.<N>`, `<name>-<YYYYMMDD>` or
//! `<name>-<YYYY-MM-DD>`, with or without a compression suffix. It is an
//! earlier Generation of the file `<name>`, not a file of its own.

use super::browse::{dirname, join, list_dir};
use super::compress::{Compression, MAGIC_BYTES};
use russh_sftp::client::SftpSession;
use tokio::io::AsyncReadExt;

const COMPRESSION: [&str; 5] = ["gz", "bz2", "xz", "zst", "Z"];

/// Where a rotation sits in the sequence of its file, by its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RotationKey {
    /// `<name>.<N>`: a bigger number is older.
    Number(u64),
    /// A date as `YYYYMMDDhhmmss` (a date alone has zeros for the time): a
    /// bigger date is newer.
    Date(u64),
}

impl RotationKey {
    /// Ascending is newest first.
    fn newest_first(self) -> i128 {
        match self {
            RotationKey::Number(n) => n as i128,
            RotationKey::Date(d) => -(d as i128),
        }
    }
}

/// The name a Rotated File was rotated from, by its name alone. `None` when
/// the name does not look like a rotation of anything.
pub fn rotated_base(name: &str) -> Option<&str> {
    rotation(name).map(|(base, _)| base)
}

/// The name a Rotated File was rotated from, and where it sits in the
/// sequence.
pub fn rotation(name: &str) -> Option<(&str, RotationKey)> {
    let mut stem = name;
    if let Some((rest, ext)) = stem.rsplit_once('.') {
        if COMPRESSION.contains(&ext) {
            stem = rest;
        }
    }
    let (base, key) = strip_rotation_suffix(stem)?;
    (!base.is_empty()).then_some((base, key))
}

fn all_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

fn strip_rotation_suffix(stem: &str) -> Option<(&str, RotationKey)> {
    // `<name>.<N>`
    if let Some((base, n)) = stem.rsplit_once('.') {
        if all_digits(n) {
            return Some((base, RotationKey::Number(n.parse().ok()?)));
        }
    }
    // `<name>-<YYYY-MM-DD>` or `<name>.<YYYY-MM-DD>`
    if stem.len() > 11 {
        let (base, tail) = stem.split_at(stem.len() - 11);
        let date = tail.strip_prefix('-').or_else(|| tail.strip_prefix('.'));
        if let Some(date) = date {
            let parts: Vec<&str> = date.split('-').collect();
            if parts.len() == 3
                && parts[0].len() == 4
                && parts[1].len() == 2
                && parts[2].len() == 2
                && parts.iter().all(|p| all_digits(p))
            {
                let day: u64 = parts.concat().parse().ok()?;
                return Some((base, RotationKey::Date(day * 1_000_000)));
            }
        }
    }
    // `<name>-<YYYYMMDD>` or `<name>-<YYYYMMDDhhmmss>`
    for len in [14usize, 8] {
        if stem.len() > len + 1 {
            let (base, tail) = stem.split_at(stem.len() - len - 1);
            if let Some(date) = tail.strip_prefix('-') {
                if date.len() == len && all_digits(date) {
                    let mut value: u64 = date.parse().ok()?;
                    if len == 8 {
                        value *= 1_000_000;
                    }
                    return Some((base, RotationKey::Date(value)));
                }
            }
        }
    }
    None
}

/// A Rotated File on a Server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RotatedFile {
    pub name: String,
    pub size: u64,
    /// Seconds since the epoch.
    pub modified: Option<u64>,
    pub key: RotationKey,
    pub compression: Compression,
}

/// The Rotated Files of the file called `base` among `listing` (name, size,
/// modified), newest first: by modified time, and when that is equal by the
/// number or date in the name. The k-th is the Generation k steps before the
/// current file. `compression` is `None` until the first bytes are read.
pub fn order_rotated(base: &str, listing: impl IntoIterator<Item = (String, u64, Option<u64>)>) -> Vec<RotatedFile> {
    let mut out: Vec<RotatedFile> = listing
        .into_iter()
        .filter_map(|(name, size, modified)| {
            let (found, key) = rotation(&name)?;
            (found == base).then_some(RotatedFile { name, size, modified, key, compression: Compression::None })
        })
        .collect();
    out.sort_by(|a, b| {
        b.modified
            .cmp(&a.modified)
            .then_with(|| a.key.newest_first().cmp(&b.key.newest_first()))
            .then_with(|| a.name.cmp(&b.name))
    });
    out
}

/// The Rotated Files of the Remote File `path`, newest first, with the
/// compression of each told by its first bytes.
pub async fn list_rotated(sftp: &SftpSession, path: &str) -> Result<Vec<RotatedFile>, String> {
    let dir = dirname(path);
    let base = path.rsplit('/').next().unwrap_or(path);
    let entries = list_dir(sftp, &dir).await?;
    let listing = entries.into_iter().filter(|e| !e.is_dir).map(|e| (e.name, e.size, e.modified));
    let mut files = order_rotated(base, listing);
    for file in &mut files {
        file.compression = sniff(sftp, &join(&dir, &file.name)).await.unwrap_or(Compression::None);
    }
    Ok(files)
}

/// The compression of a file on a Server, by its first bytes.
pub async fn sniff(sftp: &SftpSession, path: &str) -> Result<Compression, String> {
    let mut file = sftp.open(path.to_string()).await.map_err(|e| format!("{path}: {e}"))?;
    let mut head = [0u8; MAGIC_BYTES];
    let mut got = 0;
    while got < head.len() {
        let n = file.read(&mut head[got..]).await.map_err(|e| format!("{path}: {e}"))?;
        if n == 0 {
            break;
        }
        got += n;
    }
    Ok(Compression::detect(&head[..got]))
}

/// Whether `name` is a Rotated File of a file for which `has_base` says yes.
pub fn is_rotated_file(name: &str, has_base: impl Fn(&str) -> bool) -> bool {
    rotated_base(name).is_some_and(has_base)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbered_and_dated_rotations() {
        assert_eq!(rotated_base("app.log.1"), Some("app.log"));
        assert_eq!(rotated_base("app.log.12"), Some("app.log"));
        assert_eq!(rotated_base("app.log-20261019"), Some("app.log"));
        assert_eq!(rotated_base("app.log-2026-10-19"), Some("app.log"));
    }

    #[test]
    fn compressed_rotations() {
        assert_eq!(rotated_base("app.log.2.gz"), Some("app.log"));
        assert_eq!(rotated_base("app.log-20261019.xz"), Some("app.log"));
        assert_eq!(rotated_base("app.log.3.zst"), Some("app.log"));
    }

    #[test]
    fn plain_names_are_not_rotations() {
        assert_eq!(rotated_base("app.log"), None);
        assert_eq!(rotated_base("app-2026-10-19.log"), None);
        assert_eq!(rotated_base("dump.tar.gz"), None);
        assert_eq!(rotated_base("app.log.gz"), None);
        assert_eq!(rotated_base("2026"), None);
        assert_eq!(rotated_base(".1"), None);
    }

    #[test]
    fn needs_the_base_to_exist() {
        let names = ["app.log", "app.log.1"];
        let has = |b: &str| names.contains(&b);
        assert!(is_rotated_file("app.log.1", has));
        assert!(!is_rotated_file("other.log.1", has));
    }

    #[test]
    fn more_date_shapes_and_keys() {
        assert_eq!(rotation("app.log-20261019123045"), Some(("app.log", RotationKey::Date(20261019123045))));
        assert_eq!(rotation("app.log.2026-10-19"), Some(("app.log", RotationKey::Date(20261019000000))));
        assert_eq!(rotation("app.log-20261019.gz"), Some(("app.log", RotationKey::Date(20261019000000))));
        assert_eq!(rotation("app.log.3.bz2"), Some(("app.log", RotationKey::Number(3))));
    }

    #[test]
    fn leftovers_are_not_rotations() {
        assert_eq!(rotated_base("app.log.bak"), None);
        assert_eq!(rotated_base("app.log.old"), None);
        assert_eq!(rotated_base("app.log~"), None);
        // Known false positive: a year looks like a number.
        assert_eq!(rotated_base("app.log.2026"), Some("app.log"));
    }

    fn listing(items: &[(&str, u64)]) -> Vec<(String, u64, Option<u64>)> {
        items.iter().map(|(n, m)| (n.to_string(), 10, Some(*m))).collect()
    }

    #[test]
    fn logrotate_numbers_come_newest_first() {
        let l = listing(&[("app.log", 9), ("app.log.3.gz", 3), ("app.log.1", 7), ("app.log.2.gz", 5), ("app.log.bak", 8)]);
        let names: Vec<_> = order_rotated("app.log", l).into_iter().map(|f| f.name).collect();
        assert_eq!(names, ["app.log.1", "app.log.2.gz", "app.log.3.gz"]);
    }

    #[test]
    fn dateext_comes_by_modified_time() {
        let l = listing(&[("app.log-20261017.gz", 17), ("app.log-20261018", 18)]);
        let names: Vec<_> = order_rotated("app.log", l).into_iter().map(|f| f.name).collect();
        assert_eq!(names, ["app.log-20261018", "app.log-20261017.gz"]);
    }

    #[test]
    fn same_modified_time_falls_back_to_the_name() {
        let l = listing(&[("app.log.2", 5), ("app.log.1", 5), ("app.log.10", 5)]);
        let names: Vec<_> = order_rotated("app.log", l).into_iter().map(|f| f.name).collect();
        assert_eq!(names, ["app.log.1", "app.log.2", "app.log.10"]);
        let l = listing(&[("a.log-20261017", 5), ("a.log-20261019", 5), ("a.log-20261018", 5)]);
        let names: Vec<_> = order_rotated("a.log", l).into_iter().map(|f| f.name).collect();
        assert_eq!(names, ["a.log-20261019", "a.log-20261018", "a.log-20261017"]);
    }

    #[test]
    fn other_files_rotations_are_left_out() {
        let l = listing(&[("other.log.1", 5), ("app.log.1", 4)]);
        assert_eq!(order_rotated("app.log", l).len(), 1);
    }
}
