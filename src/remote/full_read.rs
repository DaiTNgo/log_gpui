//! Open in full: finding where a time starts and ends in a file without
//! reading all of it, then reading forward.
//!
//! The search reads a few places in the file (about 15–20 reads for 1 GB),
//! each a block of 64 KB, and takes the time of the first Parent Line in it.
//! Logs are mostly in time order; where writers disagree by a few seconds the
//! final forward scan still lands on the first line at or after the time.

use crate::live::backward::{ReadAt, START_SLACK_SECS};
use crate::live::line_time::line_timestamp;
use crate::live::splitter::decode_line;
use std::io;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

/// `ReadAt` over an SFTP file, for use on a blocking thread (it waits for each
/// read on the runtime).
pub struct SftpReadAt {
    pub handle: tokio::runtime::Handle,
    pub file: russh_sftp::client::fs::File,
}

impl ReadAt for SftpReadAt {
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> io::Result<usize> {
        let file = &mut self.file;
        self.handle.block_on(async {
            file.seek(io::SeekFrom::Start(offset)).await?;
            let mut done = 0;
            while done < buf.len() {
                let n = file.read(&mut buf[done..]).await?;
                if n == 0 {
                    break;
                }
                done += n;
            }
            Ok(done)
        })
    }
}

/// Size of one read while searching.
pub const PROBE_BLOCK: usize = 64 * 1024;
/// A block with no time in it is widened up to this much.
pub const PROBE_MAX: usize = 1024 * 1024;

/// The first Parent Line that begins at or after `from`, as `(offset, time)`,
/// looking at no more than `limit` bytes. `from` that is not 0 is taken as
/// the middle of a line: the line it is in is skipped.
fn first_parent_after<R: ReadAt>(src: &mut R, from: u64, size: u64, limit: usize) -> io::Result<Option<(u64, f64)>> {
    let len = (limit as u64).min(size.saturating_sub(from)) as usize;
    let mut block = vec![0u8; len];
    let n = src.read_at(from, &mut block)?;
    block.truncate(n);
    let mut pos = 0usize;
    if from > 0 {
        match block.iter().position(|b| *b == b'\n') {
            Some(i) => pos = i + 1,
            None => return Ok(None),
        }
    }
    while pos < block.len() {
        let Some(end) = block[pos..].iter().position(|b| *b == b'\n') else { break };
        let line = decode_line(&block[pos..pos + end]);
        if let Some(t) = line_timestamp(&line) {
            return Ok(Some((from + pos as u64, t)));
        }
        pos += end + 1;
    }
    Ok(None)
}

/// The offset of the first Parent Line whose time satisfies `past` (that is
/// `false` for every earlier line and `true` from the boundary on), or `size`
/// when none does.
fn find_boundary<R: ReadAt>(src: &mut R, size: u64, past: impl Fn(f64) -> bool) -> io::Result<u64> {
    // Invariant: the boundary is at `lo` or after it.
    let (mut lo, mut hi) = (0u64, size);
    while hi - lo > PROBE_BLOCK as u64 {
        let mid = lo + (hi - lo) / 2;
        let mut found = first_parent_after(src, mid, size, PROBE_BLOCK)?;
        if found.is_none() {
            found = first_parent_after(src, mid, size, PROBE_MAX)?;
        }
        match found {
            Some((at, t)) if !past(t) => lo = at,
            Some(_) => hi = mid,
            // No time anywhere in the widened block: nothing is known here.
            None => lo = (mid + PROBE_MAX as u64).min(hi),
        }
    }
    scan_forward(src, lo, size, past)
}

/// From `from` (the middle of a line unless 0) on, the first Parent Line
/// whose time satisfies `past`.
fn scan_forward<R: ReadAt>(src: &mut R, from: u64, size: u64, past: impl Fn(f64) -> bool) -> io::Result<u64> {
    let mut offset = from;
    let mut carry: Vec<u8> = Vec::new();
    let mut carry_at = from;
    let mut skipping = from > 0;
    let mut block = vec![0u8; PROBE_BLOCK];
    while offset < size {
        let n = src.read_at(offset, &mut block)?;
        if n == 0 {
            break;
        }
        offset += n as u64;
        carry.extend_from_slice(&block[..n]);
        let mut pos = 0;
        while let Some(end) = carry[pos..].iter().position(|b| *b == b'\n') {
            let at = carry_at + pos as u64;
            if skipping {
                skipping = false;
            } else if let Some(t) = line_timestamp(&decode_line(&carry[pos..pos + end])) {
                if past(t) {
                    return Ok(at);
                }
            }
            pos += end + 1;
        }
        carry.drain(..pos);
        carry_at += pos as u64;
    }
    // A last line without a newline.
    if !skipping && !carry.is_empty() {
        if let Some(t) = line_timestamp(&decode_line(&carry)) {
            if past(t) {
                return Ok(carry_at);
            }
        }
    }
    Ok(size)
}

/// Where reading starts for the Starting Time `start`: the offset of the
/// first Parent Line not older than it (less a second of slack).
pub fn find_start_offset<R: ReadAt>(src: &mut R, size: u64, start: f64) -> io::Result<u64> {
    find_boundary(src, size, |t| t >= start - START_SLACK_SECS)
}

