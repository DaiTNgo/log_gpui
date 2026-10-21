use log_gpui::model::Spans;

fn five() -> Spans {
    Spans::from_pairs(vec![
        ("timestamp", 0..23),
        ("module", 25..29),
        ("level", 32..36),
        ("message", 39..80),
        ("user", 70..72),
    ])
}

/// What a line's field spans keep (names and byte ranges, in order) is
/// unchanged by the compact form.
#[test]
fn spans_give_back_the_names_and_ranges_they_were_made_from() {
    let spans = five();
    assert_eq!(spans.len(), 5);
    assert_eq!(spans.get("module"), Some(25..29));
    assert_eq!(spans.get("message"), Some(39..80));
    assert_eq!(spans.get("nope"), None);
    let all: Vec<_> = spans.iter().collect();
    assert_eq!(all[0], ("timestamp", 0..23));
    assert_eq!(all[4], ("user", 70..72));
    let fields: Vec<_> = spans.fields().map(|(k, r)| (k.as_str(), r)).collect();
    assert_eq!(fields, vec![("user", 70..72)]);
    assert_eq!(spans.clone(), spans);
}

/// A line's five spans cost well under the 176 bytes of a list of
/// (name, usize range) pairs.
#[test]
fn five_spans_take_under_a_hundred_bytes_of_heap() {
    assert!(five().heap_bytes() <= 80, "{}", five().heap_bytes());
}
