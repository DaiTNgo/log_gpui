## Context

Sau `line-offset-index`, file local được đọc qua `IndexedFile` (Line Index trên `index::LineSource`, LRU cho dòng đang hiện). Remote thì vẫn giữ mọi dòng trong RAM.

**Đường hiện tại của Remote File** (đọc code ngày 2026-10-30):
- `ensure_remote_file_loaded` (`src/app/tabs.rs:296`) tạo một `ParsedFile::empty()` có `LiveBuffer::remote`.
- `RemoteFileSource` (`src/live/remote.rs`) gửi các event có dòng: `Tail` (1000 dòng cuối), `Backfill` (lô 5000 dòng cho tới khi đầy Live Window), `Append`, `Older` (Load more) và `Earlier` (Rotated File).
- `apply_file_events` (`src/app/live.rs:726`) chuyển các event đó cho `LiveBuffer` (`src/live/buffer.rs`). `LiveBuffer` sửa `ParsedFile.entries`, `lines`, `raw_breaks` và `byte_lens`.
- Live Window được cắt bằng `trim_to` (`buffer.rs:913`), rồi `trim_open_view` (`src/app/live_trim.rs`) bỏ các rows tương ứng khỏi view.

**Dormant Tab:** rời tab thì `parsed_files` bị bỏ. Quay lại thì `RemoteFileSource` mới tạo `Memory` mới, gọi lại `open_tail` và backfill, nên **cả Live Window được tải lại từ Server** (`src/app/live.rs:331-404`). Không có dòng, offset, inode hay `clear_mark` nào được lưu qua lần mở app.

**Remote Command** (`src/live/command.rs`): `Append` có `StreamMask` (stdout hoặc stderr) và không có byte length. Mỗi lần chạy lại là một Generation.

**Remote Copy** (`src/remote/copy.rs`): đã ghi ra đĩa, ở `remote-copies/<id>/gen-<n>.log` kèm `meta.json` (ghi atomic, có sweep lúc khởi động). Nhưng `ParsedFile::from_remote_copy` vẫn đọc cả file vào `entries`.

**SFTP sẵn có:**
- `open_sftp_random` đọc ngẫu nhiên, mỗi lần 64 KiB.
- `stat` lấy inode và kích thước qua exec.
- Có thể đọc N byte tại một offset (`read_exact_at`, `SftpReadAt`).

Không cần cài gì lên Server.

**Giới hạn của `IndexedFile`:**
- Chỉ nối thêm được ở cuối (`extend`) hoặc thêm Generation sau Generation cuối (`ChainSource`).
- Không chèn được dòng lên trên.
- Số dòng luôn bắt đầu từ 1 ở đầu Generation (`index/file.rs:191`).

## Goals / Non-Goals

**Goals:**
- Text của Remote File và Remote Command không nằm trong RAM: chỉ có Line Index và LRU, như file local (quy tắc 43 không còn ngoại lệ).
- Chọn lại Dormant Tab Remote thì dòng hiện ngay từ cache. Server chỉ gửi phần mới.
- Mở lại app cũng vậy: cache sống qua các lần mở app.
- Một đường code cho mọi nguồn: Remote dùng `IndexedFile`, `LiveBuffer` không còn giữ dòng. Nhờ đó task 12.1 của `line-offset-index` xoá được `ParsedFile::entries`, `ParsedFile::lines` và các đường dựng view cũ.
- Giữ nguyên các hành vi đã có spec: Live Window (L12, L13), Load more, Back to Live Window, Clear, Rotated File (L24), Starting Time, Remote Command (Resume By Time, Restart), Break.

**Non-Goals:**
- Không cài tailer hay agent lên Server.
- Không đồng bộ cache giữa các máy.
- Không đổi cách đọc Remote Copy, ngoài việc đọc nó qua Line Index (decision 9).
- Không nén cache.

## Decisions

### 1. Vị trí và cấu trúc cache

Cache nằm ở `<app data>/remote-cache/<server id>/<fnv64 của path, hex>/`. Remote Command dùng `cmd-<fnv64 của command text>` thay cho path.

