## Context

Mọi dòng của một view đã có thời gian hiệu lực và level trong bộ nhớ:
- `FilterIndex.effective_ts: Vec<f64>` theo từng dòng của view, giá trị `NAN` khi dòng không có giờ (`src/filter_index.rs`).
- Level, cờ Continuation và `ts_us` trong `LineMeta` của file đã Indexing (`src/index/meta.rs`), hoặc trong `LogEntry` của file đọc vào bộ nhớ. Cả hai được đọc qua `BoundRows::level` / `continues` (`src/view_data.rs`).

Minimap (`src/ui/canvas.rs`) đã vẽ bằng `gpui::canvas` + `paint_quad`, tô theo `CanvasColors::level()`, và được bật/tắt bằng `show_minimap` trong Settings. Time Range đã có (`FilterState::set_time_range`, `parse_time_bound`). Áp Time Range chỉ làm mới Focus Pane, sau 100 ms không gõ. Live đã có hook cập nhật dần `apply_rows_appended(from)` (`src/app/filter.rs`). Hiện chưa có code vẽ trục hay biểu đồ.

## Goals / Non-Goals

**Goals:**
- Đếm theo khoảng thời gian chỉ từ dữ liệu đã có trong bộ nhớ, trên background, không đọc lại file.
- Trục đủ chung để swimlane sau này chỉ cần thêm làn bên dưới.
- Kéo chọn đi qua đúng đường của Time Range đang có, không có cơ chế lọc thứ hai.

**Non-Goals:**
- Swimlane, Stream Column, chia theo cột (change sau).
- Histogram cho Raw View.
- Histogram riêng trong Focus Pane hay Focus Window (xem D9).
- Tô màu theo Highlight Rule hay theo cột khác ngoài level.

## Decisions

### D1. Một hàm đếm duy nhất trên các mảng theo dòng
`count_buckets(rows, axis, step) -> Buckets` duyệt các dòng của view một lần. Với mỗi dòng nó đọc `effective_ts[row]`, level và cờ Continuation; bỏ qua Continuation Line; dòng có giờ được cộng vào `counts[bucket][class]` (class: other / warn / error). Cùng lượt duyệt đó cũng tính:
- `filtered[bucket][class]`, bằng cách đi song song với `filter_pass` (mảng chỉ số dòng đã sắp xếp),
- `matched[bucket]` từ tập Match của Focus Tab đang mở,
- số dòng không có giờ và dòng đầu tiên trong số đó.

Với file đã Indexing, mỗi file chỉ khoá `data.read()` một lần cho cả lượt duyệt, như `merge_by_meta` đang làm, chứ không gọi `IndexedFile::meta(n)` cho từng dòng.

*Đã cân nhắc:* dựng sẵn nhiều tầng độ phân giải (pyramid). Bỏ vì một lượt duyệt `Vec<f64>` 10 triệu dòng chỉ mất khoảng chục ms trên background, trong khi pyramid làm Live và Filter phức tạp hơn nhiều.

### D2. Trục thuộc về view, không thuộc về từng dải
`TimeAxis { full: Range<f64>, visible: Range<f64>, step: Step }` nằm trong trạng thái của view. `step` được chọn từ bảng bước cố định, theo `visible.len() / (width_px / 5)`. Swimlane sau này cũng sẽ đọc `TimeAxis` này.

### D3. Tính lại hay cộng dồn
- Tính lại toàn bộ trên background (`cx.spawn` → `background_executor().spawn` → `this.update`, giống `start_preparing` trong Live) khi: đổi `visible`/`step`, đổi Filter, đổi Search Expression, đổi tập dòng của view, hoặc trục `full` phải giãn ra. Mỗi lần tính có một số thế hệ; kết quả cũ hơn trạng thái hiện tại thì bỏ.
- Cộng dồn trong `apply_rows_appended(from)`: các dòng `from..len` có giờ nằm trong `full` thì cộng thẳng vào `Buckets`; có dòng vượt ra ngoài `full` thì lên lịch tính lại. Vẽ lại được gộp, không quá 2 lần mỗi giây.
- Trim của Live Window và các thay đổi cấu trúc (thứ tự file đổi, `rebuild_open_view_keeping_position`) thì tính lại toàn bộ.

### D4. Vẽ
Module mới `src/ui/time_histogram.rs`, vẽ bằng `gpui::canvas` và `paint_quad` như minimap, cùng dùng `CanvasColors::level()`. Thứ tự lớp từ dưới lên:
1. nền Time Range (xanh nhạt),
2. cột tổng (mờ khi có Filter),
3. cột đã lọc (đậm),
4. vạch Match,
5. khung vùng đang xem (chỉ viền),
6. nhãn dòng không có giờ.

Phần trục chưa có số đếm trong lúc Indexing được vẽ mờ. Chiều cao khi thu gọn khoảng 40px.

