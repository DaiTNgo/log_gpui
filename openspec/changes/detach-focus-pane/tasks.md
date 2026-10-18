## 0. Điều kiện trước

- [ ] 0.1 Change `add-focus-pane` đã được cài và archive

## 1. Tách render Focus Pane

- [x] 1.1 Đổi hàm render Focus Pane để nhận `Entity<AppView>` cùng dữ liệu đã chuẩn bị, và gửi hành động qua `app.update(...)` thay cho `cx.listener`
- [ ] 1.2 Cửa sổ chính dùng hàm mới; kiểm tra Focus Pane dưới cửa sổ chính vẫn chạy như cũ

## 2. Focus Window

- [x] 2.1 Thêm `FocusWindowView` (root bọc trong `Root`) với `search_input`, `find_input` riêng, `observe` `AppView`
- [x] 2.2 `AppView.focus_window: Option<WindowHandle<..>>`; lệnh "Move to window" mở cửa sổ khoảng 1000×600 trên màn hình của cửa sổ chính, hoặc đưa cửa sổ đã có lên trước
- [x] 2.3 Nút "Move to window" trên Focus Pane và lệnh trong command palette
- [x] 2.4 Focus Window render ô Search, dải Focus Tab, Focus Find và danh sách dòng; trạng thái trống khi tab ở cửa sổ chính không phải file log
- [x] 2.5 Đồng bộ ô Search và Focus Find giữa hai cửa sổ qua cờ dirty, giống `sync_search_input`

## 3. Đưa về và vòng đời

- [x] 3.1 Nút "Move back" và `⌘W` trong Focus Window gọi `remove_window` và đặt `focus_window = None`
- [x] 3.2 Đóng bằng nút của hệ điều hành cũng đặt `focus_window = None` và render lại cửa sổ chính
- [x] 3.3 Đóng cửa sổ chính thì đóng luôn Focus Window
- [x] 3.4 Cửa sổ chính ẩn Focus Pane và tay kéo khi đã tách; `⌘J` khi đó đưa Focus Window lên trước

## 4. Phím tắt trong Focus Window

- [x] 4.1 `⌘F`, Enter/Shift+Enter, next/prev của Focus Find, `⌘C` hoạt động trong Focus Window

## 5. Kiểm tra

- [x] 5.1 `cargo test` xanh; `cargo clippy` không cảnh báo mới
- [ ] 5.2 Chạy app: kiểm tra từng scenario của `focus-window` (tách ra, sửa Search, đổi tab ở cửa sổ chính, bấm dòng, Move back, đóng bằng hệ điều hành, đóng cửa sổ chính, khởi động lại)