Mỗi thư mục có:
- `meta.json`, ghi atomic (tmp rồi rename), như `copy::write_meta`.
- Mỗi Generation một thư mục `gen-<n>/`. `n` tăng khi Server rotate (Generation mới) và giảm khi đọc thêm Rotated File phía trên. Số tuyệt đối do meta quản.
- Trong mỗi `gen-<n>/` là các **segment** `seg-<k>.log`: các dòng đã decode, mỗi dòng kết thúc bằng `\n`.

`meta.json` ghi theo từng Generation:
- danh sách segment theo thứ tự đọc từ trên xuống;
- số dòng đầu tiên của nó trong file trên Server (`first_line_no`);
- `server_start`, `server_end`: offset byte của phần đã cache trong file trên Server;
- `inode`;
- `head_hash`: FNV-1a của 4 KiB đầu file trên Server;
- `tail_hash`: FNV-1a của 4 KiB ngay trước `server_end`;
- tên Rotated File, nếu là Generation cũ.

Ở mức cả file, `meta.json` ghi thêm `clear_mark`, `last_opened`, `hold` (Window hoặc Extended) và `server_id`, `path` (hoặc `command`).

*Thay thế đã cân nhắc:*
- Một file cho cả Remote File: không chèn được dòng lên trên mà không viết lại cả file.
- Lưu byte gốc của Server thay cho dòng đã decode: phải viết lại `LineSplitter` và cách decode. Không đáng, vì hash đồng bộ được tính trên byte gốc ở phía source (decision 5).

### 2. Segment: chèn lên trên và cắt mà không viết lại

- **Dòng mới** (`Append`): ghi vào cuối segment cuối của Generation mới nhất. Segment đạt khoảng `window / 10` dòng thì mở segment mới.
- **Dòng cũ hơn** (`Backfill`, `Older`): ghi thành một segment mới đứng **đầu** Generation của chúng.
- **Rotated File** (`Earlier`): ghi thành một Generation mới đứng trên cùng.
- **Cắt Live Window:** bỏ nguyên segment cũ nhất, khi bỏ segment đó vẫn còn đủ `window` dòng. Vì vậy file giữ từ `window` tới khoảng `window × 1,1` dòng, đúng như ngưỡng 10% mà `trim_to_window` (`buffer.rs:892`) đang dùng.
- **Clear:** bỏ mọi segment, rồi bắt đầu segment trống ở Generation hiện tại.

*Thay thế đã cân nhắc:* viết lại file mỗi lần cắt. Với Live Window 100.000 dòng, mỗi lần cắt phải ghi khoảng 10 MB.

### 3. Đọc cache: `ChainSource` của các segment, index lại khi thay đổi cấu trúc

`IndexedFile` của một Remote File đọc qua một `ChainSource` gồm các segment theo thứ tự.
- **Nối thêm ở cuối:** ghi rồi `IndexedFile::extend`, giống file local live (`apply_indexed_events`). Phần này không đổi.
- **Thay đổi cấu trúc** (chèn lên trên, cắt, Clear, Rotated File): dựng một `IndexedFile` mới từ chain mới bằng Indexing ở nền, rồi view dựng lại và giữ vị trí, như 6.3 của `line-offset-index`. Indexing chạy khoảng 70 ns/dòng (0,21 s cho 3 triệu dòng). Live Window mặc định 100.000 dòng nên mất khoảng 7 ms; giới hạn Full Read 2 triệu dòng mất khoảng 0,15 s.
- `GenerationStart` có thêm `first_number`: số dòng của dòng đầu tiên trong Generation đó. `IndexedFile::place` đánh số `first_number + (n − pos)` thay vì `n − pos + 1`. Vì cache chỉ giữ phần cuối của file trên Server, số dòng vẫn là số thật trên Server.

