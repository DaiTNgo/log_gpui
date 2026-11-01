## Context

Change `add-time-histogram` đặt nền móng cho change này:
- `TimeAxis { full, visible, step }` thuộc về view,
- `count_buckets` chạy trên background, có số thế hệ,
- cộng dồn trong `apply_rows_appended`,
- dải vẽ bằng `gpui::canvas` trong `src/ui/time_histogram.rs`.

Mỗi Filterable Column đã có `ColumnIndex` với `row_ids: Vec<u16>` (id giá trị theo từng dòng của view), `values` (kèm số đếm), và cờ tràn khi vượt `MAX_DISTINCT` (`src/filter_index.rs`). Change này nâng `MAX_DISTINCT` từ 500 lên 1000 (D8). Source Column được dựng trong `src/filter_index/merge.rs::source_column` từ tên Source File. Quy tắc nhận diện Rotated File theo tên nằm trong `src/remote/rotated.rs` (`rotation`, `rotated_base`), cùng file với các hàm SFTP.

## Goals / Non-Goals

**Goals:**
- Làn chỉ là một cách đếm khác trên cùng dữ liệu: dùng lại `ColumnIndex.row_ids`, không parse lại và không đọc lại file.
- Log Name là một cột như mọi cột khác, nên Filter, ghim và Lane By có được mà không cần cơ chế riêng.
- Dùng một quy tắc tên duy nhất cho Rotated File, Generation và Log Name Column.

**Non-Goals:**
- Tự đoán module trong message, gom luồng bằng tay, đoán luồng theo thời gian.
- Làn lồng nhau (chia theo hai cột cùng lúc).
- Làn trong Focus Pane.

## Decisions

### D1. Tách quy tắc tên Rotated File ra module thuần
Chuyển `rotation`, `rotated_base`, `RotationKey` và `strip_rotation_suffix` sang `src/rotation_name.rs`, không phụ thuộc SFTP. `src/remote/rotated.rs` re-export và giữ phần liệt kê qua SFTP. Thêm `log_name(source: &str) -> &str`: tách phần trước tên file (sau dấu `/`, `:` hoặc ` › ` cuối cùng), áp `rotated_base` cho tên file, rồi ghép lại phần trước.

*Đã cân nhắc:* viết một hàm riêng cho Log Name Column. Bỏ vì hai quy tắc sẽ dần lệch nhau, khi đó Generation và Log Name hiểu cùng một tên theo hai cách khác nhau.

### D2. Log Name Column dựng như Source Column
`log_name_column(files, rows, row_of)` nằm cạnh `source_column` trong `merge.rs`. Nó gom file theo `log_name(source)`, mỗi Log Name là một giá trị, và `row_ids` lấy theo file của dòng. Cột được gắn `Source::LogName` và có cùng vòng đời với Source Column: tính lại khi tập file hay thứ tự file đổi. File View không có Log Name Column, vì chỉ có một file.

### D3. Đếm theo làn trong cùng lượt duyệt
`count_buckets` nhận thêm một `lanes: Option<&ColumnIndex>`. Khi có, cùng lượt duyệt đó cũng ghi:
- `lane_level[lane_slot][bucket] = max(level class)`,
- `lane_total[value_id] += 1` cho Parent Line.

Continuation Line lấy giá trị của Parent Line, tức là bỏ qua vì vạch chỉ tính theo Parent Line. Dòng có id `MISSING` vào làn "(trống)".

Tập 8 làn được chọn sau lượt duyệt từ `lane_total` (giảm dần, hoà thì theo thứ tự giá trị). Kết quả được lưu trong `LaneSet { slots: Vec<value_id>, other_count, has_empty }`. Đếm bằng mảng phẳng `values.len() × buckets` (`values.len()` đã được `MAX_DISTINCT` chặn ở 1000), rồi gom thành 8 làn + Khác + (trống). Như vậy không cần lượt duyệt thứ hai.

*Đã cân nhắc:* duyệt hai lượt (lượt đầu đếm tổng, lượt sau đếm theo cột thời gian của 8 làn). Bỏ vì mảng phẳng tối đa 1000 × khoảng 400 cột (400k ô u8) vẫn nhỏ, và một lượt duyệt thì đơn giản hơn với Live.

### D4. Live không làm nhảy làn
`Buckets::append` cộng vạch vào làn đã có trong `LaneSet`. Giá trị chưa có làn thì vào "Khác", hoặc chiếm một chỗ còn trống nếu có ít hơn 8 làn. `LaneSet` chỉ được chọn lại khi tính lại toàn bộ (D3 của `add-time-histogram`).

### D5. Lane By là trạng thái theo view
`lane_by: LaneBy` với `LaneBy = None | Column(ColumnKey) | Default`. Giá trị này được lưu cạnh Filter của view trong `src/workspace/state.rs`. `Default` được giải mỗi khi view dựng lại: dùng Log Name Column khi cột này có từ hai giá trị, nếu không thì `None`. Gợi ý trong menu lấy cột đầu tiên theo thứ tự: MODULE, rồi Field Column có tên (so không phân biệt hoa thường) `service`, `module`, `component`, `thread`, `host`, `pod`. Cột đã chọn mà không còn trong view (ví dụ đổi Log Format) thì quay về `Default`.

