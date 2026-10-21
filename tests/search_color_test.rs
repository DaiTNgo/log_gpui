use log_gpui::search_expr::{Matcher, parse};

fn matcher(q: &str) -> Matcher {
    Matcher::compile(&parse(q).unwrap().unwrap())
}

#[test]
fn each_top_level_or_branch_is_one_color_group() {
    assert_eq!(matcher("a && b || c").group_keys(), ["a", "c"]);
    assert_eq!(matcher("timeout && refused || panic").group_keys(), ["timeout", "panic"]);
    assert_eq!(matcher("a || b || c").group_keys().len(), 3);
}

#[test]
fn and_and_parentheses_never_split_a_group() {
    assert_eq!(matcher("(a || b) && c").group_keys(), ["a"]);
    assert_eq!(matcher("a && (b || c)").group_keys(), ["a"]);
    assert_eq!(matcher("a").group_keys(), ["a"]);
}

#[test]
fn group_key_is_the_first_phrase_case_folded() {
    assert_eq!(matcher("Connection REFUSED && x || y").group_keys(), ["connection refused", "y"]);
}

// ---- ranges per group ----

use log_gpui::search_expr::GroupRange;

fn gr(start: usize, end: usize, group: usize) -> GroupRange {
    GroupRange { start, end, group }
}

#[test]
fn ranges_carry_their_group() {
    let m = matcher("a && b || c");
    assert_eq!(m.find_group_ranges("a b c"), [gr(0, 1, 0), gr(2, 3, 0), gr(4, 5, 1)]);
}

#[test]
fn longer_phrase_wins_where_groups_overlap() {
    let m = matcher("err || error");
    assert_eq!(m.find_group_ranges("an error"), [gr(3, 8, 1)]);
    // Partial overlap: "abc" (3 bytes) beats "cd" (2 bytes) on the shared "c".
    let m = matcher("abc || cd");
    assert_eq!(m.find_group_ranges("abcd"), [gr(0, 3, 0), gr(3, 4, 1)]);
}

#[test]
fn equal_length_overlap_goes_to_the_earlier_group() {
    let m = matcher("ab || bc");
    assert_eq!(m.find_group_ranges("abc"), [gr(0, 2, 0), gr(2, 3, 1)]);
}

#[test]
fn overlapping_phrases_of_one_group_merge() {
    let m = matcher("ab && bc");
    assert_eq!(m.find_group_ranges("abc"), [gr(0, 3, 0)]);
}

#[test]
fn group_ranges_cover_exactly_the_plain_ranges() {
    let m = matcher("err || error || x && y");
    let line = "an error and x and y errors";
    let mut covered: Vec<(usize, usize)> = Vec::new();
    for r in m.find_group_ranges(line) {
        match covered.last_mut() {
            Some(last) if r.start <= last.1 => last.1 = last.1.max(r.end),
            _ => covered.push((r.start, r.end)),
        }
    }
    assert_eq!(covered, m.find_ranges(line));
}

// ---- canvas paint ----

use log_gpui::ui::canvas::{search_group_fills, search_paints};

#[test]
fn canvas_paints_each_range_in_its_groups_color_and_the_current_match_stronger() {
    let m = matcher("a || b");
    let fills = search_group_fills(&m.group_keys(), Tone::Light);
    assert_eq!(fills.len(), 2);
    assert_ne!(fills[0].normal.h, fills[1].normal.h);
    assert!(fills[0].current.a > fills[0].normal.a);
    assert_eq!(fills[0].current.h, fills[0].normal.h);

    let ranges = m.find_group_ranges("a b");
    let plain = search_paints(&ranges, &fills, false);
    let current = search_paints(&ranges, &fills, true);
    assert_eq!((plain[0].start, plain[0].end, plain[1].start, plain[1].end), (0, 1, 2, 3));
    assert_eq!(plain[0].fill.h, fills[0].normal.h);
    assert_eq!(plain[1].fill.h, fills[1].normal.h);
    assert!(current[0].fill.a > plain[0].fill.a);
}

#[test]
fn the_same_keyword_paints_the_same_color_in_every_matcher() {
    let one = search_group_fills(&matcher("timeout").group_keys(), Tone::Dark);
    let other = search_group_fills(&matcher("Timeout && x").group_keys(), Tone::Dark);
    assert_eq!(one[0].normal, other[0].normal);
}