*Thay thế đã cân nhắc:* cho Line Index chèn lên trên và bỏ dòng ở đầu tại chỗ, kể cả `row_ids` và `MetaTable`. Cách đó nhanh hơn, nhưng phải sửa sâu `LineIndex` (block 128 dòng cùng base tuyệt đối) và `FilterIndex`. Để sau, nếu đo thấy index lại là chậm.

### 4. `LiveBuffer` chỉ còn là trạng thái, không giữ dòng

`LiveBuffer` giữ lại: `hold`, `clear_mark`, Generation hiện tại, inode, `next_offset`, các Break đang chờ và trạng thái Load more. Nó bỏ `entries`, `lines` và `byte_lens`, vì phần đó đã nằm trong meta.

Code mới `RemoteCache` (`src/remote/cache.rs`) làm các việc:
- ghi dòng và segment;
- ghi `meta.json`;
- trả `ChainSource` cho `IndexedFile`;
- cắt, Clear, chèn lên trên.

`apply_file_events` gọi `RemoteCache`, rồi `extend` hoặc index lại.

`trim_open_view` cho Remote đổi thành dựng lại view, giống lúc cắt một file đã index. Sau khi đo mà chậm thì mới làm bản cắt tại chỗ.

### 5. Đồng bộ khi mở lại: hash của byte gốc, tính ở phía source

Cache chứa dòng đã decode (bỏ `\r`, UTF-8 lossy), nên không so được với byte trên Server. Hash vì vậy luôn được tính trên byte gốc mà `RemoteFileSource` đọc được:
- `head_hash`: tính khi đọc 4 KiB đầu file (một lần mỗi Generation).
- `tail_hash`: `Memory` giữ 4 KiB byte cuối cùng đã đọc.

Source gửi `SourceEvent::Checkpoint { generation, inode, server_end, head_hash, tail_hash }`: sau mỗi lượt đọc làm offset tăng (tối đa một lần mỗi 2 s), và khi được yêu cầu đóng. App ghi Checkpoint vào `meta.json`.

**Mở lại** (chọn lại Dormant Tab, hoặc mở app):
1. App index cache ở nền, rồi view hiện các dòng đó với số dòng đúng.
2. Source mới nhận `Resume { generation, inode, server_end, head_hash, tail_hash, first_line_no }` thay vì gọi `open_tail`. Nó `stat` file trên Server, đọc 4 KiB đầu và 4 KiB trước `server_end` qua `open_sftp_random`.
3. Mọi thứ khớp (cùng inode nếu biết, kích thước ≥ `server_end`, cả hai hash khớp): `Memory.offset = server_end`, `opened = true`, và chỉ đọc phần mới. Không backfill lại.
4. Không khớp: cache cũ trở thành Generation cũ. Generation mới có Break "rotated" (inode đổi, file cũ còn trong danh sách Rotated File), "replaced" (inode đổi, không tìm thấy file cũ) hoặc "truncated" (cùng inode nhưng ngắn hơn hoặc hash đầu đổi). Sau đó `open_tail` file mới như lần đầu, trong phạm vi Live Window.

Đây là cách `changed_on_disk` của file local làm (`index/file.rs:211`), áp lên SFTP. Một lần mở lại chỉ tốn hai lần đọc 4 KiB cộng một `stat`.

*Thay thế đã cân nhắc:*
- Hash nhiều khối (`TPT_OFFER_BLOCK` của lnav): cần tailer trên Server.
- Chỉ so kích thước: không thấy file bị ghi đè có cùng hoặc lớn hơn kích thước.

### 6. Remote Command

Mỗi lần chạy là một Generation, ghi vào `cmd-<hash>/gen-<n>/`. `stream` (stdout hoặc stderr) của mỗi dòng không còn nằm trên `LogEntry` trong RAM. Nó được lưu thành bitmap song song với segment (`seg-<k>.stream`, 1 bit mỗi dòng), rồi đưa vào `LineMeta` khi index.

Mở lại thì output của lần chạy trước hiện ngay từ cache, dưới một Break "previous run HH:MM". Sau đó lệnh chạy theo luật hiện có:
- Resume By Time nếu là Command Preset chưa sửa;
- không thì Restart.

