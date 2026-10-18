## Context

Hiện tại `FilterState::evaluate` (src/filter.rs:204) sinh `visible_indices`, và `AppView::apply_filters_and_refresh` (src/app/filter.rs:72) gán nó cho view. Mọi thành phần làm việc theo `vis_pos` (chỉ số trong `visible_indices`): canvas, overview ruler, Search (`SearchState.matched_visible_indices`), selection, chép text, Marker, Highlight Rule, status bar. Raw View không dùng `visible_indices`; nó có đường search riêng (`run_raw_search`, `raw_view::search_sections` → `RawMatch{section, line}`).

`SearchState` (src/search.rs:9) nằm trong `TabSessionState` và mọi trường đều `#[serde(skip)]`. Filter được lưu trên `LogSpace` (`filter` cho Log Space, `file_filters` cho File View). Bố cục ở src/app/view/mod.rs là `flex_col[chrome, flex_row(sidebar | main | context_sidebar), status_bar]`. Context sidebar chỉ hiện ở Parsed View. Inspector có ba tab và một `inspector_tab` toàn cục.

Tail chưa được cài: `tail_enabled` chỉ là cờ, không có đường nào thêm dòng lúc chạy. File scratch được parse lại khi rời Raw View (`reparse_edited_scratch_files`).

Change này phụ thuộc `add-text-markers-and-highlight-rules` (Marker theo đoạn text, Highlight Rule, chọn/chép nhiều ô), vốn gần xong.

## Goals / Non-Goals

**Goals:**
- View (Parsed và Raw) luôn hiện mọi dòng; Filter ∩ Search lấp đầy Focus Pane.
- Nhiều Focus Tab, mỗi tab có Search Expression và Focus Find riêng, được nhớ theo view.
- Dòng hiện tại đồng bộ hai chiều giữa view và Focus Pane.
- Inspector chỉ hiện text lưu trữ, ở cả hai view.

**Non-Goals:**
- Cài tail. Change này chỉ chừa điểm móc để tính lại Focus Tab khi có dòng mới.
- Tô Match của nhiều tab cùng lúc ở view, drill-down dạng chuỗi.
- Sửa text trong Focus Pane.

## Decisions

### 1. View hiện mọi dòng; `visible_indices` thành tập đầy đủ
`apply_filters_and_refresh` gán `visible_indices = 0..all_entries.len()` cho view, và lưu kết quả của `FilterState::evaluate` vào một trường mới `filter_pass: Arc<Vec<usize>>` (chỉ số entry, đã sắp).
- Giữ `visible_indices` thay vì xóa nó: mọi consumer (canvas, selection, chép, Marker, ruler) vẫn chạy mà không phải sửa, và `vis_pos == entry_ix` trong Parsed View.
- Phương án khác là bỏ hẳn `visible_indices` và dùng thẳng `entry_ix`. Làm vậy sạch hơn nhưng đụng hơn 15 chỗ; để dọn sau.
- `update_entry_delta_time` sẽ tính trên mọi dòng thay vì trên dòng qua Filter, vì view giờ đầy đủ.

### 2. `FocusTabs` thay cho `SearchState` đơn lẻ
Thêm module `src/focus.rs` (logic thuần, test được):
```
FocusTabs { tabs: Vec<FocusTab>, active: usize }
FocusTab  { number: u32, search: SearchState, find: FocusFind, current: Option<LineRef> }
FocusFind { query, matcher, hits: Vec<usize> /* chỉ số trong rows */, current, parse_error }
LineRef   { file_id: Arc<str>, line: usize }   // số dòng trong file, ổn định giữa hai view
```
- `FocusTabs::add()` lấy `max(number)+1`; `close(i)` bị từ chối khi chỉ còn một tab.
- `TabSessionState.search` được thay bằng `focus: FocusTabs`. Mọi chỗ đang đọc `self.search_state` chuyển sang đọc tab đang mở (accessor `active_search()` / `active_search_mut()`), để thay đổi ít nhất.
- `LineRef` thay cho `vis_pos` khi nhớ dòng hiện tại, vì `vis_pos` khác nhau giữa Raw và Parsed, và thay đổi khi parse lại.

