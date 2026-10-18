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
}

impl BackwardLines {
    /// `end` is the size of the file when reading starts.
    pub fn new(end: u64, block: usize) -> Self {
        Self { pos: end, data: Vec::new(), newlines: 0, block: block.max(1), first: true, follow_offset: end }
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
        let max_lines = max_lines.max(1);
        if self.first {
            // Only an empty file gets here.
            self.first = false;
            self.follow_offset = 0;
            self.data.clear();
            self.newlines = 0;
            return Vec::new();
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
        self.newlines = rest.iter().filter(|b| **b == b'\n').count();
        self.data = rest;
        lines
    }

    /// Synchronous convenience over a `ReadAt`.
    pub fn next_batch<R: ReadAt>(&mut self, src: &mut R, max_lines: usize) -> std::io::Result<Vec<String>> {
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
        Ok(self.take_batch(max_lines))
    }
}
