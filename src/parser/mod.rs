pub mod event;
pub mod text;

pub use event::{LogEvent, LogValue};
pub use text::{parse_text_bytes_with_max_chars, ParseResult, SchemaParseResult};
use std::path::Path;

pub fn parse_file<P: AsRef<Path>>(path: P) -> anyhow::Result<ParseResult> {
    let bytes = std::fs::read(path.as_ref())?;
    text::parse_text_bytes_with_max_chars(&bytes)
}

pub fn parse_file_with_schema<P: AsRef<Path>>(path: P) -> anyhow::Result<SchemaParseResult> {
    let bytes = std::fs::read(path.as_ref())?;
    text::parse_text_bytes_with_schema(&bytes)
}