//! Measurements for the numbers in `openspec/changes/remote-full-read/design.md`
//! (task 7.2):
//!
//! * decoding speed of each compression kind (MB/s of text out),
//! * reads of the timestamp search on a 1 GB file,
//! * with a local sshd (`SSH_TEST_DIR`, `SSH_TEST_PORT`): SFTP read speed, and
//!   time and memory of reading 2,000,000 lines into a Remote Copy.
//!
//! `cargo run --release --example measure_full_read`

use log_gpui::live::LiveBuffer;
use log_gpui::live::backward::ReadAt;
use log_gpui::remote::compress::{Compression, decoder};
use log_gpui::remote::copy;
use log_gpui::remote::full_read::{GenerationPlan, ReadControl, ReadEvent, ReadPlan, find_start_offset, run};
use log_gpui::remote::*;
use log_gpui::space_view::ParsedFile;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::time::Instant;

fn stamp(i: u64) -> String {
    let (h, m, s) = (i / 3600 % 24, i / 60 % 60, i % 60);
    format!("2026-10-19 {h:02}:{m:02}:{s:02}.{:03} INFO [worker-{}] request {i} handled path=/api/v1/items/{} status=200 took={}ms\n", i % 1000, i % 16, i * 7 % 10_000, i % 90)
}

fn rss_mb() -> f64 {
    let out = std::process::Command::new("ps").args(["-o", "rss=", "-p", &std::process::id().to_string()]).output().unwrap();
    String::from_utf8_lossy(&out.stdout).trim().parse::<f64>().unwrap_or(0.0) / 1024.0
}

struct Counting {
    file: std::fs::File,
    reads: usize,
    bytes: u64,
}

impl ReadAt for Counting {
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> std::io::Result<usize> {
        self.reads += 1;
        let n = self.file.read_at(offset, buf)?;
        self.bytes += n as u64;
        Ok(n)
    }
}