// ---- hue ----

const GOLDEN_TIMEOUT_HUE: f32 = 120.8;

use log_gpui::palette::Tone;
use log_gpui::search_color::{self, hue_distance};

#[test]
fn hue_of_a_keyword_is_stable_and_case_blind() {
    let h = search_color::hue_for_key("timeout");
    assert_eq!(h, search_color::hue_for_key("timeout"));
    assert!((0.0..360.0).contains(&h));
    assert_eq!(search_color::group_hues(&["timeout"]), [h]);
    assert_eq!(
        search_color::group_hues(&matcher("TimeOut").group_keys()),
        search_color::group_hues(&matcher("timeout").group_keys())
    );
    assert_ne!(search_color::hue_for_key("timeout"), search_color::hue_for_key("panic"));
}

#[test]
fn hue_of_a_keyword_never_changes_between_releases() {
    // Golden value: if this moves, every user's colors move with it.
    assert_eq!(search_color::hue_for_key("timeout"), GOLDEN_TIMEOUT_HUE);
}

#[test]
fn close_hues_are_pushed_apart_in_order_of_appearance() {
    let hues = search_color::group_hues(&["a", "a"]);
    assert_eq!(hues[0], search_color::hue_for_key("a"));
    assert!(hue_distance(hues[0], hues[1]) >= 30.0);
}

#[test]
fn up_to_eight_groups_are_all_at_least_30_degrees_apart() {
    let keys: Vec<String> = (0..8).map(|i| format!("kw{i}")).collect();
    let keys: Vec<&str> = keys.iter().map(String::as_str).collect();
    let hues = search_color::group_hues(&keys);
    for i in 0..hues.len() {
        for j in i + 1..hues.len() {
            assert!(hue_distance(hues[i], hues[j]) >= 30.0, "{i} {j} {hues:?}");
        }
    }
}

#[test]
fn past_twelve_groups_closeness_is_accepted_but_hues_stay_valid_and_deterministic() {
    let keys: Vec<String> = (0..40).map(|i| format!("kw{i}")).collect();
    let keys: Vec<&str> = keys.iter().map(String::as_str).collect();
    let a = search_color::group_hues(&keys);
    assert_eq!(a.len(), 40);
    assert!(a.iter().all(|h| (0.0..360.0).contains(h)));
    assert_eq!(a, search_color::group_hues(&keys));
}

#[test]
fn hue_distance_wraps_around() {
    assert_eq!(hue_distance(350.0, 10.0), 20.0);
    assert_eq!(hue_distance(10.0, 350.0), 20.0);
    assert_eq!(hue_distance(0.0, 180.0), 180.0);
}

// ---- readability ----

