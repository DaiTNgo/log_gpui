## Why

Sidebar có nhiều Project, mỗi Project có nhiều Log Space. Log Space chỉ có 1 file (upload một file log, hoặc thêm một Remote File) vẫn hiện chevron và một file row con, nên mỗi Space tốn hai dòng và chevron không mở ra gì đáng kể. Chevron chỉ có nghĩa khi Space là nhóm của nhiều file.

## What Changes

- **Chevron của Log Space** chỉ hiện khi Space có từ 2 file trở lên, hoặc có Live Glob. Áp dụng cho Imported, Scratch và Remote Log Space.
- **Log Space 1 file** (không có Live Glob) vẽ thành một dòng duy nhất trong sidebar: tên Space, không chevron, không file row con. Click vẫn mở Space canvas. Space vẫn là Space (tên, loại, thứ tự file, tab), chỉ cách vẽ khác.
- **Thêm hoặc bớt file**: từ 1 lên 2 file thì dòng thành nhóm có chevron. Từ 2 về 1 thì co lại thành dòng đơn. Trạng thái expand đã lưu (`expanded_spaces`) được giữ nhưng bị bỏ qua khi chưa đủ điều kiện hiện chevron.
- **Space có Live Glob** luôn là nhóm có chevron, kể cả khi glob chỉ khớp 1 file, vì header của glob mang pattern, Stopped và "waiting for folder".
- **Status bar** của Space 1 file hiện thông tin file duy nhất và các nút liên quan đến live: **Load more**, **Clear**, **Live/Pause** và **Back to Live Window**. Nút nào không áp dụng cho file đó thì không hiện (ví dụ Remote Command không có Load more).
- **Menu chuột phải của dòng Space 1 file** gộp phẳng các action của file duy nhất (Download, Download file, copy path, Show in Finder, ...) cùng các mục của Space, và có mục xóa. Xóa file duy nhất xóa luôn Space: đóng tab của Space, dọn Remote Copy. Scratch Log Space hỏi xác nhận trước, vì xóa file thật trong app data folder.
- Space nhiều file giữ nguyên menu file và file row như hiện tại.

**Ngoài phạm vi:**
- Raw View: đã ẩn File Section khi chỉ có 1 file (`src/app/view/raw_view.rs`), không đổi.
- Collapse của Live Glob group.
- Đổi cách lưu Project hay `expanded_spaces`.
- Space 0 file (Imported hoặc Remote sau khi xóa file cuối): xử lý như hiện tại.

## Capabilities

### New Capabilities
- `sidebar-space-row`: khi nào Log Space hiện chevron và file row, cách vẽ Space 1 file, vị trí của action file (status bar và menu chuột phải) và việc xóa file duy nhất.

### Modified Capabilities
<!-- `log-space` chưa có delta cho sidebar; mọi hành vi mới nằm trong `sidebar-space-row`. -->

## Impact

- `src/ui/sidebar.rs`: quyết định vẽ file row con và Live Glob theo `ordered_files().len()` và `space.globs`.
- `src/ui/space_menu.rs`: `SpaceRef` có cờ "một file", ẩn chevron, và menu của dòng đơn gộp action của file.
- `src/app/spaces.rs`: xóa file duy nhất xóa Space (`remove_file_from_space` và luồng xác nhận Scratch).
- Status bar (view của app): nút Load more, Clear, Live/Pause, Back to Live Window cho Space 1 file.
- Không đổi định dạng lưu Project.
