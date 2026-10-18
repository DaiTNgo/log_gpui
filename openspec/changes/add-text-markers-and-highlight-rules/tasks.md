## 1. Chuẩn bị

- [x] 1.1 Chốt thuật ngữ và rule trong `CONTEXT.md` (Marker, Level Marker, Highlight Rule, Highlight Palette; rule 21, 30–35)
- [x] 1.2 Viết ADR-0007 (Parsed View hiện text gốc; Marker tô text, Highlight Rule tô hàng)
- [ ] 1.3 Archive change `add-log-space` trước, để delta `raw-view` và `log-filter` áp lên đúng spec
- [ ] 1.4 Xác nhận với người dùng: hàng tiếp nối đi theo dòng cha khi lọc và tô (design, Quyết định 2)

## 2. Parser: text gốc và khoảng ký tự

- [ ] 2.1 Thêm `spans: HashMap<String, Range<usize>>` vào `LogEvent` cho `timestamp`, `level`, `module`, `message` và trường JSON (chuỗi: phần trong ngoặc kép, giữ escape; số và bool: nguyên token)
- [ ] 2.2 Text hiển thị của mọi ô lấy từ `raw_line[span]`; bỏ `time_str` đã định dạng và việc hiển thị `level.as_str()`; giữ `timestamp: f64` và `LogLevel` cho sắp xếp, lọc, Level Marker
- [ ] 2.3 Dòng tiếp nối thành `LogEntry` riêng (`has_time = false`, `parent`, span `message` phủ cả dòng); bỏ `sub_lines` và `is_expanded`
- [ ] 2.4 Gộp theo timestamp (`add-log-space`) giữ dòng tiếp nối liền sau dòng cha theo `parent`
- [ ] 2.5 Test: level viết tắt, timestamp nguyên văn, JSON có escape, stack trace thành nhiều hàng với số dòng đúng, Raw và Parsed hiện cùng text

## 3. Filter và Search sau khi đổi hàng

- [ ] 3.1 Hàng tiếp nối hiện khi và chỉ khi dòng cha được Filter hiện
- [ ] 3.2 Filter Value của LEVEL lấy text nguyên văn, chỉ gộp khác hoa thường
- [ ] 3.3 Time Range nhận mọi định dạng thời gian parser đọc được; placeholder lấy timestamp đầu tiên của view
- [ ] 3.4 Chạy lại toàn bộ test Search và Filter; sửa chỗ phụ thuộc chỉ số hàng hoặc `sub_lines`
- [ ] 3.5 Test: lọc `ERROR` vẫn hiện stack trace, Search khớp trên đúng dòng tiếp nối, `E` và `ERROR` là hai Filter Value, Time Range nhiều định dạng

## 4. Chọn và chép text

- [ ] 4.1 Mô hình `TextSelection { anchor, head }` theo `(row_pos, byte trong raw_line)`; hàm tính phần được chọn của từng hàng và từng ô
- [ ] 4.2 Hit-test ký tự trong ô, dùng kết quả shaping của gpui để đúng với glyph rộng
- [ ] 4.3 Kéo chuột chọn qua nhiều ô và hàng trên canvas, tự cuộn khi kéo ra mép; bấm không kéo vẫn chọn hàng
- [ ] 4.4 ⌘C chép text gốc của vùng chọn, bỏ hàng bị Filter ẩn
- [ ] 4.5 Raw View dùng cùng mô hình vùng chọn
- [ ] 4.6 Test thuần dữ liệu: phần chọn của hàng đầu, giữa, cuối; chép bỏ hàng ẩn; giao vùng chọn với span của ô

## 5. Highlight Palette

- [ ] 5.1 Token 10 màu × 3 sắc độ, có giá trị riêng cho theme sáng và tối
- [ ] 5.2 Kiểu `HighlightColor` (màu palette hoặc hex tự chọn); chọn màu chữ theo độ tương phản
- [ ] 5.3 Danh sách tối đa 8 màu gần đây, lưu trong session
- [ ] 5.4 UI palette: lưới 30 ô, hàng Recent, nút color picker
- [ ] 5.5 Test: màu chữ tương phản, hàng Recent đúng thứ tự và giới hạn

