## 1. Editor scratch thôi tự mở thanh

- [x] 1.1 Trong đoạn đồng bộ thanh màu của editor (`src/app/raw.rs` ~720–760), bỏ nhánh dựng `ColorTarget::RawMark` từ vùng chọn khác rỗng; giữ `RawRecolor` và `RecolorTextHighlight` khi vùng chọn rỗng
- [x] 1.2 Khi vùng chọn khác rỗng: để yên thanh đang mở nếu nó là `RawMark` cho đúng vùng chọn này, đóng nó nếu vùng chọn đã khác
- [x] 1.3 Test: chọn bằng kéo chuột, Shift+mũi tên và Cmd+A trong editor scratch không mở thanh; bấm lên Marker / Text Highlight vẫn mở thanh recolor

## 2. Lệnh "Mark: Color Selection…"

- [x] 2.1 Thêm `Command::ColorSelection` vào `src/keymap.rs` (id `marker.color_selection`, tên "Mark: Color Selection…", mặc định `cmd-shift-h`), thêm vào danh sách lệnh và `from_id`
- [x] 2.2 Viết hàm xử lý lệnh: tìm vùng chọn của view đang hiện (editor scratch → Raw list → Parsed View), dựng `RawMark` hoặc `MarkSelection` như lối vào tự động, mở `ColorBar` ở cuối vùng chọn; không có vùng chọn thì không làm gì
- [x] 2.3 Kiểm tra `cmd-shift-h` tới được app khi editor scratch có focus (cùng đường với `cmd-s`); chặn trước editor nếu cần
- [x] 2.4 Test: lệnh tô được Marker trong editor scratch; tạo được Text Highlight bằng "Everywhere"; mở lại thanh cho vùng chọn ở Raw list; phím `cmd-shift-h` tới được app khi editor có focus; không làm gì khi không có vùng chọn

## 3. Tài liệu

- [x] 3.1 Thêm vào mục **Marker** trong `CONTEXT.md`: trong scratch đang sửa, thanh màu để tô chỉ mở bằng lệnh "Mark: Color Selection…"
- [ ] 3.2 Kiểm tra bằng tay trong app: chọn để xóa ở scratch không còn bật thanh; Raw list và Parsed View vẫn tự mở như cũ
