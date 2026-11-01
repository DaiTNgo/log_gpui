## Why

Sau `line-offset-index` và `remote-local-cache`, file local, Archive Entry, Remote Copy và Remote File đều đọc qua Line Index, và text nằm trên đĩa. Còn ba chỗ mà memory hoặc thời gian không theo kịp hướng đó:

- **Scratch vẫn giữ cả file trong RAM.** `ensure_scratch_loaded` đọc cả file vào `BytesSource` rồi `index_now` chạy đồng bộ. Mỗi lần sửa, `reparse_edited_scratch_files` lại index một bản copy nữa từ text trong memory. Một file scratch đã sửa có thể có tới bốn bản text cùng lúc: editor, `raw.texts`, `lines_cache` và `BytesSource`. Editor và text của nó còn ở lại cả khi tab đã Dormant. Điều này trái với quy tắc 42 (Reading Never Blocks) và 43 (Text Stays On Disk).
- **Quay lại tab luôn index lại.** Quy tắc 44 bỏ Line Index ngay khi rời tab. Với 3 triệu dòng, quay lại mất khoảng 0,7 s, trong khi giữ Line Index chỉ tốn khoảng 29 B/dòng (~90 MB). `line-offset-index` (quyết định 13) đã để "giữ tab gần đây theo LRU với memory budget" cho sau, chờ đo lại. Số đo đã có.
- **Archive Entry giải nén và index trên UI thread.** Entry đã được ghi ra `archive-cache/` và đọc từ đĩa, nhưng `ensure_archive_entries_loaded` gọi `archive_cache::unpack` ngay trong luồng gọi, rồi `ParsedFile::from_path` gọi `index_now`, cũng đồng bộ. Mở lần đầu một `.tar.gz` có entry 1 GB làm đứng cửa sổ suốt lúc giải nén. Các lần sau không phải giải nén lại, nhưng vẫn index đồng bộ. Điều này trái với quy tắc 42.
- **RowCache tính theo từng file.** Mỗi `IndexedFile` giữ 4096 entry đã parse (`ROW_CACHE`). Một Log Space gộp 50 file từ Live Glob có thể giữ tới 200.000 `LogEntry`, mỗi cái ≥ 240 byte, chưa tính heap. Memory tăng theo số file, không theo số dòng đang xem.

## What Changes

- **Scratch đọc từ đĩa như file local.**
  - File scratch chưa sửa được index ở nền từ file trên đĩa (`DiskSource`), có tiến độ, như file trong Static Log Space. Không còn đọc cả file vào memory.
  - Sau khi sửa trong Raw View: khi cần Parsed View (chuyển view, chuyển tab), hệ thống lưu file trước rồi index lại từ đĩa ở nền. Không còn index từ text trong memory.
  - Editor, text đã sửa và `lines_cache` của một file scratch được bỏ khi tab của nó thành Dormant, sau khi đã lưu. Text đang sửa vẫn nằm trong editor khi tab đang mở: đó là thứ người dùng đang gõ.
  - Lưu thất bại thì không bỏ gì, và báo như hiện tại.
- **Archive Entry giải nén và index ở nền.**
  - Giải nén ra `archive-cache/` chạy ở nền, vẫn một lượt cho mỗi Archive, có tiến độ theo byte đã đọc của Archive, và huỷ được. Huỷ thì không để lại file `.part`.
  - Entry giải nén xong thì được index ở nền như file local, cùng đường với `queue_file_load`.
  - Entry đã giải nén từ trước thì vào thẳng bước index ở nền.
  - Lỗi (Archive hỏng, không tìm thấy, quá `MAX_UNCOMPRESSED_BYTES`) vẫn báo như hiện tại, sau khi việc nền kết thúc.
- **Warm Tab: giữ vài tab vừa rời trong một memory budget.**
  - Rời một tab thì Line Index của các file của nó được giữ lại, nếu tổng memory của các Warm Tab còn trong **Warm Tab Budget**. Quá budget thì tab rời lâu nhất thành Dormant như hiện tại.
  - Warm Tab không đọc gì: source live dừng, Live Glob không được theo dõi, file static không được watch. Quy tắc 44 chỉ đổi phần "bỏ dòng khỏi memory".
  - Chọn lại một Warm Tab thì view được dựng lại từ Line Index đã giữ (~60 ms với 3 triệu dòng), không index lại. Trước đó hệ thống kiểm tra file trên đĩa: file chỉ được ghi thêm thì đọc tiếp phần mới; file bị thay hoặc cắt thì index lại, như file đang mở.
  - Warm Tab Budget mặc định 512 MB, chỉnh trong Settings. 0 tắt Warm Tab, tức hành vi hiện tại.
  - Remote File và Remote Command chưa được giữ Warm trong change này. Chúng vẫn thành Dormant khi rời tab và mở lại từ Remote Cache.
- **Một Row Cache chung cho cả app.**
  - Thay `RowCache` 4096 entry của mỗi file bằng một cache dòng đã parse chung, có giới hạn theo byte (mặc định 64 MB).
  - Bỏ dòng ít dùng nhất trên toàn app, không theo từng file. Một Log Space 50 file và một file đơn tốn cùng giới hạn.
  - Bỏ Line Index của một file (Dormant, gỡ file) thì bỏ luôn các dòng của nó trong cache chung.

