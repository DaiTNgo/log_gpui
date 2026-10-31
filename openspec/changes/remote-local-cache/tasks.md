## 1. Kho cache trên đĩa

- [x] 1.1 `src/remote/cache.rs`: `RemoteCache` cho một Remote File hoặc Remote Command (decision 1, 2).
  - Thư mục `<app data>/remote-cache/<server id>/<fnv64 của path>/` (hoặc `cmd-<fnv64 của command>`), quyền 0700.
  - `meta.json` ghi atomic. Thiếu hoặc hỏng thì coi như chưa có cache.
  - `gen-<n>/seg-<k>.log`. Ghi nối thêm vào segment cuối, mở segment mới mỗi khoảng `window / 10` dòng.
  - Chèn một segment lên đầu một Generation; thêm một Generation lên trên cùng.
  - Bỏ nguyên segment cũ nhất khi vẫn còn đủ `window` dòng; Clear.
  - Lock file theo pid: tiến trình thứ hai không dùng cache.
  - Test: ghi, chèn, cắt và Clear cho đúng các dòng, theo đúng thứ tự, sau khi mở lại thư mục.
  - Đã làm: `RemoteCache::open`, `append`, `prepend`, `add_earlier_generation`, `begin_generation`, `trim_to`, `clear` và `set_checkpoint`. Segment ghi sau lần lưu cuối được đếm lại khi mở (`recover`). Test ở `tests/remote_cache_test.rs`. Chưa nối vào app.
- [x] 1.2 `ChainSource` của các segment theo thứ tự (dùng `index::ChainSource` có sẵn), để Line Index đọc cache như một file.
  - Test: `IndexedFile` trên chain cho cùng entries với việc parse cả text nối lại.
  - Đã làm: `RemoteCache::chain` trả chain các segment và nơi bắt đầu của từng Generation (vị trí, số dòng đầu). Chỉ segment cuối còn lớn lên. Khi `append` mở segment mới, app phải `push` segment đó vào chain (nhóm 4).

## 2. Line Index cho cache

- [x] 2.1 `GenerationStart.first_number`: `IndexedFile::place` đánh số `first_number + (n − pos)` (decision 3). Mặc định là 1, nên file local không đổi.
  - Test: một Generation bắt đầu ở dòng 4.001 trên Server thì dòng đầu có số 4.001, kể cả với Marker và Go to Line.
  - Đã làm: `first_number` trên `GenerationStart`, `IndexedFile::set_generations`. `place`, `line_number` và `FileEntries::position_of` (Marker, Next Marker) đánh số từ đó. Go to Line trong app và `drop_markers_that_no_longer_match` (đang coi vị trí là `line − 1`) sẽ kiểm ở nhóm 4.
- [x] 2.2 Stream của Remote Command: `seg-<k>.stream` (1 bit mỗi dòng) được ghi cạnh segment và đọc vào `LineMeta` khi index (decision 6).
  - Test: cột Stream và Filter theo stream cho kết quả như đường RAM hiện tại.
  - Đã làm: `seg-<k>.stream` giữ 1 byte mỗi dòng (0 file, 1 stdout, 2 stderr), không phải 1 bit, để ghi nối và cắt theo dòng như `.lens`. `IndexedFile::set_streams` nhận stream theo vị trí (dạng `Runs`); các row đọc qua Line Index và Filter Scan (`each_entry_at`) đều có `stream`. Test so với đường RAM nằm ở 6.1.

## 3. Source: Checkpoint và Resume

- [x] 3.1 `RemoteFileSource`:
  - Giữ 4 KiB byte gốc cuối cùng đã đọc, và hash 4 KiB đầu của mỗi Generation.
  - Gửi `SourceEvent::Checkpoint { generation, inode, server_end, head_hash, tail_hash }` sau mỗi lượt đọc làm offset tăng (tối đa mỗi 2 s), và khi đóng.
  - Test với sshd thử (`scripts/ssh-test-env.sh`): hash bằng FNV-1a của đúng các byte đó trên Server.
  - Đã làm: thay vì giữ 4 KiB cuối trong `Memory`, Checkpoint đọc lại hai khối 4 KiB qua SFTP (`read_exactly`) khi offset đã đổi và đã qua 2 s. `server_end` không tính dòng đang viết dở. Khi đóng source thì chưa gửi Checkpoint; Checkpoint 2 s gần nhất được dùng.
