## Context

Context sidebar (`src/ui/context_sidebar.rs`) là một cột rộng 320px. Bốn section xếp chồng trong cùng một vùng cuộn, Inspector chiếm phần còn lại. Nội dung từng section nằm ở `src/app/view/{filter_panel,highlights_panel,columns_panel}.rs` và `src/ui/inspector.rs`.

Thanh màu (`ColorBar`, `ColorTarget` trong `src/app/color.rs`) hiện mở theo các lối sau:

```
Parsed View      nhả chuột sau khi kéo     → MarkSelection        (color.rs open_color_bar)
Raw list (đọc)   finish_raw_selection()    → RawMark              (raw_list.rs ~283)
Raw editor       vùng chọn khác rỗng       → (đã bỏ, scratch-color-bar-on-demand)
mọi view         con trỏ trên màu có sẵn   → RawRecolor / RecolorMarker / RecolorTextHighlight
mọi view         ⌘⇧H                       → MarkSelection / RawMark  (Command::ColorSelection)
```

Hiện chỉ ô của Parsed View có menu chuột phải (`src/ui/cell_menu.rs`, gắn trong `src/ui/canvas.rs:638`). Raw View không có. Spec hiện tại cấm lệnh lọc trong menu chuột phải (`log-filter`: "Không còn các điểm vào cũ"; `highlight-rule`: "Tạo nhanh từ ô"). Raw View không áp dụng Filter (`raw-view`: "Raw View hiện đúng nội dung file").

## Goals / Non-Goals

**Goals:**
- Chọn text chỉ để chọn. Mọi hành động trên vùng chọn nằm trong một menu chuột phải dùng chung.
- Lọc được từ vùng chọn và từ Inspector, không cần mở sidebar.
- Luôn thấy Filter đang bật ngay trên bảng.
- Inspector luôn nằm trong màn hình. Sidebar gọn: section chỉ hiện tóm tắt cho tới khi mở ra.

**Non-Goals:**
- Không thêm preset Filter hay "Saved View" (spec `log-filter` vẫn cấm).
- Không áp Filter vào Raw View.
- Không đổi cách lưu Filter, Highlight Rule, Marker hay Text Highlight.
- Không làm phím tắt một chữ (`f`, `x`, `h`) trên ô đang chọn. Để change sau.

## Decisions

**1. Một hàm `selection_menu` dựng menu cho mọi view, từ một `SelectionTarget`.**
Lúc chuột phải, view dựng một `SelectionTarget` gồm:
- `text`: text đã chọn
- `single_line`: vùng chọn có nằm trong một dòng không
- `pieces`: đúng dữ liệu mà `ColorTarget::MarkSelection` / `RawMark` cần
- `cell`: `Option<(column_key, value_folded, whole)>`. `whole` là true khi vùng chọn phủ trọn giá trị ô.
- `under`: Marker hoặc Text Highlight dưới con trỏ, nếu có
- `filterable`: chỉ true ở Parsed View

`cell_menu.rs` được thay bằng `src/ui/selection_menu.rs`. Khi chuột phải mà không có vùng chọn, ô dưới con trỏ được coi như vùng chọn phủ trọn giá trị ô. Như vậy chuột phải lên ô vẫn có "Highlight rows where …" và "Show only", như cell menu cũ.
Phương án khác: giữ cell menu và thêm menu riêng cho vùng chọn. Không chọn, vì hai menu cho cùng một chỗ sẽ khác nhau tuỳ có bôi đen hay không.

**2. Bỏ mọi lối tự mở thanh màu theo vùng chọn; giữ lối recolor và ⌘⇧H.**
Bỏ lời gọi mở thanh trong `open_color_bar` (nhả chuột ở Parsed View) và trong `finish_raw_selection`. Bấm (không kéo) lên màu có sẵn vẫn mở thanh recolor. "Color…" trong menu gọi đúng đường của `Command::ColorSelection`.

**3. Hàng màu trong menu: 6 màu = Recent rồi bù bằng palette mặc định.**
"Mark here" tô với `MarkScope::Line`, "Highlight everywhere" với `MarkScope::Everywhere`. "Highlight everywhere" chỉ có khi `single_line`, cùng điều kiện mà thanh màu đang dùng. Không hiện số đếm, vì Text Highlight áp cho cả Project nên con số của view hiện tại dễ gây hiểu sai. Mỗi hàng là một `PopupMenuItem` tuỳ biến chứa các swatch. Bấm swatch thì áp màu, thêm vào Recent và đóng menu.

