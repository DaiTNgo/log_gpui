## Why

Để biết chuyện xảy ra lúc nào (lỗi bùng lên, log im bặt, volume tăng bất thường), hiện người dùng phải cuộn và đọc từng dòng. List chỉ cho thấy một màn hình dòng. Minimap đi theo trục dòng, không theo trục thời gian, nên không thấy được một khoảng 10 phút không có dòng nào. Level và timestamp của mọi dòng đã nằm trong bộ nhớ, nên có thể vẽ tổng quan theo thời gian mà không phải đọc lại file.

## What Changes

**Time Histogram**
- Dải ngang mới phía trên list. Mỗi cột là số Parent Line trong một khoảng thời gian, tô theo level: Error/Fatal đỏ, Warn cam, còn lại xám. Continuation Line không được đếm.
- Dải hiện ở mọi view mà các dòng có thời gian và có ít nhất một dòng có giờ: Log Space, File View, Focus Pane, Focus Window. Không hiện ở Raw View hay ở view không có dòng nào có giờ.
- Histogram của Focus Pane dùng chung trục thời gian với view chính.
- Độ rộng cột tự chọn theo các bước 1s, 5s, 10s, 30s, 1m, 5m, 15m, 1h, 6h, 1d. ⌘ + cuộn để zoom, có nút "Xem toàn bộ". Zoom không lọc dòng nào.
- Khi có Filter: cột mờ là tổng số dòng, phần đậm là số dòng qua Filter. Khoảng có Match của Search được đánh dấu bằng vạch dưới cột.
- Khung viền cho biết list chính đang hiện khoảng thời gian nào, và chạy theo khi cuộn.
- Bấm một cột thì nhảy tới dòng đầu của khoảng đó; thao tác nhảy không thu hẹp gì.
- Kéo chọn trên dải thì đặt Time Range, tức là thu hẹp Focus Pane về khoảng đó. Time Range hiện thành dải nền xanh nhạt. Kéo mép dải để đổi Time Range, kéo phần giữa để dời. Bấm vào chỗ trống trên dải thì xoá Time Range.
- Dòng không có thời gian không vẽ thành cột. Một nhãn ở góc dải hiện số dòng này; bấm vào thì nhảy tới dòng đầu tiên trong số đó.
- Live: dòng mới được cộng vào histogram khi tới. Lúc Indexing chưa xong, phần chưa đếm được vẽ mờ.

**Bật/tắt trên màn hình**
- Thanh công cụ của view có nhóm nút Histogram và Minimap, kèm Command trong Command Palette. Trạng thái của hai nút được nhớ toàn cục.
- **BREAKING**: bỏ "Show minimap" khỏi Settings. Giá trị đã lưu vẫn được giữ và giờ do nút trên thanh công cụ điều khiển.

**Ngoài phạm vi (change sau)**
- Swimlane và Stream Column. Change này chỉ giữ trục thời gian đủ chung để swimlane nằm ngay dưới histogram và dùng cùng trục.

## Capabilities

### New Capabilities
- `time-histogram`: Time Histogram cho một view: đếm, tô màu, trục và zoom, khung vùng đang xem, Search và Filter, nhảy, kéo chọn thành Time Range, dòng không có giờ, Live và Indexing, nút bật/tắt Histogram và Minimap.

### Modified Capabilities
- `log-filter`: Time Range cũng được đặt, đổi và xoá từ Time Histogram, và hai ô Từ/Đến hiện đúng giá trị đó.

## Impact

- Code:
  - `src/app/view/mod.rs` (thêm dải vào bố cục, thêm nhóm nút vào thanh công cụ)
  - module UI mới cho Time Histogram, vẽ bằng `gpui::canvas` như minimap trong `src/ui/canvas.rs`
  - `src/ui/focus_pane.rs`
  - `src/app/filter.rs` (`apply_rows_appended`, Time Range)
  - `src/filter.rs` (`set_time_range`)
  - `src/app/go_to.rs` (nhảy tới thời điểm)
  - `src/app/view_settings.rs` và `src/ui/settings.rs` (bỏ Show minimap)
  - `src/workspace/session.rs` (lưu `show_histogram`)
  - `src/app/commands.rs` / `src/keymap.rs` (Command mới)
- Dữ liệu lưu: thêm `show_histogram` vào trạng thái workspace, mặc định bật.
- Dữ liệu dùng lại: `FilterIndex.effective_ts`, level trong `MetaTable` / `LogEntry`, `filter_pass`. Không đọc lại file.
- Docs: thêm Time Histogram vào `CONTEXT.md`, và ADR "kéo chọn trên Time Histogram là Time Range".
