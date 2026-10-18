## Why

Người dùng đánh dấu log để đọc và chụp màn hình gửi người khác, nhưng Marker hiện chỉ là một ký hiệu nhỏ ở Gutter (◆ ⭐ 📌), chụp ra gần như không thấy. Người dùng cũng muốn mọi hàng có một giá trị nào đó, ví dụ `module = DB`, nổi lên bằng một màu riêng để lướt nhanh.

Ngoài ra Parsed View đang hiện text khác với file: `err` thành `ERROR`, timestamp bị định dạng lại, dòng không đọc được định dạng bị tự điền module `app` và level đoán từ chữ trong dòng. Người dùng gõ `err` trong Raw View mà sang Parsed View thấy `ERROR` thì mất niềm tin vào app. Việc này cũng khiến Marker không thể nằm đúng chỗ ở cả hai view.

## What Changes

**Parsed View hiện đúng text lưu trong file**
- Mọi ô lấy từ dòng hiện đúng ký tự trong dòng đó: level, timestamp, và giá trị JSON (chỉ bỏ cặp ngoặc kép bao ngoài, giữ nguyên escape). Gutter và Source Column không thuộc quy định này.
- App vẫn hiểu nghĩa giá trị để sắp xếp, lọc, đặt Level Marker và đọc Time Range, nhưng không viết lại text hiển thị.
- Không tự điền giá trị không có trong dòng: bỏ module mặc định `app` và level đoán từ chữ trong dòng.
- Mỗi dòng lưu trong file là một hàng riêng, kể cả dòng trống. Số dòng khớp với Raw View.
- **Continuation Line** (ví dụ dòng stack trace) là dòng không có timestamp và nằm sau một dòng có timestamp; dòng đó gọi là **Parent Line**. Continuation Line vẫn là một hàng riêng: ô MESSAGE hiện text của nó, các ô khác hiện `_` màu nhạt. Filter, Time Range và Highlight Rule dùng giá trị của Parent Line, nên lọc `level = ERROR` vẫn hiện cả stack trace. Số đếm của Filter Value và Level Marker chỉ tính Parent Line.
- Chọn text được qua nhiều ô và nhiều hàng như trong Raw View. Kéo ra mép thì canvas tự cuộn. ⌘C chép đúng text trong file của vùng chọn, bỏ qua hàng bị Filter ẩn.
- Filter Value của cột LEVEL hiện nguyên văn: `E` và `ERROR` là hai giá trị riêng, chỉ gộp các giá trị khác nhau về hoa thường.
- Time Range nhận mọi định dạng thời gian mà parser đọc được, với placeholder lấy từ timestamp đầu tiên.

**Marker tô màu đoạn text**
- **BREAKING**: Marker là màu người dùng tô lên **đoạn text đã chọn** trong một dòng, thay cho các loại Bookmark/Important/Note gắn vào cả dòng. Chọn qua nhiều dòng thì mỗi dòng nhận một Marker.
- Chọn text xong sẽ hiện thanh màu nổi (màu dùng gần đây, mở cả bảng màu, color picker). Chuột phải lên vùng chọn có "Mark with color". Chuột phải lên text đã tô có "Change color" và "Remove marker". Tô chồng lên Marker cũ thì phần chồng nhận màu mới.
- Marker hiện ở cả Raw View và Parsed View, và luôn nằm trên màu của Highlight Rule. Overview ruler hiện vạch màu cho dòng có Marker, bấm vào để nhảy tới.
- **BREAKING**: bỏ panel Marker ("Bookmarks & Markers") và ghi chú (`note`) của Marker.
- Ký hiệu error/warning tự động ở Gutter đổi tên thành **Level Marker**. Đây không phải Marker và không có trong danh sách nào.

**Highlight Rule tô màu hàng theo cột**
- Mỗi Highlight Rule là điều kiện trên **một cột** đi kèm một màu. Điều kiện viết giống Filter: tick Filter Value có sẵn, hoặc "Add value" để khớp chứa chuỗi; các giá trị trong một rule nối bằng OR. Dùng được trên mọi Filterable Column, kể cả Source Column; trên MESSAGE thì chỉ dùng "Add value"; không dùng TIMESTAMP.
- Chỉ áp dụng trong Parsed View, gồm cả Log Space và File View. Hàng khớp được tô nền nhạt, ô thỏa điều kiện đậm hơn. Rule ở trên trong danh sách thắng.
- Highlight Rule thuộc Project, tự nhớ, có công tắc bật/tắt.
- Mục **Highlights** mới trong context sidebar, thay cho mục Marker: mỗi rule có ô màu, cột, giá trị, số hàng khớp, công tắc, nút xóa, kéo thả đổi thứ tự, và nút "+ Add rule".
- Chuột phải lên một ô có "Highlight rows where <cột> = <giá trị>". Lệnh này tạo một Highlight Rule bình thường. Vẫn không có menu nhanh để lọc.

