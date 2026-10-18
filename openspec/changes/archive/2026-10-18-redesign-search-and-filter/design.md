## Context

Ô Search parse câu nhập bằng query language theo trường (`tokenize` → `parser` → `validator` → AST `field operator value`) rồi đánh giá bằng `QueryEngine` ([src/search.rs](src/search.rs)). Filter cũng dùng AST và `QueryEngine`, nhưng dựng AST trực tiếp từ các hàng trong popover ([src/filter.rs](src/filter.rs)), với danh sách trường và module cố định (`LogSchema::for_log_fields`, danh sách module gán cứng trong panel View).

Vấn đề của hiện trạng:
- Người dùng phải học cú pháp giống SQL để tìm một từ khóa.
- `QueryEngine` biên dịch lại `Regex::new` cho mỗi dòng, và tính offset highlight trên chuỗi đã `to_lowercase()` rồi áp lại lên chuỗi gốc.
- Tokenizer đọc theo byte ASCII, nên chữ Việt bị báo lỗi cú pháp; `Eq` dùng `eq_ignore_ascii_case`.
- Filter không phản ánh file đang mở (giá trị module gán cứng, trường JSON không chọn được).
- `SavedView` gộp filter, search và bố cục cột; "View" bị dùng cho ba thứ.

Thuật ngữ đã chốt trong [CONTEXT.md](CONTEXT.md) (Search, Search Expression, Phrase, Match, Filter, Filterable Column, Distinct Count, Filter Value, Custom Value, Hide Mode, Time Range, quy tắc 7–21). Quyết định ghi tại [ADR-0004](docs/adr/0004-search-is-text-only.md) và [ADR-0005](docs/adr/0005-filter-from-detected-columns.md).

Hiện trạng liên quan: file đã được suy ra schema khi nạp (`parse_file_with_schema`, `SchemaBuilder`); mỗi tab đã có `TabSessionState` riêng chứa filter, search, vị trí cuộn ([working_set.rs](src/workspace/working_set.rs)); mô hình `LogEntry` có `timestamp: f64` và `json_fields`.

## Goals / Non-Goals

**Goals:**
- Search text-only trên toàn dòng, compile một lần, highlight đúng offset.
- Filter xây từ cột và giá trị phát hiện khi nạp file, với checkbox, Add value, Hide Mode, ngưỡng 500 và Time Range.
- Filter và Marker riêng từng file, tự nhớ.
- Search và Filter dùng chung một hàm so khớp Unicode.
- Bỏ `SavedView`, panel View, menu nhanh và toàn bộ code field-query.

**Non-Goals:**
- Regex, phủ định, match case trong Search; regex trong Filter.
- Bỏ dấu khi gõ không dấu (tiếng Việt Mức 2).
- Số đếm cập nhật theo các Filter khác (faceted).
- Preset theo project (Saved Filter) hay Saved Layout.
- Index toàn văn hay quét song song.
- Đổi `search_across_files` (tìm tương quan giữa các file) và `InvestigationCase`; chỉ di chuyển code nếu cần.

## Decisions

### D1. Search có parser và matcher riêng
Search Expression parse thành cây nhỏ (`Phrase`, `And`, `Or`) trong module riêng. Quy tắc "mọi chuỗi khoảng trắng thỏa một dấu cách trong Phrase" không biểu diễn được bằng `Contains` của query engine cũ. **Phương án khác:** tái dùng AST với trường giả `raw_line`; bị loại vì không diễn đạt được quy tắc trên.

### D2. Chỉ `&&` và `||` là toán tử
`and`/`or`/`AND`/`OR` là chữ thường trong Phrase, vì log tiếng Anh hay có các chữ này và Phrase là các chữ liền nhau.

### D3. Ngữ pháp Search
```
expr    := or
or      := and ( "||" and )*
and     := atom ( "&&" atom )*
atom    := phrase | "(" expr ")"
phrase  := word+ | quoted
quoted  := '"' ... '"'
```
`word` là chuỗi ký tự (theo `char`) không chứa khoảng trắng và không chứa `& | ( ) "`. Phrase ngay trước hoặc sau nhóm `()` mà không có toán tử là lỗi cú pháp. Biểu thức rỗng nghĩa là "không tìm" (không Match, không lỗi).

