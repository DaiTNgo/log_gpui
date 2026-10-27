## Why

Loại Log Space hiện nay chia theo nơi file nằm: Imported cho file local, Remote cho file qua SSH. Nhưng điều người dùng muốn chọn là hành vi: xem một bản cố định, hay theo dõi file đang được ghi. Vì chia theo nơi file nằm nên file local trong Imported vừa phải "chỉ đọc" vừa phải theo dõi (line-offset-index 4b.7). Khi file local bị replace, app lại phải đoán đó là rotate hay là thay file, và đoán không chắc được. Chia theo Static / Live thì người dùng nói trước ý định, và app không phải đoán nữa.

## What Changes

- **BREAKING**: hai loại Imported và Remote được thay bằng **Static** và **Live**. Scratch giữ nguyên.
  - Static gồm file local, Archive Entry và Remote Copy.
  - Live gồm file local và Remote File (cả Live Glob và Remote Command), trộn được trên cùng một timeline.
- **Static Log Space** là một bản cố định:
  - Không có Live/Pause, Follow, Break hay Generation.
  - Một file local đổi trên đĩa (ghi thêm, bị thay bằng inode mới, hay bị ghi đè cùng inode) được đọc lại ở nền và giữ Filter.
  - Marker được giữ khi dòng còn khớp `line_hash`; Marker không còn khớp thì bị bỏ, kèm một notice.
  - Remote Copy chỉ đọc một lần và có lệnh Reload. Không giữ kết nối SSH để theo dõi.
- **Live Log Space** theo dõi file:
  - Có Live/Pause, Follow và Generation.
  - File local bị rotate, replace, truncate hay ghi đè mở một Generation mới kèm Break. Dòng của Generation cũ vẫn đọc được qua handle đang mở.
  - Load more, Live Window, Back to Live Window, "N lines held" và Clear chỉ có với Remote File, không bao giờ có với file local.
- **BREAKING**: thanh chọn của dialog New Log Space đổi từ `[ Local files | Remote | Scratch ]` thành `[ Static | Live | Scratch ]`.
  - Nấc Static nhận file, thư mục và Archive.
  - Nấc Live nhận file local, cộng thêm "Add remote files…" và "Add remote command…".
- Thả file vào cửa sổ khi không có dialog nào mở thì tạo một Static Log Space.
- Session đã lưu vẫn mở được: `Imported` được đọc thành `Static`, `Remote` được đọc thành `Live`.
- Live/Pause và Follow của file local, vốn đặt ở Imported trong line-offset-index 4b.7, chuyển sang Live Log Space.

## Capabilities

### New Capabilities

- `static-log-space`: Static Log Space đọc lại file local khi file đổi trên đĩa, giữ Marker còn khớp, và có Reload cho Remote Copy.
- `live-log-space`: Live Log Space theo dõi file local và Remote File trên một timeline. File local có Generation và Break khi rotate, replace hoặc truncate. Các thao tác chỉ dành cho Remote File.

### Modified Capabilities

- `log-space`: thanh chọn loại có ba nấc Static, Live, Scratch, và điều kiện Create đi theo từng nấc. Thả file vào cửa sổ tạo Static Log Space. Các requirement của Imported đổi thành của Static.

## Impact

- `src/workspace/log_space.rs`: `LogSpaceKind { Static, Live, Scratch }`, có serde alias cho tên cũ.
- `src/workspace/new_space.rs`, `src/app/spaces.rs`, `src/app/view/dialogs.rs`, `src/app/keyboard.rs`: thanh chọn và nội dung từng nấc. Nấc Live gộp danh sách local của form với danh sách remote của `RemoteState`.
- `src/ui/space_menu.rs`, `src/ui/sidebar.rs`, `src/app/view/status_bar.rs`: menu và status bar theo loại space, và theo loại file trong một Live space.
- `src/app/live.rs`, `src/app/loading.rs`, `src/live/local.rs`, `src/index/file.rs`:
  - file local của Static không có source live, chỉ có bộ phát hiện thay đổi;
  - file local của Live có Generation, Break và handle của Generation cũ;
  - thêm `BreakKind::Replaced` và `BreakKind::Rewritten`.
- `src/app/open_in_full.rs`, `src/app/full_read.rs`: Remote Copy vào Static Log Space, thêm Reload.
- `src/app/tabs.rs`, `src/app/remote.rs`, `src/app/globs.rs`, `src/app/raw.rs`: các nhánh theo `LogSpaceKind`.
- `CONTEXT.md`: đổi Imported Log Space thành Static Log Space, Remote Log Space thành Live Log Space; định nghĩa thêm Live File local (xem design.md).
- Liên quan tới change `line-offset-index`: phần 4b.7 và 9.2 của change đó đi theo change này.
- Test: phần lớn các test có `LogSpaceKind::Imported` hoặc `LogSpaceKind::Remote` (26 file), test dialog và test live của file local.
