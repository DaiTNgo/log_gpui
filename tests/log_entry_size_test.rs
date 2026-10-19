/// The Stream of a line fits in the padding LogEntry already had (it was 328
/// bytes before `stream` was added). The Break below the last line of a
/// Remote Copy (`break_after`, remote-full-read) is one more pointer, 8 bytes
/// a line (16 MB at the Full Read Limit): 336 is the new ceiling.
#[test]
fn the_stream_of_a_line_does_not_grow_log_entry() {
    assert!(std::mem::size_of::<log_gpui::model::LogEntry>() <= 336);
}
