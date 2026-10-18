## 1. Chuẩn bị (đã xong một phần)

- [x] 1.1 Chốt thuật ngữ và rule trong `CONTEXT.md` (Project, Log Space, Imported/Scratch, Source File, File View, Source Column, Quick Settings, Raw View, File Section, Parsed View; rule 17, 20, 22–29)
- [x] 1.2 Viết ADR-0006 (Log Space gộp theo timestamp)
- [x] 1.3 Archive change `redesign-search-and-filter` trước, để delta `log-filter` và `log-search` áp lên đúng spec
- [x] 1.4 Spike: kiểm tra `Editor`/`EditorState` của gpui-component 0.7.3 có số dòng, `readonly`, và tô sáng được một khoảng không; ghi kết quả vào `design.md` (Quyết định 5)

## 2. Mô hình Log Space

- [x] 2.1 Thêm `LogSpace` (`id`, `name`, `kind: Imported | Scratch`, `file_order: Added | Name`, `files`, `filter`, `file_filters`) và `LogFile` (`file_id`, `display_name`, `path`, `added_seq`, `markers`) vào `src/workspace/`
- [x] 2.2 Thay `Project.files` bằng `Project.log_spaces`; bỏ `filter` khỏi mô tả file; thêm `create_log_space`, `rename_log_space`, `remove_log_space`, `has_space_named` (so tên đã trim, trong cùng Project)
- [x] 2.3 Thao tác theo loại: `add_imported_files` (chỉ Imported, bỏ qua file trùng và trả về danh sách bị bỏ qua), `new_scratch_file` (chỉ Scratch, đặt tên `untitled-N.log` theo số nhỏ nhất chưa dùng), `remove_file` (Imported: chỉ gỡ khỏi danh sách; Scratch: không cho xóa file cuối)
- [x] 2.4 `ordered_files()` theo `file_order`
- [x] 2.5 Đổi `TabTarget` thành `LogSpace { space_id }` và `FileView { space_id, file_id }` (giữ `InvestigationBoard`); bỏ `LogFile` và `ChronicleStream`; thêm `mode: Raw | Parsed` và trạng thái thu gọn File Section vào `TabSessionState`
- [x] 2.6 Bump version manifest session; session cũ bị bỏ qua
- [x] 2.7 Test: tên trùng và tên rỗng, không trộn Imported/Scratch, đặt tên `untitled-N`, không xóa file scratch cuối, thứ tự file, mở lại tab đã có

## 3. Lưu trữ file scratch

- [x] 3.1 Hàm lấy thư mục `~/.config/log_gpui/spaces/<space_id>/` (cùng gốc với `default_session_path`)
- [x] 3.2 Tạo, đổi tên, xóa file scratch trên đĩa; xóa Scratch Log Space thì xóa cả thư mục của nó
- [x] 3.3 Ghi có debounce khoảng 500 ms (dùng lại cơ chế của `SessionPersistence`) và lưu ngay khi bấm ⌘S
- [x] 3.4 Save As: chọn vị trí bằng `rfd`, ghi bản sao, file scratch không đổi
- [x] 3.5 Test: tạo/xóa thư mục, tự lưu, Save As không đổi file gốc

## 4. Gộp theo timestamp

- [x] 4.1 Cache kết quả parse theo `file_id`, dùng chung cho Log Space và File View
- [x] 4.2 Tính timestamp hiệu dụng: dòng tiếp nối kế thừa dòng cha, dòng trước timestamp đầu tiên lấy timestamp đầu tiên của file, file không có timestamp nhận `+∞` và được gắn cờ
- [x] 4.3 Merge k đường theo khóa `(timestamp hiệu dụng, thứ tự file, số dòng)`, trả về danh sách `(file_idx, line_idx)`
- [x] 4.4 Merge lại khi thêm/gỡ file hoặc đổi `file_order`, không parse lại
- [x] 4.5 Test: xen kẽ, trùng timestamp, stack trace đi liền dòng cha, dòng trước timestamp đầu, file không có timestamp xuống cuối, không đổi múi giờ

## 5. Cột, Source Column và Filter

- [x] 5.1 Hợp schema theo tên cột (giữ thứ tự cột của file đầu tiên, thêm cột mới theo thứ tự file); dòng thiếu cột để trống
- [x] 5.2 Thêm Source Column ảo trước MESSAGE: ghim được, đổi độ rộng được, Auto-fit được
- [x] 5.3 Dựng chỉ mục Filter trên tập dòng đã gộp, có cột Source; File View dùng chỉ mục của riêng file
- [x] 5.4 Filter theo view: đọc và ghi `LogSpace.filter` hoặc `file_filters[file_id]` theo tab đang mở; Marker đọc và ghi trên `LogFile.markers`
- [x] 5.5 Test: hợp cột, Filter cột Source, File View không bị Filter của Log Space ảnh hưởng, Marker dùng chung giữa các view, khôi phục Filter khi mở lại