### D4. Hàm so khớp Unicode dùng chung
Một module nhỏ cung cấp: chuẩn hóa NFC, gấp hoa/thường theo Unicode (kể cả `đ/Đ`), `eq`, `contains` và `find_all` trả offset byte trên chuỗi gốc. Dòng toàn ASCII đi đường nhanh. Search và Filter đều dùng module này; không bỏ dấu (Mức 1).

**Lý do:** tránh hai nơi so khớp lệch nhau; sửa lỗi offset highlight và lỗi ASCII-only. **Phương án khác:** dựng `Regex` từ `regex::escape` và `(?i)`; gọn nhưng khó bảo đảm NFC/offset, giữ làm dự phòng.

### D5. Search giữ kết quả lần hợp lệ gần nhất
`SearchState` giữ biểu thức hợp lệ gần nhất; khi parse lỗi thì không tính lại Match, giữ danh sách và highlight cũ, và ghi lỗi để hiển thị. Search chỉ chạy trên `visible_indices` (đã đúng hiện nay), nên không đếm Match bị Filter ẩn.

### D6. Mô hình Filter mới, không dùng chung query engine
Mỗi file có một `FilterState`:
- theo cột: `{ values: tập Filter Value đã tích, custom: danh sách Custom Value, hide: bool }`;
- một `TimeRange { from: Option<f64>, to: Option<f64> }`.

Dòng hiển thị khi: với mọi cột có điều kiện, `(khớp values) OR (khớp custom)` rồi đảo nếu `hide`; và nằm trong `TimeRange`. Dòng thiếu giá trị ở cột đó là "không khớp" (nên bị ẩn ở chế độ chỉ hiện, hiện ở Hide Mode). Chỉ có `eq` và `contains`; không có `In`, `Neq`, `StartsWith`, `EndsWith`, regex, nhóm lồng nhau.

**Lý do:** với hai phép so sánh, OR trong cột và AND giữa cột, engine tổng quát là thừa. **Phương án khác:** ánh xạ sang `QueryExpression`; bị loại vì giữ lại AST và engine chỉ để phục vụ một cấu trúc phẳng.

### D7. Phát hiện cột và giá trị khi nạp file
Một lượt duyệt qua các sự kiện khi nạp file, mỗi cột lọc được có bảng `giá trị đã gấp NFC/hoa-thường → { dạng hiển thị phổ biến nhất, số dòng }`. Khi một cột vượt 500 giá trị riêng biệt thì dừng thu thập, đánh dấu "không khả dụng" và hiện "> 500". Loại `timestamp` và `message`/payload. Cột gồm các cột cố định (`level`, `module`, `request_id`) và các trường JSON phát hiện được.

### D8. Đánh giá Filter bằng id giá trị
Vì mỗi cột có tối đa 500 giá trị, gán mỗi giá trị một id nhỏ (`u16`) và lưu id theo dòng và theo cột, với một id đặc biệt cho "thiếu giá trị". Khi người dùng đổi Filter:
- `values`: tập id.
- `custom`: duyệt bảng giá trị của cột (≤500) một lần để biết id nào chứa chuỗi, được tập id.
- Đánh giá từng dòng chỉ là so sánh id, không so chuỗi trên từng dòng.

Với Custom Value gõ vào sau khi file đã có thêm giá trị mới (tail), bảng giá trị được cập nhật nên kết quả vẫn đúng.

### D9. Time Range
Tiền xử lý một lần thành timestamp hiệu dụng cho mỗi dòng: dòng không có timestamp riêng kế thừa dòng gần nhất phía trên có timestamp; dòng trước timestamp đầu tiên luôn hiện. Ô Từ/Đến nhận chuỗi theo dạng hiển thị trong cột Timestamp, parse bằng cùng bộ parse khi nạp; so sánh theo đồng hồ của file, không đổi múi giờ. Ô trống là không giới hạn. Gợi ý là giá trị nhỏ nhất và lớn nhất của file.

