## Context

Dialog New Log Space hiện có hai danh sách sống ở hai chỗ:

- File local nằm trong `NewSpaceDialog.form: NewLogSpaceForm` (`src/workspace/new_space.rs`), gồm cả Archive đang đọc nền (`begin_paths` / `finish_archive`).
- Remote File, Live Glob, Remote Command nằm trong `app.remote.new_space_files / new_space_globs / new_space_commands / new_space_starts` (`src/app/remote.rs`), do remote picker và form Remote Command ghi vào.

Loại Log Space được suy ra lúc Create (`new_space_create` trong `src/app/spaces.rs`): có remote thì Remote, có file thì Imported, không có gì thì Scratch. Việc không trộn được giữ bằng cách tắt nút bên này khi bên kia có gì, kèm `MIX_HINT_LOCAL/REMOTE` trong `src/app/view/dialogs.rs`, và `drop_paths_on_window` bỏ qua thả file khi dialog có remote.

gpui-component 0.7.3 có `TabBar::new(id).segmented().selected_index(ix).on_click(...)`, nhưng `TabBar` không nhận focus và không xử lý phím.

## Goals / Non-Goals

**Goals:**
- Loại Log Space là một trạng thái rõ ràng của dialog, đọc được từ test, không suy ra từ danh sách.
- Hai danh sách hiện có được giữ nguyên chỗ, nấc chỉ quyết định cái nào hiện và cái nào được dùng khi Create.
- Thả file vào cửa sổ tạo Imported Log Space ngay, dùng chung quy tắc nhận file của dialog.

**Non-Goals:**
- Không đổi remote picker, form Remote Command, form Starting Time, hay cách đọc Archive.
- Không nhớ nấc giữa các lần mở hay giữa các lần chạy.
- Không thêm tuỳ chọn cho Scratch (dán log ban đầu, số file).
- Không đổi "Add files…" / "Add folder…" trên Log Space đã có.

## Decisions

### Nấc là một enum trong `NewSpaceDialog`
Thêm `pub enum NewSpaceKind { Local, Remote, Scratch }` (ở `src/workspace/new_space.rs`, cạnh form, để test logic không cần GPUI) và trường `kind: NewSpaceKind` trong `NewSpaceDialog`, mặc định `Scratch`. `new_space_create` match theo `kind` thay vì đếm danh sách.
- Thay vì để `NewLogSpaceForm` giữ luôn remote: form là logic thuần trong `workspace`, còn remote list gắn với `RemoteState` và picker; dời chúng là đổi lớn không cần cho thay đổi này.

### Gạt nấc chỉ đổi `kind`
`new_space_set_kind(kind)` chỉ ghi `kind`, cập nhật tên gợi ý và `cx.notify()`. Không xoá danh sách nào. Create bỏ danh sách của nấc khác bằng cách không đọc tới nó; `open_new_space_dialog` vẫn xoá cả hai như hiện nay.
- Thay vì xoá danh sách khi gạt (có xác nhận): đã chốt giữ nguyên, không hỏi.
- Archive đang đọc ở nấc Local khi người dùng tạo từ nấc khác: kết quả `finish_archive` về sau rơi vào dialog đã đóng và bị bỏ, đúng như cách "Stop reading" đang làm.

### Điều kiện và nhãn Create theo nấc
`can_create` / `create_label` nhận `kind` và số mục remote:
- `Scratch`: "Create", chỉ cần tên hợp lệ.
- `Local`: `files_message` như hiện nay, cộng "Choose or drop files" khi danh sách trống; nhãn "Create with N file(s)" hoặc "Reading archive…".
- `Remote`: "Add remote files or a command" khi trống; nhãn từ `remote_create_label`.
Bỏ nhánh "Create empty" và câu "…Remove them to create an empty Log Space" (nay là "None of these files can be added").

### Tên gợi ý theo nấc
Giữ cờ "người dùng đã gõ tên" (đã có qua `pending_name` / Change event của `name_input`). Khi chưa gõ, `refresh_suggested_name()` chạy sau mỗi lần đổi nấc hoặc đổi danh sách: Local lấy `suggest_name(first local path, PathStyle::current)`, Remote lấy `suggest_name(first remote path, PathStyle::Unix)`, Scratch lấy "Scratch"; tất cả qua `unique_name`. Danh sách trống ở Local/Remote thì để tên trống.

### Thanh chọn và bàn phím
Dùng `TabBar::new("new-space-kind").segmented()` với ba `Tab` và `on_click` gọi `new_space_set_kind`. Bọc nó trong một `div` có `FocusHandle` riêng (`kind_focus` trong `NewSpaceDialog`), `track_focus` và `on_key_down` cho ←/→, để Tab dừng được ở đó trong focus trap hiện có. ⌘1–3 xử lý trong `handle_key_down` ở nhánh `self.new_space.is_some()` trước `return`, nên không chạm tới chuyển tab Working Set.
- Thay vì `Radio` (đã dùng trong Open in full): đã chốt segmented, trông như công tắc.

### Thả file
- Dialog mở: `new_space_add_paths` đặt `kind = Local` trước khi thêm. Bỏ điều kiện `remote > 0` trong `drop_paths_on_window`.
- Dialog đóng: hàm mới `drop_create_imported(paths)` dựng một `NewLogSpaceForm` tạm, gọi `begin_paths`, đọc các Archive bằng `background_executor` như dialog, và hiện "Reading archive…" ở status bar trong lúc chờ. Khi xong: nếu mọi mục `is_importable` và có ít nhất một mục thì gọi `create_log_space` với tên `unique_name(suggest_name(first))`; ngược lại mở dialog với `kind = Local` và đặt form tạm vào làm `form`.
- Thay vì luôn mở dialog rồi tự bấm Create: tạo ngay tránh nháy dialog và giữ dialog chỉ cho trường hợp có lỗi.

### Remote không còn bị chặn bởi file local
Bỏ việc tắt "Add remote files…" / "Add remote command…" khi có file local, và bỏ việc tắt vùng thả / "Choose…" khi có remote. Picker vẫn mở từ nấc Remote; `NewSpace` target của picker không cần biết nấc.

## Risks / Trade-offs

- [Người dùng tạo từ nấc Remote mà quên 3 file local đang ẩn, rồi mất chúng] → Đã chốt không hiện số đếm; nhãn Create ghi rõ "remote", và mất danh sách chỉ là chọn lại, không mất dữ liệu.
- [Status bar "Reading archive…" cho thả-vào-cửa-sổ cần một chỗ hiện trạng thái chưa có] → Dùng cơ chế notice/status hiện có; nếu không có chỗ phù hợp thì thêm một trạng thái tạm trong `AppView` xoá khi đọc xong.
- [Thả liên tiếp hai lần trong lúc Archive lần đầu đang đọc] → Mỗi lần thả là một form tạm và một Log Space riêng; tên dùng `unique_name` lúc tạo nên không trùng.
- [Focus trap (G5) có thể kéo focus về ô tên khi người dùng ở thanh chọn] → Focus trap chỉ kích hoạt khi focus ra ngoài dialog; `kind_focus` nằm trong dialog nên `contains_focused` vẫn đúng. Có test cho Tab tới thanh chọn.
- [Nhiều test hiện dựa vào "Create empty" và việc chặn trộn] → Sửa test cùng lúc với từng thay đổi.

## Migration Plan

Không có dữ liệu lưu nào đổi. Thay đổi chỉ ở UI; rollback là revert các commit.

## Open Questions

(không có — mọi quyết định đã chốt khi grilling)