fn luminance(rgb: u32) -> f32 {
    let ch = |shift: u32| {
        let c = ((rgb >> shift) & 0xff) as f32 / 255.0;
        if c <= 0.03928 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * ch(16) + 0.7152 * ch(8) + 0.0722 * ch(0)
}

fn contrast(a: u32, b: u32) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

fn blend(fg: u32, bg: u32, alpha: f32) -> u32 {
    let mix = |shift: u32| {
        let f = ((fg >> shift) & 0xff) as f32;
        let b = ((bg >> shift) & 0xff) as f32;
        ((f * alpha + b * (1.0 - alpha)).round() as u32) << shift
    };
    mix(16) | mix(8) | mix(0)
}

#[test]
fn colors_keep_text_readable_in_light_and_dark_for_every_hue() {
    for (tone, page, text) in [(Tone::Light, 0xffffff_u32, 0x1f2328_u32), (Tone::Dark, 0x1e1e1e, 0xd4d4d4)] {
        for step in 0..36 {
            let hue = step as f32 * 10.0;
            let rgb = search_color::group_rgb(hue, tone);
            for opacity in [search_color::MATCH_OPACITY, search_color::CURRENT_MATCH_OPACITY] {
                let bg = blend(rgb, page, opacity);
                assert!(contrast(bg, text) >= 4.5, "{tone:?} hue {hue} opacity {opacity}");
            }
        }
    }
}

#[test]
fn current_match_is_stronger_than_other_matches() {
    assert!(search_color::CURRENT_MATCH_OPACITY > search_color::MATCH_OPACITY);
}

#[test]
fn with_keyword_colors_off_every_match_paints_the_one_shared_color() {
    use log_gpui::ui::canvas::shared_search_paints;
    let m = matcher("a || b");
    let ranges = m.find_group_ranges("a b");
    let shared = gpui::hsla(0.1, 0.5, 0.5, 1.0);
    let paints = shared_search_paints(&ranges, shared);
    assert_eq!(paints.len(), 2);
    assert_eq!((paints[0].start, paints[0].end, paints[1].start, paints[1].end), (0, 1, 2, 3));
    assert_eq!(paints[0].fill, paints[1].fill);
    assert_eq!(paints[0].fill, shared.opacity(0.55));
}

// ---- minimap ticks ----

use log_gpui::ui::canvas::minimap_tick;

fn shared_tick_color() -> gpui::Hsla {
    gpui::hsla(0.1, 0.9, 0.5, 1.0)
}

fn three_fills() -> Vec<log_gpui::ui::canvas::GroupFill> {
    search_group_fills(&matcher("a || b || c").group_keys(), Tone::Light)
}

#[test]
fn a_single_group_line_ticks_in_that_groups_color() {
    let fills = three_fills();
    let tick = minimap_tick(&[1], &fills, 40.0, false, true, shared_tick_color());
    assert_eq!(tick.stripes.len(), 1);
    assert_eq!((tick.stripes[0].offset, tick.stripes[0].width), (0.0, 40.0));
    assert_eq!(tick.stripes[0].color.h, fills[1].normal.h);
}

#[test]
fn a_multi_group_line_shows_one_stripe_per_color_in_expression_order() {
    let fills = three_fills();
    let tick = minimap_tick(&[0, 2], &fills, 40.0, false, true, shared_tick_color());
    assert_eq!(tick.stripes.len(), 2);
    assert_eq!((tick.stripes[0].offset, tick.stripes[0].width), (0.0, 20.0));
    assert_eq!((tick.stripes[1].offset, tick.stripes[1].width), (20.0, 20.0));
    assert_eq!(tick.stripes[0].color.h, fills[0].normal.h);
    assert_eq!(tick.stripes[1].color.h, fills[2].normal.h);
}

#[test]
fn a_tick_too_narrow_for_stripes_takes_the_first_groups_color() {
    let fills = three_fills();
    let tick = minimap_tick(&[0, 1, 2], &fills, 4.0, false, true, shared_tick_color());
    assert_eq!(tick.stripes.len(), 1);
    assert_eq!(tick.stripes[0].width, 4.0);
    assert_eq!(tick.stripes[0].color.h, fills[0].normal.h);
}

#[test]
fn the_current_match_tick_is_stronger_and_so_is_its_row_tint() {
    let fills = three_fills();
    let plain = minimap_tick(&[0], &fills, 40.0, false, true, shared_tick_color());
    let current = minimap_tick(&[0], &fills, 40.0, true, true, shared_tick_color());
    assert_eq!(current.stripes[0].color.h, plain.stripes[0].color.h);
    assert!(current.stripes[0].color.a > plain.stripes[0].color.a);
    assert!(current.tint.a > plain.tint.a);
}

#[test]
fn with_keyword_colors_off_the_tick_is_the_single_shared_color() {
    let fills = three_fills();
    let shared = shared_tick_color();
    let tick = minimap_tick(&[0, 1], &fills, 40.0, false, false, shared);
    assert_eq!(tick.stripes.len(), 1);
    assert_eq!((tick.stripes[0].offset, tick.stripes[0].width), (0.0, 40.0));
    assert_eq!(tick.stripes[0].color, shared);
    assert_eq!(tick.tint, shared.opacity(0.3));
    assert_eq!(minimap_tick(&[0], &fills, 40.0, true, false, shared).tint, shared.opacity(0.6));
}

#[test]
fn a_line_with_no_known_group_ticks_in_the_shared_color() {
    let tick = minimap_tick(&[], &[], 40.0, false, true, shared_tick_color());
    assert_eq!(tick.stripes.len(), 1);
    assert_eq!(tick.stripes[0].color, shared_tick_color());
}
