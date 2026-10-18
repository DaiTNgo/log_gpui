## 0. Điều kiện trước

- [x] 0.1 Archive change `add-text-markers-and-highlight-rules` (hoặc xác nhận các task còn lại của nó xong) trước khi bắt đầu — đã được người dùng archive (`openspec/changes/archive/2026-10-18-add-text-markers-and-highlight-rules`); 9.1/9.2 của change đó do người dùng tự kiểm tra

## 1. Mô hình Focus Tab (logic thuần)

- [x] 1.1 Tạo `src/focus.rs` với `FocusTabs`, `FocusTab`, `FocusFind`, `LineRef`, `FocusRow`; export trong `src/lib.rs`
- [x] 1.2 `FocusTabs::add` lấy số lớn nhất cộng một, `close` từ chối khi chỉ còn một tab, `Default` là một tab 1 trống; test trong `tests/focus_tabs_test.rs`
- [x] 1.3 `FocusFind`: parse bằng `search_expr`, giữ matcher hợp lệ gần nhất, tính `hits` trên một danh sách dòng, next/prev quay vòng, hiện `i/n`; test
- [x] 1.4 Hàm thuần `compute_focus_rows(entries, filter_pass, filter_active, matcher)`: Filter ∩ Search, rỗng khi cả hai rỗng, Continuation Line chỉ khi chính nó khớp; test các scenario của spec `focus-pane` (Nội dung Focus Tab)
- [x] 1.5 Hàm tìm dòng gần nhất phía trước trong `focus_rows` cho một `LineRef`; test

## 2. Filter không còn ẩn dòng ở view

- [x] 2.1 `apply_filters_and_refresh`: `visible_indices` là mọi dòng; lưu kết quả `FilterState::evaluate` vào `filter_pass`
- [x] 2.2 `update_entry_delta_time` tính trên mọi dòng; sửa test delta time trong `tests/log_filter_test.rs`
- [x] 2.3 `SearchState::update_matches` nhận `filter_pass` (Parsed); sửa `searches_only_visible_lines` cho đúng scenario "Dòng không qua Filter" (12 Match, view vẫn có dòng `DEBUG`)
- [x] 2.4 Chép vùng chọn ở view lấy mọi dòng; thêm test cho scenario "Chép ở view"
- [x] 2.5 Status bar hiện số dòng của Focus Tab đang mở trên tổng số dòng thay cho "Filtered x%"

## 3. Gắn Focus Tab vào AppView

- [x] 3.1 Thay `search_state` bằng `focus: FocusTabs` cùng accessor `active_search()` / `active_search_mut()`; cập nhật mọi chỗ dùng (`app/search.rs`, `keyboard.rs`, `view/toolbar.rs`, canvas)
- [x] 3.2 `refresh_focus()` tính `focus_rows` cho tab đang mở (Parsed từ `filter_pass`, Raw từ `raw.matches`), giữ dòng hiện tại bằng `LineRef`, tính lại `FocusFind.hits`
- [x] 3.3 Gọi `refresh_focus()` từ `apply_filters_and_refresh`, `run_search`, `run_raw_search`, `run_raw_search_keep_position`, đổi tab, đổi view mode; thêm `on_entries_appended()` để dành cho tail
- [x] 3.4 `set_current_line(LineRef, origin)`: đồng bộ `selected_row_pos` / `raw.selected`, `FocusTab.current`, cuộn bên còn lại; next/prev của Search đi qua hàm này
- [x] 3.5 Thêm, đóng, mở Focus Tab: ô Search đổi theo tab (đánh dấu `search_input_dirty`), view nhảy tới dòng hiện tại của tab

## 4. Bố cục Focus Pane

- [x] 4.1 `FocusPaneState { visible, height }` trong `src/workspace/state.rs`, lưu cùng app state, có `serde(default)`
- [x] 4.2 Sắp lại root trong `src/app/view/mod.rs`: hàng (sidebar | main | context sidebar), tay kéo, Focus Pane, status bar; giới hạn chiều cao để view còn tối thiểu khoảng 120px
- [x] 4.3 Tay kéo chiều cao theo mẫu `ResizeDrawer`; `ContextSidebar::render` nhận chiều cao còn lại
- [x] 4.4 Nút bật/tắt trên thanh Search và phím tắt (đề xuất `⌘J`, kiểm tra trùng keymap); thêm vào command palette

## 5. Render Focus Pane

- [x] 5.1 Đổi `render_text_cell` / `CellCtx` thành `pub(crate)` và thêm `TextCell::Raw` hiện `raw_line`
- [x] 5.2 `src/ui/focus_pane.rs`: dải tab (số kèm phần đầu expression, "+", nút đóng ẩn khi còn một tab), ô Focus Find kèm `i/n` và next/prev, `uniform_list` các dòng
- [x] 5.3 Mỗi hàng: số dòng, Level Marker (Parsed), tên Source File khi nhiều file, Marker, phần tô Search và Focus Find, màu Highlight Rule chỉ khi view là Parsed View
- [x] 5.4 Gợi ý trống khi Search và Filter đều rỗng
- [x] 5.5 Bấm hàng gọi `set_current_line(.., Pane)`; chọn và chép text trong pane chỉ lấy dòng pane liệt kê
- [x] 5.6 Focus Find chỉ tô trong pane; next/prev của Focus Find đi qua `set_current_line`

## 6. Lưu Focus Tab theo view

- [x] 6.1 Thêm `focus: FocusTabs` và `file_focus: HashMap<String, FocusTabs>` vào `LogSpace` với `serde(default)`; lưu `number`, `query`, `find.query`, `current`, `active`
- [x] 6.2 Ghi trong `persist_active_session` và đọc trong `build_view_for` / `install_view`, giống Filter
- [x] 6.3 Đổi `search_text_is_not_serialized` thành test round-trip; thêm test manifest cũ mở với một tab 1 trống (`tests/session_test.rs`)

## 7. Inspector

- [x] 7.1 Bỏ `InspectorTab` và `inspector_tab`; `render_body` nhận dòng cùng các Continuation Line của nó và chỉ hiện text lưu trữ
- [x] 7.2 Lấy dòng cho Inspector từ dòng hiện tại chung (`LineRef`), cả ở Parsed lẫn Raw View
- [x] 7.3 Context sidebar hiện ở Raw View chỉ với mục Inspector; Filter và Highlights chỉ ở Parsed View

## 8. Kiểm tra

- [x] 8.1 `cargo test` xanh; `cargo clippy` không cảnh báo mới
- [ ] 8.2 Chạy app: kiểm tra từng scenario của `focus-pane`, `inspector`, `log-search`, `log-filter` trên một Log Space nhiều file và một File View, ở cả Raw lẫn Parsed View
- [x] 8.3 Đối chiếu `CONTEXT.md` và ADR-0008 với hành vi thực tế; cập nhật nếu lệch
