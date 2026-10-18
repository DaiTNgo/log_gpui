use log_gpui::search_expr::{Matcher, SearchError, SearchExpr, parse};

fn matcher(q: &str) -> Matcher {
    Matcher::compile(&parse(q).unwrap().unwrap())
}

fn p(words: &[&str]) -> SearchExpr {
    SearchExpr::Phrase(words.iter().map(|w| w.to_string()).collect())
}

#[test]
fn empty_is_none() {
    assert_eq!(parse("").unwrap(), None);
    assert_eq!(parse("   ").unwrap(), None);
}

#[test]
fn adjacent_words_form_one_phrase() {
    assert_eq!(parse("connection refused").unwrap(), Some(p(&["connection", "refused"])));
}

#[test]
fn and_or_precedence() {
    assert_eq!(
        parse("a || b && c").unwrap(),
        Some(SearchExpr::Or(vec![p(&["a"]), SearchExpr::And(vec![p(&["b"]), p(&["c"])])]))
    );
}

#[test]
fn parentheses_group() {
    assert_eq!(
        parse("(a || b) && c").unwrap(),
        Some(SearchExpr::And(vec![SearchExpr::Or(vec![p(&["a"]), p(&["b"])]), p(&["c"])]))
    );
}

#[test]
fn and_or_words_are_plain_words() {
    assert_eq!(
        parse("failed and retried").unwrap(),
        Some(p(&["failed", "and", "retried"]))
    );
    assert_eq!(parse("a OR b").unwrap(), Some(p(&["a", "OR", "b"])));
}

#[test]
fn phrase_next_to_group_is_an_error() {
    assert!(matches!(parse("(db || cache) timeout"), Err(SearchError::UnexpectedToken { .. })));
    assert!(matches!(parse("timeout (db || cache)"), Err(SearchError::UnexpectedToken { .. })));
}

#[test]
fn incomplete_expressions_are_errors() {
    assert!(matches!(parse("(timeout ||"), Err(SearchError::MissingOperand { .. })));
    assert!(matches!(parse("(timeout"), Err(SearchError::UnclosedParen { .. })));
    assert!(matches!(parse("timeout &&"), Err(SearchError::MissingOperand { .. })));
    assert!(matches!(parse("&& timeout"), Err(SearchError::MissingOperand { .. })));
    assert!(matches!(parse("timeout)"), Err(SearchError::UnexpectedToken { .. })));
    assert!(matches!(parse("a & b"), Err(SearchError::UnexpectedToken { .. })));
    assert!(matches!(parse("\"abc"), Err(SearchError::UnclosedQuote { .. })));
    assert!(matches!(parse("\"\""), Err(SearchError::EmptyPhrase { .. })));
    assert!(matches!(parse("()"), Err(SearchError::MissingOperand { .. })));
}

#[test]
fn quotes_allow_special_characters() {
    assert_eq!(parse("\"foo(bar)\"").unwrap(), Some(p(&["foo(bar)"])));
    let m = matcher("\"foo(bar)\"");
    assert!(m.is_match("call foo(bar) done"));
    assert!(!m.is_match("call foo bar done"));
}

#[test]
fn vietnamese_words_parse() {
    assert_eq!(parse("lỗi kết nối").unwrap(), Some(p(&["lỗi", "kết", "nối"])));
}

#[test]
fn phrase_matches_in_order_with_any_whitespace() {
    let m = matcher("connection refused");
    assert!(m.is_match("db connection refused by peer"));
    assert!(m.is_match("connection\t\trefused"));
    assert!(m.is_match("connection   refused"));
    assert!(!m.is_match("refused connection"));
    assert!(!m.is_match("connectionrefused"));
}

#[test]
fn case_insensitive_and_diacritic_sensitive() {
    assert!(matcher("timeout").is_match("TimeOut after 5s"));
    assert!(matcher("đã").is_match("ĐÃ xảy ra"));
    assert!(!matcher("loi").is_match("lỗi kết nối"));
    assert!(!matcher("lỗi").is_match("lối"));
}

#[test]
fn boolean_logic() {
    assert!(matcher("timeout && db").is_match("db timeout"));
    assert!(!matcher("timeout && db").is_match("only timeout"));
    assert!(matcher("timeout || refused").is_match("conn refused"));
    assert!(!matcher("timeout || refused").is_match("ok"));
    let m = matcher("(timeout || refused) && db");
    assert!(m.is_match("db refused"));
    assert!(!m.is_match("refused"));
    assert!(!m.is_match("db ok"));
}

#[test]
fn no_regex() {
    assert!(!matcher("5.0").is_match("5x0"));
    assert!(matcher("5.0").is_match("version 5.0"));
    assert!(matcher(".*").is_match("a.*b"));
    assert!(!matcher(".*").is_match("abc"));
}

#[test]
fn highlights_every_occurrence() {
    let line = "Error: another error";
    let r = matcher("error").find_ranges(line);
    assert_eq!(r, vec![(0, 5), (15, 20)]);
}

#[test]
fn highlights_phrase_span_and_multibyte() {
    let line = "đã lỗi   kết nối";
    let r = matcher("lỗi kết nối").find_ranges(line);
    assert_eq!(r.len(), 1);
    assert_eq!(&line[r[0].0..r[0].1], "lỗi   kết nối");
}

#[test]
fn highlights_all_phrases_of_the_expression() {
    let line = "timeout then refused";
    let r = matcher("timeout || refused").find_ranges(line);
    assert_eq!(r, vec![(0, 7), (13, 20)]);
}

#[test]
fn overlapping_phrase_heads_are_found() {
    // "aa ab" must be found even though the first "aa" head fails.
    let m = matcher("a ab");
    assert!(m.is_match("aa ab"));
}
