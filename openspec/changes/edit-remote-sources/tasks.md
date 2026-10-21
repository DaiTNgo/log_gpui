## 1. Mô hình: sửa mục trong Log Space

- [x] 1.1 `src/workspace/log_space.rs`: `replace_remote_target(file_id, server_id, target)`. Giữ `file_id`, `added_seq` và vị trí, tính lại `display_name`, xoá `markers`, đặt `generation = 0`. Từ chối nếu trùng mục khác (bỏ qua chính nó). Test đơn vị cho thứ tự file và cho trường hợp trùng.
- [x] 1.2 `set_start_time(file_id, Option<String>)`: chỉ đổi `start_time` và giữ `markers`. Test đơn vị.
- [x] 1.3 `edit_live_glob(glob_id, server_id, dir, pattern, plan)`: giữ `glob_id`, gỡ các file trong `plan.remove`, thêm các file trong `plan.add` (trừ khi Stopped), lọc lại `excluded`. Test đơn vị.
- [x] 1.4 Test lưu và đọc lại Project sau mỗi loại sửa: không cần migration, bản cũ vẫn đọc được.

## 2. Kế hoạch khớp lại Live Glob

- [x] 2.1 `src/remote/glob_plan.rs`: hàm thuần `plan_glob_edit(old, new, entries, space_files, limit) -> GlobEditPlan { add, remove, keep }`. Có tính `excluded`, file đã thêm bằng path và giới hạn số file.
- [x] 2.2 Test bảng cho `plan_glob_edit`: thu hẹp pattern, nới rộng pattern, đổi thư mục, đổi Server (`keep` rỗng), Stopped (`add` rỗng), vượt giới hạn, file đã thêm bằng path.

## 3. Đọc live sau khi sửa

- [x] 3.1 `LiveSourceState` có `source_seq`. `pump_live` bỏ delta của source đã đóng. Test: delta muộn của source cũ không vào view.
- [x] 3.2 `src/app/remote.rs`: `apply_remote_file_edit`, theo thứ tự: kiểm tra (kết nối, thư mục cha, Sign-in Prompt) → `replace_remote_target` hoặc `set_start_time` → `forget_file` → `persist_active_session` → thêm vào Recent Paths → `sync_live_sources`.
- [x] 3.3 `src/app/globs.rs`: `apply_live_glob_edit`. Gỡ theo đường của `remove_file_from_space`, thêm theo đường của `add_glob_files`, rồi `sync_glob_watchers` với giá trị mới.
- [x] 3.4 `src/live/source.rs`: thêm `SourceCommand::Replace { server_id, target }` và `BreakKind::CommandChanged { from, to }`. Source kết thúc Run cũ, phát Break, mở Generation mới rồi chạy lệnh mới (mở kênh trên Server mới nếu Server đổi). Test source: dòng cũ còn, có Break, Run mới đánh số từ 1.
- [x] 3.5 `src/app/remote_command.rs`: `apply_remote_command_edit`. Kiểm tra, đổi target và `display_name`, giữ `restart_on_exit` và `markers`, rồi gửi `Replace` (không `forget_file`).
- [x] 3.6 Đóng kết nối Server cũ khi không còn source nào dùng: kiểm tra hub đã làm việc này, và viết test cho trường hợp đổi Server.

## 4. Dialog chế độ Edit

- [x] 4.1 `src/app/remote_picker.rs`: `PickerTarget::Edit`. Điền sẵn `Staged` và Starting Time, chỉ một mục, nút Save tắt khi giá trị không đổi, kiểm tra trùng bỏ qua chính nó.
- [x] 4.2 Preview `+N files · −M files · K kept` cho Live Glob, kèm dòng "Stopped: matching files are added when you resume" khi glob đang Stopped.
- [x] 4.3 Dòng cảnh báo "Markers on this file will be removed" khi đổi path hoặc Server của Remote File có Marker (không hiện với Remote Command).
- [x] 4.4 Dialog Add remote command ở chế độ Edit: nạp lại preset đã lưu, báo "Edited: will rerun as typed after a lost connection" theo quy tắc hiện có, nút Save.
- [x] 4.5 Lỗi kiểm tra hiện trong dialog và dialog vẫn mở. Hủy hoặc đóng dialog trong lúc đang kiểm tra thì bỏ kết quả.

## 5. Menu và tab

- [x] 5.1 `src/ui/space_menu.rs`: thêm "Edit…" vào `glob_menu` và vào nhánh Remote File và Remote Command của `file_menu`. Không thêm cho Remote Copy, Imported và Scratch.
- [x] 5.2 Tab, Pinned Tab và File Section đổi tên theo `display_name` mới. Tab của file bị gỡ khi glob đổi thì đóng.

## 6. Test GPUI và hoàn tất

- [x] 6.1 Test GPUI (sshd local, theo `scripts/` của remote live log): sửa `/app/ap.log` thành `/app/app.log`. File giữ vị trí thứ hai, File View đổi tên, dòng hiện.
- [x] 6.2 Test GPUI: đổi Starting Time `1h` thành `3h`. Marker vẫn còn, dòng cũ hơn được nạp.
- [x] 6.3 Test GPUI: Live Glob `*.log` đổi thành `app-*.log`. Preview `−1 file · 2 kept`, file giữ lại không bị nạp lại.
- [x] 6.4 Test GPUI: sửa Remote Command preset Docker từ `api` thành `worker`. Run cũ dừng, dòng cũ và Marker còn, có Break "command changed", Run mới đánh số từ 1.
- [x] 6.5 Test: Save khi giá trị không hợp lệ thì không đổi gì. Hủy dialog thì không đổi gì.
- [x] 6.6 Cập nhật CONTEXT.md nếu cần: thêm "Edit…" vào mô tả Remote File, Live Glob và Remote Command.
