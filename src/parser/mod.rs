pub mod text;

pub use text::ParseResult;
use std::path::Path;

pub fn parse_file<P: AsRef<Path>>(path: P) -> anyhow::Result<ParseResult> {
    let bytes = std::fs::read(path.as_ref())?;
    text::parse_text_bytes_with_max_chars(&bytes)
}
