use log_gpui::unicode_text::{Needle, contains, eq, fold};

#[test]
fn case_folds_vietnamese() {
    assert!(eq("ĐÃ", "đã"));
    assert!(contains("Lỗi: ĐÃ XẢY RA", "đã xảy"));
}

#[test]
fn diacritics_are_not_ignored() {
    assert!(!contains("lỗi", "loi"));
    assert!(!contains("lối", "lỗi"));
    assert!(!eq("loi", "lỗi"));
}

#[test]
fn nfc_and_nfd_match_each_other() {
    let nfc = "l\u{1ED7}i";
    let nfd = "lo\u{0302}\u{0303}i";
    assert_ne!(nfc, nfd);
    assert!(contains(nfd, nfc));
    assert!(contains(nfc, nfd));
    assert_eq!(fold(nfc), fold(nfd));
}

#[test]
fn ascii_case_insensitive() {
    assert!(contains("Connection Refused", "connection refused"));
    assert!(eq("ERROR", "error"));
    assert!(!contains("abc", ""));
}

#[test]
fn find_all_returns_original_byte_offsets() {
    let hay = "Lỗi: lỗi khác, LỖI nữa";
    let needle = Needle::new("lỗi");
    let ranges = needle.find_all(hay);
    assert_eq!(ranges.len(), 3);
    for (s, e) in &ranges {
        assert!(hay.is_char_boundary(*s) && hay.is_char_boundary(*e));
        assert!(eq(&hay[*s..*e], "lỗi"));
    }
}

#[test]
fn offsets_are_in_the_original_when_text_is_nfd() {
    let hay = "x lo\u{0302}\u{0303}i y";
    let needle = Needle::new("l\u{1ED7}i");
    let r = needle.find_all(hay);
    assert_eq!(r.len(), 1);
    assert_eq!(&hay[r[0].0..r[0].1], "lo\u{0302}\u{0303}i");
}

#[test]
fn does_not_match_a_base_letter_inside_a_marked_letter() {
    // NFD "ô" must not match a bare "o".
    assert!(!contains("lo\u{0302}i", "loi"));
    assert!(!contains("l\u{00F4}i", "loi"));
}

#[test]
fn equals_requires_whole_string() {
    let n = Needle::new("db");
    assert!(n.equals("DB"));
    assert!(!n.equals("DB2"));
}

#[test]
fn multibyte_before_the_match() {
    let hay = "đã lỗi";
    let r = Needle::new("lỗi").find_all(hay);
    assert_eq!(r.len(), 1);
    assert_eq!(&hay[r[0].0..r[0].1], "lỗi");
}