**4. Lọc từ vùng chọn: chọn trọn ô thì tick Filter Value, chọn một phần thì thêm Custom Value.**

| Vùng chọn | Show only / Hide tạo ra |
|---|---|
| Trọn giá trị một ô của cột lọc được | Filter Value đó trên cột đó |
| Một phần ô, hoặc cột vượt `MAX_DISTINCT` | Custom Value `text` trên cột đó |
| Trong MESSAGE | Custom Value `text` trên thẻ Message |
| Qua nhiều ô hoặc nhiều dòng | Không có mục lọc |

Áp vào điều kiện đang có của cột: nếu cùng chế độ (show/hide) thì thêm giá trị vào. Nếu khác chế độ thì thay cả điều kiện của cột bằng giá trị mới với chế độ mới. Nhãn ghi đúng điều kiện: `Show only Message ∋ "POST /checkout"`, `Hide source = web`.
Phương án khác: luôn thay điều kiện của cột. Không chọn, vì "Show only error" rồi "Show only warn" thường là muốn cả hai.

**5. Filter Bar: chip theo từng cột có điều kiện, cộng Time Range.**
`src/app/view/filter_bar.rs` vẽ một hàng trên Parsed View. Hàng này chỉ hiện khi Filter của view khác rỗng. Chip gồm `cột: giá trị, giá trị` (rút gọn sau 2 giá trị), hoặc `not cột: …` cho Hide Mode, và `time: 10:30 → 10:35`. × xoá điều kiện của cột đó. Bấm vào thân chip thì mở sidebar, mở section Filter và bung đúng thẻ đó (`reveal_context_section` đã có). Cuối hàng có "Clear" và nút tạm tắt.

**6. Tạm tắt Filter là cờ không lưu trên `FilterState`.**
Thêm `#[serde(skip)] paused: bool`. Khi `paused`, bộ lọc coi như rỗng, Filter Bar và header Filter hiện "paused", chip bị gạch ngang. Mọi thao tác sửa Filter tự bỏ `paused`. Cờ không được lưu, nên mở lại view thì Filter đang bật.
Phương án khác: lưu cờ. Không chọn, vì mở lại một view thấy đủ dòng trong khi sidebar vẫn ghi có Filter sẽ gây nhầm.

**7. Sidebar: vùng cuộn ở trên, Inspector ở dưới, ngăn bằng thanh chia.**
Filter / Highlights / Columns nằm chung một vùng cuộn. Inspector nằm ngoài vùng cuộn, cao `inspector_height` (mặc định 45% chiều cao, tối thiểu 120px), kéo thanh chia để đổi. Cạnh trái sidebar kéo được để đổi `width` (280–560px, mặc định 320px). Cả hai giá trị lưu trong `ContextSidebarLayout` (trường `context_layout` của `WorkspaceState`, ghi cùng session như `focus_pane`), có default khi đọc session cũ. `ContextSidebarState` không được lưu nên không dùng nó. Chiều cao Inspector lưu dưới dạng tỉ lệ (`inspector_share`) để không phụ thuộc chiều cao cửa sổ. Gập Inspector thì vùng cuộn chiếm hết. Ở Raw View chỉ có Inspector, như hiện nay.

**8. Section dạng tóm tắt.**
- Header: tên section, phần tóm tắt bên phải ("2 active", "2 rules", "6 of 9 shown") và nút "+" (hoặc 🔍 ở Columns).
- Bỏ các dòng riêng "+ Add filter" và "+ Add rule".
- Thẻ Filter mặc định gập thành một dòng: `Level  error, warn  ×`.
- Hàng Highlight Rule: swatch, mô tả một dòng, số khớp, công tắc. Xoá qua menu chuột phải của hàng hoặc nút "⋯" hiện khi hover. Kéo cả hàng để đổi thứ tự (bỏ tay kéo riêng).
- Columns gập mặc định khi chưa có trạng thái lưu.

