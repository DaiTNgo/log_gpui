## 1. Chuẩn bị

- [ ] 1.1 Kiểm tra tay rồi archive `scratch-color-bar-on-demand` (còn task 3.2), để spec `text-marker` có requirement "Lệnh mở thanh màu cho vùng chọn" trước khi change này sửa tiếp
- [x] 1.2 Thêm `width`, `inspector_share`, `inspector_mode` vào `ContextSidebarLayout` (`WorkspaceState.context_layout`, lưu cùng session), có `serde(default)`. Test: session cũ không có các trường này vẫn đọc được

## 2. Selection Menu

- [x] 2.1 Kiểm tra `gpui_component::input::Editor` có menu chuột phải riêng không, và cách chặn nó. Ghi kết luận vào design (rủi ro thứ hai)
- [x] 2.2 Định nghĩa `SelectionTarget` (text, single_line, pieces, cell, under, filterable) và hàm dựng nó cho Parsed View (vùng chọn hoặc ô dưới con trỏ), Raw list và editor scratch
- [x] 2.3 Viết `src/ui/selection_menu.rs`: Copy; hai hàng swatch Mark here / Highlight everywhere (6 màu = Recent rồi bù bằng palette); Color… kèm phím của `Command::ColorSelection`; Show only / Hide; Highlight rows where…; Remove marker / Remove highlight; Log Format… (nếu change đang mở còn giữ mục này)
- [x] 2.4 Gắn Selection Menu vào ô Parsed View (thay `cell_menu` ở `src/ui/canvas.rs:638`), Raw list và khung editor scratch. Xoá `src/ui/cell_menu.rs`
- [x] 2.5 Bỏ việc tự mở thanh màu khi nhả chuột ở Parsed View (`open_color_bar`) và trong `finish_raw_selection`. Giữ lối recolor khi bấm lên màu có sẵn và lệnh ⌘⇧H
- [x] 2.6 Test: kéo chọn không mở thanh màu (Parsed, Raw list); Mark here và Highlight everywhere tô đúng phạm vi; vùng chọn nhiều dòng không có Highlight everywhere; Remove marker; Raw View không có mục lọc; TIMESTAMP không có Highlight rows

## 3. Lọc từ vùng chọn

- [x] 3.1 Viết hàm áp điều kiện `filter_from_selection(column, value, whole, mode)` theo quy tắc của design (quyết định 4): thêm vào khi cùng chế độ, thay khi khác chế độ
- [x] 3.2 Nối Show only / Hide trong Selection Menu với hàm này. Nhãn ghi đúng điều kiện (`level = error`, `Message ∋ "…"`)
- [x] 3.3 Test: trọn ô → Filter Value; một phần ô → Custom Value; MESSAGE → thẻ Message; thêm vào điều kiện cùng chế độ; thay khi khác chế độ; cột vượt `MAX_DISTINCT` → Custom Value

## 4. Tạm tắt Filter

- [x] 4.1 Thêm `#[serde(skip)] paused` vào `FilterState`. Bộ lọc coi Filter là rỗng khi `paused`. Mọi hàm sửa Filter tự bỏ `paused`
- [x] 4.2 Nút tạm tắt trên header section Filter (chỉ khi Filter khác rỗng). Không có thanh chip Filter ngoài sidebar
- [x] 4.3 Test: tạm tắt cho Focus Pane hiện mọi dòng và giữ điều kiện; sửa điều kiện thì bật lại; `paused` không được lưu

## 5. Context sidebar

- [x] 5.1 Tách `ContextSidebar::render` thành vùng cuộn (Filter, Highlights, Columns) và Inspector ở đáy, có thanh chia kéo được (tối thiểu 120px mỗi bên)
- [x] 5.2 Cạnh trái kéo được để đổi độ rộng (280–560px). Bỏ hằng `CONTEXT_SIDEBAR_WIDTH` ở chỗ vẽ
- [x] 5.3 Header phạm vi: tên file/Log Space và loại view
- [x] 5.4 Header section: phần tóm tắt bên phải và nút "+" / tìm field. Bỏ các dòng "+ Add filter" / "+ Add rule". Columns gập mặc định
- [x] 5.5 Test: Inspector giữ chỗ khi mở form dài; chiều cao và độ rộng được lưu; Columns gập lần đầu

## 6. Filter và Highlights gọn hơn

- [x] 6.1 Viết `render_value_picker` dùng chung: một ô nhập (gõ để lọc, Enter thêm Custom Value), số đếm căn phải. Gộp `search_input` + `value_input` và `rule_search` + `rule_input`
- [x] 6.2 Thẻ Filter dạng tóm tắt một dòng; bấm vào thì mở khối chọn giá trị; lựa chọn hiện / Hide Mode ở cuối khối
- [x] 6.3 Time Range: preset 5m / 1h / 24h / All neo theo timestamp mới nhất của view; dòng tóm tắt; bấm vào mở hai ô Từ/Đến xếp dọc có nhãn
- [x] 6.4 Hàng Highlight Rule: bỏ tay kéo riêng (kéo cả hàng), bỏ nút × thường trực, thêm "⋯" khi hover và menu chuột phải có Delete
- [x] 6.5 Form New rule: 8 màu và "…" mở palette đầy đủ; "+" khi form đang mở không tạo lại form (`open_rule_form`)
- [x] 6.6 Test: Enter trong ô nhập thêm Custom Value; preset 1h tính theo log chứ không theo giờ máy; "+" lần hai giữ nội dung form; xoá rule qua menu

## 7. Inspector

- [x] 7.1 Chế độ Fields: lấy các cột có giá trị của Parent Line đang chọn; dòng không có cột nào parse được thì chỉ có Raw
- [x] 7.2 Header Inspector: số dòng, Fields/Raw, Copy, ↑↓ (đi qua `selected_row_pos`)
- [x] 7.3 Ba nút khi trỏ vào field (Show only, Hide, Highlight rows), chỉ trong Parsed View, dùng `filter_from_selection` với `whole = true`
- [x] 7.4 Test: Fields liệt kê đúng field; Raw giữ text lưu trữ và Continuation Line; Hide từ field tạo đúng điều kiện; ↓ chọn dòng hiển thị kế tiếp

## 8. Tài liệu và kiểm tra

- [x] 8.1 Cập nhật `CONTEXT.md`: mục Marker và Text Highlight (thanh màu không tự mở; Selection Menu), thêm thuật ngữ **Selection Menu**
- [x] 8.2 Chạy các test liên quan tới những phần đã sửa
- [ ] 8.3 Kiểm tra tay trong app: chọn để copy không bật gì; chuột phải đủ mục ở Parsed, Raw list và editor scratch; tạm tắt Filter trên header; Inspector ghim ở đáy khi mở form dài
