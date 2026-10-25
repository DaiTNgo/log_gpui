//! Reading files through a Line Index (openspec change `line-offset-index`):
//! the app keeps where each line is and a few bytes of metadata per line,
//! and reads a line's text again from its `LineSource` when it is shown.

pub mod fast;
pub mod file;
pub mod handles;
pub mod indexing;
pub mod line_index;
pub mod meta;
pub mod scan;
pub mod source;
pub mod tail;
pub mod view_rows;

pub use handles::HandleBudget;
pub use line_index::LineIndex;
pub use meta::{LineMeta, MetaTable, RawMeta};
pub use indexing::Indexed;
pub use source::{BytesSource, DiskSource, LineSource};

use crate::log_format::CompiledFormat;
use crate::model::LogEntry;
use crate::parser::text::{LineParser, parse_line_with};
use std::borrow::Cow;
use std::io;
use std::ops::Range;

/// Bytes read from a source at a time while indexing it.
pub const READ_BLOCK: usize = 4 << 20;

/// A file's Line Index and the metadata of its lines.
#[derive(Debug, Clone, Default)]
pub struct FileIndex {
    pub lines: LineIndex,
    pub meta: MetaTable,
}

impl FileIndex {
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Add the next line, its bytes with its line break.
    pub fn push_line(&mut self, bytes: &[u8], format: Option<&CompiledFormat>) {
        let text = line_index::line_text(bytes);
        self.lines.push(bytes.len() as u64);
        self.meta.push(raw_meta(text, format));
    }

    pub fn shrink_to_fit(&mut self) {
        self.lines.shrink_to_fit();
        self.meta.shrink_to_fit();
    }

    pub fn heap_bytes(&self) -> usize {
        self.lines.heap_bytes() + self.meta.heap_bytes()
    }
}

/// What a line's text says of itself, read as the app's parser reads it.
pub fn raw_meta(text: &[u8], format: Option<&CompiledFormat>) -> RawMeta {
    let line = String::from_utf8_lossy(text);
    let event = parse_line_with(0, &line, format);
    let entry = LogEntry::from_event(&event);
    RawMeta { not_utf8: matches!(line, Cow::Owned(_)), ..RawMeta::of_entry(&entry) }
}

/// Index every line of `source`, in order, on this thread. The background
/// pipeline (Indexing) gives the same index faster; this is what it is
/// checked against.
pub fn index_source(source: &dyn LineSource, format: Option<&CompiledFormat>) -> io::Result<FileIndex> {
    let mut index = FileIndex::default();
    let mut carry: Vec<u8> = Vec::new();
    let mut offset = 0u64;
    loop {
        let mut buf = std::mem::take(&mut carry);
        let have = buf.len();
        buf.resize(have + READ_BLOCK, 0);
        let got = source.read_at(offset, &mut buf[have..])?;
        offset += got as u64;
        buf.truncate(have + got);
        let eof = got < READ_BLOCK;
        let whole = if eof {
            buf.len()
        } else {
            memchr::memrchr(b'\n', &buf).map_or(0, |at| at + 1)
        };
        carry = buf.split_off(whole);
        for line in line_index::split_lines(&buf) {
            index.push_line(line, format);
        }
        if eof {
            break;
        }
    }
    source.release();
    index.shrink_to_fit();
    Ok(index)
}

/// The text of lines `lines` (0-based), read from `source` in one read,
/// without their line breaks. Fails when the source no longer has all of
/// them (the file shrank).
pub fn read_lines(source: &dyn LineSource, index: &LineIndex, lines: Range<usize>) -> io::Result<Vec<String>> {
    let Some(bytes) = index.byte_range(lines.start, lines.end.min(index.len())) else {
        return Ok(Vec::new());
    };
    let want = usize::try_from(bytes.end - bytes.start).map_err(io::Error::other)?;
    let data = source.read_range(bytes.start, want)?;
    if data.len() < want {
        return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "the file no longer has these lines"));
    }
    let mut at = 0;
    Ok(index
        .lens(lines.start, lines.end)
        .into_iter()
        .map(|len| {
            let line = &data[at..at + len as usize];
            at += len as usize;
            String::from_utf8_lossy(line_index::line_text(line)).into_owned()
        })
        .collect())
}

/// Lines `lines` (0-based) parsed again into entries with `parser`, as
/// they were read when the whole file was parsed: the same line numbers,
/// and a Continuation Line at the top keeps its Parent Line above.
pub fn read_entries(
    source: &dyn LineSource,
    index: &FileIndex,
    lines: Range<usize>,
    parser: &mut LineParser,
) -> io::Result<Vec<LogEntry>> {
    let texts = read_lines(source, &index.lines, lines.clone())?;
    let parent = index
        .meta
        .get(lines.start)
        .and_then(|m| m.parent_of(lines.start))
        .map(|p| p + 1);
    parser.set_parent(parent);
    Ok(texts
        .iter()
        .enumerate()
        .map(|(i, text)| parser.parse(lines.start + i + 1, text))
        .collect())
}