### 3. Tính dòng của Focus Tab
`focus_rows: Arc<Vec<FocusRow>>` được tính cho tab đang mở:
- Parsed: duyệt `filter_pass` (hoặc mọi entry khi Filter rỗng) và giữ entry mà matcher khớp `raw_line`. Khi cả Filter lẫn Search đều rỗng thì trả về rỗng. `SearchState.update_matches` được gọi với `filter_pass` thay cho `visible_indices`, nên các Match vẫn chính là các dòng này. Vì `vis_pos == entry_ix`, `matched_visible_indices` vẫn đúng cho canvas và ruler.
- Raw: dùng lại `raw.matches` (`RawMatch{section, line}`) từ `compute_raw_matches`, không áp Filter.
- `FocusRow` là `enum { Entry(usize), Raw { section: usize, line: usize } }`, kèm hàm tra text, số dòng và tên Source File, để pane dùng một code render cho cả hai view.
- Chỉ tính cho tab đang mở. Tab khác chỉ được tính khi mở. Không cache theo tab, vì đổi tab không thường xuyên và dùng lại matcher đã compile đủ nhanh.

### 4. Điểm tính lại
Cùng một hàm `refresh_focus()` được gọi từ:
- `apply_filters_and_refresh`, nơi đã bao các trường hợp đổi Filter, khôi phục tab, và parse lại scratch;
- `run_search`, `run_raw_search`, và `run_raw_search_keep_position` (khi sửa scratch);
- đổi Focus Tab, đổi view mode;
- một hàm `on_entries_appended()` để dành cho tail sau này.

Khi tính lại, dòng hiện tại được giữ bằng cách tìm lại `LineRef` trong `focus_rows`.

### 5. Đồng bộ dòng hiện tại
Một hàm `set_current_line(LineRef, origin)`, với `origin` là `View | Pane`, cập nhật `selected_row_pos` hoặc `raw.selected`, `FocusTab.current`, và cuộn bên còn lại.
- Chọn ở view một dòng không có trong tab thì tìm nhị phân dòng gần nhất phía trước trong `focus_rows` để cuộn pane, không đặt selection trong pane.
- Next/prev của Search và Focus Find đều đi qua hàm này, nên không có hai đường đồng bộ khác nhau.

### 6. Focus Find
`FocusFind` dùng `search_expr::parse` và `Matcher` giống Search, và giữ biểu thức hợp lệ gần nhất khi đang gõ dở. `hits` là chỉ số trong `focus_rows` và được tính lại mỗi khi `focus_rows` đổi. Kết quả chỉ được tô trong pane, qua `find_ranges`. Ô nhập là một `InputState` của gpui-component đặt trên dải tab của pane; mỗi tab giữ text của nó và ô được đồng bộ khi đổi tab, giống cách `sync_search_input` làm với ô Search.

### 7. Bố cục và kéo chiều cao
Root trở thành `flex_col[chrome, flex_1 flex_row(sidebar | main | context_sidebar), resize_handle, focus_pane, status_bar]`, nên pane tự nằm dưới cả hai sidebar.
- `FocusPaneState { visible, height }` đặt cạnh `LeftSidebarState` trong src/workspace/state.rs và được lưu cùng app state.
- Kéo chiều cao làm thủ công theo mẫu `ResizeDrawer` của sidebar trái, thay vì dùng `v_resizable` của gpui-component, để cùng một kiểu với code hiện có và để lưu chiều cao vào state của mình.
- Chiều cao bị giới hạn sao cho view còn tối thiểu khoảng 120px.
- `ContextSidebar::render` nhận chiều cao còn lại thay cho `viewport_height`.

