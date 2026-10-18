pub mod event;
pub mod schema;
pub mod text;

pub use event::{LogEvent, LogValue};
pub use text::{parse_text_bytes_with_max_chars, LineParser, ParseResult, SchemaParseResult};
use std::path::Path;

use crate::archive::{FileSource, read_source};

pub fn parse_source(source: FileSource) -> anyhow::Result<ParseResult> {
    text::parse_text_bytes_with_max_chars(&read_source(source)?)
}

pub fn parse_source_with_schema(source: FileSource) -> anyhow::Result<SchemaParseResult> {
    text::parse_text_bytes_with_schema(&read_source(source)?)
}

pub fn parse_file<P: AsRef<Path>>(path: P) -> anyhow::Result<ParseResult> {
    parse_source(FileSource::Disk(path.as_ref()))
}

pub fn parse_file_with_schema<P: AsRef<Path>>(path: P) -> anyhow::Result<SchemaParseResult> {
    parse_source_with_schema(FileSource::Disk(path.as_ref()))
}