- [x] 3.2 `SourceCommand`/tham số `Resume`: thay vì `open_tail`, source `stat` file và đọc hai khối 4 KiB qua `open_sftp_random`, rồi:
  - khớp thì đọc tiếp từ `server_end`;
  - không khớp thì gửi `Break` "rotated", "replaced" hoặc "truncated", rồi `open_tail` file mới (decision 5).
  - Test: ghi thêm, rotate, ghi đè cùng kích thước, cắt ngắn, Server không có `stat`.
  - Đã làm: field `RemoteFileSource::resume` (`ResumeFrom`), giữ trong `Memory` qua lần mất kết nối. Khớp thì đọc tiếp từ `server_end`, không backfill. Không khớp thì gửi Break "rotated" (inode khác) hoặc "truncated", rồi `open_tail`. Test với sshd thử: ghi thêm, rotate, ghi đè cùng kích thước. Chưa test cắt ngắn riêng và Server không có `stat` (trường hợp đó chỉ so kích thước và hash).

## 4. App: Remote File đi qua cache

- [x] 4.0 Giai đoạn 1 (mirror, theo Migration Plan): khi tab đang mở, Remote File vẫn giữ dòng trong RAM như cũ. Khi rời tab hoặc thoát app, các dòng đó, tính tới Checkpoint cuối, được ghi ra cache. Chọn lại tab hoặc mở app thì dòng hiện ngay từ cache, rồi source đọc tiếp bằng `Resume`.
  - Đã làm:
    - `app::remote_cache`: `snapshot_remote_file` ghi lúc `sync_live_sources` và `drop_dormant_files` bỏ file, `snapshot_all_remote_files` ghi lúc thoát app, `remote_file_from_cache` nạp lại trong `ensure_remote_file_loaded`.
    - `LiveBuffer::lines_after` bỏ các dòng sau Checkpoint, để chúng được đọc lại thay vì trùng. `LiveBuffer::restore` dựng lại buffer với số dòng thật, Generation, offset và inode.
    - Độ dài byte gốc của từng dòng được lưu trong `seg-<k>.lens`, nên Load more vẫn tính đúng offset trên Server.
    - Marker không bị reset khi file được nạp lại từ cache.
    - Chỉ cho Remote File, chưa cho Remote Command.
  - Test với sshd thử: `remote_cache_app_test` (rời tab, Server ghi thêm 5 dòng, chọn lại tab: 3.000 dòng hiện ngay với số dòng thật, rồi chỉ 5 dòng mới được đọc, không trùng, không thiếu).
  - 7 test có sshd hỏng (`remote_command_source_test` ×6, `remote_edit_test::narrowing_a_live_glob…`). Chúng hỏng y hệt khi chưa có thay đổi này.
  - Text vẫn nằm trong RAM khi tab mở; phần đó là các task 4.1–4.7 (giai đoạn 2).
- [x] 4.0b `CachedFile` (`src/live/cached.rs`): giữ những gì `LiveBuffer` giữ của một Remote File, nhưng ghi dòng vào `RemoteCache` và đọc qua `IndexedFile` (decision 3, 4).
  - Gồm: số dòng tạm cho tới `TotalLines`, Break giữa các dòng (`IndexedFile::set_marks`), Generation, cắt Live Window chính xác tới từng dòng (segment được đọc từ sau các dòng đã rời, `OffsetSource`), Load more, Rotated File, Clear, `held_start` (từ `seg-<k>.lens`).
  - Test `cached_file_test`: cùng chuỗi event cho `LiveBuffer` và `CachedFile` thì ra cùng số dòng, Generation, text và Break, cùng `held_start`.
  - Cũng sửa: `parent` của Continuation Line trong Generation có `first_number` khác 1.
