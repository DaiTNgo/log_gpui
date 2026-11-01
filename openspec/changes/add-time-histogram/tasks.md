## 1. Trục và đếm (logic thuần)

- [x] 1.1 Tạo `src/time_histogram.rs` với `Step` (bảng 1s…1d), `TimeAxis { full, visible, step }`, `Buckets { counts, filtered, matched, untimed, first_untimed }`; export trong `src/lib.rs`
- [x] 1.2 `Step::for_width(visible, width_px)` chọn bước sao cho mỗi cột rộng khoảng 4–6px; test với scenario "1 giờ, 720px → 30s"
- [x] 1.3 `count_buckets(rows, axis, filter_pass, matches)`: bỏ Continuation Line, tách class other/warn/error, đi song song với `filter_pass`, đánh dấu cột có Match, đếm dòng không có giờ; test các scenario của "Cột đếm Parent Line theo level" và "Filter và Search trên histogram"
- [x] 1.4 `Buckets::append(rows_from)` cộng dồn, trả về `NeedsRecount` khi có dòng vượt ra ngoài `full`; test
- [x] 1.5 Hàm giãn `full` có làm tròn (thêm khoảng 10% hoặc một bước lớn); test
- [x] 1.6 `first_row_at(time)`: tìm nhị phân khi `effective_ts` đã sắp xếp, tìm tuần tự khi không; chuyển Go To Time sang dùng hàm này; test cả cột trống
- [x] 1.7 `format_time_bound(t, anchor_day, step)` cho ra `HH:MM:SS[.fff]` hoặc `YYYY-MM-DD HH:MM:SS`; test round-trip qua `parse_time_bound`

## 2. Trạng thái view và tính trên background

- [x] 2.1 Gắn `TimeAxis` và `Buckets` (kèm số thế hệ) vào trạng thái view; khởi tạo `full` từ `time_hint()` / min–max `effective_ts`
- [x] 2.2 `schedule_histogram_recount`: chạy `count_buckets` trên background executor, bỏ kết quả có thế hệ cũ
- [x] 2.3 Gọi tính lại khi đổi `visible`/`step`, Filter, Search Expression của Focus Tab đang mở, tập dòng của view, trim Live Window, hoặc `rebuild_open_view_keeping_position`
- [x] 2.4 Trong `apply_rows_appended(from)`: gọi `Buckets::append`, rơi về tính lại khi cần; gộp lần vẽ lại để không quá 2 lần mỗi giây
- [x] 2.5 Lúc Indexing: cập nhật `full` khi Indexing công bố thêm dòng, và ghi lại giờ đã Indexing tới để vẽ mờ phần sau
- [x] 2.6 Điều kiện hiện (D8): `show_histogram` bật, không phải Raw View, `timed_lines() > 0`

## 3. Vẽ

- [x] 3.1 `src/ui/time_histogram.rs`: `gpui::canvas` vẽ cột tổng/đã lọc theo `CanvasColors::level()`, cột mờ khi có Filter, vạch Match
- [x] 3.2 Khung vùng đang xem, lấy theo min/max giờ của các dòng danh sách đang hiện, cập nhật khi cuộn
- [x] 3.3 Dải nền Time Range với hai tay nắm ở mép
- [x] 3.4 Nhãn "n dòng không có thời gian", vẽ mờ phần đang Indexing, nhãn trục đầu/cuối, nút "Xem toàn bộ"
- [x] 3.5 Đặt dải vào bố cục `src/app/view/mod.rs` phía trên `LogCanvas`; dải thu gọn được

## 4. Tương tác

- [x] 4.1 Bấm cột → `first_row_at(bucket_start)` → `set_current_line(line, Origin::Jump)`; Filter và Focus Pane không đổi
- [x] 4.2 Kéo chọn: vẽ dải tạm trong lúc kéo, nhả chuột thì `set_time_range(format…)` và `apply_filter_change`
- [x] 4.3 Kéo mép, kéo giữa để dời, bấm ngoài dải mà không kéo để xoá Time Range
- [x] 4.4 ⌘ + cuộn zoom quanh con trỏ, kẹp trong `full`; nút "Xem toàn bộ"
- [x] 4.5 Bấm nhãn dòng không có giờ → nhảy tới `first_untimed`
- [x] 4.6 Sửa ô Từ/Đến thì dải nền trên histogram cập nhật theo (spec `log-filter`)

## 5. Focus Pane và Focus Window

- [x] 5.1 Không có histogram trong Focus Pane và Focus Window (D2, D8)

## 6. Nút bật/tắt

- [x] 6.1 Thêm `show_histogram` (mặc định `true`) vào `workspace_state` và `src/workspace/session.rs`; `set_show_histogram` trong `src/app/view_settings.rs`
- [x] 6.2 Nhóm nút Histogram và Minimap trên thanh công cụ của view
- [x] 6.3 Hai Command "Show time histogram" và "Show minimap" trong Command Palette, gán được Shortcut qua Keymap
- [x] 6.4 Bỏ dòng "Show minimap" trong `src/ui/settings.rs`; kiểm tra giá trị `show_minimap` đã lưu vẫn được đọc

## 7. Docs và kiểm tra

- [x] 7.1 Thêm Time Histogram vào `CONTEXT.md`; viết ADR "kéo chọn trên Time Histogram là Time Range"
- [x] 7.2 Đo `count_buckets` trên 10 triệu dòng trong `examples/perf_probe.rs`, mục tiêu dưới 30 ms
- [ ] 7.3 Chạy ứng dụng: Static Log Space nhiều file, file lớn đang Indexing, Live Log Space đang tail, File View không có timestamp, Raw View