### D6. Vẽ và bố cục
Các làn nằm trong một dải riêng ngay dưới Time Histogram (dải Swimlane), bật/tắt riêng bằng nút "Lanes" trên toolbar (`show_lanes`, nhớ cho mọi view như `show_histogram`), thu gọn riêng, và hiện được cả khi histogram tắt. Hai dải vẽ trên cùng `TimeAxis`: Time Range, khung vùng đang xem, ⌘ + cuộn và kéo/bấm dùng chung một đoạn code (`axis_area`). Mỗi làn cao 14px. Tên làn nằm trong cột cố định khoảng 90px ở bên trái, cắt bớt kèm tooltip; khi dải Swimlane hiện, histogram giữ cùng khoảng lề đó để hai trục thẳng hàng. Menu "Lane by" ở góc trên trái của dải Swimlane. Làn chỉ được đếm khi dải Swimlane hiện.

*Đã cân nhắc:* vẽ làn trong cùng dải với histogram (bản đầu). Bỏ vì người dùng muốn bật/tắt và thu gọn làn độc lập với histogram.

### D7. Bấm tên làn
Gọi cùng đường với tích một giá trị trong sidebar Filter: thêm `value_id` vào tập đã tích của cột, rồi `apply_filter_change`. Nếu cột đang ở Hide Mode, thao tác này cũng làm theo Hide Mode như sidebar, và kết quả là ẩn giá trị đó. Chấp nhận như vậy cho nhất quán.

### D8. `MAX_DISTINCT` từ 500 lên 1000, dùng chung cho Filter và Lane By
Chỉ đổi hằng số trong `src/filter_index.rs`. Mọi nơi khác đã đọc hằng số, kể cả test. Mảng `row_ids` vẫn là `u16`, nên vẫn dư chỗ (tối đa 65.534 giá trị). Ngưỡng dùng chung vì Lane By dùng lại đúng `row_ids` của Filter: cột lọc được thì chia làn được, và ngược lại.

*Đã cân nhắc:*
- Giữ 500 cho Filter và có ngưỡng riêng cao hơn cho Lane By. Bỏ vì phải giữ `row_ids` cho cả cột Filter đã coi là tràn, và hai ngưỡng thì khó giải thích.
- Bỏ hẳn ngưỡng. Bỏ vì cột định danh (`request_id`, `trace_id`) sẽ giữ `row_ids` và danh sách giá trị khổng lồ mà không ai dùng.

### D9. Tên "Log Name", công tắc theo Log Space, nơi lưu (quyết định khi áp dụng)
- Code đã có Stream Column (`Source::Stream`, key `stream`) cho stdout/stderr của Remote Command. Cột mới vì vậy tên là **Log Name Column**: `Source::LogName`, key `log_name`, `PinnableColumn::LogName`, tiêu đề `LOG NAME`, hàm `rotation_name::log_name`.
- Unified Canvas hiện mọi cột và mục Columns chỉ điều khiển Focus Pane. Để "mặc định ẩn" có nghĩa trong Parsed View, công tắc Log Name nằm trên Log Space (`LogSpace::log_name_column`) và hiện/ẩn cột ở cả Canvas lẫn Focus Pane.
- `lane_by` lưu trên `LogSpace` (và `file_lane_by` cho File View), cạnh `filter`/`file_filters`, không qua `state.rs`.
- `count_buckets` nhận `lanes: Option<LaneIds { ids, values }>` thay cho `&ColumnIndex`, vì lượt đếm chạy trên worker: `row_ids` được chép sang (2 byte mỗi dòng, khoảng 1 ms cho 10 triệu dòng).

## Risks / Trade-offs

- **Bộ nhớ tăng khi có cột 501–1000 giá trị** (2 byte mỗi dòng mỗi cột, cộng `values`/`spellings`). → Với 10 triệu dòng là khoảng 20 MB mỗi cột như vậy. Chấp nhận được, và đo trong `examples/mem_probe.rs`.
- **Danh sách checkbox trong sidebar Filter dài tới 1000 mục.** → Sidebar đã có ô tìm giá trị và danh sách cuộn. Kiểm tra lại độ mượt khi mở cột 1000 giá trị.

- **Có tên bị hiểu nhầm là xoay vòng**, ví dụ `release.2` hay `v1.2`. → Đây là cùng quy tắc mà Generation đang chấp nhận. Người dùng vẫn chia được theo Source Column.
- **Nhiều cột có `row_ids` thưa** (cột từ Scan chưa xong). → Chỉ những cột đã có `ColumnIndex` hoàn chỉnh mới xuất hiện trong menu. Cột đang Scan hiện ở dạng không chọn được, ghi "đang đọc".
- **Mảng phẳng lớn khi zoom rất sâu** (1000 giá trị × nhiều cột). → Số cột thời gian bị chặn bởi chiều rộng dải (tối đa khoảng 400), nên tối đa 400k ô u8.
- **Log Name của Archive Entry** có dấu ` › ` trong tên. → `log_name` tách tại ` › ` cuối cùng, kèm test.

## Migration Plan

Không cần migration dữ liệu. View cũ không có `lane_by` thì dùng `Default`. Công tắc Log Name Column mặc định tắt.

## Open Questions

- Khi chọn Lane By là Source Column trong một Log Space có file bị cắt khúc, có nên gợi ý chuyển sang Log Name Column không?
