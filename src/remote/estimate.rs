//! How big a read will be, before it starts: lines and bytes of each file,
//! and whether the disk has room. Compressed files say their own size in
//! their last or first bytes; where they do not it is a guess, marked as one.

use super::compress::Compression;

/// An unpacked size is taken to be at least this many times the packed one
/// when the file does not say, or says less than it weighs packed.
pub const GUESS_RATIO: u64 = 8;
/// Disk space needed beyond the estimate, as a fraction (1.2 times it).
pub const DISK_MARGIN_PERCENT: u64 = 120;

/// What a compressed file says of its unpacked size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unpacked {
    pub bytes: u64,
    /// A guess (the file did not say), not a size read from it.
    pub guessed: bool,
}

/// Size after unpacking by the kind and the pieces of the file that carry it:
/// `head` the first bytes, `tail` the last ones (a few KB).
pub fn unpacked_size(kind: Compression, packed: u64, head: &[u8], tail: &[u8]) -> Unpacked {
    let guess = Unpacked { bytes: packed.saturating_mul(GUESS_RATIO), guessed: true };
    let read = match kind {
        Compression::None => return Unpacked { bytes: packed, guessed: false },
        Compression::Gzip => gzip_isize(tail),
        Compression::Zstd => zstd_content_size(head),
        Compression::Xz => xz_uncompressed_size(tail),
        Compression::Bzip2 | Compression::Unsupported(_) => None,
    };
    match read {
        // A size below the packed one is a wrap of the 4 GB field, or a lie.
        Some(bytes) if bytes >= packed => Unpacked { bytes, guessed: false },
        _ => guess,
    }
}

/// gzip: the last 4 bytes are the size modulo 4 GB.
pub fn gzip_isize(tail: &[u8]) -> Option<u64> {
    let last = tail.len().checked_sub(4).map(|i| &tail[i..])?;
    Some(u64::from(u32::from_le_bytes(last.try_into().ok()?)))
}

/// zstd: the Frame Content Size of the first frame, when it is written.
pub fn zstd_content_size(head: &[u8]) -> Option<u64> {
    if head.len() < 6 || head[..4] != [0x28, 0xb5, 0x2f, 0xfd] {
        return None;
    }
    let descriptor = head[4];
    let size_flag = descriptor >> 6;
    let single_segment = descriptor & 0x20 != 0;
    let dictionary = match descriptor & 0b11 {
        0 => 0,
        1 => 1,
        2 => 2,
        _ => 4,
    };
    let mut at = 5 + usize::from(!single_segment) + dictionary;
    let width = match size_flag {
        0 if single_segment => 1,
        0 => return None,
        1 => 2,
        2 => 4,
        _ => 8,
    };
    let field = head.get(at..at + width)?;
    at += width;
    let _ = at;
    let mut value = 0u64;
    for (i, b) in field.iter().enumerate() {
        value |= u64::from(*b) << (8 * i);
    }
    if size_flag == 1 {
        value += 256;
    }
    Some(value)
}

/// xz: the Stream Footer and the Index before it, which list the unpacked
/// size of every block. `tail` has to hold them (a few KB is plenty).
pub fn xz_uncompressed_size(tail: &[u8]) -> Option<u64> {
    let footer = tail.len().checked_sub(12).map(|i| &tail[i..])?;
    if &footer[10..12] != b"YZ" {
        return None;
    }
    let backward = (u64::from(u32::from_le_bytes(footer[4..8].try_into().ok()?)) + 1) * 4;
    let index_end = tail.len() - 12;
    let index_start = index_end.checked_sub(usize::try_from(backward).ok()?)?;
    let index = &tail[index_start..index_end];
    if index.first() != Some(&0) {
        return None;
    }
    let mut pos = 1;
    let records = varint(index, &mut pos)?;
    let mut total = 0u64;
    for _ in 0..records {
        let _unpadded = varint(index, &mut pos)?;
        total = total.checked_add(varint(index, &mut pos)?)?;
    }
    Some(total)
}

fn varint(bytes: &[u8], pos: &mut usize) -> Option<u64> {
    let mut value = 0u64;
    for shift in 0..9 {
        let b = *bytes.get(*pos)?;
        *pos += 1;
        value |= u64::from(b & 0x7f) << (7 * shift);
        if b & 0x80 == 0 {
            return Some(value);
        }
    }
    None
}

/// Lines in `bytes` bytes of text, by the length of the lines of a sample
/// (`sample` is text from the end of the file). A sample with no complete
/// line counts as a line of 100 bytes.
pub fn estimate_lines(bytes: u64, sample: &[u8]) -> u64 {
    let newlines = sample.iter().filter(|b| **b == b'\n').count() as u64;
    let average = if newlines == 0 { 100 } else { (sample.len() as u64 / newlines).max(1) };
    bytes / average
}

/// What a whole read comes to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Estimate {
    pub lines: u64,
    pub bytes: u64,
    /// Some part is a guess.
    pub approximate: bool,
}

impl Estimate {
    pub fn add(&mut self, other: Estimate) {
        self.lines += other.lines;
        self.bytes += other.bytes;
        self.approximate |= other.approximate;
    }

    /// `≈ 3.4M lines (290 MB)`; the sign is dropped for an exact count.
    pub fn text(&self) -> String {
        let sign = if self.approximate { "≈ " } else { "" };
        format!("{sign}{} lines ({})", short_count(self.lines), size(self.bytes))
    }