**9. Khối chọn giá trị dùng chung.**
Hàm `render_value_picker(column, ticked, input, on_toggle, on_add)` dùng cho thẻ Filter và form New rule. Chỉ có một ô nhập: gõ chữ thì lọc danh sách, Enter thêm Custom Value. Giá trị và số đếm căn phải. Như vậy gộp được `search_input` + `value_input` của Filter và `rule_search` + `rule_input` của Highlights. Danh sách giữ giới hạn 220px như hiện nay.

**10. Time Range: preset neo theo timestamp mới nhất của view, không theo đồng hồ máy.**
5m / 1h / 24h đặt Từ = timestamp mới nhất trong view trừ đi khoảng đó, Đến để trống. "All" xoá Time Range. Lý do là log thường được mở sau khi sự việc đã xảy ra, nên "1h trước giờ máy" thường cho ra view rỗng. Dưới các preset là một dòng tóm tắt (`10:30:12 → end`). Bấm vào thì mở hai ô Từ/Đến xếp dọc, có nhãn cố định. Giá trị do histogram đặt (`add-time-histogram`) hiện trong cùng dòng tóm tắt.

**11. Inspector Fields / Raw.**
- **Fields** lấy các cột đã parse của Parent Line đang chọn: cột built-in, cột của Log Format và Detected Field có giá trị.
- **Raw** là editor chỉ đọc hiện nay.
- Mỗi field có ba nút hiện khi hover: Show only, Hide (theo quy tắc ở quyết định 4, coi như chọn trọn ô) và Highlight rows (mở form New rule đã điền sẵn).
- Dòng không parse được, hoặc dòng của Raw View không có Log Format, chỉ có Raw.
- Chế độ được nhớ trong `ContextSidebarLayout`, cùng chỗ với độ rộng.
- ↑↓ chọn dòng trước/sau trong view đang hiện, đi qua `selected_row_pos` có sẵn.

**12. Bảng màu của form New rule: 8 màu và nút "…".**
8 màu là Recent rồi bù bằng palette. "…" mở palette đầy đủ ngay trong form. Màu mặc định vẫn là `next_rule_color`. "+ Add rule" khi form đang mở chỉ đưa focus về form, không tạo lại form.

## Risks / Trade-offs

- **Người dùng quen thanh màu tự bật thấy mất tính năng.** → "Mark here" vẫn chỉ cần một bước sau chuột phải. ⌘⇧H vẫn mở thanh. Ghi vào release notes.
- **Editor scratch (`gpui_component::input::Editor`) có menu chuột phải riêng, là menu native của hệ điều hành (`NativeMenu`), không vẽ được hàng ô màu.** → Đã kiểm tra (task 2.1): tạo `EditorState` với `.context_menu(false)` để tắt menu đó, rồi gắn Selection Menu (`ContextMenuExt`) vào khung bọc editor. Khi menu tắt, chuột phải không dời con trỏ, nên Selection Menu đọc vùng chọn hoặc vị trí con trỏ hiện có.
- **Menu dài (tới 10 mục).** → Mục chỉ hiện khi dùng được: lọc chỉ ở Parsed View, "Highlight rows" chỉ khi chọn trọn ô, "Remove" chỉ khi có màu dưới con trỏ.
- **Mâu thuẫn với các change chưa archive.** → Archive `scratch-color-bar-on-demand` trước khi bắt đầu (task 1.1). Requirement Inspector trong `add-focus-pane` được thay ở đây; khi archive `add-focus-pane` phải giữ bản của change này.
- **Preset Time Range trên view live.** → Preset chỉ tính một lần lúc bấm, không trượt theo dòng mới. Chip ghi rõ giờ bắt đầu.

## Migration Plan

Không có migration dữ liệu. `WorkspaceState` và session thêm `context_layout` (`width`, `inspector_share`, `inspector_mode`), có default. Session cũ đọc được bình thường. Muốn rollback thì revert commit; các trường thừa bị bỏ qua khi đọc.

## Open Questions

- Filter Bar có nên hiện cả khi sidebar đang mở không? Đề xuất: có, vì nó nằm trên bảng chứ không thuộc sidebar.
- Có cần setting để bật lại thanh màu tự mở cho người quen dùng không? Đề xuất: chưa, chờ phản hồi.
