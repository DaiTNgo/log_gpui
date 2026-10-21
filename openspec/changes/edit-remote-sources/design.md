## Context

Remote Log Space có ba loại mục, cùng lưu là `LogFile` với `remote: Some(RemoteRef)`:
- **Remote File**: `RemoteTarget::File { path }`, kèm `start_time`.
- **Remote Command**: `RemoteTarget::Command { command, preset, restart_on_exit }`.
- File do **Live Glob** đưa vào: là Remote File có `from_glob = Some(glob_id)`. Bản thân `LiveGlob { server_id, dir, pattern, excluded, stopped }` được lưu riêng trong Log Space.

Hiện mỗi loại chỉ thêm và gỡ được (`add_live_glob`, `remove_live_glob`, `remove_file_from_space`). Việc đọc live đi theo `file_id`:
- `needed_files()` cho biết cần những source nào.
- `sync_live_sources()` và `start_missing_sources()` mở source còn thiếu.
- `forget_file()` đóng source và xoá các cache theo `file_id`.

Tab, Pinned Tab, Marker (`StoredMarker` trong `LogFile`), `parsed_files` và các cache của Raw View cũng đều theo `file_id`.

Dialog thêm có sẵn:
- File picker (`remote_picker.rs`) với `PickerTarget::{NewSpace, AddTo}` và `Staged::{File, Glob}`.
- Dialog Add remote command (`remote_command.rs`, `command_views.rs`).

## Goals / Non-Goals

**Goals:**
- Sửa tại chỗ một Remote File, Live Glob hoặc Remote Command bằng chính dialog đã dùng để thêm.
- Sau khi sửa, giữ chỗ trong sidebar, giữ thứ tự file, và giữ tab cùng Pinned Tab.
- Chỉ bỏ dòng và Marker khi nguồn thực sự đổi. Đổi Starting Time hay thu hẹp glob thì không mất gì không cần mất.
- Giá trị mới không hợp lệ thì không thay đổi gì.

**Non-Goals:**
- Sửa Log Format hay cách đọc dòng.
- Sửa nhiều mục cùng lúc, Undo.
- Đổi loại mục (file ↔ glob ↔ command).
- Sửa Remote Copy.

## Decisions

### 1. Giữ `file_id` khi thay nguồn, đóng và mở lại source

Khi path hay Server của Remote File đổi, mục được giữ nguyên `file_id`, `added_seq` và vị trí trong `files`. Chỉ `remote.target`, `remote.server_id` và `display_name` đổi. Sau đó app:
1. gọi `forget_file(file_id)` để đóng source cũ và xoá cache;
2. xoá `markers` và đặt `generation = 0`;
3. gọi `sync_live_sources()` để mở source mới theo `needed_files()`.

Nhờ vậy tab, Pinned Tab, File Section và vị trí sidebar đi theo mà không cần ánh xạ lại.

*Phương án khác*: cấp `file_id` mới rồi ánh xạ tab, pin và thứ tự từ id cũ sang id mới. Bị loại vì mọi nơi giữ `file_id` (tab, focus, Filter index, Raw View) đều phải biết về việc ánh xạ, dễ sót.

### 2. Đổi Starting Time là "nạp lại", không phải "thay nguồn"

Chỉ `start_time` đổi thì không xoá Marker. Source được mở lại với Starting Time mới qua cùng đường như khi thêm. Marker khớp theo dòng, Generation và `line_hash` như đã có, nên Marker của dòng còn được nạp tự hiện lại. Marker của dòng không còn được nạp vẫn được giữ trong `LogFile` như khi Live Window cắt dòng.

### 3. Sửa Live Glob bằng kế hoạch khớp lại có preview

Thêm `plan_glob_edit(old, new, entries, space_files, limit) -> GlobEditPlan { add, remove, keep }` cạnh `plan_glob` trong `src/remote/glob_plan.rs`. Hàm này thuần, không I/O, nên test được bằng dữ liệu.

Dialog liệt kê thư mục mới qua SFTP (như picker đang làm), gọi plan để hiện `+N · −M · K kept`, rồi Save mới áp dụng:
- `keep`: không đụng tới.
- `remove`: đi cùng đường với `remove_file_from_space` (đóng source, bỏ dòng và Marker, đóng tab).
- `add`: đi cùng đường với `add_glob_files`.

`excluded` được lọc lại: chỉ giữ đường dẫn còn khớp giá trị mới. Khi Server hoặc thư mục đổi, `keep` luôn rỗng. `glob_id` được giữ để `from_glob` của file còn giữ vẫn đúng. Stopped Live Glob thì plan bỏ trống `add`.

*Phương án khác*: coi sửa glob là gỡ rồi thêm. Bị loại vì mọi file vẫn khớp cũng bị nạp lại và mất Marker.

