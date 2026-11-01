## Why

Ngoài màu của level (error, warn) và màu Phrase của Search, phần còn lại của một dòng log hiện chỉ có một màu trắng. Người đọc phải tự tìm timestamp, module, số, id trong một khối chữ đồng màu. Các tool log khác (lnav, Log File Highlighter, Sublime Log Highlight) đều tô màu theo từng loại token, và đó là thứ làm log dễ quét bằng mắt.

## What Changes

- Một dòng log hiện **Syntax Color**: mỗi loại token trong dòng được tô màu chữ riêng (foreground), không đổi nền.
- Làm theo hai mức, mức 1 trước:
  - **Mức 1**: timestamp (xám xanh, mờ), level (màu của level: đỏ, vàng, xanh lá, xám), Continuation Line như stack trace (xám nhạt, dòng `Caused by:` đỏ nhạt).
  - **Mức 2**: thêm module/logger, số và duration, chuỗi trong dấu nháy, định danh (UUID, IP, URL, request id), `key=value` (key có màu, value giữ màu thường).
- Syntax Color hiện ở Raw View và ở Focus Pane (Raw và Parsed). Trong Parsed View của Canvas, nó chỉ tô màu trong cell Message; các cột đã tách (timestamp, level, module…) giữ nguyên cách tô hiện có.
- Thứ tự ưu tiên từ trên xuống: Marker, Highlight Rule, Search Phrase / Current Match, Syntax Color, màu mặc định. Lớp trên che lớp dưới ở đúng khoảng ký tự chúng phủ.
- Công tắc **Syntax Color** trong Settings (và một lệnh Keymap), một cài đặt cho cả app, mặc định bật, nhớ qua các lần chạy. Tắt thì mọi thứ vẽ như hiện tại.
- Bảng màu cố định theo theme (Dark, Dark Bright, Light), mỗi loại token một màu, không có UI chọn màu. Số màu tối đa 8.
- Hiệu năng: chỉ tokenize các dòng đang hiển thị, bằng quét một lượt không backtrack; chỉ cache kết quả (theo dòng, cùng vòng đời với dòng trong Row Cache) nếu đo cho thấy cần. Không đọc thêm dòng nào ngoài viewport. Dòng dài hơn một ngưỡng chỉ được tô phần đầu.

## Capabilities

### New Capabilities
- `syntax-color`: Quy tắc tô màu token trong dòng log: các loại token, bảng màu theo theme, thứ tự ưu tiên với Marker, Highlight Rule và Search, công tắc bật/tắt, giới hạn hiệu năng.

### Modified Capabilities
- `raw-view`: dòng trong Raw View được tô Syntax Color khi công tắc bật.
- `log-search`: Search Phrase và Current Match được vẽ trên Syntax Color; Search Highlight tắt không làm Syntax Color tắt theo.
- `highlight-rule`: Highlight Rule và Marker che Syntax Color ở đúng khoảng chúng phủ.

## Impact

- Code: `src/ui/canvas.rs` (`CanvasColors`, `render_text_cell`, hệ `Layer`), `src/raw_view.rs`, `src/ui/focus_pane.rs`, `src/ui/settings.rs`, `src/theme.rs`, `src/keymap.rs`, `src/workspace/session.rs` (cờ cài đặt mới), `src/app/view_settings.rs`. Thêm module tokenizer mới, ví dụ `src/syntax_color.rs`.
- Không đổi định dạng file, Line Index, parser hay dữ liệu lưu trên đĩa ngoài một cờ bool trong workspace state (thiếu thì mặc định bật).
- Không thêm dependency: dùng `regex` nếu đã có, nếu không thì tự quét byte.
- Không đổi hành vi Filter, Search, Marker, hoặc cách tính Match.
