//! Memory and time of parsing a log file: `mem_probe <file> [--live]`.
//!
//! Without `--live` the file is parsed in one go (`ParsedFile::from_bytes`).
//! With `--live` it is parsed line by line the way a live source delivers it,
//! and the rows are copied into a second list as the merged view does, so the
//! figure is what a row costs while it is open: the cached entry, the Raw
//! View line and the view's copy.
use log_gpui::parser::text::LineParser;
use log_gpui::space_view::ParsedFile;

fn rss_mb() -> f64 {
    let out = std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim().parse::<f64>().unwrap_or(0.0) / 1024.0
}

fn main() {
    let path = std::env::args().nth(1).expect("file");
    let live = std::env::args().any(|a| a == "--live");
    let bytes = std::fs::read(&path).unwrap();
    let before = rss_mb();
    let t = std::time::Instant::now();
    let (n, _keep) = if live {
        let text = String::from_utf8_lossy(&bytes);
        let mut parser = LineParser::new();
        let mut entries = Vec::new();
        let mut lines = Vec::new();
        for (i, line) in text.lines().enumerate() {
            let entry = parser.parse(i + 1, line);
            // The Raw View's line is the entry's own text, not a copy.
            lines.push(entry.raw_line.clone());
            entries.push(entry);
        }
        let after_parse = rss_mb();
        let view = entries.clone();
        println!(
            "entry + line: {:.0} bytes/line, view copy: {:.0} bytes/line",
            (after_parse - before) * 1e6 / entries.len() as f64,
            (rss_mb() - after_parse) * 1e6 / entries.len() as f64
        );
        (entries.len(), Box::new((entries, lines, view)) as Box<dyn std::any::Any>)
    } else {
        let parsed = ParsedFile::from_bytes(&bytes);
        (parsed.entries.len(), Box::new(parsed) as Box<dyn std::any::Any>)
    };
    println!(
        "{n} lines, {:.1} MB file, parse {:?}, +{:.0} MB ({:.0} bytes/line), sizeof LogEntry {}",
        bytes.len() as f64 / 1e6,
        t.elapsed(),
        rss_mb() - before,
        (rss_mb() - before) * 1e6 / n as f64,
        std::mem::size_of::<log_gpui::model::LogEntry>()
    );
}
