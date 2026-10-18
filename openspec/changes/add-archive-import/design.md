## Context

Một file của Imported Log Space hiện là `LogFile { file_id, display_name, path }`, với `file_id` chính là đường dẫn tuyệt đối. Nội dung được đọc nguyên khối bằng `std::fs::read` ở ba chỗ: `parser::parse_file*` (`src/parser/mod.rs`), `ParsedFile::from_path` (`src/space_view.rs`) và `ensure_live_file_loaded` (`src/app/tabs.rs`). Sau khi có `Vec<u8>`, mọi thứ phía sau (parse, filter index, marker, merged timeline) không quan tâm byte đến từ đâu.

File local của Imported Log Space được đọc live: `sync_live_sources` (`src/app/live.rs`) tạo một source cho mọi file không thuộc Scratch Log Space, và `ensure_live_file_loaded` gắn `LiveBuffer` vào file.

Dialog New Log Space (`src/workspace/new_space.rs`) giữ danh sách `PendingFile { path, status }`, mở rộng thư mục một cấp, đánh dấu binary bằng 8 KB đầu. Một file zip bây giờ sẽ bị đánh dấu Binary.

## Goals / Non-Goals

**Goals:**
- Nhận `.zip`, `.tar`, `.tar.gz`/`.tgz`, `.gz` trong dialog và "Add files…", chọn từng entry.
- Đọc entry vào bộ nhớ qua một điểm đọc chung, không giải nén ra đĩa.
- Entry giữ `file_id` ổn định để Marker và session còn sau khi mở lại.
- Chống zip bomb, Archive hỏng, Archive có mật khẩu mà không làm treo UI.

**Non-Goals:**
- Archive lồng nhau, Archive trên Server từ xa, Archive có mật khẩu (chỉ báo lỗi).
- `.7z`, `.rar`, `.zst`, `.bz2`, `.xz`.
- Đọc live entry, hay tự nạp lại khi Archive đổi.
- Đọc entry theo stream hoặc theo trang. Entry vẫn được đọc nguyên khối như file thường.

## Decisions

### 1. `LogFile` có thêm trường `entry`, không nhét vào chuỗi `path`

`LogFile` thêm `entry: Option<String>` (`#[serde(default)]`): đường dẫn bên trong Archive. `path` vẫn là đường dẫn thật của Archive trên đĩa. Có một kiểu nhỏ mô tả nguồn đọc:

```rust
pub enum FileSource<'a> {
    Disk(&'a Path),
    ArchiveEntry { archive: &'a Path, entry: &'a str },
}
```

`LogFile::source()` trả về kiểu này. `file_id` của entry là `"{archive}!/{entry}"` để không trùng với file thường, nhưng code không bao giờ parse ngược chuỗi này.

*Phương án đã bỏ:* mã hoá cả hai vào `path` dạng `bundle.zip!/app.log`. Gọn hơn nhưng một đường dẫn thật có thể chứa `!/`. Ngoài ra `folder()`, việc xét trùng và đọc live đều phải tự tách chuỗi, dễ sót.

*Tương thích:* session cũ không có `entry` thì đọc ra `None`, tức là file thường. Không cần migrate.

### 2. Module `src/archive.rs` là điểm đọc duy nhất

```rust
pub enum ArchiveKind { Zip, Tar, TarGz, Gz }
pub fn detect(path: &Path) -> Option<ArchiveKind>;              // magic bytes
pub fn list(path: &Path, kind: ArchiveKind) -> Result<Vec<EntryInfo>, ArchiveError>;
pub fn read_entry(path: &Path, kind: ArchiveKind, entry: &str, limit: u64) -> Result<Vec<u8>, ArchiveError>;
pub fn read_source(source: FileSource) -> io::Result<Vec<u8>>;  // Disk → fs::read
```

`EntryInfo` gồm `name`, `size` (dung lượng giải nén do Archive khai báo) và `head: Vec<u8>` (tối đa 8 KB đầu để `is_binary` dùng lại). `ArchiveError` gồm `Damaged`, `Encrypted`, `NotFound`, `TooLarge`, `Io`.

Ba chỗ đang gọi `std::fs::read` chuyển sang `read_source`. `ParsedFile::from_path` đổi thành `from_source`.

- Nhận dạng bằng magic bytes: zip `PK\x03\x04` (hoặc `PK\x05\x06` cho zip rỗng), gzip `1f 8b`, tar là chuỗi `ustar` ở offset 257. Với gzip, giải nén 512 byte đầu rồi kiểm tra `ustar` để phân biệt `TarGz` và `Gz`.
- Crate: `zip` (bỏ default features, chỉ bật `deflate`), `tar`, `flate2`. Đều là Rust thuần, khớp với hướng đã chọn ở ADR 0009.
- Với `.gz` đơn lẻ, tên entry là tên Archive bỏ `.gz`, và `read_entry` bỏ qua tham số `entry`.

