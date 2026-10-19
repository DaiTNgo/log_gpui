/// The Stream of a line fits in the padding LogEntry already had (it was 328
/// bytes before `stream` was added).
#[test]
fn the_stream_of_a_line_does_not_grow_log_entry() {
    assert!(std::mem::size_of::<log_gpui::model::LogEntry>() <= 328);
}