Output mới là Generation tiếp theo. Clear bỏ các segment của lệnh; số dòng vẫn đếm tiếp như bây giờ.

Điều chỉnh khi làm (task 2.2, 6.1):
- Cache của một Remote Command được đặt theo file id (`commands/cmd-<fnv của file id>/`), không theo câu lệnh hay Server. Edit… đổi câu lệnh hoặc Server nhưng giữ file id, và output cũ phải ở lại trên view như đường RAM.
- `seg-<k>.stream` dùng 1 byte mỗi dòng thay vì 1 bit, để ghi nối và cắt theo dòng giống `.lens`.
- Dòng bị bỏ khi Paused (Break Skipped) không có trong cache; các dòng sau đó là một "part" của cùng Generation, đánh số tiếp sau khoảng trống.
- Khi mở lại, source chạy lệnh theo kế hoạch `Reopened`: Command Preset chưa sửa thì resume từ thời gian dòng cuối mà cache giữ; nếu không thì chạy như đã gõ. Break là `Restarted(Reopened(..))`, mở Generation mới.

### 7. Quy tắc dọn cache (đã duyệt trong proposal)

- **Xoá ngay:** khi file rời Log Space, khi Log Space bị xoá, khi Edit… làm file thành file khác, hoặc khi Server bị xoá.
- **Lúc khởi động:** sweep các thư mục không có `LogFile` nào trỏ tới, như `copy::sweep`. Sau đó bỏ cache có `last_opened` cũ hơn 14 ngày.
- **Giới hạn chung:** mặc định 2 GB, chỉnh trong Settings ở mục Live. Khi tổng vượt giới hạn (kiểm sau mỗi lần ghi, tối đa một lần mỗi 10 s), app bỏ cache có `last_opened` cũ nhất trước. Cache của file thuộc tab đang mở không bao giờ bị bỏ; nếu chỉ còn những cache đó, app chỉ báo một notice.
- Xoá cả thư mục `remote-cache/` luôn an toàn. Cache hỏng hoặc thiếu `meta.json` được xử lý như chưa có cache: tải lại từ Server.

### 8. Live Window

- **Mặc định:** tăng từ 100.000 lên 1.000.000 dòng, vì text nằm trên đĩa. Với khoảng 125 byte/dòng, một Remote File đầy cache tốn khoảng 125 MB đĩa và khoảng 30 MB RAM cho Line Index.
- **Giới hạn khi người dùng chỉnh:** vẫn tới `FULL_READ_LIMIT` (2 triệu dòng).
- **Session cũ:** nếu người dùng đã chọn một số Live Window khác 100.000, số đó được giữ.

### 9. Remote Copy đọc qua Line Index

`ParsedFile::from_remote_copy` đổi thành `IndexedFile` trên `ChainSource` gồm các file `gen-*.log`, đánh số bằng `first_line_no` của từng Generation (decision 3). Không còn đường nào đọc cả file vào `entries`. Nhờ vậy task 12.1 của `line-offset-index` làm được trọn.

### 10. Cache chỉ sống trong phiên app; không vượt Live Window (chốt 2026-10-31)

Mục tiêu của cache là giảm memory, không phải giữ log lâu dài. Các quyết định dưới đây thay các phần liên quan của decision 5, 7 và 8 và mục rủi ro "Đĩa đầy".

- **Vòng đời cache:**
  - Chuyển sang tab khác (Dormant Tab): cache giữ trên đĩa, RAM của file được trả lại. Quay lại tab thì dòng hiện ngay và chỉ đọc phần mới (Checkpoint và Resume vẫn dùng trong phiên).
  - Cache của một file bị xoá khi không còn tab đang mở nào chứa file đó (đóng tab, kể cả tab của Log Space hay File View).
  - Tắt app: xoá mọi cache. Mở app: xoá mọi thư mục còn sót trong `remote-cache/` (app tắt đột ngột). Không có "mở lại app thấy dòng cũ".
  - Bỏ `last_opened` 14 ngày, giới hạn 2 GB, setting và nút "Clear Remote cache", việc kiểm dung lượng mỗi 10 s.
