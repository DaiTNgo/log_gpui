## 1. Dependency và module đọc Archive

- [x] 1.1 Thêm dependency đã pin phiên bản: `zip` (tắt default features, chỉ bật `deflate`), `tar`, `flate2`
- [x] 1.2 Tạo `src/archive.rs` với `ArchiveKind`, `EntryInfo { name, size, head }`, `ArchiveError` (Damaged, Encrypted, NotFound, TooLarge, Io)
- [x] 1.3 `detect(path)` theo magic bytes (zip, gzip, tar `ustar` ở offset 257, gzip chứa tar → TarGz); test với file đổi đuôi, `.7z`, file text, file rỗng
- [x] 1.4 `list(path, kind)` cho Zip/Tar/TarGz/Gz: chỉ trả file thường, bỏ thư mục, symlink, `__MACOSX/`, `.DS_Store`, kèm `size` khai báo và tối đa 8 KB đầu; với Gz trả một entry tên bỏ `.gz`
- [x] 1.5 Báo `Encrypted` cho zip có mật khẩu và `Damaged` cho Archive bị cắt hoặc dùng codec không bật; test bằng Archive tạo trong `tempfile`
- [x] 1.6 `read_entries(path, kind, &[entry], limit)` đọc nhiều entry trong một lần duyệt; bọc `Read::take` và trả `TooLarge` khi vượt dung lượng khai báo hoặc vượt 2 GB; test với entry khai báo sai dung lượng
- [x] 1.7 `FileSource` và `read_source(source)`: `Disk` gọi `fs::read`, `ArchiveEntry` gọi `read_entries`; `NotFound` khi Archive hoặc entry không còn

## 2. Mô hình LogFile

- [x] 2.1 Thêm `entry: Option<String>` (`#[serde(default)]`) vào `LogFile`, `LogFile::from_archive_entry(archive, entry)` với `file_id = "{archive}!/{entry}"` và tên hiển thị `<tên archive> › <entry>` (Gz: tên bỏ `.gz`)
- [x] 2.2 `LogFile::source()` trả `FileSource`; `folder()` của entry trả thư mục chứa Archive
- [x] 2.3 Xét trùng khi thêm file vào Imported Log Space theo Archive + entry; test thêm lại `bundle.zip` chỉ thêm entry mới, và cùng tên entry ở Archive khác vẫn được thêm
- [x] 2.4 Test session: lưu và đọc lại LogFile có `entry`, session cũ không có `entry` vẫn đọc thành file thường, Marker của entry còn sau khi mở lại

## 3. Đọc nội dung qua nguồn chung

- [x] 3.1 `parser::parse_file*` và `ParsedFile::from_path` chuyển sang nhận `FileSource` (`from_source`), gọi `read_source`
- [x] 3.2 `ensure_file_loaded` / `ensure_live_file_loaded` dùng `file.source()`; entry đi nhánh tĩnh không có `LiveBuffer`; gom entry cùng Archive để đọc một lần
- [x] 3.3 Thông báo lỗi nêu tên hiển thị của entry khi Archive mất, entry mất, Damaged hoặc TooLarge; file vẫn ở lại Log Space với nội dung rỗng
- [x] 3.4 `sync_live_sources` bỏ qua file có `entry`; `active_space_is_live` trả `false` khi mọi file là entry; test space trộn entry và file thường vẫn live cho file thường

## 4. Danh sách chờ trong dialog

- [x] 4.1 `PendingFile` chuyển sang `PendingSource` (`Disk` / `Entry { archive, entry, size }`); thêm `PendingStatus::LargeFile`, `Damaged`, `Encrypted` với cảnh báo "Large file", "Damaged archive", "Password-protected archive"
- [x] 4.2 `add_paths` và `expand_paths` mở rộng Archive thành entry (cả Archive nằm trong thư mục được thả); entry binary và Archive lồng nhau ra `Binary`; test các scenario trong `specs/archive-import`
- [x] 4.3 `total_uncompressed()`, và `can_create` / nhãn Create bỏ qua mục không thêm được, tắt khi tổng > 2 GB kèm lý do; test nhãn "Create with N files" với entry và LargeFile
- [x] 4.4 Tạo Log Space và "Add files…" tạo `LogFile` từ `PendingSource`

## 5. UI

- [x] 5.1 Liệt kê Archive trên background executor; danh sách hiện "Reading <tên>…" cho đến khi có kết quả, rồi thay bằng các entry
- [x] 5.2 Dialog và "Add files…" hiện tên `bundle.zip › path`, cảnh báo mới, và lý do vượt 2 GB dưới danh sách
- [x] 5.3 Sidebar, tab, File Section header và Source Column dùng tên hiển thị của entry
- [x] 5.4 Status bar không hiện Follow/Pause cho Log Space chỉ có entry

## 6. Tài liệu và kiểm tra

- [x] 6.1 CONTEXT.md: thêm Archive và Archive Entry, sửa quy tắc 24–25 nói Archive Entry cũng chỉ đọc và không live
- [ ] 6.2 Chạy `cargo test` và `cargo clippy`; thử tay trên app: thả `.zip`, `.tar.gz`, `app.log.1.gz`, zip có mật khẩu, tar.gz bị cắt; mở lại Project kiểm tra Marker