- [x] 4.1 Cờ `remote_cache_enabled` trên `AppView` (mặc định tắt cho tới nhóm 9). Khi bật, `ensure_remote_file_loaded` mở `RemoteCache` và một `IndexedFile` trên chain của nó thay cho `LiveBuffer` giữ dòng.
- [x] 4.2 `Tail` và `Append` ghi vào cache rồi `IndexedFile::extend`, như `apply_indexed_events` của file local.
  - Test: view, Filter, Search, Highlight và Marker giống hệt đường RAM với cùng chuỗi event.
- [x] 4.3 `Backfill`, `Older` (Load more), `Earlier` (Rotated File): chèn segment hoặc Generation lên trên, index lại ở nền, view dựng lại và giữ vị trí (decision 3). Breaks `StartOfLoaded`, `BeforeStartTime`, `EarlierAvailable` và `LeftWindow` giữ nguyên.
- [x] 4.4 Cắt Live Window, Back to Live Window và Clear đổi thành thao tác trên cache, rồi index lại.
  - Marker của dòng rời cache bị bỏ, như L12.
  - `hold_info` lấy từ `LiveBuffer` chỉ còn trạng thái (decision 4).
  - Test: so với `trimming_one_file_equals_building_the_view_again`.
- [x] 4.5 Break của source (Rotated, Truncated, Disconnected, Reconnected) mở Generation mới trong cache đúng như `begin_generation_state` hiện tại.
- [x] 4.6 `Checkpoint` được ghi vào `meta.json`; `last_opened` được cập nhật khi tab được chọn.
  - Đã làm 4.1–4.6 cùng lúc vì chúng dùng chung một nhánh event (`apply_cached_events`), nên chỉ có một commit:
    - Cờ `remote_cache_index`. `ensure_remote_file_loaded` mở `CachedFile`, nạp lại từ cache nếu có Checkpoint (`CachedFile::restore`), không thì tạo mới. `ParsedFile` chỉ có `indexed`, không có `entries` và `lines`.
    - Mọi event của Remote File đi qua `CachedFile`. Cắt Live Window và cấu trúc đổi thì view dựng lại.
    - `hold_info`, Clear, Load more, Rotated File, Back to Live Window và đổi Live Window đều chạy với `CachedFile`.
    - Rời tab thì `CachedFile::close` bỏ các dòng sau Checkpoint cuối (`RemoteCache::drop_bottom`).
  - Test:
    - `cached_remote_app_test`: không cần sshd. Cùng chuỗi event thì view (rows, Break, Filter, Match, `hold_info`) giống đường RAM, và không có text trong memory.
    - `remote_cache_app_test` (sshd thử): với cờ bật, quay lại tab chỉ đọc 5 dòng mới.
- [ ] 4.7 Ghi cache lỗi (đĩa đầy) thì báo một lần, và file đó quay về giữ dòng trong `BytesSource` cho tới khi đóng tab.
  - Hiện tại: lỗi ghi chỉ hiện thành notice. Chưa có đường quay về giữ dòng trong memory.

## 5. Mở lại từ cache

- [ ] 5.1 Chọn lại Dormant Tab và mở app: nếu Remote File có cache thì
  - index cache ở nền (như file local);
  - view hiện dòng với số dòng thật và Marker;
  - source khởi động bằng `Resume` từ Checkpoint cuối, không `open_tail`.
  - Test: quay lại tab chỉ đọc phần mới từ Server; mở lại app hiện dòng trước khi kết nối xong.
- [ ] 5.2 Marker của Generation cũ không còn bị reset khi source khởi động lại (`start_missing_sources`, `src/app/live.rs:392`). Chúng khớp theo `generations_ago` của cache.

## 6. Remote Command

