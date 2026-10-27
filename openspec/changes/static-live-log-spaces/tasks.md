Mỗi task là một commit, và test phải xanh sau mỗi commit. Nhóm 3 cần việc của change `new-space-kind-switch` đã được commit, vì nó dựng tiếp trên thanh chọn của change đó.

## 1. Đổi loại Log Space

- [x] 1.1 `LogSpaceKind { Static, Live, Scratch }` với `#[serde(alias = "Imported")]` và `#[serde(alias = "Remote")]`. Đổi mọi nhánh `match` trong `src` và `tests` theo tên mới, chưa đổi hành vi. Test: một session cũ có `Imported` và `Remote` mở thành `Static` và `Live`, rồi được lưu lại bằng tên mới.
- [x] 1.2 Notice một lần khi session có Imported Log Space chứa file local ("Imported Log Spaces are now Static…").
- [ ] 1.3 Sidebar: ba biểu tượng cho Static, Live, Scratch. Menu chuột phải theo bảng loại file (requirement "Loại file theo loại Log Space"): Live có "Add files…", "Add remote files…" và "Add remote command…".

## 2. Static Log Space

- [ ] 2.1 `Fingerprint` trong `IndexedFile`: inode, kích thước đã index, hash 4 KiB đầu và hash 4 KiB cuối. Cập nhật sau mỗi `extend()`. `IndexedFile::changed_on_disk()` trả về `Same`, `Appended` hoặc `Replaced`. Test trên file thật cho `mv`, `cp` dài hơn, `cp` cùng kích thước, truncate và ghi thêm.
- [ ] 2.2 File local của Static không có `LocalFileSource`. Thay vào đó là `FileWatch` poll metadata cho tab đang mở:
  - `Appended` thì gọi `extend()` và không tự cuộn;
  - `Replaced` thì gọi `read_indexed_again`, có lưu session trước.
  Bỏ Live/Pause và Follow khỏi status bar của Static. Test: hai scenario `mv` và `cp` của spec `static-log-space`.
- [ ] 2.3 Tab từ Dormant được mở lại thì so `Fingerprint` trước khi hiện. File bị xoá thì hiện trạng thái "File deleted" và chờ file có lại.
- [ ] 2.4 Marker sau khi đọc lại: giữ Marker còn khớp `line_hash`, bỏ Marker không khớp, kèm notice "N Markers no longer match the file". Dùng chung với `full_read.rs`.
- [ ] 2.5 "Reload" cho Remote Copy: chạy lại `full_read` với cùng Starting Time, giữ Marker theo 2.4. Remote Copy không giữ SSH session.

## 3. Dialog `[ Static | Live | Scratch ]`

- [ ] 3.1 `NewSpaceKind { Static, Live, Scratch }`, nhãn và dòng mô tả của từng nấc, ⌘1–3.
- [ ] 3.2 Nấc Live có danh sách local riêng (một `NewLogSpaceForm` thứ hai) cạnh danh sách remote. Nhãn Create gộp cả hai, ví dụ "Create with 1 file and 2 remote files". Archive ở nấc Live có cảnh báo và không được thêm.
- [ ] 3.3 Thả file: ở nấc Live thì vào danh sách của Live, ở nấc khác thì gạt sang Static. Thả vào cửa sổ khi không có dialog nào mở thì tạo Static.
- [ ] 3.4 Sửa `tests/new_space_dialog_test.rs`, `tests/new_space_test.rs` và `tests/archive_test.rs` theo nấc mới.

## 4. Live Log Space: file local

- [ ] 4.1 Chuyển Live/Pause và Follow của file local (line-offset-index 4b.7) từ Static sang Live. Một Live space trộn file local với Remote File trên cùng timeline; Pause dừng mọi file. Test: scenario "Trộn trên một timeline" và "Pause dừng cả hai".
- [ ] 4.2 Status bar và menu: Load more, Open in full, Live Window, Back to Live Window, "N lines held" và Clear chỉ hiện khi space có Remote File, và chỉ tác động lên Remote File. Clear không bỏ dòng local. File View của file local không có các mục này.
- [ ] 4.3 `BreakKind::Replaced` và `BreakKind::Rewritten`. `LocalFileSource` đọc nlink của handle cũ bằng `fstat` khi thấy inode mới; `extend()` dùng `Fingerprint` để nhận ra ghi đè cùng inode.
- [ ] 4.4 Generation của file local: mỗi Generation là một `IndexedFile` cùng handle `stay_open` của nó, gộp theo (Generation, dòng). Sau Break "rotated" hoặc "replaced", dòng cũ vẫn đọc được; sau "truncated" hoặc "rewritten", dòng cũ bị bỏ. Đây là line-offset-index 9.2. Test: bốn scenario của requirement "Generation của file local".
- [ ] 4.5 Khi tab thành Dormant, đóng handle của các Generation cũ. Hết `HandleBudget` thì đóng Generation cũ nhất trước.

## 5. Tài liệu

- [ ] 5.1 Áp các thay đổi của CONTEXT.md trong design.md quyết định 9. Làm sau khi việc của change `new-space-kind-switch` được commit.
- [ ] 5.2 Sửa `openspec/changes/line-offset-index`: requirement "File local không có thao tác của Remote" trong `specs/line-index/spec.md` nói về file local của Live Log Space; task 4b.7 và 9.2 trỏ sang change này.
