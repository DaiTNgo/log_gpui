use log_gpui::search_expr::{Matcher, parse};

fn matcher(q: &str) -> Matcher {
    Matcher::compile(&parse(q).unwrap().unwrap())
}

#[test]
fn every_distinct_phrase_is_its_own_color_whatever_the_operators() {
    assert_eq!(matcher("a && b || c").phrase_keys(), ["a", "b", "c"]);
    assert_eq!(matcher("timeout && refused || panic").phrase_keys(), ["timeout", "refused", "panic"]);
    assert_eq!(matcher("(a || b) && c").phrase_keys(), ["a", "b", "c"]);
    assert_eq!(matcher("error && \"error code\"").phrase_keys(), ["error", "error code"]);
    assert_eq!(matcher("a").phrase_keys(), ["a"]);
}

#[test]
fn identical_phrases_in_one_expression_share_one_color() {
    assert_eq!(matcher("error || (error && x)").phrase_keys(), ["error", "x"]);
    let m = matcher("error || (error && x)");
    // Both `error` occurrences are the same phrase index.
    assert_eq!(m.find_phrase_ranges("error x"), [gr(0, 5, 0), gr(6, 7, 1)]);
}

#[test]
fn phrase_key_is_case_folded() {
    assert_eq!(matcher("Connection REFUSED && Y").phrase_keys(), ["connection refused", "y"]);
    assert_eq!(matcher("ERROR || error").phrase_keys(), ["error"]);
}

// ---- ranges per phrase ----

use log_gpui::search_expr::PhraseRange;

fn gr(start: usize, end: usize, phrase: usize) -> PhraseRange {
    PhraseRange { start, end, phrase }
}

#[test]
fn ranges_carry_their_phrase() {
    let m = matcher("a && b || c");
    assert_eq!(m.find_phrase_ranges("a b c"), [gr(0, 1, 0), gr(2, 3, 1), gr(4, 5, 2)]);
}

#[test]
fn longer_phrase_wins_where_phrases_overlap() {
    let m = matcher("err || error");
    assert_eq!(m.find_phrase_ranges("an error"), [gr(3, 8, 1)]);
    // `error` inside `"error code"`, with `&&` between them.
    let m = matcher("error && \"error code\"");
    assert_eq!(m.find_phrase_ranges("error: error code"), [gr(0, 5, 0), gr(7, 17, 1)]);
    // Partial overlap: "abc" (3 bytes) beats "cd" (2 bytes) on the shared "c".
    let m = matcher("abc || cd");
    assert_eq!(m.find_phrase_ranges("abcd"), [gr(0, 3, 0), gr(3, 4, 1)]);
}

#[test]
fn equal_length_overlap_goes_to_the_earlier_phrase() {
    let m = matcher("ab || bc");
    assert_eq!(m.find_phrase_ranges("abc"), [gr(0, 2, 0), gr(2, 3, 1)]);
}

#[test]
fn overlapping_phrases_with_and_stay_separate_colors() {
    let m = matcher("ab && bc");
    assert_eq!(m.find_phrase_ranges("abc"), [gr(0, 2, 0), gr(2, 3, 1)]);
}

#[test]
fn phrases_in_lists_every_phrase_on_the_line_even_when_covered() {
    let m = matcher("error && \"error code\" || zzz");
    // `error` is fully covered by `"error code"` in the ranges, but occurs.
    assert_eq!(m.find_phrase_ranges("error code"), [gr(0, 10, 1)]);
    assert_eq!(m.phrases_in("error code"), [0, 1]);
    assert_eq!(m.phrases_in("zzz error"), [0, 2]);
}

#[test]
fn phrase_ranges_cover_exactly_the_plain_ranges() {
    let m = matcher("err || error || x && y");
    let line = "an error and x and y errors";
    let mut covered: Vec<(usize, usize)> = Vec::new();
    for r in m.find_phrase_ranges(line) {
        match covered.last_mut() {
            Some(last) if r.start <= last.1 => last.1 = last.1.max(r.end),
            _ => covered.push((r.start, r.end)),
        }
    }
    assert_eq!(covered, m.find_ranges(line));
}

// ---- canvas paint ----

use log_gpui::ui::canvas::{search_phrase_fills, search_paints};

