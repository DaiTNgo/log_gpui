## 1. Nấc trong logic

- [x] 1.1 `src/workspace/new_space.rs`: thêm `NewSpaceKind { Local, Remote, Scratch }`; `can_create` / `create_label` / hướng dẫn trống nhận `kind` và số mục remote ("Create" cho Scratch, "Choose or drop files" và "Add remote files or a command" khi trống, bỏ "Create empty" và câu "…create an empty Log Space"); test bảng cho cả ba nấc
- [x] 1.2 Tên gợi ý: hàm thuần chọn tên theo `kind` và mục đầu của danh sách (Scratch → "Scratch"), qua `unique_name`; test "Scratch 2" khi trùng và danh sách trống thì tên trống
- [x] 1.3 `NewSpaceDialog` có `kind` (mặc định `Scratch`); `open_new_space_dialog` đặt lại `kind` và tên gợi ý; `new_space_set_kind(kind)` chỉ đổi nấc và làm mới tên gợi ý khi người dùng chưa gõ tên
- [x] 1.4 `new_space_create` tạo theo `kind`, chỉ đọc danh sách của nấc đang hiện; test Create ở nấc Remote khi nấc Local có file thì file local không được thêm

## 2. Dialog

- [x] 2.1 `src/app/view/dialogs.rs`: thanh chọn `TabBar::new("new-space-kind").segmented()` dưới tiêu đề, dòng mô tả theo nấc, rồi ô Name, rồi nội dung chỉ của nấc đang chọn (debug selector cho từng phần); nấc Scratch chỉ có mô tả
- [x] 2.2 Bỏ `MIX_HINT_LOCAL/REMOTE` và việc tắt nút bên này khi bên kia có mục; hiện hướng dẫn trống ở Local/Remote; test không còn câu nhắc và các nút Remote bật khi nấc Local có file
- [x] 2.3 Gạt sang Remote không mở picker; danh sách nấc bị ẩn không hiện số đếm; test gạt qua lại giữ nguyên hai danh sách
- [x] 2.4 Test mở dialog: nấc Scratch, tên "Scratch", nút "Create" bật; mở lại sau khi tạo Remote vẫn là Scratch

## 3. Bàn phím

- [x] 3.1 `src/app/keyboard.rs`: ⌘1/⌘2/⌘3 chọn nấc khi dialog mở (trước `return` của nhánh New Log Space); test các tab Working Set không đổi
- [x] 3.2 Bọc thanh chọn bằng `div` có `kind_focus`, `track_focus` và `on_key_down` cho ←/→; Tab dừng ở thanh chọn trong focus trap; test Tab tới thanh chọn, ←/→ đổi nấc, focus không thoát khỏi dialog

## 4. Thả file

- [x] 4.1 Thả vào dialog đang mở: `new_space_add_paths` đặt `kind = Local`; bỏ điều kiện bỏ qua khi có remote trong `drop_paths_on_window`; test thả khi ở nấc Remote giữ danh sách Remote
- [x] 4.2 Thả vào cửa sổ khi không có dialog nào mở: form tạm, `begin_paths`, đọc Archive nền với "Reading archive…" ở status bar; mọi mục dùng được thì `create_log_space` Imported với `unique_name(suggest_name(first))` và mở nó
- [x] 4.3 Có mục không dùng được hoặc không có mục nào: mở dialog ở nấc Local với form tạm làm danh sách; test thả `api.log` + file binary
- [x] 4.4 Test thả một file, nhiều file (tên "api 2" khi trùng), thư mục, Archive (Log Space tạo sau khi đọc xong), và thả khi đang có dialog khác thì bỏ qua

## 5. Dọn dẹp

- [x] 5.1 Sửa các test cũ dựa vào "Create empty", suy loại từ danh sách, hoặc chặn trộn local/remote (`tests/new_space_dialog_test.rs`, `tests/new_space_test.rs`, `tests/archive_test.rs`, các test picker/command NewSpace)
- [x] 5.2 `CONTEXT.md`: định nghĩa Scratch / Imported Log Space theo cách tạo mới (chọn nấc, thả file vào cửa sổ)
- [x] 5.3 Chạy toàn bộ test và đếm cảnh báo clippy không tăng