### 3. Danh sách chờ chứa nguồn thay vì chuỗi đường dẫn

`PendingFile.path: String` thay bằng `PendingFile { source: PendingSource, status }`, trong đó `PendingSource` là `Disk(String)` hoặc `Entry { archive, entry, size }`. `PendingStatus` thêm `LargeFile` (vẫn được thêm), `Damaged` và `Encrypted` (không được thêm). `add_paths` gọi `archive::detect`: nếu là Archive thì `list` rồi đẩy từng entry; lọc thư mục, symlink, `__MACOSX/`, `.DS_Store`. `importable_*` trả về nguồn để tạo `LogFile`. Thêm `total_uncompressed()` để tính nút Create.

Hiện tại `add_paths` chạy đồng bộ. Liệt kê một tar.gz lớn phải giải nén cả file, nên với Archive, việc liệt kê chạy trên background executor của gpui. Trong lúc chờ, danh sách hiện một dòng "Reading bundle.tar.gz…". File thường vẫn xử lý đồng bộ như cũ.

### 4. Chống zip bomb ở hai lớp

- Lúc liệt kê: dùng `size` mà Archive khai báo để hiện cảnh báo `LargeFile` (> 200 MB) và chặn Create khi tổng > 2 GB.
- Lúc đọc: `read_entry` bọc reader bằng `Read::take(min(declared + 1, limit))`. Đọc ra nhiều hơn `declared` hoặc chạm `limit` (2 GB) thì trả `TooLarge`. Lớp này chặn được Archive khai báo sai dung lượng.

Hai ngưỡng này là hằng số trong `archive.rs`, chưa đưa vào Settings.

### 5. Entry là tĩnh: bỏ qua ở đường đọc live

`sync_live_sources` bỏ qua các file có `entry.is_some()`, cùng chỗ đang bỏ qua Scratch. `ensure_*_loaded` nhìn `file.source()`: với entry thì gọi nhánh tĩnh (không có `LiveBuffer`). `active_space_is_live` trả `false` khi mọi file của space là entry, nên Follow/Pause không hiện. Space trộn entry với file thường vẫn live cho các file thường.

### 6. Đọc lại khi mở Project dùng lại `list`/`read_entry`

Không cache nội dung giải nén. Mở lại Project thì đọc lại entry từ Archive. Zip đọc thẳng được entry qua central directory. Tar/tar.gz phải duyệt tuần tự đến entry, chấp nhận được vì việc này chỉ chạy khi mở file.

Khi một space có nhiều entry từ cùng một tar.gz, `ensure_file_loaded` gom theo Archive và đọc tất cả entry cần trong một lần duyệt (`read_entries(path, kind, &[entry])`). Nhờ vậy không phải giải nén N lần.

## Risks / Trade-offs

- [Tar.gz vài GB, mở lại chậm vì phải duyệt tuần tự] → Đọc trên background như file lớn hiện nay. Gom nhiều entry vào một lần duyệt. Nếu vẫn chậm thì sau này cache danh sách offset.
- [Entry lớn chiếm RAM gấp nhiều lần dung lượng nén] → Cảnh báo `LargeFile` và trần 2 GB tổng. Đây cũng là giới hạn sẵn có với file thường.
- [Crate `zip` kéo theo nhiều codec (bzip2, zstd, aes)] → Tắt default features, chỉ bật `deflate`. Zip dùng codec khác sẽ ra `Damaged` với lý do "unsupported compression".
- [Tên entry không phải UTF-8, hoặc có `..`/đường dẫn tuyệt đối] → Không giải nén ra đĩa nên không có path traversal. Tên chỉ dùng để hiển thị và làm khoá, được chuyển bằng `to_string_lossy`.
- [Archive bị thay trên đĩa nhưng entry cùng tên có nội dung khác] → Marker dựa vào `line_hash` nên Marker không khớp sẽ tự bị bỏ, như với file thường.

## Migration Plan

Không có dữ liệu cần chuyển. Trường `entry` mới có giá trị mặc định. Muốn rollback thì một session có entry, khi mở bằng bản cũ, sẽ coi Archive là file thường và đọc ra rác. Chấp nhận được vì đây là app desktop chạy một phiên bản.

## Open Questions

- Có nên cho thả thẳng một `.gz` vào Log Space đang live để đọc kèm file xoay vòng (`app.log` + `app.log.1.gz`) như một chuỗi Generation không? Tạm thời không: chúng là hai file riêng, gộp theo timestamp.
