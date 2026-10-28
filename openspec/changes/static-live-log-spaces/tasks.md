Mỗi task là một commit, và test phải xanh sau mỗi commit. Nhóm 3 cần việc của change `new-space-kind-switch` đã được commit, vì nó dựng tiếp trên thanh chọn của change đó.

## 1. Đổi loại Log Space

- [x] 1.1 `LogSpaceKind { Static, Live, Scratch }` với `#[serde(alias = "Imported")]` và `#[serde(alias = "Remote")]`. Đổi mọi nhánh `match` trong `src` và `tests` theo tên mới, chưa đổi hành vi. Test: một session cũ có `Imported` và `Remote` mở thành `Static` và `Live`, rồi được lưu lại bằng tên mới.
- [x] 1.2 Notice một lần khi session có Imported Log Space chứa file local ("Imported Log Spaces are now Static…").
- [x] 1.3 Sidebar: ba biểu tượng cho Static, Live, Scratch (Live dùng `Activity` thay cho `Globe`, vì Live space có cả file local). Menu của Static và Scratch đã đúng bảng loại file; "Add files…" của Live làm ở 4.1, cùng lúc file local chạy được trong Live space.

## 2. Static Log Space

- [x] 2.1 `Fingerprint` trong `IndexedFile`: inode, kích thước đã index, hash 4 KiB đầu và hash 4 KiB cuối. Cập nhật sau mỗi `extend()`. `IndexedFile::changed_on_disk(path)` trả về `Same`, `Appended`, `Replaced` hoặc `Gone`; khi kích thước và mtime của path vẫn như lần trước thấy không đổi thì chỉ đọc metadata. Test trên file thật cho `mv`, `cp` dài hơn, `cp` cùng kích thước, truncate và ghi thêm.
- [x] 2.2 File local của Static không có `LocalFileSource`. Thay vào đó là `FileWatch` poll metadata cho tab đang mở:
  - `Appended` thì gọi `extend()` và không tự cuộn;
  - `Replaced` thì gọi `read_indexed_again`, có lưu session trước.
  Bỏ Live/Pause và Follow khỏi status bar của Static. Test: hai scenario `mv` và `cp` của spec `static-log-space`.
  - Đã làm: `watch_static_files` chạy trong `pump_live`, mỗi `LOCAL_POLL_INTERVAL`, chỉ cho file của tab đang mở. `active_space_is_live` chỉ đúng với Live, nên status bar của Static tự bỏ Live/Pause và Follow (không sửa `status_bar.rs`). Đọc lại file thì lưu session trước để giữ Filter, cho cả Live. Các test về Follow, Pause và Dormant Tab của file local chuyển sang Live space.
- [x] 2.3 Tab từ Dormant được mở lại thì so `Fingerprint` trước khi hiện. File bị xoá thì hiện trạng thái "File deleted" và chờ file có lại.
  - Đã làm: Dormant Tab vốn bỏ dòng của file và đọc lại từ đĩa khi được chọn, nên không cần so `Fingerprint`; có test. File bị xoá: dòng vẫn còn (đọc qua handle đang mở), status bar có "File deleted" ("<tên> deleted" khi space có nhiều file), file có lại ở path thì được đọc lại.
- [x] 2.4 Marker sau khi đọc lại: giữ Marker còn khớp `line_hash`, bỏ Marker không khớp, kèm notice "N Markers no longer match the file". Dùng chung với `full_read.rs`.
  - Đã làm: mỗi lần một file local đọc qua Line Index xong (`file_read`), Marker của Generation hiện tại được so với text dòng của nó; đúng cả cho file đổi trong lúc app đóng. Marker không có `line_hash` được giữ.
  - Còn lại: chưa gộp với `restore_markers_after_read_again` của `full_read.rs` (Remote Copy đọc từ dòng đang giữ, không qua Line Index); gộp khi làm 2.5.
