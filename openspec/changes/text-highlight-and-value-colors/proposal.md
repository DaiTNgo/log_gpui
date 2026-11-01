## Why

Marker chỉ tô đúng một chỗ trên một dòng. Muốn lần theo một id (`conn-7f3a`, `req-91bc`) qua hàng nghìn dòng thì người dùng không có cách nào làm mọi chỗ trùng nổi lên trên view chính: Search Highlight chỉ tô trong Focus Pane, còn Highlight Rule tô cả row theo giá trị cột. Trong một Log Space gộp nhiều file, các luồng (thread, request, host) đan xen mà không có màu nào giúp mắt tách chúng ra, trong khi lnav tô tự động mỗi giá trị định danh một màu ổn định và klogg cho tô mọi chỗ có đoạn text đã chọn.

## What Changes

- Thanh màu nổi cạnh vùng chọn có thêm lựa chọn phạm vi: **"Chỉ dòng này"** tạo Marker như hiện nay, **"Mọi chỗ có “<text>”"** tạo Text Highlight. Vẫn một thao tác, một thanh màu.
- **Text Highlight** (khái niệm mới): một đoạn text và một màu, thuộc Project. Mọi chỗ có đoạn text đó (không phân biệt hoa thường, không regex) được tô nền, trong cả Raw View lẫn Parsed View, ở mọi Log Space của Project, kể cả dòng mới tới trong Live Log Space.
- Menu chuột phải trên chỗ đang có Text Highlight có "Change color" và "Remove highlight", giống Marker. Không có danh sách Text Highlight ở sidebar.
- **Value Color** (khái niệm mới, kiểu lnav): app tự tô **màu chữ** của mỗi giá trị cell theo hash của giá trị, cùng giá trị luôn cùng màu ở mọi file, mọi view, mọi phiên. Áp dụng cho mọi cột trừ timestamp, message/payload, level và cột kiểu Number/Boolean. Người dùng không bật/tắt theo cột và không đổi màu từng giá trị; chỉ có một công tắc toàn app trong Settings, bật mặc định.
- Thứ tự lớp màu được định nghĩa lại: Marker > Text Highlight > Value Color > Highlight Rule. Người dùng tô thì có nền; app tô thì chỉ đổi màu chữ.
- Highlight Palette và hàng Recent dùng chung cho Text Highlight.
- CONTEXT.md thêm Text Highlight và Value Color; ADR mới ghi lý do giữ Marker bên cạnh Text Highlight và vì sao Value Color không cho người dùng can thiệp.

## Capabilities

### New Capabilities
- `text-highlight`: tô mọi chỗ có một đoạn text bằng một màu, tạo từ thanh màu nổi, đổi màu và gỡ bằng chuột phải, thuộc Project.
- `value-color`: màu chữ tự động theo hash cho giá trị của các cột định danh, có công tắc toàn app.

### Modified Capabilities
- `text-marker`: thanh màu nổi có lựa chọn phạm vi (Marker hoặc Text Highlight); Marker nằm trên Text Highlight và Value Color; menu chuột phải phân biệt Marker và Text Highlight.
- `highlight-palette`: palette, color picker và hàng Recent dùng chung cho cả Text Highlight.

## Impact

- `src/app/color.rs`, `src/app/view/color_bar.rs`: `ColorTarget` và thanh màu có lựa chọn phạm vi, thêm đích Text Highlight.
- `src/ui/cell_menu.rs` và menu chuột phải của Raw View: thêm "Change color" / "Remove highlight".
- `src/workspace/project.rs`: Project lưu danh sách Text Highlight (serde, `#[serde(default)]` để session cũ vẫn mở được).
- `src/ui/canvas.rs` và phần vẽ Raw View: vẽ Text Highlight và Value Color theo đúng thứ tự lớp; overview ruler không đổi (chỉ Marker và Match).
- `src/search_color.rs`: dùng lại `hue_for_key` / `phrase_rgb` (FNV-1a, cố định qua phiên) để sinh màu chữ cho Value Color.
- `src/ui/settings.rs`, workspace state: thêm công tắc Value Colors.
- `CONTEXT.md`, `docs/adr/0016-…`.
- Không đổi định dạng file log, Index hay Filter.
