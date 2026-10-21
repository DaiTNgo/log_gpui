//! What the app does with the events of a Remote Command.

use log_gpui::live::{BreakKind, LiveBuffer, RestartKind, SourceStatus, StreamMask};
use log_gpui::model::Stream;
use log_gpui::space_view::ParsedFile;

const NAME: &str = "web1$ docker logs -f api";

fn text(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

#[test]
fn lines_are_numbered_from_one_and_keep_their_stream() {
    let mut parsed = ParsedFile::empty();
    let mut buf = LiveBuffer::command(NAME);
    let streams: StreamMask = [Stream::Stdout, Stream::Stderr, Stream::Stdout].into_iter().collect();
    let range = buf.apply_append_streams(&mut parsed, &text(&["a", "boom", "c"]), &[], Some(&streams));
    assert_eq!(range, 0..3);
    assert_eq!(parsed.entries.iter().map(|e| e.index).collect::<Vec<_>>(), vec![1, 2, 3]);
    assert_eq!(
        parsed.entries.iter().map(|e| e.stream).collect::<Vec<_>>(),
        vec![Some(Stream::Stdout), Some(Stream::Stderr), Some(Stream::Stdout)]
    );
}

#[test]
fn lines_of_a_file_have_no_stream() {
    let mut parsed = ParsedFile::empty();
    let mut buf = LiveBuffer::loaded("app.log", 0);
    buf.apply_append_streams(&mut parsed, &text(&["a"]), &[], None);
    assert_eq!(parsed.entries[0].stream, None);
}

#[test]
fn a_continuation_line_has_the_stream_of_its_own_output() {
    let mut parsed = ParsedFile::empty();
    let mut buf = LiveBuffer::command(NAME);
    let streams: StreamMask = [Stream::Stdout, Stream::Stderr].into_iter().collect();
    buf.apply_append_streams(
        &mut parsed,
        &text(&["2026-10-19T10:02:03Z GET /a 500", "  at handler (a.js:3)"]),
        &[],
        Some(&streams),
    );
    assert!(parsed.entries[1].is_continuation());
    assert_eq!(parsed.entries[1].stream, Some(Stream::Stderr));
}

#[test]
fn the_stream_mask_holds_more_than_one_word_of_bits() {
    let streams: Vec<Stream> = (0..200).map(|i| if i % 7 == 0 { Stream::Stderr } else { Stream::Stdout }).collect();
    let mask: StreamMask = streams.iter().copied().collect();
    assert_eq!(mask.len(), 200);
    for (i, s) in streams.iter().enumerate() {
        assert_eq!(mask.get(i), Some(*s), "bit {i}");
    }
    assert_eq!(mask.get(200), None);
}

#[test]
fn exit_then_restart_draw_one_break_and_start_a_new_generation() {
    let mut parsed = ParsedFile::empty();
    let mut buf = LiveBuffer::command(NAME);
    buf.apply_append_streams(&mut parsed, &text(&["a", "b"]), &[], None);
    buf.apply_break(BreakKind::Exited(Some(0)), "10:02");
    assert_eq!(buf.generation, 0, "an exit alone does not start a Generation");
    buf.apply_break(BreakKind::Restarted(RestartKind::Manual), "10:03");
    assert_eq!(buf.generation, 1);
    buf.apply_append_streams(&mut parsed, &text(&["c"]), &[], None);
    assert_eq!(parsed.entries[2].index, 1, "numbers start again");
    assert_eq!(parsed.entries[2].generation, 1);
    assert_eq!(
        parsed.entries[2].break_before.as_ref().unwrap().label,
        "web1$ docker logs -f api exited (0) 10:02; web1$ docker logs -f api restarted 10:03"
    );
}

#[test]
fn a_resumed_run_keeps_its_generation_and_numbers() {
    let mut parsed = ParsedFile::empty();
    let mut buf = LiveBuffer::command(NAME);
    buf.apply_append_streams(&mut parsed, &text(&["a", "b"]), &[], None);
    buf.apply_break(BreakKind::Disconnected, "10:02");
    buf.apply_break(BreakKind::Resumed(10 * 3600 + 2 * 60 + 3), "10:03");
    buf.apply_append_streams(&mut parsed, &text(&["c"]), &[], None);
    assert_eq!(buf.generation, 0);
    assert_eq!(parsed.entries[2].index, 3);
    assert_eq!(
        parsed.entries[2].break_before.as_ref().unwrap().label,
        "web1$ docker logs -f api disconnected 10:02 – reconnected 10:03, resumed from 10:02:03"
    );
}

#[test]
fn a_typed_command_run_again_after_a_disconnection_says_lines_may_be_missing() {
    let mut parsed = ParsedFile::empty();
    let mut buf = LiveBuffer::command("web1$ tail -F /tmp/x.log");
    buf.apply_append_streams(&mut parsed, &text(&["a"]), &[], None);
    buf.apply_break(BreakKind::Disconnected, "10:02");
    buf.apply_break(BreakKind::Restarted(RestartKind::AfterDisconnect), "10:03");
    buf.apply_append_streams(&mut parsed, &text(&["b"]), &[], None);
    assert_eq!(buf.generation, 1);
    assert_eq!(parsed.entries[1].index, 1);
    assert_eq!(
        parsed.entries[1].break_before.as_ref().unwrap().label,
        "web1$ tail -F /tmp/x.log disconnected 10:02 – reconnected 10:03, command restarted, lines may be missing or repeated"
    );
}

#[test]
fn exited_ranks_between_waiting_for_file_and_reconnecting() {
    let exited = SourceStatus::Exited(Some(1)).severity();
    assert!(SourceStatus::WaitingForFile.severity() < exited);
    assert!(exited < SourceStatus::Reconnecting.severity());
    assert!(SourceStatus::Live.severity() < SourceStatus::WaitingForFile.severity());
    assert_eq!(SourceStatus::Exited(Some(127)).label(), "Exited (code 127)");
}

#[test]
fn an_edited_command_leaves_its_lines_and_starts_a_new_run_after_a_break_naming_both_commands() {
    let (mut buf, mut parsed) = (LiveBuffer::loaded("web1$ docker logs api", 0), log_gpui::space_view::ParsedFile::empty());
    buf.apply_append_streams(&mut parsed, &["old one".to_string(), "old two".to_string()], &[], None);
    buf.apply_break(BreakKind::Exited(None), "10:02");
    buf.apply_command_changed("web1$ docker logs api", "web1$ docker logs worker", "10:03");
    assert_eq!(buf.generation, 1);
    assert_eq!(buf.name, "web1$ docker logs worker");
    buf.apply_append_streams(&mut parsed, &["new one".to_string()], &[], None);
    // The old lines stay; the new one is line 1 of the next Generation, under a Break.
    assert_eq!(parsed.lines.len(), 3);
    assert_eq!(parsed.entries[2].index, 1);
    assert_eq!(parsed.entries[2].generation, 1);
    let mark = parsed.entries[2].break_before.as_ref().expect("a Break above the first new line");
    assert_eq!(mark.kind, BreakKind::CommandChanged);
    assert!(mark.label.contains("command changed"), "{}", mark.label);
    assert!(mark.label.contains("web1$ docker logs api") && mark.label.contains("web1$ docker logs worker"), "{}", mark.label);
}
