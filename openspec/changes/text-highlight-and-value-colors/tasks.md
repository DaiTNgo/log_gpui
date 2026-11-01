## 1. Docs

- [x] 1.1 Thêm **Text Highlight** và **Value Color** vào mục Marking của `CONTEXT.md` (có `_Avoid_`), sửa Highlight Palette thành dùng chung cho Text Highlight, ghi rõ Marker vẫn là một dòng
- [x] 1.2 Viết `docs/adr/0016-text-highlight-and-value-colors.md`: Marker là vị trí đã tìm thấy, Text Highlight giúp tìm, Value Color tự động không cho can thiệp, thứ tự lớp nền/màu chữ, các phương án đã loại

## 2. Text Highlight: dữ liệu và so khớp

- [x] 2.1 Tạo `src/text_highlight.rs` với `TextHighlight { text, key, color, seq }`, thêm/đổi màu theo `key` và gỡ theo `key`; unit test cho trường hợp trùng hoa thường
- [x] 2.2 Viết hàm so khớp trả `(Range<usize>, color)` cho một dòng, đã giải quyết chồng lấn theo `seq`; test cho ký tự đặc biệt, chữ có dấu, chồng lấn và chuỗi lặp liền nhau
- [x] 2.3 Thêm `Project.text_highlights` với `#[serde(default)]`; test mở session cũ không có trường này

## 3. Thanh màu và menu chuột phải

- [x] 3.1 Thêm `MarkScope` vào `ColorBar` và các đích `TextHighlight` / `RecolorTextHighlight` vào `ColorTarget`; `apply_bar_color` tạo Text Highlight khi phạm vi là Everywhere, ghi màu vào Recent và lưu session
- [x] 3.2 Vẽ nút chọn phạm vi hai phần trong `src/app/view/color_bar.rs`: mặc định "Chỉ dòng này" mỗi lần mở, cắt giữa text dài, tắt "Mọi chỗ" (có tooltip) khi vùng chọn qua nhiều dòng hoặc chỉ có khoảng trắng
- [x] 3.3 Alt+bấm một ô màu thì tạo Text Highlight; ghi phím này trong tooltip của ô màu
- [x] 3.4 Đếm số dòng chứa đoạn text ở background, hiện "· N" khi xong, hủy khi đổi vùng chọn hoặc đóng thanh
- [x] 3.5 Thêm "Change highlight color" và "Remove highlight" vào `cell_menu` và menu chuột phải của Raw View, độc lập với mục của Marker
- [x] 3.6 Test logic AppView: tạo, tạo lại cùng text, đổi màu, gỡ, và gỡ Marker không làm mất Text Highlight bên dưới

## 4. Vẽ Text Highlight

- [x] 4.1 Thêm khoảng Text Highlight vào `CellCtx` và vẽ trong Parsed View theo thứ tự nền: Marker > Match > Text Highlight > hàng chọn > Highlight Rule
- [x] 4.2 Vẽ Text Highlight trong Raw View và Focus Pane; màu chữ trên nền chọn theo `text_tone_on`
- [x] 4.3 Xác nhận overview ruler không có vạch Text Highlight, text được copy không đổi, và dòng live mới được tô

## 5. Value Color

- [x] 5.1 Viết hàm chọn cột được tô từ cột của Log Format (loại timestamp, message/payload, level, Number/Boolean/Json); unit test
- [x] 5.2 Viết `value_rgb(value, tone, avoid_hues)` dùng lại `hue_for_key` / `phrase_rgb`, đẩy hue khỏi màu Text Highlight, có đủ tương phản với nền trang; test cho tính ổn định, cùng màu khi khác hoa thường, việc né màu và tương phản ở cả hai theme
- [x] 5.3 Truyền Value Color vào `CellCtx` cho mỗi ô ở Parsed View và Focus Pane; nhường cho màu chữ theo tương phản ở chỗ có nền Marker hoặc Text Highlight và trên hàng đang chọn
- [x] 5.4 Thêm `show_value_colors` (mặc định `true`) vào workspace state, một hàng trong Settings cạnh Search Highlight, và một lệnh Keymap để bật/tắt

## 6. Kiểm tra

- [x] 6.1 Chạy test của các module đã đổi (`marker`, `text_highlight`, `search_color`, `highlight`, `app::color`, `app::marking`) và `cargo clippy`
- [ ] 6.2 Chạy app với một Log Space gộp nhiều file: tạo, đổi màu, gỡ Text Highlight trong Raw và Parsed; xem Value Color ở cả theme sáng lẫn tối; tắt/bật công tắc rồi khởi động lại
