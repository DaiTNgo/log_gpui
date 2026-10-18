## 1. Chuẩn bị

- [x] 1.1 Chốt thuật ngữ và rule trong `CONTEXT.md` (Marker, Level Marker, Highlight Rule, Highlight Palette; rule 21, 30–35)
- [x] 1.2 Viết ADR-0007 (Parsed View hiện text gốc; Marker tô text, Highlight Rule tô hàng)
- [x] 1.3 Archive change `add-log-space` trước, để delta `raw-view` và `log-filter` áp lên đúng spec
- [x] 1.4 Chốt Continuation Line: một hàng riêng, ô hiện `_` nhạt, lọc và tô theo Parent Line, chỉ đếm Parent Line (`CONTEXT.md` rule 19, 30)

## 2. Parser: text gốc và khoảng ký tự

- [x] 2.1 Thêm `spans: HashMap<String, Range<usize>>` vào `LogEvent` cho `timestamp`, `level`, `module`, `message` và trường JSON (chuỗi: phần trong ngoặc kép, giữ escape; số và bool: nguyên token)
- [x] 2.2 Text hiển thị của mọi ô lấy từ `raw_line[span]`; bỏ `time_str` đã định dạng và việc hiển thị `level.as_str()`; giữ `timestamp: f64` và `LogLevel` cho sắp xếp, lọc, Level Marker
- [x] 2.3 Bỏ module mặc định `app` và level đoán từ cả dòng; dòng không đọc được định dạng không có level hay module
- [x] 2.4 Giữ dòng trống thành entry, không `continue`
- [x] 2.5 Lượt nhận diện Continuation Line: dòng không có timestamp sau một dòng có timestamp nhận `parent` là số dòng của Parent Line, chỉ có span `message`; dòng trước timestamp đầu tiên không có `parent`
- [x] 2.6 Gộp theo timestamp (`add-log-space`) giữ Continuation Line liền sau Parent Line theo `parent`
- [x] 2.7 Test: level viết tắt, timestamp nguyên văn, JSON có escape, không có `app` hay level đoán, dòng trống thành hàng, nhận diện Continuation Line (có và không có Parent Line), Raw và Parsed hiện cùng text

## 3. Filter và Search sau khi đổi hàng

- [x] 3.1 `value_of(row, column)` đọc giá trị qua `parent` cho Filter, Time Range và Highlight Rule; Continuation Line hiện khi và chỉ khi Parent Line hiện
- [x] 3.2 Filter Value của LEVEL lấy text nguyên văn, chỉ gộp khác hoa thường; số đếm không tính Continuation Line
- [x] 3.3 Time Range nhận mọi định dạng thời gian parser đọc được; placeholder lấy timestamp đầu tiên của view
- [x] 3.4 Chạy lại toàn bộ test Search và Filter; sửa chỗ phụ thuộc chỉ số hàng hoặc `sub_lines`
- [x] 3.5 Test: lọc `ERROR` vẫn hiện stack trace, Search khớp trên đúng Continuation Line và không khớp `_`, `ERROR` kèm 20 dòng stack trace đếm là 1, `E` và `ERROR` là hai Filter Value, Time Range nhiều định dạng

## 4. Chọn và chép text

- [x] 4.1 Mô hình `TextSelection { anchor, head }` theo `(row_pos, byte trong raw_line)`; hàm tính phần được chọn của từng hàng và từng ô
- [x] 4.2 Hit-test ký tự trong ô, dùng kết quả shaping của gpui để đúng với glyph rộng
- [x] 4.3 Kéo chuột chọn qua nhiều ô và hàng trên canvas, tự cuộn khi kéo ra mép; bấm không kéo vẫn chọn hàng
- [x] 4.4 ⌘C chép text gốc của vùng chọn, bỏ hàng bị Filter ẩn
- [x] 4.5 Raw View dùng cùng mô hình vùng chọn
- [x] 4.6 Test thuần dữ liệu: phần chọn của hàng đầu, giữa, cuối; chép bỏ hàng ẩn; giao vùng chọn với span của ô

## 5. Highlight Palette

