## Why

Sau khi thêm vào Remote Log Space, Remote File, Live Glob và Remote Command không sửa được nữa. Gõ nhầm một ký tự trong path, muốn đổi `*.log` thành `app-*.log`, muốn đổi `--tail 1000` thành `--tail 50` hay chuyển cùng file sang Server khác, người dùng đều phải xoá rồi thêm lại. Làm vậy thì mất vị trí trong sidebar, mất File View đang mở và mất Marker, kể cả khi chỉ đổi Starting Time của cùng một file.

## What Changes

- Menu chuột phải trong sidebar của **Remote File**, **Live Glob** và **Remote Command** có thêm mục **"Edit…"**. Mục này mở lại đúng dialog đã dùng khi thêm (file picker hoặc dialog Add remote command), điền sẵn giá trị hiện tại, và nút xác nhận đổi thành **Save**.
- **Remote File**: sửa được path, Server và Starting Time.
  - Chỉ đổi Starting Time: vẫn là file đó. File được nạp lại theo Starting Time mới, Marker được giữ.
  - Đổi path hoặc Server: thành một file khác. Dòng cũ và Marker của file cũ bị bỏ. File mới giữ chỗ cũ trong sidebar và trong thứ tự file, File View đang mở chuyển sang file mới, rồi file mới được đọc như khi vừa thêm (kể cả trạng thái "Waiting for file").
- **Live Glob**: sửa được pattern, thư mục và Server. Trước khi Save, dialog hiện kết quả so với hiện tại, ví dụ `+3 files · −2 files · 5 kept`:
  - File vẫn khớp được giữ nguyên, kể cả dòng, Marker và việc đọc đang chạy.
  - File không còn khớp bị gỡ, như "Remove from Log Space". Đây không phải trạng thái Gone.
  - File mới khớp được thêm, theo giới hạn số file của Live Glob.
  - Live Glob đang là Stopped Live Glob thì vẫn Stopped: chỉ gỡ file không còn khớp, chờ Resume mới thêm file mới.
- **Remote Command**: sửa được lệnh, giá trị của Command Preset và Server. Save sẽ kết thúc Run đang chạy, **giữ dòng và Marker cũ**, vẽ Break "command changed" ghi lệnh cũ và lệnh mới, rồi chạy lệnh mới thành Run mới bắt đầu từ dòng 1, giống như Restart. Quy tắc "sửa tay lệnh của preset thì không được Resume By Time" vẫn giữ như lúc thêm. Restart on exit giữ nguyên giá trị.
- **Quy tắc chung**:
  - Save chỉ áp dụng khi giá trị mới hợp lệ, với cùng kiểm tra như lúc thêm: kết nối được Server, thư mục cha tồn tại, không trùng với Remote File, Live Glob hay Remote Command khác của Log Space. Nếu không hợp lệ thì không đổi gì và dialog báo lỗi.
  - Không đổi gì thì nút Save bị tắt.
  - Giá trị mới được lưu vào Project và thêm vào Recent Paths của Server.
- Remote Copy không có "Edit…", vì nó đã có "Read again". Imported Log Space và Scratch Log Space cũng không thay đổi gì.

**Ngoài phạm vi:**
- Sửa Log Format hay cách đọc dòng (thuộc Giai đoạn 2 của `custom-columns`).
- Chọn encoding hay múi giờ cho file.
- Sửa nhiều file cùng lúc.
- Undo.
- Chuyển một Remote File thành Live Glob hay ngược lại. Muốn vậy thì gỡ rồi thêm.

## Capabilities

### New Capabilities
- `remote-source-edit`: sửa Remote File, Live Glob và Remote Command đã có trong Remote Log Space, gồm cách xử lý dòng, Marker, File View và trạng thái live sau khi sửa.

### Modified Capabilities
<!-- Các spec về Remote Log Space (`remote-log-space`, `remote-command`, `remote-starting-time`) còn nằm trong những change chưa archive, nên change này không viết delta cho chúng. Mọi hành vi mới đều nằm trong `remote-source-edit`. -->

## Impact

- `src/ui/space_menu.rs`: thêm "Edit…" vào `glob_menu` và `file_menu` (nhánh Remote và Remote Command).
- `src/app/remote_picker.rs`: thêm `PickerTarget::Edit { space_id, member }`, điền sẵn `Staged`, chỉ cho chọn một mục, Save thay cho Done, và preview `+/−/kept` cho glob.
- `src/app/remote_command.rs` và `src/app/view/command_views.rs`: chế độ sửa của dialog Add remote command.
- `src/workspace/log_space.rs`: thêm `replace_remote_target`, `edit_live_glob` và `set_start_time`, giữ `file_id`, `added_seq` và chỗ trong thứ tự file.
- `src/app/live.rs`, `src/app/globs.rs` và `src/remote/glob_plan.rs`: đóng và mở lại việc đọc, lập kế hoạch khớp lại cho glob đã sửa.
- `src/app/tabs.rs`: File View và Pinned Tab đi theo file đã thay. Tab của file bị gỡ khi glob đổi thì đóng.
- Dữ liệu Project vẫn theo dạng cũ, không cần migration.
