## Why

Context sidebar đang dồn mọi thứ thành một cột cuộn chung. Khi mở form "New rule" hay một thẻ Filter, Inspector (phần người dùng nhìn nhiều nhất) bị đẩy ra khỏi màn hình. Time Range bị cắt chữ trong ô rộng 140px. Filter và Highlights làm cùng một việc (chọn cột, tick giá trị, thêm "contains") nhưng mỗi bên một kiểu. Bấm lại "+ Add rule" khi form đang mở thì xoá trắng form.

Muốn lọc thì phải mở sidebar và đi qua bốn bước. Đóng sidebar thì không còn thấy đang lọc gì. Thanh màu tự bật mỗi lần chọn text, trong khi người dùng chọn text chủ yếu để copy. Vì thanh bật sẵn, menu chuột phải hầu như không được dùng, và không có chỗ nào để lọc ngay từ đoạn text vừa chọn.

## What Changes

- **BREAKING (hành vi)**: Chọn text không còn tự mở thanh màu, ở mọi view (Parsed View, Raw View chỉ đọc, editor scratch). Chuột phải vào vùng chọn mở **Selection Menu**:
  - Copy
  - **Mark here** với 6 màu
  - **Highlight everywhere** với 6 màu, không có số đếm
  - **Color…** (⌘⇧H): thanh màu đầy đủ
  - **Show only** / **Hide** theo cột
  - **Highlight rows where <cột> = <giá trị>…**, chỉ khi chọn trọn giá trị một ô
  - **Remove marker** / **Remove highlight**, khi vùng chọn nằm trên màu có sẵn
- Bấm (không kéo) lên Marker hay Text Highlight có sẵn vẫn mở thanh màu để đổi màu hoặc gỡ, như hiện nay.
- Raw View có menu chuột phải lần đầu: dùng chính Selection Menu.
- **BREAKING (spec)**: Menu chuột phải được có lệnh lọc. Spec hiện tại cấm điều này ("Menu chuột phải SHALL NOT có lệnh lọc").
- Filter vẫn chỉ nằm trong context sidebar, vì nó chỉ lọc Focus Pane còn bảng luôn hiện toàn bộ log. Header của section Filter có nút tạm tắt toàn bộ Filter mà không xoá điều kiện nào.
- Context sidebar sắp xếp lại:
  - Nút "+" nằm trên header của section.
  - Mỗi điều kiện Filter là một dòng tóm tắt, bấm vào mới mở chi tiết.
  - Time Range có preset 5m / 1h / 24h / All và một dòng tóm tắt mở ra hai ô Từ/Đến xếp dọc.
  - Hàng Highlight Rule gọn hơn: xoá qua menu, kéo cả hàng để đổi thứ tự.
  - Columns gập mặc định.
  - Inspector ghim ở đáy với thanh chia kéo được.
- Inspector có hai chế độ: **Fields** (key → value, mỗi field có nút lọc, ẩn và highlight) và **Raw** (text lưu trữ như hiện nay). Thêm nút Copy.
- Form "New rule" và thẻ Filter dùng chung một khối chọn giá trị: một ô vừa tìm vừa thêm "contains", số đếm căn phải. Bảng màu của form rút còn 8 màu.
- Sửa lỗi: "+ Add rule" khi form đang mở không xoá form nữa.

## Capabilities

### New Capabilities
- `context-sidebar`: bố cục context sidebar (các section tóm tắt, Inspector ghim ở đáy, đổi độ rộng), và Inspector chế độ Fields/Raw cùng các nút trên field.
- `selection-menu`: menu chuột phải trên vùng chọn, dùng chung cho Parsed View và Raw View.

### Modified Capabilities
- `text-marker`:
  - Thanh màu không còn tự mở khi chọn text; chỉ mở bằng Selection Menu ("Color…"), lệnh ⌘⇧H hoặc khi bấm lên màu có sẵn.
  - Mark và Highlight everywhere được làm thẳng từ Selection Menu.
- `log-filter`:
  - Thêm tạm tắt Filter, và lọc từ vùng chọn / Inspector.
  - Bỏ điều cấm lệnh lọc trong menu chuột phải.
  - Time Range có preset; thẻ Filter hiện dạng tóm tắt.
- `highlight-rule`:
  - Mục Highlights gọn hơn: nút "+" trên header, xoá qua menu, form dùng chung khối chọn giá trị.
  - "Highlight rows where…" chuyển vào Selection Menu.
- `raw-view`: Raw View có menu chuột phải (Selection Menu); bỏ việc tự mở thanh màu.

## Impact

- **UI**:
  - `src/ui/context_sidebar.rs`, `src/ui/inspector.rs`
  - `src/app/view/filter_panel.rs`, `highlights_panel.rs`, `columns_panel.rs`, `color_bar.rs`
  - Mới: `src/ui/selection_menu.rs` (thay `cell_menu.rs`)
- **Mở thanh màu**:
  - `src/app/color.rs`: `open_color_bar`, nhánh `MarkSelection`
  - `src/app/raw_list.rs`: `finish_raw_selection`
  - `src/app/raw.rs`: đoạn đồng bộ của editor
- **Filter**: `src/filter.rs` (cờ tạm tắt, không lưu), `src/app/filter.rs`, `src/app/highlights.rs`
- **Change đang mở**:
  - `scratch-color-bar-on-demand` (chưa archive) cũng sửa requirement "Thanh màu nổi khi chọn text"; change này thay thế phần đó. Cần archive change đó trước, hoặc gộp vào.
  - `add-focus-pane` có requirement "Inspector SHALL NOT có tab Fields". Change này đảo lại quyết định đó.
  - `add-time-histogram` đặt Time Range từ histogram; preset ở đây ghi vào cùng Time Range.
- Không đổi dữ liệu lưu, trừ độ rộng sidebar và chiều cao Inspector (thêm vào `ContextSidebarState`, có giá trị mặc định).