- [x] 5.1 Token 10 màu × 3 sắc độ, có giá trị riêng cho theme sáng và tối
- [x] 5.2 Kiểu `HighlightColor` (màu palette hoặc hex tự chọn); chọn màu chữ theo độ tương phản
- [x] 5.3 Danh sách tối đa 8 màu gần đây, lưu trong session
- [x] 5.4 UI palette: lưới 30 ô, hàng Recent, nút color picker
- [x] 5.5 Test: màu chữ tương phản, hàng Recent đúng thứ tự và giới hạn

## 6. Marker theo đoạn text

- [x] 6.1 Đổi `StoredMarker` thành `{ line, range, color, line_hash }`; bỏ `MarkerType::{Bookmark, Important, Note}`, `note`, `cycle`, `toggle_bookmark`, `marker_color`
- [x] 6.2 Thao tác thuần dữ liệu: thêm Marker (cắt Marker chồng lên), gỡ Marker tại vị trí, đổi màu; vùng chọn nhiều dòng tạo một Marker mỗi dòng
- [x] 6.3 Level Marker thành hàm thuần theo `LogLevel`, chỉ vẽ ở Gutter của Parent Line và dòng đứng riêng
- [x] 6.4 Thanh màu nổi cạnh vùng chọn; menu chuột phải "Mark with color", "Change color", "Remove marker"
- [x] 6.5 Vẽ `_` nhạt cho ô của Continuation Line (không chọn được, không nhận Marker, không chép); vẽ Marker trong Parsed View (phần giao với span của ô) và trong Raw View; thứ tự lớp: nền → Highlight Rule → hàng chọn → Match → Marker → vùng chọn
- [x] 6.6 Overview ruler: một vạch mỗi dòng có Marker, bấm để cuộn tới
- [x] 6.7 Bỏ panel Marker (`marker_panel.rs`) và phần note trong Inspector
- [x] 6.8 Giữ việc bỏ Marker khi dòng scratch bị sửa (so `line_hash`)
- [x] 6.9 Test: tô một đoạn, tô qua nhiều dòng, tô chồng, gỡ, Marker giống nhau giữa hai view, Marker bị bỏ khi sửa dòng scratch, khôi phục sau khi khởi động lại

## 7. Highlight Rule

- [x] 7.1 `HighlightRule { id, column, values, custom, color, enabled }` và `Project.highlight_rules` có thứ tự; lưu trong session
- [x] 7.2 So khớp dùng lại hàm của Filter; cột thiếu giá trị không khớp; MESSAGE chỉ Custom Value; không cho TIMESTAMP
- [x] 7.3 Tính màu cho hàng đang hiện (rule bật đầu tiên khớp thắng), có cache theo phiên bản rule và dữ liệu; Continuation Line lấy màu theo giá trị của Parent Line
- [x] 7.4 Vẽ nền hàng sắc độ nhạt, ô thỏa điều kiện sắc độ vừa; chỉ trong Parsed View
- [x] 7.5 `ContextSection::Marker` đổi thành `Highlights`; danh sách rule với ô màu, cột, giá trị, số hàng khớp, công tắc, xóa, kéo thả đổi thứ tự
- [x] 7.6 Form "+ Add rule" dùng lại UI chọn giá trị của Filter
- [x] 7.7 Menu chuột phải trên ô "Highlight rows where <cột> = <giá trị>" (MESSAGE dùng text đang chọn; không có trên TIMESTAMP và trên ô `_`)
- [x] 7.8 Test: OR trong một rule, thứ tự ưu tiên, tắt rule, rule dùng chung giữa các Log Space, rule trên Source Column, Raw View không tô

## 8. Lưu trữ và dọn dẹp

- [x] 8.1 Bump version session; Marker cũ và `note` bị bỏ khi đọc
- [x] 8.2 Sửa `sample.rs` và các test dùng `MarkerType`, `sub_lines`, `is_expanded`
- [x] 8.3 `cargo build` và `cargo test` qua hết

## 9. Kiểm tra trên app

- [ ] 9.1 Chạy app thử: level và timestamp nguyên văn, stack trace nhiều hàng với `_` nhạt, lọc `ERROR` vẫn thấy stack trace, kéo chọn nhiều hàng và chép, tô Marker ở hai view, tạo Highlight Rule bằng form và bằng chuột phải, đổi thứ tự rule, đổi theme sáng/tối, chụp màn hình thấy rõ màu
- [ ] 9.2 Rà soát lại các spec so với hành vi thực tế
