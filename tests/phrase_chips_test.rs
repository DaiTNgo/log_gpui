use log_gpui::palette::Tone;
use log_gpui::search_expr::{Matcher, parse};
use log_gpui::ui::canvas::{find_phrase_fills, search_phrase_fills};
use log_gpui::ui::phrase_chips::{find_chips, search_chips};

fn matcher(q: &str) -> Matcher {
    Matcher::compile(&parse(q).unwrap().unwrap())
}

#[test]
fn search_chips_follow_phrase_order_and_match_fills() {
    let m = matcher("Timeout && panic || timeout || Disk");
    let chips = search_chips(Some(&m), Tone::Light, true);
    let keys = m.phrase_keys();
    let fills = search_phrase_fills(&keys, Tone::Light);
    assert_eq!(chips.len(), 3);
    for (i, chip) in chips.iter().enumerate() {
        assert_eq!(chip.label, keys[i]);
        assert_eq!(chip.fill, fills[i].normal);
    }
    assert_eq!(chips[0].label, "timeout");
}

#[test]
fn no_chips_without_expression_or_when_colors_off() {
    let m = matcher("a || b");
    assert!(search_chips(None, Tone::Dark, true).is_empty());
    assert!(search_chips(Some(&m), Tone::Dark, false).is_empty());
    assert!(find_chips(&["a"], &[], Tone::Dark, false).is_empty());
}

#[test]
fn find_chips_share_search_hue_for_shared_phrases() {
    let search = matcher("timeout || panic");
    let sk = search.phrase_keys();
    let find = matcher("panic || other");
    let fk = find.phrase_keys();
    let chips = find_chips(&fk, &sk, Tone::Dark, true);
    let fills = find_phrase_fills(&fk, &sk, Tone::Dark);
    let search_fills = search_phrase_fills(&sk, Tone::Dark);
    assert_eq!(chips.len(), 2);
    assert_eq!(chips[0].label, "panic");
    assert_eq!(chips[0].fill, fills[0].normal);
    assert_eq!(chips[0].fill, search_fills[1].normal);
    assert_eq!(chips[1].fill, fills[1].normal);
}
