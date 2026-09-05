pub mod text;

use crate::model::LogEntry;
use std::path::Path;

pub fn parse_file<P: AsRef<Path>>(path: P) -> anyhow::Result<Vec<LogEntry>> {
    let bytes = std::fs::read(path.as_ref())?;
    text::parse_text_bytes(&bytes)
}
