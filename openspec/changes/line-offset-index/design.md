## Context

### Cách đọc file hiện tại

**Mở một file local:**
- `ensure_live_file_loaded` (`src/app/tabs.rs:181`) gọi `std::fs::read` cho cả file.
- `ParsedFile::from_bytes` (`src/space_view.rs:41`) parse mọi dòng thành `LogEntry`, chạy một luồng (`parse_entries`, `src/parser/text.rs:81`).
- Toàn bộ việc này chạy trên UI thread, bên trong `restore_active_session`.

**Dựng view:**
- `build_view_for` (`src/app/tabs.rs:308`) gọi `ensure_space_loaded`, nên đọc **mọi file của Log Space**, kể cả khi tab chỉ là File View.
- `build_file_view` clone `parsed.entries` (`src/space_view.rs:237`). `build_space_view` tạo mảng entry mới bằng `timeline::materialize` (`src/space_view.rs:214`).
- Sau đó `FilterIndex::build` chạy trên mọi entry.
- Toàn bộ việc này lặp lại **mỗi lần chuyển tab**.

**Live:**
- `LiveBuffer` (`src/live/buffer.rs:128`) giữ dòng của file live và cắt theo Live Window.
- `LocalFileSource` tail file local từ `local_offset`.

**Sau khi dựng xong, mọi tính năng duyệt trên `all_entries: Vec<LogEntry>`:**
- `FilterIndex` (`src/filter_index.rs:324`)
- `highlight_paints` (`src/app/highlights.rs:22`)
- Search
- Minimap
- Raw View, qua `ParsedFile::lines`

### Số đo (release, `examples/mem_probe.rs` và `examples/perf_probe.rs`)

| | 376MB / 3 triệu dòng | 1 triệu dòng |
|---|---|---|
| Parse | 7,5 s | 2,1 s |
| RAM sau parse | +1,4 GB (468 B/dòng) | |
| Bản copy của view | +1,1 GB (389 B/dòng) | |
| RSS tối đa | 3,1 GB | |
| `FilterIndex::build` | | 223 ms |
| Tick một Filter Value | | 4–7 ms |
| Search (UI / nền) | | 2 ms / 38 ms |

`size_of::<LogEntry>()` = 240 byte, chưa tính heap. Text chỉ khoảng 125 byte/dòng, nên phần lớn memory là struct và các vùng cấp phát kèm theo.

### Những phần đã hợp với cách mới

- `ColumnIndex::row_ids: Vec<u16>` và `FilterIndex::effective_ts: Vec<f64>` đã là dạng "id giá trị mỗi dòng" kiểu lnav. Tick Filter Value chỉ duyệt mảng này.
- `StoredMarker` (`src/workspace/log_space.rs:21`) lưu theo `(line, generations_ago, line_hash)`, tách khỏi `LogEntry`.
- `par.rs` đã có khung quét song song theo dòng.

### Tham khảo (đã đọc mã nguồn)

- **klogg**:
  - Lưu vị trí dòng nén thành khối 128 dòng: một vị trí tuyệt đối cộng các delta mã hoá varint SIMD.
  - Đọc lại dòng bằng `read`, nhờ page cache.
  - Index trong pipeline IO 5 MiB có giới hạn prefetch.
  - Search song song theo khối 10.000 dòng; kết quả là Roaring bitmap số dòng.
  - Giải nén archive ra file tạm.
- **lnav**:
  - `logline` 24 byte mỗi dòng: thời gian µs, offset, level 4 bit, cờ continuation, bloom bits.
  - Gộp nhiều file bằng k-way merge tăng dần trên `(file, line)` đóng gói.
  - Filter là bitmask `u32` mỗi dòng; danh sách dòng hiện là `Vec<u32>`.
  - Indexing có giới hạn thời gian mỗi lượt.