### 8. Render Focus Pane
Thêm `src/ui/focus_pane.rs` với một `uniform_list`. Mỗi hàng gồm:
- Gutter: số dòng, và Level Marker khi ở Parsed View;
- tên Source File, chỉ khi view có nhiều file;
- một ô text.

Ô text dùng lại `render_text_cell`/`CellCtx` của canvas (đổi thành `pub(crate)`), với một biến thể `TextCell::Raw` để hiện `raw_line` thay cho `payload`. Cách này cho Marker, phần tô Search, phần tô Focus Find, chọn text và màu Highlight Rule (chỉ khi Parsed) cùng một đường vẽ. Cuộn ngang thì pane có thanh riêng, không dùng chung với canvas.

### 9. Lưu trữ
`FocusTabs` được lưu trên `LogSpace` cạnh Filter: `focus: FocusTabs` cho Log Space và `file_focus: HashMap<String, FocusTabs>` cho File View, đều có `#[serde(default)]` để manifest version 4 vẫn đọc được, và mặc định là một tab 1 trống.
- Chỉ lưu `number`, `query`, `find.query`, `current` và `active`. Matcher và kết quả được tính lại khi mở.
- Lưu cùng chỗ với Filter thay vì trong `TabSessionState`, vì spec yêu cầu nhớ theo view giống Filter.
- Test `search_text_is_not_serialized` phải đổi thành kiểm tra round-trip.

### 10. Inspector
- Bỏ `InspectorTab`. `render_body` nhận `Option<InspectorLine>` gồm text của dòng và các Continuation Line theo sau. Với Parsed View, đó là các entry liền sau có cờ continuation. Với Raw View, đó là các dòng liền sau trong section mà dòng tương ứng ở dạng Parsed là continuation, hoặc chỉ một dòng khi không xác định được.
- Context sidebar hiện ở cả Raw View; khi đó chỉ có mục Inspector, còn Filter và Highlights bị ẩn.

### 11. Status bar
"Filtered x%" đổi thành số dòng của Focus Tab đang mở so với tổng số dòng.

## Risks / Trade-offs

- [Giữ `visible_indices` như một danh sách đầy đủ tốn bộ nhớ O(n)] → Chấp nhận được, vì nó vẫn là một `Vec<usize>` như hiện nay. Dọn sau.
- [Tính `focus_rows` trên file lớn mỗi phím gõ] → Đã có debounce và matcher compile sẵn như Search hiện nay. Nếu chậm thì dùng `filter_pass` làm tập duyệt (nhỏ hơn) và tính ở background.
- [Hai đường search Raw và Parsed lệch nhau] → `FocusRow` gom cả hai về một kiểu để pane chỉ có một code render. Phần còn khác chỉ là nguồn dòng.
- [Đồng bộ hai chiều dễ gây vòng cuộn] → Mọi thay đổi đi qua `set_current_line(origin)`, và chỉ cuộn bên không phải `origin`.
- [Đổi nghĩa Filter là thay đổi lớn với người dùng hiện có] → Đã ghi trong ADR-0008 và CONTEXT.md. Status bar và gợi ý trống của pane nói rõ Filter giờ tác động lên Focus Pane.
- [Change phụ thuộc chưa archive] → Áp dụng change này sau khi `add-text-markers-and-highlight-rules` được archive.

## Migration Plan

- Manifest cũ không có `focus`/`file_focus`: `serde(default)` cho một tab 1 trống. Không bump version.
- Filter đã lưu vẫn giữ nguyên, chỉ đổi chỗ tác động.
- Rollback: bỏ các trường mới. Dữ liệu cũ vẫn đọc được vì không có trường nào bị xóa.

## Open Questions

- Phím tắt bật/tắt Focus Pane: đề xuất `⌘J`. Kiểm tra trùng với keymap hiện có khi cài.
