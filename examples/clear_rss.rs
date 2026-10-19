//! Memory a Remote File's cache takes before and after Clear and Back to Live
//! Window (task 7.3 of remote-clear-and-load-more). Run with
//! `cargo run --release --example clear_rss`.

use log_gpui::live::LiveBuffer;
use log_gpui::space_view::ParsedFile;

/// Memory the process is charged for: `footprint` where it exists (it leaves
/// out pages the allocator freed but the system has not taken back), else RSS.
fn rss_mb() -> f64 {
    let pid = std::process::id().to_string();
    if let Ok(out) = std::process::Command::new("footprint").args(["-p", &pid]).output() {
        let text = String::from_utf8_lossy(&out.stdout).to_string();
        if let Some(line) = text.lines().find(|l| l.contains("Footprint:")) {
            let rest = line.split("Footprint:").nth(1).unwrap_or("").trim();
            let mut parts = rest.split_whitespace();
            if let (Some(n), Some(unit)) = (parts.next().and_then(|n| n.parse::<f64>().ok()), parts.next()) {
                return match unit {
                    "KB" => n / 1024.0,
                    "GB" => n * 1024.0,
                    _ => n,
                };
            }
        }
    }
    let out = std::process::Command::new("ps").args(["-o", "rss=", "-p", &pid]).output().unwrap();
    String::from_utf8_lossy(&out.stdout).trim().parse::<f64>().unwrap_or(0.0) / 1024.0
}

#[cfg(target_os = "macos")]
fn give_back() {
    unsafe extern "C" {
        fn malloc_zone_pressure_relief(zone: *mut std::ffi::c_void, goal: usize) -> usize;
    }
    // SAFETY: a null zone means every zone; a goal of 0 means as much as can be.
    unsafe { malloc_zone_pressure_relief(std::ptr::null_mut(), 0) };
}

#[cfg(not(target_os = "macos"))]
fn give_back() {}

fn fill(count: usize) -> (ParsedFile, LiveBuffer) {
    let mut parsed = ParsedFile::from_text("");
    let mut buf = LiveBuffer::remote("web1:/app.log");
    let lines: Vec<String> = (0..count)
        .map(|i| {
            format!(
                "2026-09-05 10:{:02}:{:02}.{:03} INFO [worker-{}] request {i} handled in {} ms for user u{}",
                i / 3600 % 60,
                i / 60 % 60,
                i % 1000,
                i % 8,
                i % 300,
                i % 5000
            )
        })
        .collect();
    let bytes: Vec<u32> = lines.iter().map(|l| l.len() as u32 + 1).collect();
    buf.apply_tail_with(&mut parsed, &lines, 0, &bytes, bytes.iter().map(|b| u64::from(*b)).sum());
    (parsed, buf)
}

fn main() {
    let base = rss_mb();
    println!("start: {base:.0} MB");

    let (mut parsed, mut buf) = fill(100_000);
    let full = rss_mb();
    println!("100,000 lines held: {full:.0} MB (+{:.0})", full - base);
    buf.clear(&mut parsed, "10:42:11");
    let after_free = rss_mb();
    println!("after Clear: {after_free:.0} MB");
    give_back();
    let after_give_back = rss_mb();
    println!("after Clear and giving the allocator's cache back: {after_give_back:.0} MB");
    drop((parsed, buf));
    give_back();

    let base = rss_mb();
    let (mut parsed, mut buf) = fill(500_000);
    let full = rss_mb();
    println!("500,000 lines held: {full:.0} MB (+{:.0})", full - base);
    buf.back_to_window(&mut parsed, 100_000);
    println!("after Back to Live Window (100,000): {:.0} MB", rss_mb());
    give_back();
    println!("after giving the allocator's cache back: {:.0} MB", rss_mb());
    drop((parsed, buf));
}