- **Remote Command:** quay lại tab trong cùng phiên vẫn thấy output lần chạy trước dưới Break "previous run", lệnh chạy lại bên dưới (decision 6). Đóng tab hoặc tắt app thì mất.
- **Live Window:** giữ mặc định 1.000.000 dòng (decision 8, task 9.1).
- **Không vượt Live Window:** bỏ Load more (Remote và file local), chế độ giữ thêm (`HoldMode::Extended`), Back to Live Window, setting "Load more step", và "Load earlier generation". Break ở đầu khi còn Rotated File chỉ gợi ý Open in full. Muốn xem xa hơn Live Window thì dùng Open in full (Remote Copy, vẫn lưu bền như cũ).
- **Ghi cache lỗi (đĩa đầy):** báo một notice và ngừng đọc file đó; các dòng đã có vẫn xem được. Không quay về giữ dòng trong RAM. Thay task 4.7.
- **Kết nối SSH:** chuyển tab thì giữ kết nối như hiện tại. Khi không còn tab đang mở nào có file (Remote File, Remote Command, Live Glob) của một Server, app đóng kết nối tới Server đó.
- **Remote Copy:** không đổi, lưu tới khi file hoặc Log Space của nó bị xoá.

## Risks / Trade-offs

- **[Rủi ro] Đĩa đầy hoặc ghi lỗi.** → Báo một notice và ngừng đọc file đó; dòng đã có vẫn xem được (decision 10).
- **[Rủi ro] Index lại khi chèn lên trên hoặc cắt làm view nháy, hoặc chậm với 2 triệu dòng.** → Index lại ở nền, view giữ rows cũ cho tới khi xong (như 6.3). Đo bằng `perf_probe`; nếu trên 100 ms với Live Window mặc định thì làm bản cắt tại chỗ (decision 3).
- **[Rủi ro] Hash khớp nhưng file đã khác ở giữa** (bị sửa ở giữa, không đụng đầu và cuối). → Chấp nhận, như file local. Log thường chỉ được ghi thêm.
- **[Rủi ro] Hai cửa sổ app, hoặc hai Log Space, dùng chung một Remote File.** → Một `RemoteCache` cho mỗi `file_id` trong tiến trình. Hai tiến trình app: cache có lock file `lock` (pid). Tiến trình thứ hai không dùng cache và chạy kiểu RAM như hiện tại.
- **[Đánh đổi] Cache chứa log của Server trên máy người dùng**, có thể nhạy cảm. → Thư mục nằm trong `<app data>`, quyền 0700, và chỉ sống trong phiên app (decision 10).
- **[Rủi ro] Server không có `stat` qua exec, nên không biết inode.** → So kích thước cộng hai hash. Rotate có thể hiện thành "truncated", như hiện nay (`remote.rs:401`).

## Migration Plan

1. `RemoteCache` và Checkpoint được thêm vào cạnh `LiveBuffer`, sau một cờ (`remote_cache_enabled`, mặc định tắt trong test cũ). Cờ là field của app, như `index_local_files`.
2. Chuyển từng event sang ghi cache, có test so với đường RAM (giống `trimming_one_file_equals_building_the_view_again`).
3. Bật mặc định. Sau đó xoá đường RAM của Remote, `ParsedFile::entries` và `ParsedFile::lines` (12.1 của `line-offset-index`).
4. Rollback: tắt cờ là quay về đường RAM, cho tới bước 3. Thư mục `remote-cache/` không ảnh hưởng gì nếu không dùng.

## Open Questions

- Khi Pause, có nên dừng ghi cache không? Hiện tại source dừng đọc khi Pause, nên không có gì để ghi. Giữ như vậy.
- Remote Command dùng `--since` (Resume By Time) khi mở lại app sau nhiều giờ: có giới hạn bao lâu thì Restart thay vì Resume không? Đề xuất: dùng luật hiện có của `ResumePoint`, không thêm giới hạn.
