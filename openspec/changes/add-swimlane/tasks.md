## 0. Điều kiện trước

- [x] 0.1 Change `add-time-histogram` đã được áp dụng (`TimeAxis`, `count_buckets`, dải trong `src/ui/time_histogram.rs`)

## 1. Ngưỡng giá trị riêng biệt

- [x] 1.1 `MAX_DISTINCT` 500 → 1000 trong `src/filter_index.rs`; chạy `tests/log_filter_test.rs`; thêm test cột 800 giá trị vẫn lọc được
- [x] 1.2 Đổi 500 thành 1000 trong `CONTEXT.md` (Filterable Column, quy tắc 15), ADR 0005, ADR 0016; đo bộ nhớ với một cột 1000 giá trị trong `examples/mem_probe.rs`; kiểm tra sidebar Filter mở cột 1000 giá trị vẫn mượt
  - Đã đo: bộ nhớ +2 byte/dòng (33 so với 31 byte/dòng). Sidebar Filter lúc đầu chậm: mở cột 1000 giá trị mất ~40 ms mỗi khung hình vì dựng đủ 1000 Checkbox. Đã ảo hoá danh sách giá trị dài (`uniform_list` trong `src/app/view/filter_panel.rs`, từ 12 giá trị): còn ~2,5 ms, chỉ dựng ~10 dòng đang thấy; test `tests/filter_value_list_test.rs`.

## 2. Quy tắc tên dùng chung

- [x] 2.1 Chuyển `rotation`, `rotated_base`, `RotationKey`, `strip_rotation_suffix` sang `src/rotation_name.rs`; `src/remote/rotated.rs` re-export; các test hiện có vẫn qua
- [x] 2.2 `log_name(source)`: giữ phần trước tên file (`/`, `:`, ` › ` cuối cùng), bỏ hậu tố nén rồi hậu tố xoay vòng; test các scenario của "Log Name của Source File" và Archive Entry

## 3. Log Name Column

- [x] 3.1 Thêm `Source::LogName` và `log_name_column(files, rows, row_of)` cạnh `source_column` trong `src/filter_index/merge.rs`; test "Thêm khúc mới"
- [x] 3.2 Log Name Column trong Parsed View: thứ tự ngay sau Source Column, ghim được, mặc định ẩn
- [x] 3.3 Công tắc Log Name Column trong mục Columns, nhớ theo Log Space; Filter giữ hiệu lực khi ẩn
- [x] 3.4 Tính lại Log Name Column khi thêm/bớt file hoặc đổi tên Remote File
- [x] 3.5 Test Filter theo Log Name Column: tích `api.log` thì Focus Pane gồm cả `api.log.1`

## 4. Đếm theo làn

- [x] 4.1 `count_buckets` nhận `lanes: Option<&ColumnIndex>`, ghi `lane_level` theo mảng phẳng và `lane_total`; bỏ qua Continuation Line; `MISSING` vào "(trống)"
- [x] 4.2 `LaneSet::choose(lane_total)`: 8 giá trị nhiều nhất (hoà thì theo thứ tự giá trị), "Khác (n)", có/không "(trống)"; test "Nhiều thread"
- [x] 4.3 `Buckets::append` cộng vào làn đã có, giá trị mới vào "Khác" hoặc chỗ trống; test "Giá trị mới khi tail"
- [x] 4.4 Làn đếm mọi dòng, không theo Filter; test "Filter không làm mất làn"

## 5. Lane By

- [x] 5.1 `LaneBy = None | Column(ColumnKey) | Default` trong trạng thái view, lưu và khôi phục cạnh Filter
- [x] 5.2 Giải `Default`: Log Name Column khi có ≥ 2 giá trị, nếu không thì `None`; quay về `Default` khi cột đã chọn không còn
- [x] 5.3 Gợi ý cột: MODULE, rồi Field Column `service`/`module`/`component`/`thread`/`host`/`pod`; test các scenario của "Lane By mặc định"
- [x] 5.4 Cột tràn `MAX_DISTINCT` hoặc đang Scan thì không chọn được, kèm lý do

## 6. Vẽ và tương tác

- [x] 6.1 Vẽ làn dưới histogram: 14px mỗi làn, vạch theo level cao nhất, cột tên khoảng 90px dùng chung làm lề trái cho histogram
- [x] 6.2 Zoom, khung vùng đang xem và dải Time Range trải qua cả các làn; thu gọn dải thì ẩn làn
- [x] 6.3 Menu "Lane by" (popover góc trên trái) với "Không chia", gợi ý, cột không chọn được
- [x] 6.4 Bấm tên làn → tích giá trị trong Filter của cột (theo Hide Mode như sidebar); "Khác" và "(trống)" không làm gì
- [x] 6.5 Bấm vạch → dòng đầu tiên của giá trị đó trong cột thời gian đó, `Origin::Jump`
- [x] 6.6 Tooltip tên làn đầy đủ khi bị cắt

## 7. Docs và kiểm tra

- [x] 7.1 Thêm Log Name Column, Swimlane, Lane By vào `CONTEXT.md` (kèm _Avoid_)
- [x] 7.2 Đo `count_buckets` có làn trên 10 triệu dòng trong `examples/perf_probe.rs`, so với khi không có làn
- [ ] 7.3 Chạy ứng dụng: Log Space `api.log*` + `db.log*`, file `message.0`/`message.1` có field `service`, Live Log Space đang tail, Archive Entry
  - Đã chạy trên app thật: Log Space `api.log` + `api.log.1` + `db.log` + `cache.log` (3 làn, vạch đúng chỗ, cột LOG NAME), và cột `thread` 1000 giá trị (8 làn + "Other (992 values)"). Chưa chạy: Live Log Space đang tail, `message.0`/`message.1`, Archive Entry (đã có test tự động).