- [ ] 6.1 Mỗi lần chạy là một Generation trong `cmd-<hash>/`, có stream.
  - Mở lại thì output lần trước hiện ngay dưới Break "previous run HH:MM", rồi lệnh chạy theo Resume By Time hoặc Restart.
  - Clear bỏ các segment, số dòng đếm tiếp.
  - Test: `remote_command_space_test` chạy với cache bật.

## 7. Remote Copy qua Line Index

- [ ] 7.1 `ParsedFile::from_remote_copy` thành `IndexedFile` trên chain các `gen-*.log`, đánh số theo `first_line_no` của từng Generation (decision 9). Reload và `restore_markers_after_reload` đọc qua Line Index.
  - Test: các test của `remote-full-read` và Open in full.

## 8. Dọn cache

- [x] 8.1 Xoá cache khi file rời Log Space, khi Log Space bị xoá, khi Edit… làm nó thành file khác, và khi Server bị xoá.
  - Đã làm: sau mỗi lần file rời app (`forget_file`), xoá Project, xoá Server, app sweep các cache không còn `LogFile` nào trỏ tới. Cache mà một Log Space khác vẫn trỏ tới thì giữ. File đang đọc qua một cache không còn là của nó (Edit…) thì thôi đọc cache đó trước.
  - Chỉ xoá thư mục chỉ chứa những gì cache ghi; cache mà một cửa sổ app khác đang giữ `lock` thì để lại.
- [x] 8.2 Lúc khởi động: sweep các thư mục không có `LogFile` trỏ tới (như `copy::sweep`), và bỏ cache có `last_opened` cũ hơn 14 ngày.
- [x] 8.3 Giới hạn dung lượng chung, mặc định 2 GB, trong Settings mục Live, kèm nút "Clear Remote cache".
  - Kiểm sau khi ghi (tối đa mỗi 10 s). Bỏ cache được mở lâu nhất trước; không bỏ cache của tab đang mở.
  - Test: vượt giới hạn với một tab đang mở.
  - Đã làm: Settings › General có hàng "Remote cache" (512 MB đến 10 GB, dung lượng hiện tại, nút "Clear Remote cache"). Nếu chỉ còn cache của tab đang mở mà vẫn vượt, app báo một notice một lần. 8.1–8.3 nằm chung một commit vì dùng chung `sweep`/`list`/`remove`.

## 9. Bật mặc định và dọn đường RAM

- [ ] 9.1 Live Window mặc định 1.000.000 dòng. Session đã chọn số khác 100.000 thì giữ số đó (decision 8).
- [x] 9.2 Bật `remote_cache_enabled` mặc định. Chạy lại toàn bộ test, kể cả test cần sshd (`SSH_TEST_DIR`/`SSH_TEST_PORT`).
  - Đã làm: `remote_cache_index` (giai đoạn 2) bật mặc định. Toàn bộ test có sshd thử: 1351 qua, 5 hỏng. Cả 5 thuộc nhóm đã hỏng trước thay đổi này (`remote_command_source_test` ×4, `remote_edit_test::narrowing_a_live_glob…`). Remote Command vẫn giữ dòng trong memory (nhóm 6).
- [ ] 9.3 Xoá đường RAM của Remote:
  - `LiveBuffer` không còn giữ dòng;
  - `trim_open_view` cho file giữ trong RAM;
  - `ParsedFile::entries`, `ParsedFile::lines`, `FileEntries::Held` cho Remote.
  - Làm task 12.1 của `line-offset-index`, và đánh dấu task 9.4 của change đó đã được thay bằng change này.
- [ ] 9.4 Đo bằng `perf_probe` và `mem_probe` với một Remote File 1.000.000 dòng (sshd thử): RSS, thời gian mở lại tab, thời gian index lại khi chèn hoặc cắt. Ghi vào design.md.
- [ ] 9.5 Cập nhật CONTEXT.md:
  - thêm thuật ngữ Remote Cache;
  - sửa Live Window;
  - sửa L10 (dòng nhận được giữ trên đĩa giữa các lần mở);
  - bỏ ngoại lệ Remote ở quy tắc 43.