#[test]
fn canvas_paints_each_range_in_its_phrases_color_and_the_current_match_stronger() {
    let m = matcher("a || b");
    let fills = search_phrase_fills(&m.phrase_keys(), Tone::Light);
    assert_eq!(fills.len(), 2);
    assert_ne!(fills[0].normal.h, fills[1].normal.h);
    assert!(fills[0].current.a > fills[0].normal.a);
    assert_eq!(fills[0].current.h, fills[0].normal.h);

    let ranges = m.find_phrase_ranges("a b");
    let plain = search_paints(&ranges, &fills, false);
    let current = search_paints(&ranges, &fills, true);
    assert_eq!((plain[0].start, plain[0].end, plain[1].start, plain[1].end), (0, 1, 2, 3));
    assert_eq!(plain[0].fill.h, fills[0].normal.h);
    assert_eq!(plain[1].fill.h, fills[1].normal.h);
    assert!(current[0].fill.a > plain[0].fill.a);
}

#[test]
fn the_same_phrase_paints_the_same_color_in_every_matcher() {
    let one = search_phrase_fills(&matcher("timeout").phrase_keys(), Tone::Dark);
    let other = search_phrase_fills(&matcher("Timeout && x").phrase_keys(), Tone::Dark);
    assert_eq!(one[0].normal, other[0].normal);
}

#[test]
fn a_phrase_keeps_its_color_whatever_operators_surround_it() {
    let one = search_phrase_fills(&matcher("timeout").phrase_keys(), Tone::Dark);
    let other = search_phrase_fills(&matcher("TIMEOUT || x").phrase_keys(), Tone::Dark);
    assert_eq!(one[0], other[0]);
}

#[test]
fn identical_phrases_get_one_fill_and_distinct_ones_differ() {
    let m = matcher("error || (error && x)");
    let fills = search_phrase_fills(&m.phrase_keys(), Tone::Dark);
    assert_eq!(fills.len(), 2);
    assert_ne!(fills[0].normal.h, fills[1].normal.h);
}

#[test]
fn hues_are_separated_over_distinct_phrases_only() {
    // 8 distinct phrases among 12 words, all at least 30 degrees apart.
    let m = matcher("p0 && p1 || p2 && (p3 || p4) || p0 && p5 || p6 && p7 && p1 || p2");
    let keys = m.phrase_keys();
    assert_eq!(keys.len(), 8);
    let hues = search_color::phrase_hues(&keys);
    for i in 0..hues.len() {
        for j in i + 1..hues.len() {
            assert!(hue_distance(hues[i], hues[j]) >= 30.0, "{i} {j} {hues:?}");
        }
    }
    assert_eq!(hues, search_color::phrase_hues(&keys));
}

// ---- hue ----

const GOLDEN_TIMEOUT_HUE: f32 = 120.8;

use log_gpui::palette::Tone;
use log_gpui::search_color::{self, hue_distance};

#[test]
fn hue_of_a_phrase_is_stable_and_case_blind() {
    let h = search_color::hue_for_key("timeout");
    assert_eq!(h, search_color::hue_for_key("timeout"));
    assert!((0.0..360.0).contains(&h));
    assert_eq!(search_color::phrase_hues(&["timeout"]), [h]);
    assert_eq!(
        search_color::phrase_hues(&matcher("TimeOut").phrase_keys()),
        search_color::phrase_hues(&matcher("timeout").phrase_keys())
    );
    assert_ne!(search_color::hue_for_key("timeout"), search_color::hue_for_key("panic"));
}

#[test]
fn hue_of_a_phrase_never_changes_between_releases() {
    // Golden value: if this moves, every user's colors move with it.
    assert_eq!(search_color::hue_for_key("timeout"), GOLDEN_TIMEOUT_HUE);
}

#[test]
fn close_hues_are_pushed_apart_in_order_of_appearance() {
    let hues = search_color::phrase_hues(&["a", "a"]);
    assert_eq!(hues[0], search_color::hue_for_key("a"));
    assert!(hue_distance(hues[0], hues[1]) >= 30.0);
}

#[test]
fn up_to_eight_phrases_are_all_at_least_30_degrees_apart() {
    let keys: Vec<String> = (0..8).map(|i| format!("kw{i}")).collect();
    let keys: Vec<&str> = keys.iter().map(String::as_str).collect();
    let hues = search_color::phrase_hues(&keys);
    for i in 0..hues.len() {
        for j in i + 1..hues.len() {
            assert!(hue_distance(hues[i], hues[j]) >= 30.0, "{i} {j} {hues:?}");
        }
    }
}