    pub fn over(&self, limit: usize) -> bool {
        self.lines > limit as u64
    }
}

/// `812`, `12.4K`, `3.4M`.
pub fn short_count(n: u64) -> String {
    match n {
        0..=9_999 => n.to_string(),
        10_000..=999_999 => format!("{:.1}K", n as f64 / 1_000.0),
        _ => format!("{:.1}M", n as f64 / 1_000_000.0),
    }
}

fn size(bytes: u64) -> String {
    crate::live::buffer::size_text(bytes)
}

/// Whether `free` bytes are enough for `needed` plus the margin.
pub fn disk_has_room(needed: u64, free: u64) -> bool {
    needed.saturating_mul(DISK_MARGIN_PERCENT) / 100 <= free
}

/// Free bytes on the disk that holds `dir`, when the system can tell.
pub fn free_bytes(dir: &std::path::Path) -> Option<u64> {
    #[cfg(unix)]
    {
        let out = std::process::Command::new("df").arg("-Pk").arg(dir).output().ok()?;
        let text = String::from_utf8_lossy(&out.stdout);
        let line = text.lines().nth(1)?;
        let available_kb: u64 = line.split_whitespace().nth(3)?.parse().ok()?;
        Some(available_kb.saturating_mul(1024))
    }
    #[cfg(not(unix))]
    {
        let _ = dir;
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn gzip_says_its_size_in_the_last_four_bytes() {
        let text = "a line of log text\n".repeat(5_000);
        let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        enc.write_all(text.as_bytes()).unwrap();
        let packed = enc.finish().unwrap();
        let got = unpacked_size(Compression::Gzip, packed.len() as u64, &packed[..6], &packed);
        assert_eq!(got, Unpacked { bytes: text.len() as u64, guessed: false });
    }

    #[test]
    fn a_gzip_size_below_the_packed_size_is_a_guess() {
        // 34 bytes of text, 50 packed: the field cannot be the whole story.
        let packed = include_bytes!("testdata/t.gz");
        let got = unpacked_size(Compression::Gzip, packed.len() as u64, &packed[..6], packed);
        assert_eq!(got, Unpacked { bytes: packed.len() as u64 * 8, guessed: true });
    }

    #[test]
    fn zstd_says_its_size_in_the_first_frame() {
        // Written by the zstd tool from a 100 KB file.
        let text = "a line of log text\n".repeat(5_000);
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(text.as_bytes()).unwrap();
        let out = std::process::Command::new("zstd").arg("-q").arg("-c").arg(file.path()).output();
        let Ok(out) = out else { return };
        if !out.status.success() {
            return;
        }
        assert_eq!(zstd_content_size(&out.stdout), Some(text.len() as u64));
        let got = unpacked_size(Compression::Zstd, out.stdout.len() as u64, &out.stdout[..16], &[]);
        assert!(!got.guessed);
    }

    #[test]
    fn zstd_without_a_written_size_is_a_guess() {
        // Magic, descriptor 0 (no size, no single segment), window byte.
        let head = [0x28, 0xb5, 0x2f, 0xfd, 0x00, 0x58, 0x00, 0x00];
        assert_eq!(zstd_content_size(&head), None);
        assert!(unpacked_size(Compression::Zstd, 1000, &head, &[]).guessed);
    }

    #[test]
    fn xz_lists_the_size_of_its_blocks_in_the_index() {
        let packed = include_bytes!("testdata/t.xz");
        assert_eq!(xz_uncompressed_size(packed), Some(34));
        assert_eq!(xz_uncompressed_size(&packed[..20]), None);
    }

    #[test]
    fn bzip2_is_always_a_guess() {
        let got = unpacked_size(Compression::Bzip2, 1000, b"BZh91A", &[]);
        assert_eq!(got, Unpacked { bytes: 8000, guessed: true });
    }

    #[test]
    fn plain_files_are_exact() {
        assert_eq!(unpacked_size(Compression::None, 123, &[], &[]), Unpacked { bytes: 123, guessed: false });
    }

    #[test]
    fn lines_by_the_length_of_the_sample() {
        let sample = b"0123456789\n".repeat(50);
        assert_eq!(estimate_lines(11_000, &sample), 1_000);
        assert_eq!(estimate_lines(10_000, b"no newline here"), 100);
    }

    #[test]
    fn the_text_of_an_estimate() {
        let mut e = Estimate { lines: 3_400_000, bytes: 290 * 1024 * 1024, approximate: true };
        assert_eq!(e.text(), "≈ 3.4M lines (290 MB)");
        e.approximate = false;
        assert_eq!(e.text(), "3.4M lines (290 MB)");
        assert!(e.over(2_000_000) && !e.over(4_000_000));
        let mut sum = Estimate::default();
        sum.add(Estimate { lines: 5, bytes: 50, approximate: false });
        sum.add(e);
        assert_eq!(sum.lines, 3_400_005);
        assert!(!sum.approximate);
    }

    #[test]
    fn disk_room_needs_a_margin() {
        assert!(disk_has_room(1_000, 1_200));
        assert!(!disk_has_room(1_000, 1_100));
        assert!(!disk_has_room(3 << 30, 2 << 30));
    }

    #[test]
    fn the_disk_of_a_folder_has_some_free_space() {
        let dir = tempfile::tempdir().unwrap();
        if let Some(free) = free_bytes(dir.path()) {
            assert!(free > 0);
        }
    }
}
