## 1. Cờ cài đặt và bảng màu

- [x] 1.1 Thêm `show_syntax_color` vào workspace state (`src/workspace/session.rs`), mặc định `true` khi thiếu, kèm test đọc bản lưu cũ
- [x] 1.2 Thêm `set_show_syntax_color` và lệnh toggle ở `src/app/view_settings.rs`, lệnh Keymap ở `src/keymap.rs`
- [x] 1.3 Thêm công tắc Syntax Color vào `src/ui/settings.rs`
- [x] 1.4 Thêm bảng màu theo loại token vào `CanvasColors` cho Dark, Dark Bright, Light; test tương phản ≥ 4.5:1 với nền từng theme

## 2. Tokenizer mức 1

- [x] 2.1 Tạo `src/syntax_color.rs` với `Token`, `TokenKind`, `tokenize(line, level)` quét một lượt, range theo byte trên ranh giới char
- [x] 2.2 Nhận timestamp ở đầu dòng và từ khóa level chữ hoa, test với các định dạng timestamp có trong `src/parser`
- [x] 2.3 Nhận Continuation Line (dim) và `Caused by:`, test stack trace Java và Rust
- [x] 2.4 Giới hạn `MAX_TOKENIZE_BYTES = 4096`, test dòng 1 MB và dòng tiếng Việt/emoji

## 3. Vẽ mức 1

- [x] 3.1 Trong `render_text_cell` (`src/ui/canvas.rs`) đổi token thành `Layer` chỉ có `color`, đặt dưới mọi layer khác
- [x] 3.2 Raw View: gọi tokenizer cho các dòng trong viewport, bỏ qua khi công tắc tắt
- [x] 3.3 Focus Pane: áp cho dòng ở cả Raw và Parsed View
- [x] 3.4 Parsed View của Canvas: chỉ tô cell Message, bỏ loại timestamp/level
- [x] 3.5 Kiểm tra thứ tự ưu tiên: Marker, Highlight Rule, Phrase che Syntax Color; Search Highlight tắt không tắt Syntax Color
- [x] 3.6 Chạy các test của phần đã đổi (canvas, raw_view, settings), xem thử trong app trên cả ba theme

## 4. Tokenizer và vẽ mức 2

- [x] 4.1 Nhận số và duration đứng riêng, không nhận số nằm giữa chữ
- [x] 4.2 Nhận chuỗi trong dấu nháy kép (dùng `memchr`), có xử lý dấu `\"` thoát
- [x] 4.3 Nhận UUID, IPv4, URL `http(s)://`
- [x] 4.4 Nhận `key=value` (chỉ màu key) và module/logger
- [x] 4.5 Chốt bảng màu cuối, giữ tối đa 8 màu ngoài level
- [x] 4.6 Test các scenario trong `specs/syntax-color/spec.md`

## 5. Hiệu năng

- [x] 5.1 Benchmark tokenizer: 100 dòng viewport × 200 byte, đo thời gian mỗi khung
- [x] 5.2 Không cần cache: đo được khoảng 100 µs mỗi màn hình, đã ghi vào `design.md`
- [ ] 5.3 Xác nhận cuộn file lớn trong Raw View không tăng memory theo kích thước file

## 6. Tài liệu

- [x] 6.1 Thêm **Syntax Color** vào `CONTEXT.md` (kèm _Avoid_) và quy tắc thứ tự ưu tiên vào danh sách rules