### D10. Tail
Khi file lớn lên, giá trị mới được thêm vào cuối bảng của cột (chưa tích), số đếm tăng theo dòng mới, nhưng không đổi theo Filter khác. Cột vượt 500 trở thành không khả dụng cho việc thêm điều kiện mới; điều kiện đã bật vẫn chạy.

### D11. Filter và Marker riêng theo file, tự nhớ
`FilterState` và Marker được lưu theo file (khóa là id/đường dẫn của `LogFileDescriptor`) trong manifest session và khôi phục khi mở lại file. Search Expression không lưu (`#[serde(skip)]` cho phần truy vấn). Manifest tăng version; khi đọc version cũ, `saved_views` và filter cũ (hàng field/operator) bị bỏ.

### D12. Bỏ phần giao diện và code không còn dùng
Bỏ panel View, danh sách module gán cứng, menu nhanh, `SavedView`. Giữ Pin Toggle trên Column Header, kéo đổi độ rộng, Auto-fit; density chuyển sang settings. Xóa `src/query/` (parser, validator, `language_service`, `completion_provider`, AST, engine) sau khi xác nhận không còn tham chiếu; kiểm tra `schema.rs` và `parser/text.rs` (đang dùng `LogSchema`) trước khi xóa.

## Risks / Trade-offs

- [Người đã quen field-query trong Search hoặc regex trong Filter mất khả năng đó] → Filter theo cột và Hide Mode phủ phần lớn nhu cầu; ghi trong ADR và release note.
- [Cột không được phát hiện thì không lọc được] → kiểm thử phát hiện cột với log text, JSON và logcat/syslog; hiện số cột phát hiện.
- [Giá trị bị mất ở cột quá 500, ví dụ `request_id`] → chủ đích; dùng Search để tìm một id.
- [Bảng id theo dòng và theo cột tốn bộ nhớ] → `u16` cho tối đa vài cột lọc được; đo trên file lớn.
- [Session cũ mất Filter và preset] → chấp nhận, thông báo một lần khi mở session cũ.
- [Cú pháp Search có lỗi biên (ngoặc lồng, `"` chưa đóng)] → bộ test riêng, gồm các ca gõ dở.
- [Highlight cũ không khớp chữ đang gõ khi biểu thức lỗi] → chấp nhận, đã có chỉ báo lỗi.
- [Dòng nối tiếp kế thừa timestamp sai khi log bị xen kẽ nhiều luồng] → chấp nhận; quy tắc đơn giản và dự đoán được.

## Migration Plan

1. Thêm hàm so khớp Unicode dùng chung và test.
2. Thêm module Search Expression (parser, matcher) cùng test; chưa nối UI.
3. Nối `SearchState` và `SearchEngine` sang matcher mới; thêm giữ kết quả khi lỗi.
4. Thêm bước phát hiện cột và giá trị khi nạp file, `FilterState` mới và Time Range, cùng test.
5. Làm lại giao diện Filter (chọn cột kèm Distinct Count, checkbox, Add value, Hide Mode, Time Range); bỏ panel View và menu nhanh; chuyển density sang settings.
6. Lưu và khôi phục Filter và Marker theo file; bump version manifest; bỏ `saved_views` và filter cũ khi đọc.
7. Xóa `SavedView`, `src/query/` và các test field-query khi không còn tham chiếu.

**Rollback:** từng bước là một commit riêng; bước 1–3 hoàn tác độc lập. Bước 6 làm mất dữ liệu cũ khi lưu lại, nên tách riêng và đặt gần cuối.

## Open Questions

- Bố cục UI của Filter trong context sidebar (thứ tự: chọn cột, danh sách giá trị, Add value, Hide Mode, Time Range) và cách hiển thị các điều kiện đang bật.
- Marker hiện có lưu theo file chưa, hay đang lưu ở nơi khác? Cần kiểm tra trước bước 6.
- Giá trị đồng thời bật cả Filter Value và Custom Value trong cùng cột sẽ hiển thị ra sao trên giao diện (cần phân biệt "bằng" và "chứa").