### D5. Tương tác
- **Bấm một cột:** tìm dòng đầu tiên có `effective_ts >= bucket_start`, rồi `set_current_line(line, Origin::Jump)`. Phần tìm dòng được tách thành `first_row_at(time)` để Go To Time cũng dùng. Trong Log Space, `effective_ts` đã sắp xếp nên tìm nhị phân (như `partition_rows`). File View có thể có giờ lùi, nên tìm tuần tự.
- **Kéo:** trong lúc kéo chỉ vẽ dải tạm. Khi nhả chuột mới gọi `FilterState::set_time_range(from, to)` với chuỗi đã định dạng, rồi `apply_filter_change`. Như vậy Focus Pane chỉ được tính lại một lần.
- **Kéo mép / kéo giữa:** chạm trong vùng 4px quanh mép thì là kéo mép; chạm trong dải là dời; chạm ngoài dải mà không kéo thì xoá Time Range.
- **⌘ + cuộn:** đổi `visible` quanh vị trí con trỏ, kẹp trong `full`. Nút "Xem toàn bộ" đặt `visible = full`.

### D6. Định dạng Time Range do histogram đặt
Dùng `HH:MM:SS` (thêm phần lẻ giây khi `step` nhỏ hơn 1 giây) khi cả hai đầu nằm trong `anchor_day`. Ngược lại dùng `YYYY-MM-DD HH:MM:SS`. Cả hai dạng đều là dạng `parse_time_bound` đã đọc được, nên chuỗi trong ô và giá trị lọc khớp nhau.

*Đã cân nhắc:* lưu Time Range ở dạng số. Bỏ vì như vậy là có hai nguồn sự thật cho cùng một Filter, và người dùng không đọc được.

### D7. Bật/tắt
Thêm `show_histogram: bool` (mặc định `true`) vào `workspace_state`, đặt cạnh `show_minimap` và lưu trong `src/workspace/session.rs`. Nhóm nút trên thanh công cụ gọi `set_show_histogram` / `set_show_minimap`, và hai Command mới trong Command Palette cũng gọi hai hàm đó. Bỏ dòng "Show minimap" trong `src/ui/settings.rs`. Giá trị `show_minimap` đã lưu vẫn được đọc như cũ.

### D8. Điều kiện hiện
Một view có histogram khi `show_histogram` đang bật, view không phải Raw View, và `FilterIndex.timed_lines() > 0`. Focus Pane và Focus Window không có histogram (D9).

### D9. Focus Pane và Focus Window không có histogram riêng
Focus Pane là con của root trong `src/app/view/mod.rs`, nằm dưới cả hàng sidebar | main column | context sidebar, nên rộng hết cửa sổ. Dải chính chỉ rộng bằng main column. Cùng `TimeAxis` vì vậy không có nghĩa là cùng pixel: khi context sidebar mở, cùng một thời điểm nằm ở hai vị trí x khác nhau trên hai dải. Không vẽ histogram trong Pane. Phần đậm (dòng qua Filter) trên dải chính đã cho thấy những gì Pane đang liệt kê, ngay trên cùng một trục với tổng số dòng.

Focus Window cũng không có dải, để Focus Tab trông như nhau dù ở trong Pane hay đã tách ra cửa sổ riêng, và để chỉ có một nơi vẽ histogram (view chính). Muốn xem phân bố theo thời gian thì xem dải của view chính.

*Đã cân nhắc:* (a) vẽ dải trong Pane theo đúng x và bề rộng của main column. Bỏ vì phải truyền bounds của main column xuống Pane, và phần thụt hai bên trông lạc chỗ. (b) chuyển Focus Pane vào trong main column. Bỏ vì như vậy đổi bố cục của cả hai sidebar (`pane_taken`), quá rộng so với change này.

## Risks / Trade-offs

- **Trục tiếp tục giãn trong Live, làm tính lại liên tục.** → Khi giãn, làm tròn `full.end` lên thêm 10% khoảng hoặc lên một bước lớn, để vài phút sau mới phải giãn tiếp.
- **File View có giờ lùi**, nên cột và khung vùng đang xem không đơn điệu. → Khung lấy min/max giờ của các dòng đang hiện. Tìm khi bấm cột thì làm tuần tự.
- **Lúc Indexing, chưa biết `full.end`.** → Dùng `time_hint()` (min/max đã biết) và giãn dần. Vẽ mờ từ giờ của dòng cuối đã Indexing tới hết trục.
- **Định dạng chuỗi đi ngược qua `parse_time_bound` làm mất độ chính xác** dưới 1 giây. → Thêm phần lẻ giây khi `step` nhỏ hơn 1 giây, và có test round-trip.
- **Bỏ mục trong Settings**, nên người dùng quen tìm ở đó sẽ không thấy. → Vẫn có Command trong Command Palette với cùng tên "Show minimap".

## Migration Plan

Không cần migration dữ liệu. `show_histogram` thiếu thì mặc định `true`; `show_minimap` giữ nguyên khoá lưu. Muốn quay lại thì bỏ nhóm nút và trả lại dòng trong Settings.

## Open Questions

- Ở chiều rộng dải nhỏ (cửa sổ hẹp), số cột tối thiểu là bao nhiêu thì nên ẩn dải thay vì vẽ quá thô?
