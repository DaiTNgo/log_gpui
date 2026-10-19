//! Time of the work done on a large log: `perf_probe [lines]`.
//!
//! Generates a log of `lines` lines (1,000,000 by default), parses it, and
//! times each step the app runs when the user opens the file, types in
//! Search, ticks a Filter value or adds a Highlight Rule, plus the work done
//! on every frame.
use log_gpui::filter::FilterState;
use log_gpui::filter_index::FilterIndex;
use log_gpui::highlight::{HighlightRule, paint_rows};

use log_gpui::search::SearchState;
use log_gpui::space_view::ParsedFile;
use std::time::Instant;

fn time<T>(label: &str, f: impl FnOnce() -> T) -> T {
    let t = Instant::now();
    let out = f();
    println!("{label:<40} {:>10.2?}", t.elapsed());
    out
}

fn main() {
    let n: usize = std::env::args().nth(1).and_then(|a| a.parse().ok()).unwrap_or(1_000_000);
    let levels = ["INFO", "DEBUG", "WARN", "ERROR"];
    let tags = ["auth", "api", "db", "cache", "worker"];
    let mut text = String::with_capacity(n * 100);
    for i in 0..n {
        let s = i % 86_400;
        text.push_str(&format!(
            "2026-09-05 {:02}:{:02}:{:02}.{:03} [{}] [{}] request {} handled user={} took {}ms\n",
            s / 3600, (s / 60) % 60, s % 60, i % 1000,
            levels[i % 4], tags[i % 5], i, i % 977, i % 300
        ));
    }
    println!("{n} lines, {:.1} MB", text.len() as f64 / 1e6);

    let parsed = time("parse", || ParsedFile::from_bytes(text.as_bytes()));
    let mut entries = parsed.entries.clone();
    let index = time("FilterIndex::build", || FilterIndex::build(&entries));

    let mut filter = FilterState::new();
    let pass = time("Filter evaluate (none)", || filter.evaluate(&index, &mut entries));
    filter.toggle_value("level", "error");
    let pass_err = time("Filter evaluate (level=error)", || filter.evaluate(&index, &mut entries));
    println!("  pass {} / {}", pass_err.len(), pass.len());

    let rule = HighlightRule::new(1, "level", &["warn".into()], &[], log_gpui::palette::next_rule_color(0)).ok();
    if let Some(rule) = rule {
        time("paint_rows (1 rule)", || paint_rows(&[rule], &entries));
    }

    let mut search = SearchState::new();
    for q in ["t", "ti", "timeout", "user=42"] {
        search.query = q.into();
        time(&format!("Search update_matches '{q}'"), || search.update_matches(&entries, &pass));
        println!("  matches {}", search.total_matches());
    }

    // Per frame: the canvas looks for the longest message among every row.
    let visible: Vec<usize> = (0..entries.len()).collect();
    let _ = &visible;
    // Worst case of finding a line: not the selected row, so a full scan.
    let target = entries.last().unwrap();
    time("line lookup, full scan (last)", || {
        entries.iter().position(|e| {
            e.index == target.index && e.generation == target.generation
                && e.file_id.as_deref().unwrap_or("") == target.file_id.as_deref().unwrap_or("")
        })
    });
    time("visible_indices rebuild", || (0..entries.len()).collect::<Vec<usize>>());
}
