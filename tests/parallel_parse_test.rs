//! Parsing a file on several cores gives the entries parsing it in one go
//! gives (openspec change `line-offset-index`: opening faster).

use log_gpui::parser::text::{parse_entries, parse_entries_parallel};

fn samples() -> Vec<(&'static str, String)> {
    let mut big = String::new();
    for i in 0..3000 {
        if i % 13 == 4 {
            big += "java.lang.IllegalStateException: boom\n\tat a.b(C.java:12)\n\n";
        }
        if i % 101 == 7 {
            big += "{\"time\":\"2026-09-05T10:00:03Z\",\"level\":\"warn\",\"msg\":\"json\"}\n{\"level\":\"info\",\"msg\":\"no time\"}\n";
        }
        big += &format!("2026-09-05 10:{:02}:{:02}.{:03} [INFO] [api] request {i} user={}\n", i / 60 % 60, i % 60, i % 1000, i % 7);
    }
    vec![
        ("big", big),
        ("empty", String::new()),
        ("no break at the end", "2026-09-05 10:00:00 [INFO] [a] one\nlast without break".into()),
        ("untimed head", "banner\nINFO not a time\n\n2026-09-05 10:00:00 [INFO] [a] x\n  more\n".into()),
        ("only untimed", "a\nb\n\nc\n".repeat(50)),
        ("crlf", "2026-09-05 10:00:00 [WARN] [x] a\r\n  more\r\n2026-09-05 10:00:02 [DEBUG] [x] b\r\n".repeat(30)),
        ("lossy", String::from_utf8_lossy(b"2026-09-05 10:00:00 [INFO] [a] bad \xff\n\xfe\xfe\n").into_owned()),
    ]
}

#[test]
fn blocks_parsed_apart_give_the_same_entries() {
    for (name, text) in samples() {
        let want: Vec<String> = parse_entries(&text).iter().map(|e| format!("{e:?}")).collect();
        for block in [1, 7, 64, 1000, 1 << 20] {
            let mut seen = Vec::new();
            let got = parse_entries_parallel(&text, block, |done| {
                seen.push(done);
                true
            })
            .unwrap();
            let got: Vec<String> = got.iter().map(|e| format!("{e:?}")).collect();
            assert_eq!(got.len(), want.len(), "{name} in blocks of {block}");
            for (i, (g, w)) in got.iter().zip(&want).enumerate() {
                assert_eq!(g, w, "{name} in blocks of {block}: line {}", i + 1);
            }
            assert!(seen.windows(2).all(|w| w[0] <= w[1]), "progress only grows");
            if !text.is_empty() {
                assert_eq!(seen.last(), Some(&text.len()), "{name}: progress reaches the end");
            }
        }
    }
}

#[test]
fn a_stopped_parse_gives_nothing() {
    let (_, text) = samples().remove(0);
    assert!(parse_entries_parallel(&text, 64, |_| false).is_none());
}