#[test]
fn past_twelve_phrases_closeness_is_accepted_but_hues_stay_valid_and_deterministic() {
    let keys: Vec<String> = (0..40).map(|i| format!("kw{i}")).collect();
    let keys: Vec<&str> = keys.iter().map(String::as_str).collect();
    let a = search_color::phrase_hues(&keys);
    assert_eq!(a.len(), 40);
    assert!(a.iter().all(|h| (0.0..360.0).contains(h)));
    assert_eq!(a, search_color::phrase_hues(&keys));
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
            let rgb = search_color::phrase_rgb(hue, tone);
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
fn with_phrase_colors_off_every_match_paints_the_one_shared_color() {
    use log_gpui::ui::canvas::shared_search_paints;
    let m = matcher("a || b");
    let ranges = m.find_phrase_ranges("a b");
    let shared = gpui::hsla(0.1, 0.5, 0.5, 1.0);
    let paints = shared_search_paints(&ranges, shared);
    assert_eq!(paints.len(), 2);
    assert_eq!((paints[0].start, paints[0].end, paints[1].start, paints[1].end), (0, 1, 2, 3));
    assert_eq!(paints[0].fill, paints[1].fill);
    assert_eq!(paints[0].fill, shared.opacity(search_color::SHARED_SEARCH_OPACITY));
}

// ---- minimap ticks ----

use log_gpui::ui::canvas::minimap_tick;

fn shared_tick_color() -> gpui::Hsla {
    gpui::hsla(0.1, 0.9, 0.5, 1.0)
}

fn three_fills() -> Vec<log_gpui::ui::canvas::PhraseFill> {
    search_phrase_fills(&matcher("a || b || c").phrase_keys(), Tone::Light)
}

#[test]
fn a_single_phrase_line_ticks_in_that_phrases_color() {
    let fills = three_fills();
    let tick = minimap_tick(&[1], &fills, 40.0, false, true, shared_tick_color());
    assert_eq!(tick.stripes.len(), 1);
    assert_eq!((tick.stripes[0].offset, tick.stripes[0].width), (0.0, 40.0));
    assert_eq!(tick.stripes[0].color.h, fills[1].normal.h);
}

#[test]
fn a_multi_phrase_line_shows_one_stripe_per_color_in_expression_order() {
    let fills = three_fills();
    let tick = minimap_tick(&[0, 2], &fills, 40.0, false, true, shared_tick_color());
    assert_eq!(tick.stripes.len(), 2);
    assert_eq!((tick.stripes[0].offset, tick.stripes[0].width), (0.0, 20.0));
    assert_eq!((tick.stripes[1].offset, tick.stripes[1].width), (20.0, 20.0));
    assert_eq!(tick.stripes[0].color.h, fills[0].normal.h);
    assert_eq!(tick.stripes[1].color.h, fills[2].normal.h);
}

#[test]
fn a_tick_too_narrow_for_stripes_takes_the_first_phrases_color() {
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
fn with_phrase_colors_off_the_tick_is_the_single_shared_color() {
    let fills = three_fills();
    let shared = shared_tick_color();
    let tick = minimap_tick(&[0, 1], &fills, 40.0, false, false, shared);
    assert_eq!(tick.stripes.len(), 1);
    assert_eq!((tick.stripes[0].offset, tick.stripes[0].width), (0.0, 40.0));
    assert_eq!(tick.stripes[0].color, shared);
    assert_eq!(tick.tint, shared.opacity(search_color::MINIMAP_TINT_OPACITY));
    assert_eq!(minimap_tick(&[0], &fills, 40.0, true, false, shared).tint, shared.opacity(search_color::MINIMAP_CURRENT_TINT_OPACITY));
}

#[test]
fn a_line_with_no_known_phrase_ticks_in_the_shared_color() {
    let tick = minimap_tick(&[], &[], 40.0, false, true, shared_tick_color());
    assert_eq!(tick.stripes.len(), 1);
    assert_eq!(tick.stripes[0].color, shared_tick_color());
}

// ---- Raw view: the same phrase colors as the Canvas ----

fn shared_fill() -> log_gpui::ui::canvas::PhraseFill {
    log_gpui::ui::canvas::PhraseFill {
        normal: gpui::hsla(0.55, 0.85, 0.55, 0.35),
        current: gpui::hsla(0.55, 0.9, 0.55, 0.7),
    }
}

fn raw_paints(q: &str, line: &str, current: bool, phrase_colors: bool) -> Vec<(usize, usize, gpui::Hsla)> {
    let m = matcher(q);
    let fills = log_gpui::ui::canvas::search_phrase_fills(&m.phrase_keys(), Tone::Dark);
    let fills = phrase_colors.then_some(fills.as_slice());
    log_gpui::ui::canvas::line_match_paints(&m, line, current, fills, shared_fill())
        .iter()
        .map(|p| (p.start, p.end, p.fill))
        .collect()
}

#[test]
fn raw_view_paints_each_phrase_in_its_canvas_color() {
    let fills = log_gpui::ui::canvas::search_phrase_fills(&matcher("a || b").phrase_keys(), Tone::Dark);
    let p = raw_paints("a || b", "a b a", false, true);
    assert_eq!(p, [(0, 1, fills[0].normal), (2, 3, fills[1].normal), (4, 5, fills[0].normal)]);
    assert_ne!(fills[0].normal, fills[1].normal);
}

#[test]
fn raw_view_current_match_uses_the_stronger_phrase_color() {
    let fills = log_gpui::ui::canvas::search_phrase_fills(&matcher("a || b").phrase_keys(), Tone::Dark);
    assert_eq!(raw_paints("a || b", "b", true, true), [(0, 1, fills[1].current)]);
}

#[test]
fn raw_view_longer_phrase_wins_an_overlap() {
    let q = "connection || connection refused";
    let fills = log_gpui::ui::canvas::search_phrase_fills(&matcher(q).phrase_keys(), Tone::Dark);
    assert_eq!(raw_paints(q, "connection refused", false, true), [(0, 18, fills[1].normal)]);
}

#[test]
fn raw_view_toggle_off_paints_everything_in_the_old_single_color() {
    let s = shared_fill();
    assert_eq!(raw_paints("a || b", "a b", false, false), [(0, 1, s.normal), (2, 3, s.normal)]);
    assert_eq!(raw_paints("a || b", "a b", true, false), [(0, 1, s.current), (2, 3, s.current)]);
}

#[test]
fn raw_view_paints_nothing_on_a_line_that_fails_the_whole_expression() {
    assert!(raw_paints("a && b", "only a here", false, true).is_empty());
    assert!(raw_paints("a && b", "only a here", false, false).is_empty());
}

#[test]
fn a_phrase_fully_covered_by_a_longer_one_still_gets_its_stripe() {
    let m = matcher("error && \"error code\"");
    let line = "error code";
    // Only the longer phrase survives overlap resolution...
    assert_eq!(m.find_phrase_ranges(line).len(), 1);
    // ...but the minimap stripes come from the phrases that occur.
    let fills = search_phrase_fills(&m.phrase_keys(), Tone::Light);
    let tick = minimap_tick(&m.phrases_in(line), &fills, 40.0, false, true, shared_tick_color());
    assert_eq!(tick.stripes.len(), 2);
    assert_eq!(tick.stripes[0].color.h, fills[0].normal.h);
    assert_eq!(tick.stripes[1].color.h, fills[1].normal.h);
}

// ---- Focus Find follows Search for shared phrases ----

#[test]
fn focus_find_reuses_searchs_hue_for_a_shared_phrase_and_separates_the_rest() {
    let search = ["a", "b", "c"];
    let search_hues = search_color::phrase_hues(&search);
    let find = ["z", "c", "a", "y"];
    let hues = search_color::phrase_hues_following(&find, &search);
    assert_eq!(hues[1], search_hues[2]);
    assert_eq!(hues[2], search_hues[0]);
    // The others keep 30 degrees from everything already placed.
    for (i, &h) in hues.iter().enumerate() {
        if i == 1 || i == 2 {
            continue;
        }
        for j in [1usize, 2] {
            assert!(hue_distance(h, hues[j]) >= 30.0, "{i} {j} {hues:?}");
        }
    }
    assert_eq!(hues, search_color::phrase_hues_following(&find, &search));
}

#[test]
fn without_a_search_expression_focus_find_hues_are_the_plain_ones() {
    let find = ["z", "c", "a"];
    assert_eq!(search_color::phrase_hues_following(&find, &[]), search_color::phrase_hues(&find));
}

#[test]
fn find_fills_match_search_fills_for_a_shared_phrase_even_when_plain_hues_would_collide() {
    // Find a pair of keys whose plain hues are within the gap, so that in
    // `x || y` the second one is pushed but alone it would not be.
    let keys: Vec<String> = (0..200).map(|i| format!("k{i}")).collect();
    let (x, y) = keys
        .iter()
        .flat_map(|a| keys.iter().map(move |b| (a, b)))
        .find(|(a, b)| a != b && hue_distance(search_color::hue_for_key(a), search_color::hue_for_key(b)) < 30.0)
        .unwrap();
    let search = search_phrase_fills(&[x.as_str(), y.as_str()], Tone::Dark);
    let find = log_gpui::ui::canvas::find_phrase_fills(&[y.as_str()], &[x.as_str(), y.as_str()], Tone::Dark);
    assert_eq!(find[0], search[1]);
    assert_ne!(search[1].normal.h, search_phrase_fills(&[y.as_str()], Tone::Dark)[0].normal.h);
}