**Highlight Palette**
- Bảng 10 màu × 3 sắc độ (nhạt, vừa, đậm), đọc rõ ở cả nền sáng lẫn tối, dùng chung cho Marker và Highlight Rule. Có color picker tự do; màu tự chọn được nhớ vào hàng "Recent".

## Capabilities

### New Capabilities
- `parsed-view-fidelity`: text trong Parsed View đúng như lưu trong file, mỗi dòng một hàng, Continuation Line dùng giá trị của Parent Line, chọn và chép nhiều ô/hàng.
- `text-marker`: Marker tô màu đoạn text, thanh màu nổi, đổi màu/gỡ, hiện ở hai view, overview ruler, Level Marker.
- `highlight-rule`: Highlight Rule theo cột, thứ tự ưu tiên, mục Highlights, tạo nhanh từ ô, phạm vi Project.
- `highlight-palette`: bảng màu cố định nhiều sắc độ, color picker, màu gần đây.

### Modified Capabilities
- `log-filter`: Time Range nhận mọi định dạng thời gian đọc được; context sidebar gồm Filter, Highlights và Inspector (không còn Marker); menu chuột phải trên ô chỉ có "Highlight rows where…".
- `raw-view`: Marker trong Raw View là màu tô trên đoạn text, không còn gắn cả dòng.

> `raw-view` nằm trong change `add-log-space` chưa archive. Cần archive `add-log-space` trước change này.

## Impact

- **Parser:** [text.rs](src/parser/text.rs) và [event.rs](src/parser/event.rs) giữ vị trí ký tự của từng trường trong dòng gốc. Level và timestamp hiển thị lấy từ đoạn gốc thay vì `level.as_str()` hay timestamp đã định dạng. Nhận diện Continuation Line và gắn `parent`; bỏ module `app` và level đoán; giữ dòng trống.
- **Model:** [model.rs](src/model.rs) bỏ `marker: Option<MarkerType>`, `note`, `sub_lines`, `is_expanded`, `marker_color`; thêm danh sách Marker theo khoảng ký tự. [marker.rs](src/marker.rs) thay `MarkerType` bằng Level Marker và Marker màu. [log_space.rs](src/workspace/log_space.rs) đổi `StoredMarker` (bỏ loại và `note`, thêm khoảng ký tự và màu, giữ hash dòng để bỏ Marker khi dòng scratch bị sửa).
- **UI:** [canvas.rs](src/ui/canvas.rs) thêm chọn text nhiều ô/hàng, vẽ Marker và màu Highlight Rule, vẽ `_` nhạt cho ô của Continuation Line, bỏ nút mở rộng của dữ liệu mẫu; [raw.rs](src/app/raw.rs) tô Marker theo vùng chọn; thanh màu nổi và Highlight Palette mới; mục Highlights thay [marker_panel.rs](src/app/view/marker_panel.rs); `ContextSection::Marker` thành `Highlights` trong [state.rs](src/workspace/state.rs); [inspector.rs](src/ui/inspector.rs) bỏ phần `sub_lines` và note.
- **Lưu trữ:** Highlight Rule lưu theo Project trong session; Marker cũ (theo dòng) không được chuyển đổi; bump version session.
- **Test:** sửa test dùng `MarkerType`, `sub_lines` và mẫu trong [sample.rs](src/sample.rs); thêm test cho text nguyên văn, mỗi dòng một hàng, nhận diện Continuation Line, lọc và tô theo Parent Line, chép vùng chọn, Marker theo khoảng ký tự, ưu tiên Highlight Rule, Time Range nhiều định dạng.
- **Tài liệu:** [CONTEXT.md](CONTEXT.md) đã cập nhật (Marker, Level Marker, Highlight Rule, Highlight Palette; rule 21, 30–35); [ADR-0007](docs/adr/0007-parsed-view-shows-stored-text.md) ghi lại quyết định.
