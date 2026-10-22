## 1. Quy tắc hiển thị

- [x] 1.1 `src/workspace/log_space.rs`: `shows_group()` = `files.len() >= 2 || !globs.is_empty()`. Test đơn vị: 0, 1, 2 file, 1 file có glob.
- [x] 1.2 `src/ui/sidebar.rs`: Space không `shows_group()` thì không vẽ file row và glob con. `expanded_spaces` không bị sửa.
- [x] 1.3 `src/ui/space_menu.rs`: `SpaceRef` có cờ nhóm. Ẩn chevron (và vùng click của nó) khi không phải nhóm.

## 2. Status bar

- [x] 2.1 Status bar hiện thông tin file duy nhất khi Space 1 file.
- [ ] 2.2 Nút Load more, Clear, Live/Pause, Back to Live Window cho Space 1 file, theo điều kiện hiện có của từng loại (Remote File, Remote Command, local followed live). Test: Remote Command không có Load more. (Đã làm nút, chưa có test.)

## 3. Menu và xóa

- [x] 3.1 Menu chuột phải của Space 1 file: gộp action của file (Download, Download file, copy path, Show in Finder, ...) và mục xóa. Space nhiều file giữ menu cũ.
- [x] 3.2 Xóa file duy nhất xóa Space: `delete_copy_of`, đóng tab, lưu Project. Dùng luồng `DeleteLogSpace` sẵn có (đóng tab, `delete_copy_of`, xóa thư mục Scratch). Luồng này hỏi xác nhận cho mọi loại Space, không chỉ Scratch.
- [ ] 3.3 Test: Imported và Remote xóa ngay, Scratch hỏi xác nhận, tab đóng.

## 4. Kiểm tra

- [ ] 4.1 Test chuyển 1 → 2 → 1 file: chevron và file row đổi đúng, `expanded_spaces` không đổi. (Đã có test `shows_group`; chưa có test ở mức app.)
- [x] 4.2 Test Space có Live Glob khớp 1 file vẫn có chevron.
- [ ] 4.3 Chạy thử trong app: upload 1 file, thêm file thứ 2, remote 1 file.
- [ ] 4.4 Cập nhật `CONTEXT.md` (thuật ngữ cho dòng Space đơn) nếu cần.
