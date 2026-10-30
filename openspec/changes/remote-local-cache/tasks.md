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
- [ ] 2.2 Stream của Remote Command: `seg-<k>.stream` (1 bit mỗi dòng) được ghi cạnh segment và đọc vào `LineMeta` khi index (decision 6).
  - Test: cột Stream và Filter theo stream cho kết quả như đường RAM hiện tại.

## 3. Source: Checkpoint và Resume

- [ ] 3.1 `RemoteFileSource`:
  - Giữ 4 KiB byte gốc cuối cùng đã đọc, và hash 4 KiB đầu của mỗi Generation.
  - Gửi `SourceEvent::Checkpoint { generation, inode, server_end, head_hash, tail_hash }` sau mỗi lượt đọc làm offset tăng (tối đa mỗi 2 s), và khi đóng.
  - Test với sshd thử (`scripts/ssh-test-env.sh`): hash bằng FNV-1a của đúng các byte đó trên Server.
- [ ] 3.2 `SourceCommand`/tham số `Resume`: thay vì `open_tail`, source `stat` file và đọc hai khối 4 KiB qua `open_sftp_random`, rồi:
  - khớp thì đọc tiếp từ `server_end`;
  - không khớp thì gửi `Break` "rotated", "replaced" hoặc "truncated", rồi `open_tail` file mới (decision 5).
  - Test: ghi thêm, rotate, ghi đè cùng kích thước, cắt ngắn, Server không có `stat`.

## 4. App: Remote File đi qua cache

- [ ] 4.1 Cờ `remote_cache_enabled` trên `AppView` (mặc định tắt cho tới nhóm 9). Khi bật, `ensure_remote_file_loaded` mở `RemoteCache` và một `IndexedFile` trên chain của nó thay cho `LiveBuffer` giữ dòng.
- [ ] 4.2 `Tail` và `Append` ghi vào cache rồi `IndexedFile::extend`, như `apply_indexed_events` của file local.
  - Test: view, Filter, Search, Highlight và Marker giống hệt đường RAM với cùng chuỗi event.
- [ ] 4.3 `Backfill`, `Older` (Load more), `Earlier` (Rotated File): chèn segment hoặc Generation lên trên, index lại ở nền, view dựng lại và giữ vị trí (decision 3). Breaks `StartOfLoaded`, `BeforeStartTime`, `EarlierAvailable` và `LeftWindow` giữ nguyên.
- [ ] 4.4 Cắt Live Window, Back to Live Window và Clear đổi thành thao tác trên cache, rồi index lại.
  - Marker của dòng rời cache bị bỏ, như L12.
  - `hold_info` lấy từ `LiveBuffer` chỉ còn trạng thái (decision 4).
  - Test: so với `trimming_one_file_equals_building_the_view_again`.
- [ ] 4.5 Break của source (Rotated, Truncated, Disconnected, Reconnected) mở Generation mới trong cache đúng như `begin_generation_state` hiện tại.
- [ ] 4.6 `Checkpoint` được ghi vào `meta.json`; `last_opened` được cập nhật khi tab được chọn.
- [ ] 4.7 Ghi cache lỗi (đĩa đầy) thì báo một lần, và file đó quay về giữ dòng trong `BytesSource` cho tới khi đóng tab.

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

- [ ] 8.1 Xoá cache khi file rời Log Space, khi Log Space bị xoá, khi Edit… làm nó thành file khác, và khi Server bị xoá.
- [ ] 8.2 Lúc khởi động: sweep các thư mục không có `LogFile` trỏ tới (như `copy::sweep`), và bỏ cache có `last_opened` cũ hơn 14 ngày.
- [ ] 8.3 Giới hạn dung lượng chung, mặc định 2 GB, trong Settings mục Live, kèm nút "Clear Remote cache".
  - Kiểm sau khi ghi (tối đa mỗi 10 s). Bỏ cache được mở lâu nhất trước; không bỏ cache của tab đang mở.
  - Test: vượt giới hạn với một tab đang mở.

## 9. Bật mặc định và dọn đường RAM

- [ ] 9.1 Live Window mặc định 1.000.000 dòng. Session đã chọn số khác 100.000 thì giữ số đó (decision 8).
- [ ] 9.2 Bật `remote_cache_enabled` mặc định. Chạy lại toàn bộ test, kể cả test cần sshd (`SSH_TEST_DIR`/`SSH_TEST_PORT`).
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
