//! A scan over rows that was told to stop gives up between chunks instead of
//! running to the end.
use log_gpui::par;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

#[test]
fn an_uncancelled_scan_gives_what_filter_gives() {
    let items: Vec<usize> = (0..100_000).collect();
    let cancel = AtomicBool::new(false);
    let got = par::filter_cancellable(&items, |i| i % 7 == 0, &cancel);
    assert_eq!(got, Some(par::filter(&items, |i| i % 7 == 0)));
}

#[test]
fn a_scan_cancelled_before_it_starts_looks_at_almost_nothing() {
    let items: Vec<usize> = (0..1_000_000).collect();
    let cancel = AtomicBool::new(true);
    let looked = AtomicUsize::new(0);
    let got = par::filter_cancellable(
        &items,
        |_| {
            looked.fetch_add(1, Ordering::Relaxed);
            true
        },
        &cancel,
    );
    assert_eq!(got, None);
    assert!(looked.load(Ordering::Relaxed) < 100_000, "looked at {}", looked.load(Ordering::Relaxed));
}

#[test]
fn a_scan_cancelled_midway_stops_early_in_small_and_large_logs() {
    for len in [5_000usize, 1_000_000] {
        let items: Vec<usize> = (0..len).collect();
        let cancel = AtomicBool::new(false);
        let looked = AtomicUsize::new(0);
        let got = par::filter_cancellable(
            &items,
            |_| {
                if looked.fetch_add(1, Ordering::Relaxed) == 100 {
                    cancel.store(true, Ordering::Relaxed);
                }
                true
            },
            &cancel,
        );
        assert_eq!(got, None, "len {len}");
        assert!(looked.load(Ordering::Relaxed) < len / 2 + 20_000, "len {len}: looked at {}", looked.load(Ordering::Relaxed));
    }
}