fn main() {
    let dir = tempfile::tempdir().unwrap();
    // ── Decoding speed ──
    let text: String = (0..1_500_000u64).map(stamp).collect();
    let raw = dir.path().join("raw.log");
    std::fs::write(&raw, &text).unwrap();
    println!("decode: {:.0} MB of log text", text.len() as f64 / 1e6);
    for (tool, args, kind) in [
        ("gzip", vec!["-k", "-c"], Compression::Gzip),
        ("zstd", vec!["-q", "-c"], Compression::Zstd),
        ("xz", vec!["-k", "-c"], Compression::Xz),
        ("bzip2", vec!["-k", "-c"], Compression::Bzip2),
    ] {
        let out = std::process::Command::new(tool).args(&args).arg(&raw).output();
        let Ok(out) = out else { continue };
        let packed = out.stdout;
        let started = Instant::now();
        let mut reader = decoder(kind, std::io::Cursor::new(packed.clone())).unwrap();
        let mut sink = vec![0u8; 1 << 20];
        let mut total = 0usize;
        loop {
            let n = reader.read(&mut sink).unwrap();
            if n == 0 {
                break;
            }
            total += n;
        }
        let secs = started.elapsed().as_secs_f64();
        println!(
            "  {:<6} packed {:>6.1} MB ({:.1}x), decoded {:.0} MB in {:.2} s = {:.0} MB/s",
            tool,
            packed.len() as f64 / 1e6,
            text.len() as f64 / packed.len() as f64,
            total as f64 / 1e6,
            secs,
            total as f64 / 1e6 / secs
        );
    }

    // ── Timestamp search on 1 GB ──
    let big = dir.path().join("big.log");
    {
        let mut f = std::io::BufWriter::new(std::fs::File::create(&big).unwrap());
        let mut i = 0u64;
        let mut written = 0u64;
        while written < 1u64 << 30 {
            let line = stamp(i);
            f.write_all(line.as_bytes()).unwrap();
            written += line.len() as u64;
            i += 1;
        }
        println!("search: file of {:.2} GB, {i} lines", written as f64 / 1e9);
    }
    let size = std::fs::metadata(&big).unwrap().len();
    let wanted = log_gpui::live::line_time::line_timestamp(&stamp(4_000_000)).unwrap();
    let mut src = Counting { file: std::fs::File::open(&big).unwrap(), reads: 0, bytes: 0 };
    let started = Instant::now();
    let at = find_start_offset(&mut src, size, wanted).unwrap();
    println!(
        "  start of line 4,000,000 found at byte {at} with {} reads ({:.1} MB read) in {:?}",
        src.reads,
        src.bytes as f64 / 1e6,
        started.elapsed()
    );
    std::fs::remove_file(&big).unwrap();

    // ── Over SSH ──
    let (Ok(key_dir), Some(user)) = (std::env::var("SSH_TEST_DIR"), std::env::var("USER").ok()) else {
        println!("ssh: SSH_TEST_DIR not set, skipped");
        return;
    };
    let port: u16 = std::env::var("SSH_TEST_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(2222);
    let mut server = Server::new("web1", "127.0.0.1", &user, AuthMethod::Key { path: format!("{key_dir}/userkey") });
    server.port = port;
    let mut store = ServerStore::default();
    let server_id = store.save(server).unwrap();
    let (hub, requests) = RemoteHub::new(Arc::new(MemorySecrets::default()), Arc::new(Mutex::new(KnownHosts::default())));
    std::thread::spawn(move || {
        while let Ok(req) = requests.recv_blocking() {
            let _ = req.reply.send(true);
        }
    });
    let lines = 2_000_000u64;
    let remote_file = dir.path().join("app.log");
    {
        let mut f = std::io::BufWriter::new(std::fs::File::create(&remote_file).unwrap());
        for i in 0..lines {
            f.write_all(stamp(i).as_bytes()).unwrap();
        }
    }
    let size = std::fs::metadata(&remote_file).unwrap().len();
    println!("ssh: {lines} lines, {:.0} MB, loopback sshd", size as f64 / 1e6);
    let copies = tempfile::tempdir().unwrap();
    let plan = ReadPlan {
        server_id,
        server_name: "web1".into(),
        path: remote_file.to_string_lossy().to_string(),
        generations: vec![GenerationPlan {
            distance: 0,
            name: "app.log".into(),
            path: remote_file.to_string_lossy().to_string(),
            size,
            modified: None,
            compression: Compression::None,
            start_byte: None,
            skip_lines: 0,
            older_dropped: false,
        }],
        start: None,
        to: None,
        start_text: None,
        to_text: None,
        limit_lines: 2_000_000,
        dir: copy::create_dir(copies.path()).unwrap(),
    };
    let (tx, rx) = async_channel::unbounded();
    let rss_before = rss_mb();
    let started = Instant::now();
    hub.spawn(run(hub.clone(), Arc::new(Mutex::new(store)), plan, tx, Arc::new(ReadControl::default())));
    let mut parsed = ParsedFile::empty();
    let mut buffer = LiveBuffer::command("web1:app.log");
    let (mut got, mut first_lines_at, mut peak) = (0usize, None, 0.0f64);
    while let Ok(event) = rx.recv_blocking() {
        match event {
            ReadEvent::GenerationStart { first_line_no, .. } => buffer.begin_copy_generation(first_line_no, true, "rotated"),
            ReadEvent::Lines { lines, bytes } => {
                first_lines_at.get_or_insert_with(|| started.elapsed());
                got += lines.len();
                buffer.apply_append_with(&mut parsed, &lines, &bytes);
                peak = peak.max(rss_mb());
            }
            ReadEvent::Done(_) | ReadEvent::Failed { .. } => break,
            _ => {}
        }
    }
    let total = started.elapsed();
    println!(
        "  read {got} lines in {:.1} s ({:.0} MB/s over sftp+parse), first lines after {:?}",
        total.as_secs_f64(),
        size as f64 / 1e6 / total.as_secs_f64(),
        first_lines_at.unwrap_or_default()
    );
    println!(
        "  memory: {rss_before:.0} MB before, {peak:.0} MB peak, {:.0} MB after ({:.2} KB per line)",
        rss_mb(),
        (rss_mb() - rss_before) * 1024.0 / got.max(1) as f64
    );
    // Plain SFTP speed, for comparison with the decoders.
    println!("  copy folder: {:.0} MB on disk", copy::total_size(copies.path()) as f64 / 1e6);
}
