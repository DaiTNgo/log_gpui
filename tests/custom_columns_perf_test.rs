//! Timings of custom-columns task 4.1 on 1,000,000 lines. Run with
//! `cargo test --release --test custom_columns_perf_test -- --ignored --nocapture`.

use log_gpui::extract::{ExtractionRule, LineConfig, RuleScope, snapshot};
use log_gpui::filter_index::FilterIndex;
use log_gpui::log_format::compile_template;
use log_gpui::parser::text::{adopt, parse_text_bytes, reread, stored_lines};
use std::time::Instant;

const LINES: usize = 1_000_000;

fn text() -> String {
    let mut s = String::with_capacity(LINES * 90);
    for i in 0..LINES {
        let user = ["alice", "bob", "carol", "dave"][i % 4];
        s.push_str(&format!(
            "2026-10-20 10:{:02}:{:02}.{:03} [INFO] [api] request {i} user={user} took={}ms\n",
            (i / 60_000) % 60,
            (i / 1000) % 60,
            i % 1000,
            i % 900
        ));
    }
    s
}

fn ms(t: Instant) -> u128 {
    t.elapsed().as_millis()
}

#[test]
#[ignore]
fn rule_and_format_on_a_million_lines() {
    let text = text();
    let t = Instant::now();
    let mut entries = parse_text_bytes(text.as_bytes()).unwrap();
    println!("parse (Auto-detect):                 {:>6} ms", ms(t));
    let mut index = FilterIndex::build(&entries);

    // Adding an Extraction Rule.
    let rule = ExtractionRule {
        id: 1,
        name: "user".into(),
        pattern: r"(?:^|[^\w.\-])user=([\w.\-]+)".into(),
        scope: RuleScope::Line,
        description: String::new(),
        enabled: true,
    };
    let config = LineConfig::new(&[rule]);
    let t = Instant::now();
    let lines = snapshot(&entries);
    println!("rule: copy lines out (UI thread):    {:>6} ms", ms(t));
    let t = Instant::now();
    let read = config.read_snapshot(&lines);
    println!("rule: read spans (background):       {:>6} ms", ms(t));
    let t = Instant::now();
    for (e, x) in entries.iter_mut().zip(read) {
        e.extra = x;
    }
    index.rebuild_fields(&entries);
    println!("rule: adopt + Filter columns (UI):   {:>6} ms", ms(t));
    assert_eq!(index.coverage("user").0, LINES);

    // Changing the Log Format.
    let format = compile_template(
        "%{TIMESTAMP:timestamp} [%{LEVEL:level}] [%{WORD:module}] request %{INT:req} user=%{WORD:who} took=%{NOTSPACE:took}",
    )
    .unwrap();
    let t = Instant::now();
    let stored = stored_lines(&entries);
    println!("format: copy lines out (UI thread):  {:>6} ms", ms(t));
    let t = Instant::now();
    let fresh = reread(&stored, Some(&format));
    println!("format: read again (background):     {:>6} ms", ms(t));
    let t = Instant::now();
    for (e, f) in entries.iter_mut().zip(fresh) {
        adopt(e, f);
    }
    println!("format: adopt (UI thread):           {:>6} ms", ms(t));
    let t = Instant::now();
    let index = FilterIndex::build(&entries);
    println!("format: rebuild Filter index (UI):   {:>6} ms", ms(t));
    assert_eq!(index.coverage("took").0, LINES);
}
