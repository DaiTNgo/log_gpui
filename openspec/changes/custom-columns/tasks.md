> **Đã bị thay thế một phần (ADR-0015).** Extraction Rule, "Make column…" và Column Label đã bị gỡ; Log Format là cách duy nhất để định nghĩa cột, và mục Columns chỉ bật/tắt cột của Focus Pane. Các task về Extraction Rule và Column Label đã làm xong rồi bị gỡ. Các phần nhắc đến chúng dưới đây chỉ còn là lịch sử, không còn đúng với app. Xem `docs/adr/0015-log-format-is-the-only-way-to-define-columns.md` và `CONTEXT.md`.

## 0. Giai đoạn 0 — Nền Field Column

- [x] 0.1 Đo bộ nhớ của `Spans` với 10 trường × 1.000.000 dòng. Ghi kết quả vào design.md, và chọn giữ `(&'static str, Range<usize>)` hay nén thành `(u16, u32, u32)`
- [x] 0.2 Viết test hồi quy cho Filter và Highlight Rule trên trường JSON (tên cột, Filter Value, số đếm, Continuation Line) trước khi đổi gì
- [x] 0.3 Lưu trường JSON dạng span trong `Spans` (khoảng byte của giá trị trong dòng), bỏ chép text vào `json_fields`; `column_value` đọc qua span. Test hồi quy 0.2 phải vẫn qua
- [x] 0.4 `src/filter_index.rs`: đổi `Source::Json` thành `Source::Field(FieldKey)`; `FilterIndex` đếm thêm Coverage (số Parent Line có giá trị) cho mỗi cột
- [x] 0.5 `src/column.rs`: thêm `PinnableColumn::Field(FieldKey)`, `FieldMaxChars` theo tên trường, thứ tự cột của view (cố định → Source → Field Column), và thứ tự ghim theo thứ tự cột
- [x] 0.6 `src/ui/canvas.rs` và `src/ui/cells.rs`: vẽ số Field Column bất kỳ, ô cắt từ `raw_line` theo span, ô trống khi không có giá trị, `_` cho Continuation Line, không vẽ cột không có giá trị trong view
- [x] 0.7 Column Header của Field Column: Pin Toggle, Column Resize Handle, Auto-fit, và Coverage làm tròn xuống kèm tooltip số dòng
- [x] 0.8 Column Label: nhấp đúp để sửa ngay tại chỗ (Enter, Esc, để trống là trả về), chặn trùng tên, lưu theo Project với `#[serde(default)]`; hiện Label ở Column Header, Filter, mục Highlights và mục Columns; lỗi trùng tên chỉ hiện khi Enter hoặc rời ô, kèm icon, và ẩn khi gõ tiếp
- [x] 0.9 Mục Columns trong context sidebar (cạnh Filter, Highlights, Inspector): nhóm *Built-in* và *Detected* (lúc này chỉ có trường JSON); mỗi cột có Label, số dòng hoặc `0 lines here`, công tắc (trừ MESSAGE), menu `⋯` Rename; nhóm trống thì ẩn; trạng thái lưu theo Project
- [x] 0.10 Ẩn cột: mọi cột trừ MESSAGE và Gutter, kể cả cột cố định; cột ẩn vẫn được đọc cho Filter, Time Range, Highlight Rule và Level Marker; công tắc Source Column dùng chung trạng thái với Quick Settings
- [x] 0.11 Kéo đổi thứ tự Field Column trong mục Columns; cột mới thêm vào cuối; thứ tự vẽ tách khỏi thứ tự ưu tiên của rule 38
- [x] 0.12 Nhóm *Detected*: xếp theo số dòng, tối đa 50 kèm ô tìm, mặc định tắt
- [x] 0.13 Menu chuột phải trên Column Header: "Hide column" (không có trên MESSAGE), "Rename", "Manage columns…" (mở mục Columns và làm nổi dòng của cột)
- [x] 0.14 Test GPUI: bật trường JSON trong mục Columns thì cột hiện; ẩn LEVEL mà Filter `level` vẫn chạy; kéo đổi thứ tự; ghim; Auto-fit; Coverage `99%`; đổi Label không làm hỏng Filter và Highlight Rule; Continuation Line hiện `_`; Quick Settings không có danh sách cột

