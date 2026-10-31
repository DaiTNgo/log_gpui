//! Memory and time of reading a log file: `mem_probe <file> [--parsed | --live]`.
//!
//! By default the file is read as the app reads a local file: Indexing
//! builds its Line Index from disk (`IndexedFile::index_now`), and what it
//! costs is the index, not the text. `--parsed` parses it in one go into
//! entries held in memory (`ParsedFile::from_bytes`), as a file held whole
//! is.
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

/// Peak RSS of this process so far, in MB.
fn max_rss_mb() -> f64 {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    // SAFETY: getrusage fills the struct it is given.
    let usage = unsafe {
        libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr());
        usage.assume_init()
    };
    // Bytes on macOS, KiB on Linux.
    let bytes = if cfg!(target_os = "macos") { usage.ru_maxrss as f64 } else { usage.ru_maxrss as f64 * 1024.0 };
    bytes / 1e6
}

fn indexed(path: &str) {
    use log_gpui::index::file::IndexedFile;
    use log_gpui::index::{DiskSource, HandleBudget, LineSource};
    let len = std::fs::metadata(path).unwrap().len();
    let before = rss_mb();
    let t = std::time::Instant::now();
    let source: std::sync::Arc<dyn LineSource> = std::sync::Arc::new(DiskSource::on_demand(path, HandleBudget::new(16)));
    let file = IndexedFile::index_now(source, Default::default()).unwrap();
    let took = t.elapsed();
    let n = file.len();
    println!(
        "{n} lines, {:.1} MB file, Indexing {took:?}, +{:.0} MB ({:.0} bytes/line, index {:.0} bytes/line), max RSS {:.0} MB",
        len as f64 / 1e6,
        rss_mb() - before,
        (rss_mb() - before) * 1e6 / n as f64,
        file.heap_bytes() as f64 / n as f64,
        max_rss_mb()
    );
}

fn main() {
    let path = std::env::args().nth(1).expect("file");
    let live = std::env::args().any(|a| a == "--live");
    if !live && !std::env::args().any(|a| a == "--parsed") {
        return indexed(&path);
    }
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
        (parsed.entries().len(), Box::new(parsed) as Box<dyn std::any::Any>)
    };
    println!(
        "{n} lines, {:.1} MB file, parse {:?}, +{:.0} MB ({:.0} bytes/line), max RSS {:.0} MB, sizeof LogEntry {}",
        bytes.len() as f64 / 1e6,
        t.elapsed(),
        rss_mb() - before,
        (rss_mb() - before) * 1e6 / n as f64,
        max_rss_mb(),
        std::mem::size_of::<log_gpui::model::LogEntry>()
    );
}
