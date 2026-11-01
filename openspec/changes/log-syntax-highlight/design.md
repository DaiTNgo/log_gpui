## Context

Một cell text được vẽ bởi `render_text_cell` (`src/ui/canvas.rs`) như một chuỗi các `Layer { range, background, color }` chồng lên nhau: Search, Focus Find, Phrase, Marker, vùng chọn. Chữ mặc định lấy màu theme; màu level chỉ áp ở gutter và ở dòng lỗi/cảnh báo. Raw View và Focus Pane dùng cùng đường vẽ này (`line_match_paints` cho Raw View). Vì vậy Syntax Color không cần đường vẽ mới: nó là một nhóm `Layer` chỉ có `color` (không có `background`), đặt dưới mọi layer khác.

File có thể lớn (xem Big-file direction): mọi thứ chạy theo viewport, không bao giờ theo cả file.

## Goals / Non-Goals

**Goals:**
- Mỗi dòng đang hiển thị được tô theo loại token, đọc được trên cả ba theme.
- Không làm chậm cuộn, không đụng Line Index, Filter, Search.
- Đi từng mức: mức 1 (timestamp, level, Continuation Line) giao được riêng, mức 2 thêm token vào cùng khung.

**Non-Goals:**
- Không có grammar theo từng định dạng log, không đọc Log Format để tô.
- Không cho người dùng sửa bảng màu hay thêm quy tắc (đó là việc của Highlight Rule).
- Không tô JSON/XML lồng trong message theo cú pháp riêng.
- Không tô nền; nền thuộc Marker, Highlight Rule, Search.

## Decisions

**1. Tokenizer là một hàm thuần `tokenize(line: &str, scope: Scope) -> Vec<Token>`** (`Scope::Line` cho cả dòng, `Scope::Message { continuation }` cho cell Message của Parsed View) trong module mới `src/syntax_color.rs`, trả về `Token { range: Range<usize>, kind: TokenKind }`, range theo byte của dòng đã lưu, không chồng nhau, sắp theo vị trí. Thuần nên test được không cần UI.
- Thay thế đã xét: gắn token vào `LogEntry` lúc parse. Bỏ, vì Raw View không có `LogEntry`, và nó làm Row Cache nặng thêm cho mọi dòng kể cả dòng không bao giờ hiện.

**2. Quét một lượt bằng tay, không backtrack**, dùng `memchr` cho dấu nháy và `=`. Không dùng một regex lớn cho mọi loại. Mỗi loại token là một nhận dạng nhỏ tại vị trí hiện tại (bắt đầu bằng chữ số, `"`, `[`, chữ cái hoa…).
- Thay thế: `regex` với alternation. Bỏ vì khó đảm bảo thời gian tuyến tính trên dòng dài và khó kiểm soát thứ tự ưu tiên giữa các loại. Dùng `regex` chỉ cho UUID nếu viết tay quá dài.
- Dòng dài hơn `MAX_TOKENIZE_BYTES` (4096) chỉ được tokenize phần đầu; phần sau giữ màu mặc định.

**3. Parsed View dùng cột sẵn có, Raw View và Focus Pane dùng tokenizer.** Trong Canvas, timestamp, level, module đã có cột và màu riêng, nên tokenizer chỉ chạy trên cell Message (bỏ các loại timestamp/level/module). Raw View không có cột, nên chạy đủ. Tránh tô hai lần một thứ.

**4. Ưu tiên bằng thứ tự layer.** Syntax Color là layer thấp nhất, chỉ đặt `color`. Marker đặt cả nền và màu chữ nên che hẳn nó; Search Phrase đặt nền, và màu chữ do nền đó quyết định như hiện tại. Không cần luật so sánh mới. Highlight Rule chỉ có ở Parsed View, che Syntax Color ở cell nó phủ.

**5. Bảng màu trong `CanvasColors`, theo `dark`/`Light`**, một `Hsla` mỗi `TokenKind`. Dark Bright dùng bảng Dark nâng lightness cùng cách `brighten` đang làm. Tối đa 8 màu: timestamp, module, số/duration, chuỗi, định danh, key, stack trace, `Caused by`. Level dùng lại `CanvasColors::level`.

**6. Công tắc là cờ `show_syntax_color` trong workspace state** (cạnh `show_search_highlight`), mặc định `true` khi thiếu trong file cũ (`#[serde(default = ...)]`). Có mục trong Settings và một lệnh Keymap. Tắt thì không gọi tokenizer.

**7. Không cache.** (Đã đo: 100 dòng × 171 byte mất khoảng 100 µs mỗi màn hình, nên không cần cache.)

*Ban đầu:* Mỗi lần vẽ, tokenize các dòng trong viewport (vài chục dòng, mỗi dòng dưới 4 KB, quét tuyến tính). Chỉ khi benchmark cho thấy vượt ngân sách mới cache theo dòng, cùng vòng đời với dòng trong Row Cache.

**8. Mức 1 và mức 2 dùng chung `TokenKind`.** Mức 1 chỉ phát ra `Timestamp`, `Level`, `StackTrace`, `CausedBy`. Mức 2 thêm loại khác, không đổi chữ ký hàm hay chỗ vẽ.

## Risks / Trade-offs

- [Nhận nhầm token, ví dụ số trong tên biến] → Chỉ nhận số đứng riêng (ranh giới từ hai phía) hoặc có đơn vị duration; thà bỏ sót còn hơn tô sai.
- [Quá nhiều màu làm rối] → Trần 8 màu; mức 2 giao riêng và có công tắc tắt.
- [Màu trùng hue của Phrase] → Phrase vẽ bằng nền và che Syntax Color; không phải đổi hue của Phrase.
- [Chi phí khi cuộn nhanh] → Ngưỡng 4096 byte và tính theo viewport; có benchmark trong tasks trước khi quyết định cache.
- [Dark Bright và Light chưa đủ tương phản] → Kiểm tra từng màu với nền của từng theme (tỉ lệ tương phản ≥ 4.5:1 cho chữ) khi chọn bảng màu.
- [Dòng chứa ký tự đa byte] → Mọi range tính theo byte của dòng đã lưu và cắt theo ranh giới char; có test với tiếng Việt và emoji.