## 1. Giai đoạn 1 — Extraction Rule

- [x] 1.1 Thêm `LineConfig` (`src/parser/text.rs`) và truyền vào `parse_events` cùng `LineParser`; cấu hình rỗng phải cho kết quả y như hiện nay (test so sánh trên các fixture sẵn có)
- [x] 1.2 `CompiledExtractionRule`: kiểm tra đúng một capture group, đặt `size_limit`, chạy trên cả dòng, áp thứ tự ưu tiên của rule 38; dùng `RegexSet` để lọc nhanh
- [x] 1.3 Đọc lại span ở nền theo khối (`src/par.rs`) khi rule đổi; dựng lại `FilterIndex`; hiện tiến độ trên Column Header; dòng live mới dùng rule mới ngay
- [x] 1.4 Bộ đoán cách khớp từ vùng chọn: key đứng trước, nhóm ngoặc thứ N, từ thứ N của message hoặc của dòng. Chỉ giữ ứng viên lấy ra đúng đoạn chọn; xếp theo Coverage trên mẫu 1.000 dòng; mô tả dễ đọc cho mỗi ứng viên. Unit test cho từng loại
- [x] 1.5 Gợi ý Name (key, hình dạng `ip`/`uuid`/`duration`), chặn tên của cột cố định, báo "Values will join column X" khi trùng Field Column
- [x] 1.6 Dialog New Column: Name, lựa chọn cách khớp, Edit regex, Add column; Esc đóng dialog
- [x] 1.7 Preview bằng dòng thật: tối đa 3 dòng khớp có đoạn được lấy tô sáng (dòng gốc đứng đầu), tối đa 1 dòng "not matched" chọn từ dòng cùng kiểu với dòng gốc, cắt quanh đoạn tô sáng kèm `…`, bấm dòng thì view nhảy tới mà dialog vẫn mở; kèm số dòng, % và 5 giá trị thường gặp
- [x] 1.8 Luật lỗi dùng chung cho ô regex và ô template (dùng lại ở 2.5): biên dịch mỗi lần gõ; chưa hợp lệ thì giữ Preview hợp lệ gần nhất ở dạng mờ kèm nhãn "Showing last valid pattern" và tắt nút lưu; lỗi (viền, icon, câu nói cách sửa) chỉ hiện sau 500 ms ngừng gõ hoặc khi rời ô, và ẩn khi gõ tiếp. Name dùng cùng thời điểm báo lỗi
- [x] 1.9 "Make column…" trong `src/ui/cell_menu.rs` và trong menu dòng của Focus Pane; chỉ có khi vùng chọn nằm trong một dòng và không phải `_`
- [x] 1.10 Nhóm *Rules* của mục Columns: mô tả cách khớp, số dòng, công tắc, menu `⋯` Edit, Rename, Delete; thứ tự trong nhóm là thứ tự ưu tiên; lưu theo Project
- [x] 1.11 Filter: Field Column là nguồn cột; điều kiện trên cột không còn được đọc được giữ kèm chú thích "column no longer read"
- [x] 1.12 Empty state của Focus Pane khi Filter dùng cột không còn được đọc: tên cột, lý do, nút "Remove condition" và "Turn column back on"
- [x] 1.13 Test GPUI: bôi đen `alice` trong `user=alice` → Make column… → Preview có dòng tô sáng và dòng "not matched" → cột `user` hiện, lọc được, tô màu được; gõ dở regex thì Preview giữ bản cũ và chưa báo lỗi; tắt rule khi đang lọc thì Focus Pane hiện empty state và "Turn column back on" bật lại được; mở lại Project thì rule vẫn còn

