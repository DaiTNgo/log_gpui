//! Where each line of a file starts (decision 1): blocks of 128 lines, each
//! the absolute start of its first line and the varint length of every line
//! in it. Finding line `n` decodes one block, whatever the size of the file;
//! the spike measured ~2.3 bytes per line on a 376MB log.

/// Lines per block.
pub const BLOCK_LINES: usize = 128;

/// The lines of a file, as positions in its bytes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LineIndex {
    /// Start of each block's first line.
    bases: Vec<u64>,
    /// Position in `lens` of each block's first length.
    starts: Vec<u32>,
    /// Length of every line, its line break included, as varints.
    lens: Vec<u8>,
    lines: usize,
    /// End of the last line indexed: where the next line starts.
    end: u64,
}

fn push_varint(out: &mut Vec<u8>, mut v: u64) {
    while v >= 0x80 {
        out.push((v as u8) | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

/// The varint at `*at`, moving `*at` past it.
fn read_varint(bytes: &[u8], at: &mut usize) -> u64 {
    let mut v = 0u64;
    let mut shift = 0;
    loop {
        let b = bytes[*at];
        *at += 1;
        v |= u64::from(b & 0x7f) << shift;
        if b < 0x80 {
            return v;
        }
        shift += 7;
    }
}

impl LineIndex {
    pub fn new() -> Self {
        Self::default()
    }

    /// Lines indexed.
    pub fn len(&self) -> usize {
        self.lines
    }

    pub fn is_empty(&self) -> bool {
        self.lines == 0
    }

    /// Bytes of the file the lines cover.
    pub fn end(&self) -> u64 {
        self.end
    }

    /// Add the next line, `len` bytes with its line break.
    pub fn push(&mut self, len: u64) {
        if self.lines % BLOCK_LINES == 0 {
            self.bases.push(self.end);
            self.starts.push(u32::try_from(self.lens.len()).expect("Line Index over 4GB"));
        }
        push_varint(&mut self.lens, len);
        self.lines += 1;
        self.end += len;
    }

    /// Start and length (with its line break) of line `n`, 0-based.
    pub fn span(&self, n: usize) -> Option<(u64, u64)> {
        if n >= self.lines {
            return None;
        }
        let block = n / BLOCK_LINES;
        let mut at = self.starts[block] as usize;
        let mut start = self.bases[block];
        for _ in 0..n % BLOCK_LINES {
            start += read_varint(&self.lens, &mut at);
        }
        Some((start, read_varint(&self.lens, &mut at)))
    }

    /// Where lines `from..to` start and end in the file: one read for all
    /// of them.
    pub fn byte_range(&self, from: usize, to: usize) -> Option<std::ops::Range<u64>> {
        if from >= to {
            return None;
        }
        let (start, _) = self.span(from)?;
        let (last, len) = self.span(to - 1)?;
        Some(start..last + len)
    }

    /// The lengths of lines `from..to`, with their line breaks.
    pub fn lens(&self, from: usize, to: usize) -> Vec<u64> {
        let to = to.min(self.lines);
        if from >= to {
            return Vec::new();
        }
        let mut out = Vec::with_capacity(to - from);
        let block = from / BLOCK_LINES;
        let mut at = self.starts[block] as usize;
        for _ in 0..from % BLOCK_LINES {
            read_varint(&self.lens, &mut at);
        }
        for _ in from..to {
            out.push(read_varint(&self.lens, &mut at));
        }
        out
    }

    /// Drop the last line (a live file's last line was cut off before its
    /// line break and grew since); its length.
    pub fn pop(&mut self) -> Option<u64> {
        let n = self.lines.checked_sub(1)?;
        let block = n / BLOCK_LINES;
        let mut at = self.starts[block] as usize;
        for _ in 0..n % BLOCK_LINES {
            read_varint(&self.lens, &mut at);
        }
        let len_at = at;
        let len = read_varint(&self.lens, &mut at);
        self.lens.truncate(len_at);
        if n % BLOCK_LINES == 0 {
            self.bases.pop();
            self.starts.pop();
        }
        self.lines = n;
        self.end -= len;
        Some(len)
    }

    /// Drop the lines from line `n` on.
    pub fn truncate(&mut self, n: usize) {
        while self.lines > n {
            self.pop();
        }
    }

    /// Free what the vectors hold beyond their lines.
    pub fn shrink_to_fit(&mut self) {
        self.bases.shrink_to_fit();
        self.starts.shrink_to_fit();
        self.lens.shrink_to_fit();
    }

    /// Memory the index holds.
    pub fn heap_bytes(&self) -> usize {
        self.bases.capacity() * 8 + self.starts.capacity() * 4 + self.lens.capacity()
    }
}

/// The text of a line read with its line break: without `\n`, and without
/// the `\r` of a `\r\n` (as `str::lines` reads it).
pub fn line_text(bytes: &[u8]) -> &[u8] {
    match bytes.strip_suffix(b"\n") {
        Some(text) => text.strip_suffix(b"\r").unwrap_or(text),
        None => bytes,
    }
}

/// The lines of `data`, each with its line break; the last one without
/// when `data` does not end in one.
pub fn split_lines(data: &[u8]) -> impl Iterator<Item = &[u8]> {
    let mut start = 0;
    let mut ends = memchr::memchr_iter(b'\n', data);
    std::iter::from_fn(move || {
        if start >= data.len() {
            return None;
        }
        let end = ends.next().map_or(data.len(), |e| e + 1);
        let line = &data[start..end];
        start = end;
        Some(line)
    })
}