- [x] 2.5 "Reload" cho Remote Copy: chạy lại `full_read` với cùng Starting Time, giữ Marker theo 2.4. Remote Copy không giữ SSH session.
  - Đã làm: lệnh "Read again" đã có sẵn và đổi tên thành "Reload" (`reload_copy`). Kiểm tra Marker dùng chung `keep_markers_that_match` với 2.4, nên Reload cũng hiện notice "N Markers no longer match the file". Test: Reload giữ Marker còn khớp, và Remote Copy không đổi khi file trên server có thêm dòng cho tới khi Reload.

## 3. Dialog `[ Static | Live | Scratch ]`

- [ ] 3.1 `NewSpaceKind { Static, Live, Scratch }`, nhãn và dòng mô tả của từng nấc, ⌘1–3.
- [ ] 3.2 Nấc Live có danh sách local riêng (một `NewLogSpaceForm` thứ hai) cạnh danh sách remote. Nhãn Create gộp cả hai, ví dụ "Create with 1 file and 2 remote files". Archive ở nấc Live có cảnh báo và không được thêm.
- [ ] 3.3 Thả file: ở nấc Live thì vào danh sách của Live, ở nấc khác thì gạt sang Static. Thả vào cửa sổ khi không có dialog nào mở thì tạo Static.
- [ ] 3.4 Sửa `tests/new_space_dialog_test.rs`, `tests/new_space_test.rs` và `tests/archive_test.rs` theo nấc mới.

## 4. Live Log Space: file local

- [x] 4.1 Chuyển Live/Pause và Follow của file local (line-offset-index 4b.7) từ Static sang Live. Menu của Live space có thêm "Add files…". Một Live space trộn file local với Remote File trên cùng timeline; Pause dừng mọi file. Test: scenario "Trộn trên một timeline" và "Pause dừng cả hai".
  - Đã làm: Live space nhận file trên đĩa (Archive Entry bị để ngoài, kèm notice), file local trong Live đọc qua Line Index và được theo dõi, menu của file local trong Live là menu của file local. Test hai file local trên một timeline và Pause cả hai.
  - Còn lại: chưa có test trộn file local với Remote File thật (đi qua đường source theo từng file như nhau); Live/Pause của file local trong Static bỏ ở 2.2.
- [x] 4.2 Status bar và menu: Load more, Open in full, Live Window, Back to Live Window, "N lines held" và Clear chỉ hiện khi space có Remote File, và chỉ tác động lên Remote File. Clear không bỏ dòng local. File View của file local không có các mục này.
  - Đã làm: `hold_info` của file local luôn là `None`, nên status bar, menu file và Raw View không có Load more, Back to Live Window, "N lines held" hay Clear cho file local, kể cả trong File View; bỏ phần Load more cho file local còn từ 4b.7. Menu của Live space chỉ có "Open in full…", "Download all" và "Clear all files" khi `LogSpace::has_remote()` (có Remote File hoặc Live Glob). Open in full, Download và Clear vốn đã chỉ lấy Remote File. Test: Live space chỉ có file local, và Clear trong space trộn với Remote File thật (cần sshd).
- [ ] 4.3 `BreakKind::Replaced` và `BreakKind::Rewritten`. `LocalFileSource` đọc nlink của handle cũ bằng `fstat` khi thấy inode mới; `extend()` dùng `Fingerprint` để nhận ra ghi đè cùng inode.
- [ ] 4.4 Generation của file local: mỗi Generation là một `IndexedFile` cùng handle `stay_open` của nó, gộp theo (Generation, dòng). Sau Break "rotated" hoặc "replaced", dòng cũ vẫn đọc được; sau "truncated" hoặc "rewritten", dòng cũ bị bỏ. Đây là line-offset-index 9.2. Test: bốn scenario của requirement "Generation của file local".
- [ ] 4.5 Khi tab thành Dormant, đóng handle của các Generation cũ. Hết `HandleBudget` thì đóng Generation cũ nhất trước.

## 5. Tài liệu

- [ ] 5.1 Áp các thay đổi của CONTEXT.md trong design.md quyết định 9. Làm sau khi việc của change `new-space-kind-switch` được commit.
- [ ] 5.2 Sửa `openspec/changes/line-offset-index`: requirement "File local không có thao tác của Remote" trong `specs/line-index/spec.md` nói về file local của Live Log Space; task 4b.7 và 9.2 trỏ sang change này.
