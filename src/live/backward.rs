use super::splitter::decode_line;

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
#[derive(Debug, Clone)]
pub struct BackwardLines {
    /// Start of the bytes read so far.
    pos: u64,
    /// Bytes from `pos` up to the first newline read so far: the end of a
    /// line whose start is still further up.
    carry: Vec<u8>,
    block: usize,
    first: bool,
    follow_offset: u64,
}

impl BackwardLines {
    /// `end` is the size of the file when reading starts.
    pub fn new(end: u64, block: usize) -> Self {
        Self { pos: end, carry: Vec::new(), block: block.max(1), first: true, follow_offset: end }
    }

    /// Where forward reading continues: just after the last newline.
    pub fn follow_offset(&self) -> u64 {
        self.follow_offset
    }

    pub fn is_done(&self) -> bool {
        self.pos == 0 && self.carry.is_empty()
    }

    /// The next `max_lines` complete lines above what was returned before, in
    /// file order. Empty when there is nothing more.
    pub fn next_batch<R: ReadAt>(&mut self, src: &mut R, max_lines: usize) -> std::io::Result<Vec<String>> {
        let max_lines = max_lines.max(1);
        // `data` holds whole lines (after its first newline) plus `carry`.
        let mut data = std::mem::take(&mut self.carry);
        loop {
            let newlines = data.iter().filter(|b| **b == b'\n').count();
            // One more than wanted: the first segment of `data` may be cut.
            if self.pos == 0 || newlines > max_lines {
                break;
            }
            let take = (self.block as u64).min(self.pos) as usize;
            self.pos -= take as u64;
            let mut block = vec![0u8; take];
            let n = src.read_at(self.pos, &mut block)?;
            block.truncate(n);
            block.extend_from_slice(&data);
            data = block;
            if self.first {
                if let Some(last) = data.iter().rposition(|b| *b == b'\n') {
                    // Drop the unfinished tail line.
                    self.follow_offset = self.pos + last as u64 + 1;
                    data.truncate(last + 1);
                    self.first = false;
                } else if self.pos == 0 {
                    // No newline in the whole file: nothing complete.
                    self.follow_offset = 0;
                    data.clear();
                    self.first = false;
                } else {
                    // Keep reading: the last newline is still further up.
                    continue;
                }
            }
        }
        if self.first {
            // Only reached with an empty file.
            self.first = false;
            self.follow_offset = 0;
        }
        // `data` is [start of a possibly cut line][\n]line\n line\n ...
        // Split it into segments; every segment ends with a newline.
        let mut segments: Vec<&[u8]> = data.split_inclusive(|b| *b == b'\n').collect();
        // The first segment is complete only when we reached the file start.
        let mut cut_head: Option<&[u8]> = None;
        if self.pos > 0 && !segments.is_empty() {
            cut_head = Some(segments.remove(0));
        }
        let keep_from = segments.len().saturating_sub(max_lines);
        let extra = &segments[..keep_from];
        let mut carry = Vec::new();
        if let Some(head) = cut_head {
            carry.extend_from_slice(head);
        }
        for s in extra {
            carry.extend_from_slice(s);
        }
        let lines: Vec<String> = segments[keep_from..]
            .iter()
            .map(|s| decode_line(&s[..s.len() - 1]))
            .collect();
        self.carry = carry;
        Ok(lines)
    }
}
