use super::line_time::line_timestamp;
use super::splitter::decode_line;

/// Lines this much older than the Starting Time are still kept: writers of a
/// log do not agree on the second.
pub const START_SLACK_SECS: f64 = 1.0;

/// Bytes of a file at an offset. A local file and an SFTP handle both fit.
pub trait ReadAt {
    /// Read up to `buf.len()` bytes at `offset`; fewer only at the end.
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> std::io::Result<usize>;
}

impl ReadAt for std::fs::File {
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> std::io::Result<usize> {
        use std::io::{Read, Seek, SeekFrom};
        self.seek(SeekFrom::Start(offset))?;
        let mut done = 0;
        while done < buf.len() {
            let n = self.read(&mut buf[done..])?;
            if n == 0 {
                break;
            }
            done += n;
        }
        Ok(done)
    }
}

impl ReadAt for Vec<u8> {
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> std::io::Result<usize> {
        let start = (offset as usize).min(self.len());
        let n = buf.len().min(self.len() - start);
        buf[..n].copy_from_slice(&self[start..start + n]);
        Ok(n)
    }
}

/// Reads the lines of a file from the end towards the start, a block at a
/// time. The first batch holds the last complete lines; the unfinished line
/// after the last newline is left to the forward reader (`follow_offset`).
///
/// The reader only plans the reads (`want_block`) and takes the bytes
/// (`absorb_block`), so a synchronous file and an SFTP handle driven with
/// `await` share the same logic.
#[derive(Debug, Clone)]
pub struct BackwardLines {
    /// Start of the bytes held in `data`.
    pos: u64,
    /// Bytes from `pos` on. Its first segment (up to the first newline) is
    /// the end of a line whose start is still further up, unless `pos` is 0.
    data: Vec<u8>,
    newlines: usize,
    block: usize,
    first: bool,
    follow_offset: u64,
    /// Starting Time: reading stops at the first Parent Line older than it.
    stop_before: Option<f64>,
    stopped: bool,
}

impl BackwardLines {
    /// `end` is the size of the file when reading starts.
    pub fn new(end: u64, block: usize) -> Self {
        Self { pos: end, data: Vec::new(), newlines: 0, block: block.max(1), first: true, follow_offset: end, stop_before: None, stopped: false }
    }

    /// Stop at the first Parent Line older than `time` (less a second of
    /// slack): that line and the Continuation Lines of it are not given, and
    /// nothing older is read.
    pub fn with_stop_before(mut self, time: Option<f64>) -> Self {
        self.stop_before = time;
        self
    }

    /// Whether a line older than the Starting Time was found.
    pub fn reached_start_time(&self) -> bool {
        self.stopped
    }

    /// Where forward reading continues: just after the last newline.
    pub fn follow_offset(&self) -> u64 {
        self.follow_offset
    }

    pub fn is_done(&self) -> bool {
        self.pos == 0 && self.data.is_empty()
    }

    /// The next block to read as `(offset, length)`, or `None` when enough is
    /// held to give `max_lines` lines.
    pub fn want_block(&self, max_lines: usize) -> Option<(u64, usize)> {
        if self.pos == 0 {
            return None;
        }
        // One more newline than wanted: the first segment may be cut.
        if !self.first && self.newlines > max_lines.max(1) {
            return None;
        }
        let take = (self.block as u64).min(self.pos) as usize;
        Some((self.pos - take as u64, take))
    }

    /// Add the block read at the offset `want_block` gave.
    pub fn absorb_block(&mut self, mut block: Vec<u8>) {
        self.pos -= block.len().min(self.pos as usize) as u64;
        block.extend_from_slice(&self.data);
        self.data = block;
        if self.first {
            if let Some(last) = self.data.iter().rposition(|b| *b == b'\n') {
                // Drop the unfinished tail line.
                self.follow_offset = self.pos + last as u64 + 1;
                self.data.truncate(last + 1);
                self.first = false;
            } else if self.pos == 0 {
                // No newline in the whole file: nothing complete.
                self.follow_offset = 0;
                self.data.clear();
                self.first = false;
            }
        }
        self.newlines = self.data.iter().filter(|b| **b == b'\n').count();
    }

    /// The next `max_lines` complete lines above what was returned before, in
    /// file order. Empty when there is nothing more. Call once `want_block`
    /// says nothing more is needed.
    pub fn take_batch(&mut self, max_lines: usize) -> Vec<String> {
        self.take_batch_with_lens(max_lines).0
    }

    /// `take_batch`, and the length of each line in the file, line end
    /// included.
    pub fn take_batch_with_lens(&mut self, max_lines: usize) -> (Vec<String>, Vec<u32>) {
        let max_lines = max_lines.max(1);
        if self.first {
            // Only an empty file gets here.
            self.first = false;
            self.follow_offset = 0;
            self.data.clear();
            self.newlines = 0;
            return (Vec::new(), Vec::new());
        }
        let data = std::mem::take(&mut self.data);
        let mut segments: Vec<&[u8]> = data.split_inclusive(|b| *b == b'\n').collect();
        // The first segment is whole only when we reached the file start.
        let mut cut_head: Option<&[u8]> = None;
        if self.pos > 0 && !segments.is_empty() {
            cut_head = Some(segments.remove(0));
        }
        let keep_from = segments.len().saturating_sub(max_lines);
        let mut rest = Vec::new();
        if let Some(head) = cut_head {
            rest.extend_from_slice(head);
        }
        for s in &segments[..keep_from] {
            rest.extend_from_slice(s);
        }
        let lines: Vec<String> = segments[keep_from..]
            .iter()
            .map(|s| decode_line(&s[..s.len() - 1]))
            .collect();
        let lens: Vec<u32> = segments[keep_from..].iter().map(|s| s.len() as u32).collect();
        self.newlines = rest.iter().filter(|b| **b == b'\n').count();
        self.data = rest;
        self.cut_at_start_time(lines, lens)
    }