Ngoài phạm vi:
- Bỏ đường đọc Archive Entry vào memory (`ensure_live_file_loaded`, `ParsedFile::from_source`): việc dọn dẹp riêng.
- `drop_top` và `drop_bottom` của Remote Cache đọc cả segment: chờ số đo của task 9.4 trong `remote-local-cache`.
- `OtelInput` của `otel-view` nằm trong memory: ngoại lệ có chủ ý (quy tắc 47).
- Warm Tab cho Remote File và Remote Command.
- Phản ứng với memory pressure của hệ điều hành (xem Open Questions trong design.md).

## Domain changes

Change này không sửa CONTEXT.md vì các session khác đang làm song song. Khi áp dụng change, cần cập nhật CONTEXT.md như sau.

**Thuật ngữ mới (nhóm Views / Spaces):**
- **Warm Tab**: một tab người dùng vừa rời mà Line Index của các file của nó vẫn được giữ trong memory, trong Warm Tab Budget. Nó không đọc gì cho tới khi được chọn lại; khi đó view hiện lại ngay, chỉ đọc phần file mới ghi thêm. Quá budget thì tab rời lâu nhất thành Dormant Tab. _Avoid_: cached tab, background tab, kept tab.
- **Warm Tab Budget**: memory tối đa mà mọi Warm Tab được giữ, một giá trị chung cho cả app. 0 nghĩa là rời tab nào thì tab đó thành Dormant ngay. _Avoid_: tab cache size, memory limit.

**Thuật ngữ đổi nghĩa:**
- **Dormant Tab**: "let go when the user left it" thành "let go when the user left it and it no longer fits in the Warm Tab Budget".
- **Scratch Log Space**: thêm "Its files are read from disk like any local file; what is being typed is saved before the Parsed View reads it."
- **Archive Entry**: câu "It is read straight into memory, never extracted to disk, and never read live." đã sai so với code từ `line-offset-index`. Đổi thành "It is unpacked once, in the background, to the app's archive cache and read from there like a local file; it is never read live."

**Quy tắc đổi:**
- **44. Only The Open Tab Reads** → "When the app opens only the chosen tab is read; every other tab is a Dormant Tab. Only the open tab reads: a tab the user leaves stops reading at once. Its Line Indexes stay in memory as a Warm Tab while they fit in the Warm Tab Budget, the tab left longest ago going Dormant first; a Dormant Tab reads its files again when chosen (a Remote File from its Remote Cache, then only what is new on the Server). Closing the last tab that shows files of a Server ends the connection to it."
- **43. Text Stays On Disk**: thêm "A scratch file's text is in memory only while its editor is open in the open tab."

## Capabilities

### New Capabilities
- `memory-budget`: Warm Tab và Warm Tab Budget; Scratch đọc từ đĩa và index lại từ đĩa sau khi sửa; editor của Scratch bỏ khi tab Dormant; Archive Entry giải nén và index ở nền; Row Cache chung có giới hạn byte.

### Modified Capabilities
<!-- `dormant-tab` mới chỉ có trong change `line-offset-index`, chưa archive vào openspec/specs/. Vì vậy hành vi mới được viết dưới dạng ADDED trong `memory-budget`, và nói rõ requirement "Rời tab thì bỏ dòng, giữ kết nối" của `dormant-tab` đổi ra sao. Nếu `line-offset-index` được archive trước khi change này được áp dụng, chuyển phần đó thành MODIFIED của `dormant-tab`. -->

## Impact

- **Code chịu ảnh hưởng chính:**
  - `src/app/tabs.rs` (`ensure_scratch_loaded`, `scratch_parsed`)
  - `src/app/raw.rs` (`reparse_edited_scratch_files`, `flush_scratch_saves`, editor và `texts`, `lines_cache`)
  - `src/app/live.rs` (`drop_dormant_files`, `sync_live_sources`, `is_dormant_tab`)
  - `src/app/loading.rs` (`queue_file_load`, `start_queued_loads`, `drop_dormant_loads`)
  - `src/app/tabs.rs` (`ensure_archive_entries_loaded`), `src/archive_cache.rs` (`unpack`: tiến độ và huỷ)
  - `src/index/view_rows.rs` (`RowCache`, `ROW_CACHE`), `src/index/file.rs` (`IndexedFile::cache`, ước lượng memory)
  - Tab strip (hiển thị Warm khác Dormant hay không: xem design.md)
  - `src/ui/settings.rs`, `src/workspace/state.rs`, `src/workspace/session.rs` (Warm Tab Budget)
- **Settings:** thêm Warm Tab Budget (MB).
- **Session:** thêm một giá trị Settings, có mặc định khi thiếu. Không đổi format khác.
- **Đo:** `perf_probe` (chuyển tab Warm và Dormant), `mem_probe` (Log Space nhiều file với Row Cache chung).
- **Phụ thuộc:** nối tiếp `line-offset-index` (Dormant Tab, Line Index) và `remote-local-cache` (Remote vẫn Dormant khi rời).
