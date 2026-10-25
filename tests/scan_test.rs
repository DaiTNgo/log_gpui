//! Scans (openspec change `line-offset-index`, task 6.1, spec
//! `progressive-scan`): reading lines again in the background, around the
//! line shown first, with a running total, cancelled by the next Scan.

use log_gpui::index::scan::{Found, Matcher, SCAN_LINES, Scan, block_order, scan_all};
use log_gpui::index::{BytesSource, LineIndex, LineSource, index_source};
use log_gpui::search_expr::{self, Matcher as SearchMatcher};
use std::sync::Arc;

fn text(lines: usize) -> String {
    (0..lines)
        .map(|i| {
            let what = if i % 97 == 0 { "timeout" } else if i % 31 == 0 { "time" } else { "ok" };
            format!("2026-09-05 10:00:{:02} [INFO] [api] request {i} {what} user={}\n", i % 60, i % 50)
        })
        .collect()
}

fn setup(lines: usize) -> (String, Arc<dyn LineSource>, Arc<LineIndex>) {
    let text = text(lines);
    let source = BytesSource::new(text.clone().into_bytes());
    let index = index_source(&source, None).unwrap();
    (text, Arc::new(source), Arc::new(index.lines))
}

fn search(query: &str) -> Arc<Matcher> {
    let matcher = SearchMatcher::compile(&search_expr::parse(query).unwrap().unwrap());
    Arc::new(move |first: usize, lines: &[&str]| {
        lines.iter().enumerate().filter(|(_, l)| matcher.is_match(l)).map(|(i, _)| (first + i) as u32).collect()
    })
}

fn expected(text: &str, query: &str) -> Vec<u32> {
    let matcher = SearchMatcher::compile(&search_expr::parse(query).unwrap().unwrap());
    text.lines().enumerate().filter(|(_, l)| matcher.is_match(l)).map(|(i, _)| i as u32).collect()
}

#[test]
fn a_scan_finds_what_reading_every_line_finds() {
    let (text, source, index) = setup(35_000);
    for query in ["timeout", "user=42", "time && !timeout"] {
        let found = scan_all(&*source, &*index, 0..index.len(), &*search(query));
        assert!(found.done, "{query}");
        assert_eq!(found.lines, expected(&text, query), "{query}");
        assert_eq!((found.scanned, found.total), (35_000, 35_000));
    }
}

#[test]
fn blocks_around_the_line_shown_come_first() {
    let order = block_order(0..55_000, 31_000);
    assert_eq!(order[0], 30_000..40_000, "the block of the line shown");
    assert_eq!(order[1], 40_000..50_000);
    assert_eq!(order[2], 20_000..30_000);
    assert_eq!(order.last(), Some(&(0..SCAN_LINES)));
    let mut all: Vec<_> = order.clone();
    all.sort_by_key(|r| r.start);
    assert_eq!(all.len(), 6, "every block once");
    assert_eq!(all[5], 50_000..55_000);
}

#[test]
fn the_running_total_grows_until_the_end() {
    let (text, source, index) = setup(200_000);
    let scan = Scan::start(source, index.clone(), 0..index.len(), 150_000, search("timeout"));
    let updates = scan.updates.clone();
    let end = scan.wait();
    let all: Vec<Found> = std::iter::from_fn(|| updates.try_recv().ok()).collect();
    assert!(all.windows(2).all(|w| w[0].scanned <= w[1].scanned), "progress only grows");
    assert!(all.iter().all(|f| f.lines.windows(2).all(|w| w[0] < w[1])), "matches stay sorted");
    assert!(end.done);
    assert_eq!(end.lines, expected(&text, "timeout"));
    assert_eq!(all.last().unwrap(), &end);
}

#[test]
fn the_match_on_screen_comes_before_the_rest() {
    let (_, source, index) = setup(300_000);
    // A matcher that only sees one block at a time: the first block it gets
    // holds the line shown.
    let first = Arc::new(std::sync::Mutex::new(None::<usize>));
    let seen = first.clone();
    let matcher: Arc<Matcher> = Arc::new(move |start: usize, _: &[&str]| {
        seen.lock().unwrap().get_or_insert(start);
        Vec::new()
    });
    Scan::start(source, index.clone(), 0..index.len(), 200_010, matcher).wait();
    let first = first.lock().unwrap().unwrap();
    assert!((190_000..=210_000).contains(&first), "started at {first}");
}

#[test]
fn a_cancelled_scan_stops_and_is_not_done() {
    let (_, source, index) = setup(300_000);
    let slow: Arc<Matcher> = Arc::new(|_: usize, _: &[&str]| {
        std::thread::sleep(std::time::Duration::from_millis(20));
        Vec::new()
    });
    let scan = Scan::start(source, index.clone(), 0..index.len(), 0, slow);
    scan.cancel();
    let found = scan.wait();
    assert!(!found.done);
    assert!(found.scanned < found.total);
}

#[test]
fn new_lines_are_scanned_alone() {
    let (text, source, index) = setup(30_000);
    let older = scan_all(&*source, &*index, 0..20_000, &*search("timeout"));
    let newer = scan_all(&*source, &*index, 20_000..30_000, &*search("timeout"));
    let mut all = older;
    all.append(newer);
    assert_eq!(all.lines, expected(&text, "timeout"));
    assert_eq!((all.scanned, all.total, all.done), (30_000, 30_000, true));
}

#[test]
fn a_file_that_shrank_ends_the_scan_with_an_error() {
    let (text, _, index) = setup(30_000);
    let short: Arc<dyn LineSource> = Arc::new(BytesSource::new(text.as_bytes()[..text.len() / 2].to_vec()));
    let found = Scan::start(short, index.clone(), 0..index.len(), 0, search("timeout")).wait();
    assert!(!found.done);
    assert!(found.error.is_some());
}