    /// Drop what is older than the Starting Time, if the batch reaches it.
    fn cut_at_start_time(&mut self, mut lines: Vec<String>, mut lens: Vec<u32>) -> (Vec<String>, Vec<u32>) {
        let Some(time) = self.stop_before else { return (lines, lens) };
        let limit = time - START_SLACK_SECS;
        let old = lines.iter().rposition(|l| line_timestamp(l).is_some_and(|t| t < limit));
        let Some(old) = old else { return (lines, lens) };
        // The Continuation Lines after that Parent Line belong to it.
        let keep_from = lines[old + 1..]
            .iter()
            .position(|l| line_timestamp(l).is_some())
            .map_or(lines.len(), |i| old + 1 + i);
        lines.drain(..keep_from);
        lens.drain(..keep_from);
        self.stopped = true;
        self.pos = 0;
        self.data.clear();
        self.newlines = 0;
        (lines, lens)
    }

    /// Synchronous convenience over a `ReadAt`.
    pub fn next_batch<R: ReadAt>(&mut self, src: &mut R, max_lines: usize) -> std::io::Result<Vec<String>> {
        Ok(self.next_batch_with_lens(src, max_lines)?.0)
    }

    /// `next_batch`, and the length of each line in the file.
    pub fn next_batch_with_lens<R: ReadAt>(
        &mut self,
        src: &mut R,
        max_lines: usize,
    ) -> std::io::Result<(Vec<String>, Vec<u32>)> {
        while let Some((offset, len)) = self.want_block(max_lines) {
            let mut block = vec![0u8; len];
            let n = src.read_at(offset, &mut block)?;
            block.truncate(n);
            if n < len {
                // A short read inside the file: the file shrank; stop here.
                self.absorb_block(block);
                self.pos = 0;
                break;
            }
            self.absorb_block(block);
        }
        Ok(self.take_batch_with_lens(max_lines))
    }
}

#[cfg(test)]
mod start_time_tests {
    use super::*;

    fn lines_from(text: &str, t: Option<f64>, max: usize) -> (Vec<String>, bool) {
        let mut src = text.as_bytes().to_vec();
        let mut b = BackwardLines::new(src.len() as u64, 16).with_stop_before(t);
        let got = b.next_batch(&mut src, max).unwrap();
        (got, b.reached_start_time())
    }

    fn t(s: &str) -> f64 {
        line_timestamp(&format!("{s} INFO x")).unwrap()
    }

    const LOG: &str = "2026-10-19 09:00:00 INFO old\n  old continued\n2026-10-19 10:00:00 INFO a\n  a continued\n2026-10-19 10:05:00 INFO b\n";

    #[test]
    fn stops_at_the_first_older_parent_line_and_keeps_continuations_of_new_ones() {
        let (got, stopped) = lines_from(LOG, Some(t("2026-10-19 10:00:00")), 100);
        assert!(stopped);
        assert_eq!(got, ["2026-10-19 10:00:00 INFO a", "  a continued", "2026-10-19 10:05:00 INFO b"]);
    }

    #[test]
    fn a_line_without_time_between_new_lines_stays() {
        let text = "2026-10-19 10:00:00 INFO a\nno time\n2026-10-19 10:01:00 INFO b\n";
        let (got, stopped) = lines_from(text, Some(t("2026-10-19 09:00:00")), 100);
        assert!(!stopped);
        assert_eq!(got.len(), 3);
    }

    #[test]
    fn a_second_of_skew_is_tolerated() {
        let text = "2026-10-19 09:59:59 INFO a\n2026-10-19 10:00:01 INFO b\n";
        let (got, stopped) = lines_from(text, Some(t("2026-10-19 10:00:00")), 100);
        assert!(!stopped);
        assert_eq!(got.len(), 2);
        let text = "2026-10-19 09:59:58 INFO a\n2026-10-19 10:00:01 INFO b\n";
        let (got, stopped) = lines_from(text, Some(t("2026-10-19 10:00:00")), 100);
        assert!(stopped);
        assert_eq!(got, ["2026-10-19 10:00:01 INFO b"]);
    }

    #[test]
    fn everything_older_gives_no_lines() {
        let (got, stopped) = lines_from(LOG, Some(t("2026-10-19 11:00:00")), 100);
        assert!(stopped);
        assert!(got.is_empty());
    }

    #[test]
    fn without_a_start_time_nothing_changes() {
        let (got, stopped) = lines_from(LOG, None, 100);
        assert!(!stopped);
        assert_eq!(got.len(), 5);
    }

    #[test]
    fn the_start_time_is_found_across_batches() {
        // Only two lines per batch: the old line is in a later batch.
        let mut src = LOG.as_bytes().to_vec();
        let mut b = BackwardLines::new(src.len() as u64, 16).with_stop_before(Some(t("2026-10-19 10:00:00")));
        let first = b.next_batch(&mut src, 2).unwrap();
        assert_eq!(first, ["  a continued", "2026-10-19 10:05:00 INFO b"]);
        assert!(!b.reached_start_time());
        let second = b.next_batch(&mut src, 3).unwrap();
        assert_eq!(second, ["2026-10-19 10:00:00 INFO a"]);
        assert!(b.reached_start_time() && b.is_done());
    }
}