## 6. Dialog tạo Log Space

- [x] 6.1 `NewLogSpaceForm` thuần dữ liệu: `name`, `files: Vec<PendingFile { path, status: Ok | Binary | Duplicate }>`, `can_create()`, `kind()`, nhãn nút ("Create empty" / "Create with N files")
- [x] 6.2 Nhận đường dẫn: mở rộng thư mục một cấp, nhận diện binary (byte NUL hoặc không phải UTF-8 trong 8 KB đầu), đánh dấu trùng
- [x] 6.3 UI modal trong `context_view`: ô tên kèm thông báo lỗi, vùng kéo thả (`ExternalPaths`), nút Browse (`rfd`, chọn nhiều file), danh sách file có nút bỏ, Create/Cancel
- [x] 6.4 Create: tạo Imported hoặc Scratch (tạo `untitled.log`), mở tab của Log Space mới
- [x] 6.5 Test `NewLogSpaceForm`: tên rỗng hoặc khoảng trắng, trùng tên trong Project, khác Project không chặn, suy ra loại, file binary không được đếm, bỏ file

## 7. Sidebar và tab

- [x] 7.1 Sidebar: dòng Log Space có chevron, biểu tượng Imported/Scratch, danh sách file theo `ordered_files()`, cờ "No timestamps"; nút **+** mở dialog
- [x] 7.2 Menu chuột phải trên Log Space: Rename, Add files… hoặc New scratch file, Sort files by → Added / Name, Delete (có xác nhận)
- [x] 7.3 Menu chuột phải trên file: Imported có Remove from Log Space, Reveal in Finder; Scratch có Rename, Save As…, Delete (có xác nhận, tắt khi là file cuối)
- [x] 7.4 Bấm Log Space mở tab Log Space, bấm file mở tab File View; mở lại thì kích hoạt tab đã có; đóng tab khi Log Space hoặc file bị xóa
- [x] 7.5 Tab bar hiện tên Log Space hoặc `tên file · tên Log Space`

## 8. Toolbar: Raw/Parsed và Quick Settings

- [x] 8.1 Nút chuyển Raw/Parsed trên toolbar; chế độ mặc định khi mở (Imported: Parsed; Scratch rỗng: Raw; còn lại: chế độ dùng lần cuối)
- [x] 8.2 Popover Quick Settings: công tắc "Show source file" (nhớ theo Log Space, mặc định bật khi có từ hai file) và density (chuyển từ settings sang)
- [x] 8.3 Gỡ density khỏi màn settings

## 9. Raw View

- [x] 9.1 Danh sách ảo hóa các File Section: header (chevron, tên, thư mục), nội dung có số dòng, nhớ trạng thái thu gọn theo file
- [x] 9.2 Imported: render chỉ đọc, chọn và sao chép được
- [x] 9.3 Scratch: sửa được theo kết quả spike 1.4 (`EditorState` cho mỗi section, hoặc ô sửa tại chỗ); nội dung dùng chung theo `file_id` giữa các tab
- [x] 9.4 Search trong Raw View: tìm trên mọi dòng, bỏ qua Filter, tô sáng, nút trước/sau đi xuyên các section và tự mở section đang thu gọn; không có nút Aa / wd / regex
- [x] 9.5 Marker trong Raw View: đặt và gỡ trên dòng; lưu hash nội dung dòng, sau mỗi lần lưu thì bỏ Marker có hash không còn khớp
- [x] 9.6 Chuyển từ Raw sang Parsed thì parse lại file scratch đã đổi, merge lại, Filter giữ điều kiện còn giá trị thì làm mới
- [x] 9.7 Test: Marker bị bỏ khi sửa dòng, Search trong Raw đếm cả dòng bị Filter ẩn, parse lại sau khi sửa

## 10. Dọn dẹp và kiểm tra

- [x] 10.1 Gỡ code dùng `Project.files`, `TabTarget::LogFile`, `ChronicleStream`; sửa `project_test`, `session_test`, `workspace_test`, `filter_test`
- [x] 10.2 `cargo build` và `cargo test` qua hết
- [ ] 10.3 Chạy app thử: tạo Imported bằng kéo thả và bằng Browse, tạo Scratch rồi dán log, chuyển Raw/Parsed, Source Column và Quick Settings, File View, menu chuột phải, Save As, khởi động lại thấy dữ liệu còn
- [ ] 10.4 Rà soát lại các spec so với hành vi thực tế
