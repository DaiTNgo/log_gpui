//! Memory of the field spans of a line (custom-columns task 0.1). Run with
//! `cargo test --release --test span_memory_test -- --ignored --nocapture`.

use log_gpui::model::Spans;
use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::HashMap;
use std::ops::Range;
use std::sync::atomic::{AtomicIsize, Ordering};

struct Counting;

static LIVE: AtomicIsize = AtomicIsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        LIVE.fetch_add(layout.size() as isize, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size() as isize, Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

const LINES: usize = 1_000_000;
const FIELDS: usize = 10;

fn names() -> Vec<String> {
    let mut names: Vec<String> = ["timestamp", "level", "module", "message"].iter().map(|s| s.to_string()).collect();
    names.extend((0..FIELDS).map(|i| format!("field_{i}")));
    names
}

fn measure<T>(build: impl Fn(usize) -> T) -> f64 {
    let before = LIVE.load(Ordering::Relaxed);
    let kept: Vec<T> = (0..LINES).map(&build).collect();
    let after = LIVE.load(Ordering::Relaxed);
    drop(kept);
    (after - before) as f64 / LINES as f64
}

#[test]
#[ignore]
fn spans_of_ten_fields_on_a_million_lines() {
    let names = names();
    let map: HashMap<String, Range<usize>> =
        names.iter().enumerate().map(|(i, n)| (n.clone(), i * 10..i * 10 + 8)).collect();
    let fixed: HashMap<String, Range<usize>> =
        names.iter().take(4).enumerate().map(|(i, n)| (n.clone(), i * 10..i * 10 + 8)).collect();
    let _ = Spans::from_map(&map);

    let spans_fixed = measure(|_| Spans::from_map(&fixed));
    let spans_all = measure(|_| Spans::from_map(&map));
    let json_text = measure(|i| {
        (0..FIELDS)
            .map(|f| (format!("field_{f}"), format!("value-{i}")))
            .collect::<Vec<(String, String)>>()
    });
    let packed = |n: u16| {
        measure(move |_| -> std::sync::Arc<[(u16, u32, u32)]> { (0..n).map(|i| (i, u32::from(i) * 10, 8)).collect() })
    };
    let (packed_fixed, packed_all) = (packed(4), packed(names.len() as u16));

    println!("bytes per line, {LINES} lines:");
    println!("  Spans, 4 fixed fields:              {spans_fixed:.0}");
    println!("  Spans, 4 fixed + {FIELDS} fields:          {spans_all:.0}");
    println!("  json_fields text, {FIELDS} fields:          {json_text:.0}");
    println!("  packed (u16, u32, u32), 4 / 14:     {packed_fixed:.0} / {packed_all:.0}");
    println!(
        "  {FIELDS} more fields cost {:.0} MB as spans, {:.0} MB packed; dropping json_fields saves {:.0} MB",
        (spans_all - spans_fixed) * LINES as f64 / 1e6,
        (packed_all - packed_fixed) * LINES as f64 / 1e6,
        json_text * LINES as f64 / 1e6,
    );
}