## 6. Marker theo đoạn text

- [ ] 6.1 Đổi `StoredMarker` thành `{ line, range, color, line_hash }`; bỏ `MarkerType::{Bookmark, Important, Note}`, `note`, `cycle`, `toggle_bookmark`, `marker_color`
- [ ] 6.2 Thao tác thuần dữ liệu: thêm Marker (cắt Marker chồng lên), gỡ Marker tại vị trí, đổi màu; vùng chọn nhiều dòng tạo một Marker mỗi dòng
- [ ] 6.3 Level Marker thành hàm thuần theo `LogLevel`, chỉ vẽ ở Gutter
- [ ] 6.4 Thanh màu nổi cạnh vùng chọn; menu chuột phải "Mark with color", "Change color", "Remove marker"
- [ ] 6.5 Vẽ Marker trong Parsed View (phần giao với span của ô) và trong Raw View; thứ tự lớp: nền → Highlight Rule → hàng chọn → Match → Marker → vùng chọn
- [ ] 6.6 Overview ruler: một vạch mỗi dòng có Marker, bấm để cuộn tới
- [ ] 6.7 Bỏ panel Marker (`marker_panel.rs`) và phần note trong Inspector
- [ ] 6.8 Giữ việc bỏ Marker khi dòng scratch bị sửa (so `line_hash`)
- [ ] 6.9 Test: tô một đoạn, tô qua nhiều dòng, tô chồng, gỡ, Marker giống nhau giữa hai view, Marker bị bỏ khi sửa dòng scratch, khôi phục sau khi khởi động lại

## 7. Highlight Rule

- [ ] 7.1 `HighlightRule { id, column, values, custom, color, enabled }` và `Project.highlight_rules` có thứ tự; lưu trong session
- [ ] 7.2 So khớp dùng lại hàm của Filter; cột thiếu giá trị không khớp; MESSAGE chỉ Custom Value; không cho TIMESTAMP
- [ ] 7.3 Tính màu cho hàng đang hiện (rule bật đầu tiên khớp thắng), có cache theo phiên bản rule và dữ liệu; hàng tiếp nối lấy màu dòng cha
- [ ] 7.4 Vẽ nền hàng sắc độ nhạt, ô thỏa điều kiện sắc độ vừa; chỉ trong Parsed View
- [ ] 7.5 `ContextSection::Marker` đổi thành `Highlights`; danh sách rule với ô màu, cột, giá trị, số hàng khớp, công tắc, xóa, kéo thả đổi thứ tự
- [ ] 7.6 Form "+ Add rule" dùng lại UI chọn giá trị của Filter
- [ ] 7.7 Menu chuột phải trên ô "Highlight rows where <cột> = <giá trị>" (MESSAGE dùng text đang chọn; không có trên TIMESTAMP)
- [ ] 7.8 Test: OR trong một rule, thứ tự ưu tiên, tắt rule, rule dùng chung giữa các Log Space, rule trên Source Column, Raw View không tô

## 8. Lưu trữ và dọn dẹp

- [ ] 8.1 Bump version session; Marker cũ và `note` bị bỏ khi đọc
- [ ] 8.2 Sửa `sample.rs` và các test dùng `MarkerType`, `sub_lines`, `is_expanded`
- [ ] 8.3 `cargo build` và `cargo test` qua hết

## 9. Kiểm tra trên app

- [ ] 9.1 Chạy app thử: level và timestamp nguyên văn, stack trace nhiều hàng, kéo chọn nhiều hàng và chép, tô Marker ở hai view, tạo Highlight Rule bằng form và bằng chuột phải, đổi thứ tự rule, đổi theme sáng/tối, chụp màn hình thấy rõ màu
- [ ] 9.2 Rà soát lại các spec so với hành vi thực tế