## 2. Giai đoạn 2 — Log Format

- [x] 2.1 Biên dịch template thành regex: `%{TOKEN}`, `%{TOKEN:field}`, named group, khoảng trắng thành `\s+`, escape ký tự khác, neo `^…$`; báo lỗi kèm vị trí. Unit test cho từng Format Token
- [x] 2.2 Format Token `TIMESTAMP` là hợp các dạng mà `read_timestamp` hiểu; trường `timestamp`, `level`, `module`, `message` điền vào cột cố định; dòng không khớp thì dùng Auto-detect
- [x] 2.3 Thư viện Log Format trong cấu hình app (cạnh Server): tạo, sửa, nhân bản, xoá; xoá Log Format đang dùng thì hỏi lại kèm danh sách nơi đang dùng
- [x] 2.4 Mẫu có sẵn: nginx/Apache combined, Python `logging`, log4j `PatternLayout` mặc định; mỗi mẫu có fixture test khớp 100%
- [x] 2.5 Trình soạn Log Format: xem trước 20 dòng của view, tô màu theo trường, đánh dấu dòng không khớp, tỉ lệ khớp, nhảy tới dòng không khớp đầu tiên; dùng luật lỗi của 1.8, thêm gạch chân đúng phần sai của template
- [x] 2.6 Chọn Log Format cho Log Space (ô chọn ở đầu nhóm *From Log Format*) và cho Source File (menu file trong sidebar), lưu theo Project; thứ tự ưu tiên file → Log Space → glob → Auto-detect; glob chỉ so tên file
- [x] 2.7 Nhóm *From Log Format* của mục Columns: liệt kê trường theo thứ tự template (trừ 4 trường cố định), mỗi trường có công tắc hiện/ẩn
- [x] 2.8 Đổi Log Format thì đọc lại file ở nền, dựng lại timeline, giữ Marker; Remote Log Space đọc lại các dòng trong Live Window
- [x] 2.9 Test GPUI: áp mẫu nginx cho `access.log` → có cột `client`, `status`; ẩn `referrer`; xếp xen theo thời gian với file khác; gõ dở `%{IP` thì xem trước giữ bản cũ, ngừng gõ thì gạch chân; Marker giữ sau khi đổi Log Format

## 3. Giai đoạn 3 — Detected Field key=value

- [x] 3.1 Quét `key=value` kiểu logfmt trong span `message`, lấy lần xuất hiện đầu; giá trị có ngoặc kép giữ cả ngoặc kép. Unit test cho `a == b`, key lặp lại, `\"` trong chuỗi
- [x] 3.2 Chỉ đếm số dòng cho trường chưa bật; chỉ lưu span khi trường được bật (đọc lại ở nền khi bật)
- [x] 3.3 Thêm trường `key=value` vào nhóm *Detected* của mục Columns; chỉ là Filterable Column và dùng được cho Highlight Rule khi đã bật
- [x] 3.4 Live: key mới thêm vào danh sách với công tắc tắt; số đếm tăng và giảm theo Live Window; trường hết dòng thì biến mất trừ khi đang bật
- [x] 3.5 Test GPUI: log logfmt → bật `user` trong nhóm *Detected* → cột hiện và lọc được; tắt khi đang lọc thì điều kiện được giữ kèm chú thích

## 4. Hoàn tất

- [x] 4.1 Đo hiệu năng: thêm Extraction Rule và đổi Log Format trên 1.000.000 dòng; view không bị chặn; ghi số liệu vào design.md
- [x] 4.2 Cập nhật CONTEXT.md theo mục "Domain changes" của proposal (thuật ngữ mới gồm mục Columns, rule 2, 3, 17, 21, 23, rule mới 36–40)
- [x] 4.3 Thêm ADR 0012 "Users Define Columns; Regex Stays Out Of Search And Filter" và ghi chú mở rộng vào ADR-0005