- **lnav** (đối chiếu thêm qua DeepWiki, https://deepwiki.com/tstack/lnav):
  - Khi một Log Format đã khớp, `log_format::scan` khoá vào nó (`lf_pattern_locks`), và `date_time_scanner` được chuyên biệt hoá cho định dạng thời gian của file. Các dòng sau thử mẫu đã khoá trước.
  - Giá trị field không được dựng sẵn cho mọi dòng; chúng được trích khi cần (`log_vtab_impl::extract`).
  - Stdin và output của lệnh (`piper::looper`) được ghi ra file trong workdir, rồi index như file thường.
  - Remote File được mirror vào `workdir/remotes`. Kết nối lại chỉ gửi khối mới nhờ `TPT_OFFER_BLOCK` kèm SHA-256.
  - `file_collection` tôn trọng `RLIMIT_NOFILE` để không cạn file descriptor.
- **less**: không có chỉ mục đầy đủ, chỉ cache thưa các cặp `(line, pos)`.
- **VS Code, Zed**: giữ cả file trong RAM, không hợp với log nhiều GB.

## Goals / Non-Goals

**Goals:**
- Mở file không bao giờ chặn UI. Các dòng đầu hiện trong vòng 300 ms.
- RAM tỷ lệ với số dòng ở mức không quá khoảng 50 byte mỗi dòng, không tỷ lệ với kích thước text.
- Chuyển tab đã được đọc trong dưới 50 ms, không clone dòng.
- Mọi thao tác cần đọc text chạy như một Scan ở nền, có tiến độ, kết quả hiện dần, và huỷ được.
- Một đường đọc dòng cho mọi nguồn: file local, Archive Entry, Remote Copy, Scratch, và Live Window của Remote File.
- Giữ nguyên hành vi đã có spec của Filter, Search, Marker, Highlight Rule, Merged Timeline và Raw View. Chỉ khác thời điểm kết quả xuất hiện.

**Non-Goals:**
- Mirror Remote File về cache local rồi index theo kiểu lnav. Remote vẫn dùng Live Window; Line Index của nó nằm trên bộ đệm trong memory (quyết định 2).
- Đọc ngẫu nhiên trong `.gz` bằng điểm đồng bộ kiểu zran.
- Lưu Line Index xuống đĩa để mở lại Project không phải index lại.
- Regex, hoặc đổi cú pháp Search Expression.
- Sửa được file Imported.

## Decisions

### 1. Line Index: vị trí dòng nén theo khối 128 dòng

Mỗi file có một `LineIndex`:
- Chia thành khối 128 dòng. Mỗi khối có vị trí đầu tuyệt đối (`u64`), cộng độ dài từng dòng mã hoá varint (hoặc stream-vbyte).
- Tra dòng `n` bằng cách giải mã một khối, nên thời gian không đổi theo kích thước file.
- Thêm dòng ở cuối (live tail) chỉ cần nối vào khối cuối.

*Thay thế đã cân nhắc:*
- `Vec<u64>` mỗi dòng: đơn giản, 8 byte/dòng. Giữ làm phương án dự phòng nếu spike cho thấy nén không đáng.
- Chỉ mục thưa kiểu less (một điểm mỗi N dòng): bị loại. Merged Timeline và Filter cần tới dòng thứ `n` bất kỳ trong O(1).
- `mmap`: bị loại. File live có thể bị truncate trong lúc map, gây SIGBUS; Windows giữ khoá file. Đọc bằng `FileExt::read_at` / `seek_read` và để page cache lo phần cache.

### 2. Nguồn dòng chung: `LineSource`

Mọi nguồn cung cấp hai thao tác: `read_at(offset, len)` và `len()`.

| Nguồn | Hiện thực |
|---|---|
| File local, Remote Copy | file trên đĩa |
| Archive Entry | file tạm đã giải nén (quyết định 9) |
| Scratch | bytes của văn bản trong editor (`raw.texts`) |
| Remote File, Remote Command | bytes của Live Window trong memory |

Line Index, Scan, Raw View và vẽ dòng chỉ biết `LineSource`. Đây là "một đường cho mọi cỡ file và mọi nguồn". Remote vẫn tốn memory cho text của Live Window, nhưng không còn tốn thêm `LogEntry` cho mỗi dòng.

**Hướng đích (change sau):** mọi nguồn đều là file trên đĩa, như lnav làm với `piper` và `tailer`.
- Remote File và Remote Command ghi dòng nhận được vào một file cache trong `<app data>/remote-cache/`, rồi được index như file local.
- Live Window của Remote khi đó không còn là giới hạn memory.
- Một Dormant Tab Remote quay lại được ngay từ cache, rồi tail tiếp từ kích thước đã có.

Vì thế `LineSource` trong memory chỉ là bước tạm cho Remote. API của `LineSource` (`read_at`, `len`, thêm vào cuối) SHALL NOT giả định nguồn nằm trong memory, để change sau chỉ phải thay hiện thực.

*Thay thế đã cân nhắc:*
- Giữ đường `LogEntry` cũ cho Scratch và Remote: bị loại. Người dùng chọn một đường duy nhất.
- Mirror Remote về đĩa ngay trong change này: để sau (Non-Goals), vì phải đổi cả `RemoteFileSource`.

### 3. Metadata cố định mỗi dòng: `LineMeta`

Mỗi dòng giữ khoảng 16–24 byte:
- **effective timestamp**: µs, dạng `i64`; dòng không có timestamp riêng thì lấy của dòng trên.
- **`has_time`**
- **level**: 4 bit
- **khoảng cách tới Parent Line**: `u32`, 0 nếu không phải Continuation Line
- **cờ**: không UTF-8, có ANSI, …

Không lưu theo từng dòng:
- **Generation**: lưu thành danh sách khoảng dòng của mỗi Generation, vì nó chỉ đổi ở Break.
- **Stream** của Remote Command: lưu cùng cách, theo khoảng dòng.

Phần còn lại của `LogEntry` gồm spans, `extra`, `request_id`, `trace_id` và `markers`. Phần này chỉ được tạo khi dòng được parse lại để hiện (quyết định 5).

*Thay thế đã cân nhắc:*
- Thu nhỏ `LogEntry` và vẫn giữ cho mọi dòng: bị loại. Dù gọn hết mức, nó vẫn ≥ 100 byte/dòng cộng text.

### 4. Cột lọc được: giữ `ColumnIndex::row_ids`, dựng trong lúc Indexing

`row_ids: Vec<u16>` mỗi cột, danh sách giá trị, số đếm, `MAX_DISTINCT = 500` và Coverage giữ nguyên ý nghĩa. Thứ thay đổi là thời điểm và cách dựng:
- Dựng dần theo từng khối trong lúc Indexing, nên picker thấy giá trị và số đếm tăng dần.
- Bật một Detected Field mới, hoặc đổi Log Format, chạy một Scan để dựng `row_ids` cho cột đó. Đổi Log Format thì index lại `LineMeta`, vì timestamp và level có thể đổi theo.

Tick Filter Value và Time Range chỉ duyệt `row_ids` và `LineMeta`, nên vẫn tức thì.

**Dựng lúc Indexing hay khi cần.** lnav không dựng sẵn giá trị field cho mọi dòng; nó trích khi cần. App cần số đếm của Filter Value nên vẫn phải có `row_ids`, nhưng có hai cách:
- **(a) Dựng lúc Indexing** cho mọi cột phát hiện được. Picker có số ngay, nhưng Indexing chậm hơn.
- **(b) Chỉ dựng lúc Indexing cho các cột có sẵn trong mọi dòng** (level và các cột của Log Format). Detected Field (`key=value`, trường JSON) chỉ được dựng bằng một Scan khi người dùng mở picker hay Highlight Rule của cột đó lần đầu, hoặc khi cột đó có điều kiện trong Filter đã lưu.

Spike đo cả hai (task 1.2). Mặc định chọn (a) nếu Indexing vẫn đạt mục tiêu ≤ 2 s; nếu không, chọn (b).

### 5. Text theo nhu cầu: đọc theo khối, LRU các dòng đã parse

Khi vẽ, canvas xin các dòng đang hiện. Hệ thống:
1. Gom các dòng gần nhau thành một lần `read_at` duy nhất.
2. Chuyển bytes sang text (lossy như hiện tại).
3. Parse lại thành `LogEntry` bằng `LineParser` hiện có, gắn Marker và paint của Highlight Rule.
4. Giữ kết quả trong một LRU khoảng 4.096 dòng, khoá theo `(file, generation, line)`.

Trong lúc chờ đọc, dòng chưa có thì vẽ placeholder, rồi `cx.notify()` khi đọc xong. Đọc từ page cache thường chỉ dưới 1 ms cho một màn hình.

`ParsedFile::lines: Vec<SharedString>` của Raw View bị bỏ. Raw View đọc qua cùng LRU này.

### 6. Dòng của view: chỉ số, không phải bản copy

- **File View**: dòng của view chính là `0..line_count`; không cần mảng.
- **Log Space**: thứ tự gộp là `Vec<u64>`, mỗi phần tử đóng gói `(file_idx: 16 bit, line: 48 bit)`. Nó được dựng bằng k-way merge tăng dần theo `effective timestamp` trên con trỏ "đã gộp tới đâu" của mỗi file (cách của lnav). Dòng mới, hay khối vừa index, chỉ gộp phần mới.
  - Một file có timestamp không tăng dần bên trong vẫn dùng thứ tự gộp ổn định như `timeline.rs` hiện tại. Spike sẽ kiểm tra chi phí.
- **`visible_indices`**: kết quả của Filter, vẫn là `Vec<u32>`/`Vec<usize>` trên chỉ số của view.
- Chuyển tab: Line Index và thứ tự gộp nằm trong cache theo file và theo Log Space, nên chỉ cần trỏ view vào đó. Không clone và không dựng lại `FilterIndex`.

*Thay thế đã cân nhắc:*
- Giữ `all_entries` nhưng dùng `Arc` chung: bị loại. Nó vẫn là `LogEntry` cho mọi dòng.

### 7. Indexing: pipeline ở nền, công bố theo lô

1. Một luồng IO đọc khối khoảng 4 MiB, giới hạn khoảng 4 khối đọc trước.
2. Các worker tìm `\n` bằng `memchr` và parse các trường cần cho `LineMeta` cùng `row_ids`, song song theo khối.
3. Một bước nối tuần tự nối các khối theo thứ tự. Bước này giải quyết Continuation Line vắt qua ranh giới khối và timestamp kế thừa.
4. Cứ mỗi khoảng 50 ms, hoặc mỗi lô, hệ thống gửi phần mới sang UI: dòng mới, giá trị cột mới, tiến độ tính bằng byte đã đọc trên tổng số byte.
5. Mỗi lần Indexing có một token huỷ (theo thế hệ). Đóng tab, chuyển tab trước khi xong, hay đổi Log Format đều huỷ lần đang chạy.

**Khoá format theo file (như `lf_pattern_locks` của lnav).** Khi một mẫu timestamp, hoặc một Log Format đang ở Auto-detect, đã khớp đủ số dòng liên tiếp (ví dụ 64), worker khoá vào mẫu đó cho file và thử nó trước ở mọi dòng sau. Chỉ khi mẫu đã khoá không khớp thì mới quay lại dò toàn bộ. Khoá được chia sẻ cho các khối đang song song sau khi khối đầu đã xác định. Khoá không đổi kết quả parse, chỉ đổi thứ tự thử, nên test so `LineMeta` với `parse_entries` vẫn phải khớp từng dòng. Spike đo Indexing có khoá và không khoá.

Lô đầu được ưu tiên nhỏ để màn hình đầu hiện trong vòng 300 ms. Với file live, view mở ở **cuối file** (L3), nên Indexing đọc xuôi từ đầu nhưng canvas chỉ chờ tới khi lô cuối tới. Để có màn hình đầu ngay, hệ thống đồng thời đọc ngược vài khối cuối và hiện chúng tạm, rồi thay bằng dòng đã index khi Indexing tới nơi. Spike sẽ đo xem bước này có cần không.

### 8. Scan: quét song song có tiến độ, kết quả là tập dòng

Scan dùng cho Search, Filter Custom Value, Highlight Rule có Custom Value, Detected Field mới và đổi Log Format.

- Chia các dòng cần quét thành khối khoảng 10.000 dòng. Mỗi khối: `read_at` một lần, rồi đánh giá trên text bằng mã so khớp hiện có (`search_expr`, so khớp Unicode dùng chung).
- Khối gần vị trí đang xem được quét trước. Nhờ vậy Match ở màn hình hiện tại xuất hiện sớm.
- Kết quả là tập số dòng của view, đã sắp xếp (`Vec<u32>`, hoặc Roaring bitmap nếu spike cho thấy cần), gửi dần sang UI cùng tiến độ và tổng tạm thời.
- Gõ thêm một ký tự thì huỷ Scan cũ.
- Kết quả của truy vấn gần nhất được giữ cho tới khi dòng đổi. Live append chỉ quét phần mới.

Search trong Parsed View vẫn chỉ đếm trên dòng Filter đang hiện; Raw View vẫn đếm mọi dòng (giữ nguyên spec `log-search`).

### 9. Archive Entry: giải nén ra file tạm

Archive Entry được giải nén một lần, theo luồng, vào `<app data>/archive-cache/<hash của archive + entry>/`. Sau đó nó được index như file local.
- Các entry của cùng một Archive vẫn giải nén trong một lượt.
- Giới hạn `MAX_UNCOMPRESSED_BYTES` giữ nguyên.
- Thư mục được dọn khi Log Space bị xoá, và được quét dọn khi app khởi động, giống `remote::copy::sweep`.

*Thay thế đã cân nhắc:*
- Index thẳng trên `.gz` bằng điểm đồng bộ kiểu zran (lnav): để sau. Phức tạp hơn, và chỉ có ích cho gzip.

### 10. Highlight Rule

- **Rule theo Filter Value**: so khớp trên `row_ids`. Màu của mỗi dòng lưu thành 1 byte (số thứ tự rule, `u8`), tính song song cho cả view, thay cho `Vec<Option<RowPaint>>`. Minimap đọc mảng này.
- **Rule có Custom Value**: cần text. Dòng đang hiện được tính lúc parse cho LRU; phần còn lại của view được lấp bằng một Scan ở nền.

### 11. Marker

`StoredMarker` giữ nguyên.
- Mỗi file có một bảng tra đã sắp xếp `(generation, line) → markers`. Lúc vẽ, chỉ tra các dòng đang hiện.
- `line_hash` được kiểm khi dòng được đọc ra. Marker lệch hash bị bỏ, như `prune_markers` hiện làm.
- Nhảy tới Marker kế tiếp dùng bảng đã sắp xếp.

### 12. Live tail của file local

- `LocalFileSource` tiếp tục đọc phần thêm vào. Bytes mới được index như một lô nhỏ, rồi thêm vào Line Index, `row_ids` và thứ tự gộp.
- **Phát hiện rotate và truncate**: so kích thước file với vị trí đã index, cộng hash 4 KiB đầu và 4 KiB cuối đã index (cách của klogg). Lệch thì là Generation mới, kèm Break.
- **Text của Generation cũ**: hệ thống giữ file handle đã mở cho mỗi Generation. Trên Unix, handle vẫn đọc được file đã đổi tên hoặc bị xoá. Với `copytruncate` thì text cũ không còn trên đĩa (Risks).
- Live Window của file local không cần nữa, vì text nằm trên đĩa. Nó vẫn áp cho Remote (quyết định 2).

### 13. Dormant Tab

- Khi mở app, chỉ tab đang chọn được đọc. `needed_files` và `sync_glob_watchers` chỉ tính tab đang mở (active), không tính mọi tab của tab strip (`src/app/live.rs:224`, `src/app/globs.rs:98`).
- File View chỉ đọc file của nó, không gọi `ensure_space_loaded` cho cả Log Space.
- Rời một tab: hệ thống bỏ Line Index và LRU của các file mà không tab đang mở nào dùng, và đóng source live của chúng.
  - Scratch được lưu (`flush_scratch_saves`) trước khi bỏ.
  - Session SSH của Server trong `RemoteHub` được giữ. Hub vốn chỉ bỏ session khi lỗi hoặc khi sửa Server.
- Quay lại tab thì index lại từ nguồn, với tiến độ. Filter, Focus Tab, vị trí cuộn và Marker đã nằm trong `TabSessionState` và `LogSpace`.
- Tab strip hiện trạng thái Indexing (tiến độ) và Dormant.

*Thay thế đã cân nhắc:*
- Giữ các tab gần đây theo LRU với memory budget: để sau. Khi Line Index chỉ còn khoảng 50 byte/dòng, có thể giữ nhiều tab hơn mà vẫn rẻ. Change này chỉ làm "rời tab thì bỏ", rồi đo lại.

### 14. Spike trước, có cổng go/no-go

Trước mọi thay đổi trong app, một spike riêng (`examples/index_probe.rs`) đo trên file 376MB / 3 triệu dòng (cùng định dạng như số đo ở trên) và trên một log thật của người dùng:

| Chỉ số | Mục tiêu |
|---|---|
| Thời gian Indexing (8 lõi, page cache nóng) | ≤ 2 s |
| Lô đầu tiên (1.000 dòng) | ≤ 300 ms |
| Line Index + `LineMeta` + `row_ids` | ≤ 50 byte/dòng (≤ 150 MB RSS tăng thêm) |
| Một màn hình 60 dòng (`read_at` + parse) | ≤ 2 ms |
| Scan Search trên mọi dòng (page cache nóng) | ≤ 1 s |
| Scan Search (page cache lạnh, `purge`) | ghi lại, không chặn |
| k-way merge của 3 file × 1 triệu dòng | ≤ 300 ms |

Spike cũng so sánh:
- varint với `Vec<u64>`;
- `Vec<u32>` với Roaring cho kết quả Scan;
- Indexing có khoá format và không khoá (quyết định 7);
- `row_ids` dựng lúc Indexing cho mọi cột (a) với chỉ cho cột có sẵn trong mọi dòng (b), kèm thời gian Scan dựng một Detected Field khi cần (quyết định 4).

- **Go**: đạt mọi mục tiêu có ký hiệu ≤.
- **No-go**: ghi số vào design này và quay lại bàn. Ví dụ nếu bước parse timestamp quá chậm, cân nhắc chỉ parse timestamp và level lúc Indexing, còn cột lọc thì dựng bằng Scan khi mở picker lần đầu.

#### Kết quả spike (2026-10-25)

Đo bằng `examples/index_probe.rs`, bản release, máy 12 lõi, page cache nóng. File đo là file 376MB / 3.000.000 dòng của số đo nền, sinh bằng `index_probe --gen`. Đo lại `mem_probe` trên cùng file và cùng máy: parse 7,4 s, +1.310 MB (437 B/dòng), RSS tối đa 1,84 GB.

| Chỉ số | Mục tiêu | Đo được | |
|---|---|---|---|
| Indexing, có khoá format | ≤ 2 s | 0,11 s | đạt |
| Indexing, không khoá (parser của app cho mọi dòng) | ≤ 2 s | 1,84 s (b), 2,01 s (a) | sát ngưỡng |
| Lô đầu (1.000 dòng) | ≤ 300 ms | 1 ms có khoá, 13–17 ms không khoá | đạt |
| Line Index + `LineMeta` + `row_ids` | ≤ 50 B/dòng | 21,3 B/dòng (b), 23,3 B/dòng (a); RSS +41–59 MB | đạt |
| Một màn hình 60 dòng (`read_at` + parse) | ≤ 2 ms | p50 0,15 ms, p99 0,17 ms (đầu, giữa, cuối, nhảy ngẫu nhiên) | đạt |
| Scan Search `timeout`, `user=42` (nóng) | ≤ 1 s | 49 ms, 47 ms | đạt |
| Scan Search (lạnh, sau `sudo purge`, SSD trong máy) | ghi lại | 73 ms cho cả hai truy vấn | đạt |
| k-way merge 3 file × 1 triệu dòng | ≤ 300 ms | 23 ms; thêm 1.000 dòng mỗi file 21 µs | đạt |

Phần kiểm đúng:
- Có khoá và không khoá cho `LineMeta` và `row_ids` giống nhau trên mọi dòng.
- 300.000 dòng đầu khớp `parse_text_bytes_with_config` cộng `value_of` (0 sai lệch).
- Thứ tự gộp trùng với `timeline::merge` trên cùng dữ liệu, kể cả Continuation Line. Gộp tăng dần cho cùng kết quả với gộp một lần.

Những so sánh:
- **Mã hoá Line Index**: khối 128 dòng + varint tốn 1,27 B/dòng, `Vec<u64>` tốn 8 B/dòng. Chi phí giải mã đã nằm trong 0,15 ms của một màn hình.
- **Tập kết quả**: với 33.781 Match, `Vec<u32>` là 131 KB và Roaring là 66 KB, dựng Roaring mất 0,14 ms. Roaring càng có lợi khi Match dày.
- **Khoá format**: nhanh hơn khoảng 17 lần.
- **(a) và (b)**: chênh 2 B/dòng và khoảng 0,2 s khi không khoá. Một Scan dựng Detected Field `user` theo kiểu (b) mất 42 ms.

Cần đọc kỹ con số khoá. Phần "có khoá" của spike là một bộ đọc viết tay cho dạng `YYYY-MM-DD HH:MM:SS.f [LEVEL] [tag] message`. Nó không cấp phát và không dựng `LogEvent` hay `LogEntry`. Phần lớn mức tăng 17 lần đến từ đó chứ không chỉ từ việc bỏ thử các mẫu khác. Một file mà không bộ đọc nhanh nào khớp sẽ đi đường không khoá, khoảng 1,9 s trên 3 triệu dòng.

Ngoài ra, file đo khớp bộ đọc nhanh ở 100% số dòng, và ba Detected Field của nó (`user`, `trace_id`, `path`) đều vượt `MAX_DISTINCT`, nên (a) ở đây rẻ hơn với log có nhiều field ít giá trị.

**Chốt:**
- Line Index dùng khối 128 dòng + varint.
- Tập kết quả của Scan dùng Roaring bitmap (`roaring`).
- Dùng khoá format, kèm bộ đọc nhanh không cấp phát cho các dạng phổ biến (task 3.1b). Dạng nào chưa có bộ đọc nhanh thì đi parser hiện tại.
- `row_ids` theo (a), vì Indexing vẫn đạt ≤ 2 s cả khi không khoá. Nếu log thật (task 1.6) cho thấy (a) chậm thì chuyển sang (b); Scan của (b) chỉ mất vài chục ms.

**Kết luận: go.** Mọi mục tiêu có ký hiệu ≤ đều đạt. Còn một việc chưa xong nhưng không chặn cổng: chạy trên log thật của người dùng (task 1.6). Lần chạy dưới sudo đo Indexing không khoá 1,28–1,40 s, nhanh hơn lần đầu.

## Risks / Trade-offs

- **Thay đổi lõi rất lớn**: `all_entries` được dùng ở rất nhiều nơi (Filter, Search, Focus Pane, Highlight, Minimap, Marker, Go to, Live trim, test). → Làm theo từng giai đoạn trong tasks.md, mỗi giai đoạn giữ test xanh. Dựng một lớp truy vấn (`ViewRows`) trước, chuyển dần các chỗ dùng sang đó, rồi mới bỏ `all_entries`.
- **Kết quả không còn tức thì với file nhỏ**: Search và Custom Value của một file 1.000 dòng cũng đi qua Scan. → Người dùng đã chấp nhận. Scan của file nhỏ thường xong trong lô đầu, nên trên thực tế không thấy khác.
- **`copytruncate` làm mất text của Generation cũ** khi file bị truncate tại chỗ. → Phát hiện truncate ngay lúc poll. Dòng của Generation cũ còn trong LRU vẫn hiện; dòng khác hiện "content no longer on disk" và không đọc được. Câu hỏi mở: có sao phần đuôi chưa đọc vào cache của app không.
- **File Imported bị sửa khi đang mở** (không phải append): offset trỏ sai dòng. → Kiểm tra kích thước và hash đoạn đầu mỗi lần đọc một khối. Lệch thì index lại và báo.
- **Ổ đĩa chậm hoặc ổ mạng**: `read_at` theo nhu cầu có thể chậm hơn RAM. → Đọc gom theo khối, đọc trước quanh vị trí cuộn, và vẽ placeholder thay vì chờ.
- **Continuation Line và timestamp kế thừa vắt qua ranh giới khối song song.** → Giải quyết ở bước nối tuần tự (quyết định 7), và có test riêng cho khối cắt giữa stack trace.
- **Text không phải UTF-8, CRLF, dòng rất dài.** → Giữ cách lossy và `lines()` như hiện tại. Với dòng dài, `read_at` đọc theo độ dài thật của dòng; có test với dòng 1 MB.
- **Field có quá nhiều giá trị khác nhau**: `MAX_DISTINCT` vẫn bỏ `row_ids` khi tràn, nên RAM không tăng.
- **Cạn file descriptor**: giữ handle cho mỗi Generation (quyết định 12), cộng nhiều file và nhiều tab, có thể chạm `RLIMIT_NOFILE` (macOS mặc định 256 cho một tiến trình mở từ Finder). → Mở handle theo nhu cầu cho file không live, và đóng sau khi đọc xong một lượt. Chỉ giữ handle mở cho file live và cho Generation cũ còn dòng trong view. Đóng mọi handle của file thuộc Dormant Tab. Đếm số handle đang mở, và báo người dùng trước khi chạm giới hạn, như `file_collection` của lnav.
- **Windows**: dùng `seek_read`. File đang mở có thể chặn việc rotate của ứng dụng khác. → Mở file với chia sẻ đọc, ghi và xoá (`FILE_SHARE_DELETE`), như `LocalFileSource` làm hiện tại.

## Migration Plan

- Không đổi format session. `StoredMarker`, `FilterState` và `TabSessionState` giữ nguyên.
- Phần trong app làm theo thứ tự trong tasks.md:
  1. spike
  2. `LineSource` và `LineIndex`
  3. lớp `ViewRows`
  4. Indexing ở nền
  5. Filter
  6. Scan
  7. Highlight và Marker
  8. Raw View
  9. live tail
  10. Archive
  11. Dormant Tab
  12. bỏ đường cũ
- Đường cũ (`ParsedFile::entries`, `all_entries`) chỉ tồn tại trong lúc chuyển đổi và bị xoá ở bước cuối, không giữ làm fallback.
- Rollback: mỗi giai đoạn là một commit riêng, nên có thể revert. Không có dữ liệu cần chuyển đổi.
- Thư mục `archive-cache/` có thể xoá bất kỳ lúc nào; app tự giải nén lại.

## Open Questions

- Với `copytruncate`, có nên sao phần đuôi của file local live vào cache của app để Generation cũ không mất không?
- Đổi Log Format có cho phép giữ view cũ cho tới khi index lại xong, hay xoá ngay và hiện tiến độ?
- Sau change này, có làm change "Remote mirror về cache local" (hướng đích của quyết định 2) để bỏ Live Window của Remote không? Nếu làm, cần quyết định:
  - Dung lượng tối đa của cache, và khi nào dọn (lnav dọn sau 2 ngày).
  - Có đồng bộ theo hash khối như `TPT_OFFER_BLOCK` của lnav không. Cách này giúp khi kết nối lại, hay khi mở lại Project, chỉ phải tải phần mới thay vì đọc lại từ đầu. Với SFTP hiện có, có thể đọc trước vài khối cuối của file trên Server để so hash với cache, mà không phải cài tailer lên Server.
- LRU dòng đã parse nên là 4.096 dòng hay tính theo byte?
- Có cần lưu Line Index xuống đĩa cho file rất lớn để mở lại Project không phải index lại không? Ngoài phạm vi; để quyết sau khi có số đo của spike.