### 4. Dialog: thêm chế độ Edit cho dialog đã có

- `PickerTarget::Edit { space_id, file_id | glob_id }`: picker điền sẵn `Staged` và Starting Time, giới hạn đúng một mục (không cho thêm mục thứ hai), nút là Save, Save tắt khi giá trị bằng giá trị gốc.
- Dialog Add remote command có `CommandTarget::Edit { space_id, file_id }` theo cùng cách. Preset được nạp lại từ `preset` đã lưu để các ô preset điền sẵn.

Kiểm tra hợp lệ dùng lại hàm của luồng thêm. Kiểm tra trùng bỏ qua chính mục đang sửa.

*Phương án khác*: làm dialog Edit riêng. Bị loại vì trùng logic duyệt SFTP, Starting Time, Command Preset và Sign-in Prompt.

### 5. Thứ tự áp dụng: kiểm tra trước, đổi sau

Save chạy theo hai bước:
1. **Kiểm tra** (bất đồng bộ): kết nối, kiểm tra thư mục cha, liệt kê glob. Bước này không đổi trạng thái gì.
2. **Áp dụng** (đồng bộ trên UI thread): đổi `LogSpace`, `persist_active_session()`, thêm vào Recent Paths, rồi `refresh_after_space_change()`.

Nếu người dùng đóng dialog trong lúc đang kiểm tra thì kết quả kiểm tra bị bỏ.

### 6. Sửa Remote Command là một Run mới, không phải thay nguồn

Sửa Remote Command đi cùng đường với Restart: không `forget_file`, không xoá `markers` và `parsed_files`. Sau khi đổi `remote.target`, `remote.server_id` và `display_name`, app gửi cho source một lệnh mới `SourceCommand::Replace { server_id, target }`. Source kết thúc Run đang chạy, phát Break kiểu mới `BreakKind::CommandChanged { from, to }` (nhãn `command changed: <cũ> → <mới>`), mở Generation mới bằng `begin_generation_for`, rồi chạy lệnh mới. Nếu Server đổi, source mở kênh exec trên kết nối của Server mới qua hub.

Resume By Time của Run mới chỉ dựa vào dòng của chính Run đó. Khi Run mới chưa nhận dòng nào có timestamp thì dùng quy tắc "chạy lại nguyên văn" như hiện có.

*Phương án khác*: bỏ hết dòng cũ như khi đổi path của Remote File. Bị loại vì người dùng thường chỉ chỉnh tham số (`--tail`, container, unit) trong lúc điều tra và cần nhìn lại output trước đó. Remote File thì khác: đổi path là đổi hẳn sang một file khác.

### 7. Tên hiển thị và tab

`display_name` được tính lại theo `<server>:<path>` hoặc `<server>$ <lệnh>`. Tab đọc tên từ `LogFile` nên tự đổi. Tab của file bị gỡ do glob đổi thì đóng, như khi gỡ file.

## Risks / Trade-offs

- [Delta muộn từ source cũ có cùng `file_id` đến sau khi source mới đã mở] → Mỗi lần mở source, gắn cho nó một số thứ tự (`source_seq`) lưu trong `LiveSourceState`. `pump_live` bỏ delta có `source_seq` khác với source hiện tại.
- [Danh sách thư mục lúc preview đã cũ khi bấm Save] → Áp dụng đúng kế hoạch đã hiện. File xuất hiện sau đó được glob watcher bắt như thường lệ.
- [Đổi Server làm kết nối cũ không còn mục nào] → Dùng cơ chế hiện có: hub đóng kết nối khi không còn source nào dùng nó.
- [Bỏ Marker khi đổi path hoặc Server của Remote File có thể làm người dùng bất ngờ] → Nếu file có Marker, dialog ghi "Markers on this file will be removed" cạnh nút Save.
- [Dòng của Run cũ hiện tên lệnh mới ở Source Column] → Chấp nhận: Source Column theo tên hiện tại của Remote Command, và lệnh cũ được ghi trên Break "command changed". Tên theo từng Run có thể làm sau nếu cần.
- [Output của lệnh mới trùng với dòng cũ (ví dụ `--tail 1000` cùng container)] → Chấp nhận: Break tách rõ hai Run, như với Restart.
- [Filter đang chọn giá trị chỉ có ở file cũ] → Filter giữ nguyên, như khi gỡ file. Empty state hiện có của Focus Pane đã xử lý trường hợp không còn dòng khớp.

## Migration Plan

Không đổi định dạng lưu: chỉ thêm giá trị mới vào các trường đã có. Rollback bằng cách revert code. Project đã sửa vẫn mở được ở bản cũ.

## Open Questions

- Chưa có.