/// Where reading ends for the time `to`: the offset of the first Parent Line
/// later than it, so the last line kept keeps its Continuation Lines.
pub fn find_end_offset<R: ReadAt>(src: &mut R, size: u64, to: f64) -> io::Result<u64> {
    find_boundary(src, size, |t| t > to)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Counting<R> {
        inner: R,
        reads: usize,
    }

    impl<R: ReadAt> ReadAt for Counting<R> {
        fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> io::Result<usize> {
            self.reads += 1;
            self.inner.read_at(offset, buf)
        }
    }

    fn stamp(secs: u64) -> String {
        let (h, m, s) = (secs / 3600 % 24, secs / 60 % 60, secs % 60);
        format!("2026-10-19 {h:02}:{m:02}:{s:02}")
    }

    /// One line per second, `n` lines, a Continuation Line after every 7th.
    fn steady_log(n: u64) -> (Vec<u8>, Vec<(u64, u64)>) {
        let mut data = Vec::new();
        let mut starts = Vec::new();
        for i in 0..n {
            starts.push((data.len() as u64, i));
            data.extend_from_slice(format!("{} INFO request {i} handled by worker\n", stamp(i)).as_bytes());
            if i % 7 == 0 {
                data.extend_from_slice(b"    at some.frame(Frame.java:1)\n");
            }
        }
        (data, starts)
    }

    fn secs_of(text: &str) -> f64 {
        line_timestamp(&format!("{text} INFO x")).unwrap()
    }

    #[test]
    fn finds_the_exact_line_in_a_large_steady_log() {
        let (data, starts) = steady_log(60_000);
        let size = data.len() as u64;
        for wanted in [0, 1, 17, 30_000, 41_234, 59_999] {
            let mut src = Counting { inner: data.clone(), reads: 0 };
            let at = find_start_offset(&mut src, size, secs_of(&stamp(wanted))).unwrap();
            // One second of slack: the line a second earlier is included.
            let first = wanted.saturating_sub(1);
            let expected = starts.iter().find(|(_, i)| *i == first).unwrap().0;
            assert_eq!(at, expected, "start {wanted}");
            assert!(src.reads < 40, "{} reads", src.reads);
        }
    }

    #[test]
    fn start_past_the_end_is_the_size() {
        let (data, _) = steady_log(1000);
        let size = data.len() as u64;
        let at = find_start_offset(&mut data.clone(), size, secs_of("2026-10-20 00:00:00")).unwrap();
        assert_eq!(at, size);
    }

    #[test]
    fn start_before_the_first_line_is_zero() {
        let (data, _) = steady_log(1000);
        let size = data.len() as u64;
        assert_eq!(find_start_offset(&mut data.clone(), size, secs_of("2026-10-18 00:00:00")).unwrap(), 0);
    }

    #[test]
    fn end_offset_keeps_continuation_lines_of_the_last_line() {
        let (data, starts) = steady_log(20_000);
        let size = data.len() as u64;
        let at = find_end_offset(&mut data.clone(), size, secs_of(&stamp(7_000))).unwrap();
        let next = starts.iter().find(|(_, i)| *i == 7_001).unwrap().0;
        assert_eq!(at, next);
        // Line 7000 is the 1001st multiple of 7: its stack frame is before `at`.
        assert!(String::from_utf8_lossy(&data[..at as usize]).ends_with("(Frame.java:1)\n"));
    }

    #[test]
    fn a_log_that_is_a_few_seconds_out_of_order() {
        // Two writers: every other line is up to 3 seconds behind.
        let mut data = Vec::new();
        let mut starts = Vec::new();
        for i in 0..30_000u64 {
            let skew = if i % 2 == 1 { 3 } else { 0 };
            starts.push((data.len() as u64, i));
            data.extend_from_slice(format!("{} INFO line {i} from a writer\n", stamp(i + 10 - skew.min(i + 10))).as_bytes());
        }
        let size = data.len() as u64;
        let wanted = 12_000;
        let at = find_start_offset(&mut data.clone(), size, secs_of(&stamp(wanted + 10))).unwrap();
        // Lands within a few lines of the exact one, never after the first line at the time.
        let exact = starts.iter().find(|(_, i)| *i == wanted).unwrap().0;
        let a_few = starts.iter().find(|(_, i)| *i == wanted - 8).unwrap().0;
        assert!(at <= exact && at >= a_few, "at {at}, between {a_few} and {exact}");
    }

    #[test]
    fn a_long_stretch_without_times_does_not_stop_the_search() {
        let mut data = Vec::new();
        let mut starts = Vec::new();
        for i in 0..2000u64 {
            starts.push((data.len() as u64, i));
            data.extend_from_slice(format!("{} INFO early {i}\n", stamp(i)).as_bytes());
        }
        // 3 MB of stack trace with no time in it.
        let filler = "    at com.example.Deep.frame(Deep.java:99)\n".repeat(75_000);
        data.extend_from_slice(filler.as_bytes());
        for i in 2000..4000u64 {
            starts.push((data.len() as u64, i));
            data.extend_from_slice(format!("{} INFO late {i}\n", stamp(i)).as_bytes());
        }
        let size = data.len() as u64;
        let at = find_start_offset(&mut data.clone(), size, secs_of(&stamp(3500))).unwrap();
        let exact = starts.iter().find(|(_, i)| *i == 3499).unwrap().0;
        assert_eq!(at, exact);
    }

    #[test]
    fn a_file_with_no_times_gives_the_size() {
        let data = b"just text\nmore text\n".repeat(10_000);
        let size = data.len() as u64;
        assert_eq!(find_start_offset(&mut data.clone(), size, 1e9).unwrap(), size);
    }

    #[test]
    fn a_small_file_and_an_empty_one() {
        let (data, _) = steady_log(10);
        let size = data.len() as u64;
        assert_eq!(find_start_offset(&mut data.clone(), size, secs_of(&stamp(0))).unwrap(), 0);
        assert_eq!(find_start_offset(&mut Vec::<u8>::new(), 0, 5.0).unwrap(), 0);
    }
}
