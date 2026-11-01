## Context

Sau `line-offset-index` và `remote-local-cache`, các file được đọc qua Line Index đều nằm trong `AppView::parsed_files: HashMap<file_id, ParsedFile>`. `ParsedFile` giờ chỉ bọc một `Arc<IndexedFile>`. Ba chỗ còn lại:

**Scratch** (`src/app/tabs.rs`, `src/app/raw.rs`):
- `ensure_scratch_loaded` gọi `read_source`, đưa cả file vào `BytesSource`, rồi gọi `IndexedFile::index_now` đồng bộ trên UI thread.
- Sửa trong Raw View: `on_scratch_edited` ghi text vào `raw.texts`, đánh dấu `raw.edited` và `raw.unsaved`, rồi hẹn lưu sau `SCRATCH_SAVE_DEBOUNCE` (500 ms).
- `reparse_edited_scratch_files` (gọi cuối `ensure_files_loaded`) index lại từ `raw.texts` qua `scratch_parsed` → `BytesSource`.
- Editor (`raw.editors`) được dựng từ `section.lines.all().join("\n")` và giữ cả text. `raw.lines_cache` giữ `Vec<SharedString>` tách từ `raw.texts`.
- `drop_dormant_files` chỉ bỏ `parsed_files`; `raw.editors`, `raw.texts` và `raw.lines_cache` chỉ bị bỏ khi file bị gỡ (`forget_file`).
- Quy tắc 26: chỉ một editor cho mỗi file, và chỉ khi file là file duy nhất được hiện.

**Rời tab** (`src/app/live.rs`):
- `sync_live_sources` dừng các source không còn cần, rồi `drop_dormant_files` bỏ mọi `parsed_files` không thuộc `active_file_ids()`, rồi `drop_dormant_loads`.
- Chọn lại tab thì `ensure_files_loaded` → `queue_file_load` index lại cả file. Số đo của task 12.2 trong `line-offset-index`: 0,69–0,72 s với 3 triệu dòng; dựng lại view từ Line Index có sẵn: 58 ms.
- `start_missing_sources` đã biết đi tiếp từ phần đã index của một file live ("A file read through its Line Index follows from what is indexed"). `IndexedFile::changed_on_disk` trả `Same`, `Appended`, `Replaced` hoặc `Gone`.

**Archive Entry** (`src/app/tabs.rs`, `src/archive_cache.rs`):
- `ensure_files_loaded` gọi `ensure_archive_entries_loaded` trước tiên. Hàm này gom entry theo Archive rồi gọi `archive_cache::unpack` ngay trong luồng gọi.
- `unpack` bỏ qua entry đã có file trong thư mục của Archive. Entry còn thiếu được ghi trong một lượt `visit_entries`, dưới tên `.part`, rồi đổi tên khi ghi xong.
- Sau đó mỗi entry được mở bằng `ParsedFile::from_path` → `IndexedFile::index_now`, cũng đồng bộ.
- `start_queued_loads` (đường nền của file local) lấy `file.path` làm file cần index. Với Archive Entry, `file.path` là path của **Archive**, không phải của entry đã giải nén.

**RowCache** (`src/index/view_rows.rs`, `src/index/file.rs`):
- Mỗi `IndexedFile` có `cache: Mutex<RowCache>` với `ROW_CACHE = 4096` entry, key `(0, generation, line)`.
- Đọc theo khối `READ_AROUND = 256` dòng.
- Bỏ entry theo LRU, một phần tư mỗi lần, đếm số entry chứ không đếm byte.

## Goals / Non-Goals

**Goals:**
- File scratch không bao giờ được đọc nguyên vào memory để index, và Indexing của nó không chặn UI (quy tắc 42, 43).
- Giải nén và index Archive Entry không chặn UI (quy tắc 42).
- Quay lại một tab vừa rời không phải index lại, trong một memory budget người dùng chỉnh được.
- Memory dành cho dòng đã parse không tăng theo số file đang mở.
- Hành vi với budget 0 giống hệt hiện tại, để có đường lùi.

**Non-Goals:**
- Warm Tab cho Remote File và Remote Command: chúng đã có Remote Cache, và đường đồng bộ khi mở lại phức tạp hơn nhiều.
- Phản ứng với memory pressure của hệ điều hành.
- Lưu Line Index xuống đĩa giữa các lần chạy.
- Thu nhỏ Line Index hay `FilterIndex`.
- Các mục ngoài phạm vi trong proposal (Archive Entry trong memory, segment của Remote Cache, `OtelInput`).

## Decisions

### 1. Scratch đi chung đường với file local

File scratch có `path` trong thư mục dữ liệu của app. `ensure_scratch_loaded` gọi `queue_file_load(file)` như file trong Static Log Space, để `DiskSource::on_demand` đọc nó qua Line Index ở nền. `scratch_parsed` và nhánh `BytesSource` bị bỏ. Log Format và Detected Field lấy từ `index_options_for`, như file local.

*Thay thế đã cân nhắc:* giữ `BytesSource` nhưng chuyển `index_now` ra nền. Cách này hết chặn UI nhưng vẫn giữ một bản text nữa. Bị loại, vì file đã nằm trên đĩa.

### 2. Sửa xong thì lưu trước, rồi index lại từ đĩa

`reparse_edited_scratch_files` đổi thành:
1. Gọi `flush_scratch_saves()` cho các file trong `raw.edited`.
2. Với mỗi file lưu được: bỏ `parsed_files[file_id]` và gọi `queue_file_load` để index lại từ đĩa ở nền.
3. File lưu thất bại: giữ `raw.edited` và `parsed_files` cũ. Notice đã có trong `flush_scratch_saves`.

Trong lúc index lại, Parsed View hiện tiến độ (quy tắc 42). File scratch thường nhỏ, nên lô đầu xong gần như ngay. Một file scratch lớn thì giờ được index ở nền thay vì làm đứng UI.

Một file vừa sửa phải index lại cả file, vì scratch sửa được ở bất cứ dòng nào: không có "chỉ ghi thêm". Chuyện này chỉ xảy ra khi rời Raw View, không xảy ra mỗi phím gõ.

*Thay thế đã cân nhắc:* index lại theo phần thay đổi (diff giữa text cũ và mới, chỉ index lại từ dòng đầu tiên khác). Nhanh hơn với file lớn, nhưng Line Index hiện chỉ hỗ trợ thêm vào cuối. Để sau, nếu đo thấy chậm.

### 3. Editor của Scratch theo vòng đời của tab

- Tab đang mở: editor giữ text như hiện tại. Đó là thứ người dùng đang gõ, nên là ngoại lệ hợp lệ của quy tắc 43.
- Warm Tab: giữ `raw.editors`, `raw.texts` và `raw.lines_cache` để con trỏ, vùng chọn và undo còn nguyên. Kích thước text được tính vào Warm Tab Budget.
- Dormant Tab: sau khi `flush_scratch_saves` lưu được, bỏ `raw.editors`, `raw.texts`, `raw.lines_cache`, `raw.edited` và `raw.save_gen` của file. Lưu thất bại thì giữ tất cả (file vẫn nằm trong `raw.unsaved`).

`lines_cache` hiện chỉ dùng khi có `raw.texts`. Nó được bỏ cùng `raw.texts`.

### 3b. Archive Entry: giải nén rồi index trong cùng một việc nền

- `ensure_archive_entries_loaded` không còn gọi `unpack` trực tiếp. Mỗi Archive (gom như hiện tại) thành một việc nền:
  1. `unpack(root, archive, kind, names, MAX_UNCOMPRESSED_BYTES)` chạy trên `background_executor`.
  2. Xong thì trả về UI thread map `entry → path đã giải nén`. Mỗi entry thành công được đưa vào hàng đợi index với **path đã giải nén** (xem dưới). Entry lỗi được ghi `ParsedFile::failed` kèm notice như hiện tại.
- Hàng đợi load cần biết path thật để index. `load_queue` đổi từ `Vec<LogFile>` thành `Vec<QueuedLoad { file: LogFile, read_path: PathBuf }>`. File local có `read_path = file.path`; Archive Entry có `read_path` là file trong archive cache. `start_queued_loads` index `read_path` thay vì `file.path`. Mọi thứ khác (tiến độ Indexing, `file_read`, `again`, huỷ) dùng chung.
- **Tiến độ:** `FileLoad.progress` thêm biến thể `Progress::Unpacking { read: Arc<AtomicU64>, total: u64 }`, với `total` là kích thước file Archive và `read` là số byte đã đọc của Archive. Tab hiện "Unpacking …" rồi chuyển sang tiến độ Indexing. Mọi entry của cùng Archive dùng chung một bộ đếm.
- **Huỷ:** `unpack` nhận thêm `cancel: &AtomicBool`, được kiểm tra trong `copy_bounded` sau mỗi khối. Huỷ thì trả `ArchiveError::Cancelled`, xoá `.part` (đường xoá khi lỗi đã có), và không ghi notice. `drop_dormant_loads` đặt `cancel` cho việc giải nén của file không còn được đọc, như với Indexing.
- **Gọi trùng:** trong lúc một Archive đang được giải nén, các entry của nó nằm trong `loads`, nên `ensure_files_loaded` gọi lại không tạo việc thứ hai (cùng kiểm tra `loads.contains_key` của `queue_file_load`).
- Entry đã có trong archive cache: `unpack` trả về ngay mà không đọc Archive, nên cùng đường trên vẫn đúng. Việc nền xong gần như tức thì, rồi entry vào hàng đợi index.
- `ParsedFile::from_path` không còn dùng cho Archive Entry.

*Thay thế đã cân nhắc:*
- Giải nén ở nền nhưng index đồng bộ khi xong: vẫn chặn UI với entry lớn. Bị loại.
- Index trong lúc đang giải nén (đọc file `.part` khi nó lớn dần): dòng hiện sớm hơn với entry rất lớn, nhưng phải xử lý file đang được ghi như một file live. Để sau, nếu người dùng hay mở Archive vài GB.
- Mỗi entry một việc giải nén: đơn giản hơn nhưng đọc lại Archive nhiều lần với `.tar.gz`. Bị loại; giữ "một lượt mỗi Archive" như hiện tại.

### 4. Warm Tab: giữ `parsed_files`, không giữ view

Warm được tính theo **file**, không theo tab. Một file có thể thuộc nhiều tab (Log Space và File View của nó), và `parsed_files` đã được khoá theo `file_id`.

- `AppView` thêm `warm: WarmFiles`, gồm `file_id → (left_at: u64, bytes: usize)`, xếp theo `left_at`. `left_at` là một bộ đếm tăng mỗi lần rời tab.
- `drop_dormant_files` đổi thành:
  1. Các file không thuộc `active_file_ids()` và không phải Remote File hay Remote Command (quyết định 6) được đưa vào `warm`, với `left_at` của lần rời này. File đã có trong `warm` được cập nhật `left_at`.
  2. Các file của tab đang mở được bỏ khỏi `warm`.
  3. Trong khi tổng `bytes` vượt budget: bỏ file có `left_at` nhỏ nhất. Bỏ khỏi `parsed_files`, `close_cached`, quên dòng trong Row Cache, bỏ editor theo quyết định 3.
  4. Gọi `release_freed_memory()` nếu đã bỏ file nào.
- Không giữ `ViewData`, `FilterIndex` hay kết quả Scan của tab. Chọn lại tab thì view được dựng lại từ Line Index (58 ms với 3 triệu dòng). Search và Custom Value chạy Scan lại như khi mở tab: chúng đã chạy ở nền theo quy tắc 42.
- `ensure_files_loaded` không đổi: file có trong `parsed_files` thì không index lại (`queue_file_load` đã có kiểm tra `contains_key`).

*Thay thế đã cân nhắc:*
- Giữ cả view (`ViewData`, `FilterIndex`, Match): chuyển tab nhanh hơn ~60 ms nhưng tốn thêm memory theo số dòng, và state của view phải được giữ đúng khi Filter hay Log Format đổi ở chỗ khác. Bị loại.
- Giới hạn theo số tab (giữ N tab gần nhất): đơn giản hơn nhưng không chặn được memory khi một tab có file rất lớn. Bị loại; budget theo byte trả lời đúng câu hỏi "tốn bao nhiêu memory".

### 5. Ước lượng memory của một file

Thêm `IndexedFile::heap_bytes() -> usize`: tổng phần heap của Line Index (offset nén), `MetaTable` hay `LineMeta`, `row_ids` và giá trị của các cột lọc được, Generation và Break. Phần này ước lượng theo `capacity` của các `Vec` và kích thước các block, không duyệt từng dòng. Với Scratch còn editor, cộng thêm độ dài text trong `raw.texts`, nhân 3 cho editor, `texts` và `lines_cache`.

Ước lượng được kiểm tra bằng `mem_probe`: chênh lệch so với RSS đo được phải dưới 20% trên file 3 triệu dòng. Không cần chính xác tuyệt đối: budget là để chặn trên, không phải kế toán.

*Thay thế đã cân nhắc:* đo RSS thật trước và sau khi bỏ một file. Không làm được: allocator không trả memory ngay, và các file dùng chung allocator.

### 6. Remote File và Remote Command luôn Dormant khi rời

Nhánh `sync_live_sources` cho source không còn cần đã gọi `close_cached` và `parsed_files.remove`. Remote File và Remote Command giữ nguyên đường này. Lý do:
- Mở lại từ Remote Cache đã nhanh: không cần tải lại từ Server.
- Giữ Line Index trên `ChainSource` của Remote Cache trong lúc source đóng phải xử lý thêm trường hợp cache bị cắt (Live Window) hay bị dọn (quy tắc dọn cache) khi tab đang Warm.

Có thể mở rộng sau khi task 9.4 của `remote-local-cache` có số đo.

### 7. Chọn lại Warm Tab: kiểm tra file trước khi dùng

Khi một file trong `warm` được dùng lại (tab của nó được chọn), hệ thống bỏ nó khỏi `warm` rồi:
- **Live Log Space**: `start_missing_sources` đi tiếp từ phần đã index như hiện tại. Đường đó đã nhận ra ghi thêm, rotate và truncate bằng fingerprint. Không thêm gì.
- **Static Log Space**: gọi `IndexedFile::changed_on_disk(path)`.
  - `Same`: dùng nguyên.
  - `Appended`, `Replaced`: xử lý như khi `watch_static_files` thấy file đổi trong lúc tab mở. Hiện tại đường đó index lại.
  - `Gone`: như file biến mất trong lúc tab mở.
- **Scratch**: file chỉ đổi qua app, nên dùng nguyên. Có `raw.edited` thì đi theo quyết định 2.
- **Archive Entry, Remote Copy**: file trong thư mục dữ liệu của app, không đổi. Dùng nguyên.

### 8. Một Row Cache chung, giới hạn theo byte

- Thêm `RowCache` toàn cục (`static ROWS: OnceLock<SharedRowCache>`), key là `(file_key: u64, generation: u32, line: usize)`. `file_key` là một id duy nhất cấp cho mỗi `IndexedFile` khi tạo, từ `AtomicU64`.
- Mỗi entry tính `bytes ≈ size_of::<LogEntry>() + độ dài dòng theo Line Index + 64` (phần heap của field và `Arc`). Giới hạn mặc định `ROW_CACHE_BYTES = 64 MB`. Vượt 125% thì bỏ entry dùng lâu nhất, còn 100%, như cách cắt hiện tại.
- Để giảm tranh chấp lock giữa UI thread và các luồng đọc nền, chia cache thành 16 shard theo `file_key`. Mỗi shard có LRU riêng và giới hạn `ROW_CACHE_BYTES / 16`.
  - Một Log Space 1 file chỉ dùng một shard, tức 4 MB. Với ~400 byte/entry, đó là khoảng 10.000 dòng, vẫn hơn 4096 dòng hiện tại và đủ cho vài màn hình cộng `READ_AROUND`.
- `IndexedFile` bỏ field `cache` và giữ `file_key`. Các chỗ gọi `self.cache.lock()...forget_file(0)` chuyển sang `rows().forget(self.file_key)`. `Drop` của `IndexedFile` gọi `forget` để dòng của file đã bỏ không nằm lại trong cache.

*Thay thế đã cân nhắc:*
- Một `Mutex` duy nhất cho cả app: đơn giản nhất, nhưng Scan và Indexing ở nền đọc entry qua cùng cache với UI thread. Rủi ro giật khi cuộn. Shard rẻ hơn đo rồi sửa.
- Giữ cache mỗi file nhưng có một "kế toán" chung (tổng byte toàn cục, bỏ ở file có tick cũ nhất): cần tham chiếu ngược tới mọi cache. Phức tạp hơn shard mà không lợi gì.
- Shard theo `hash(key)` thay vì theo `file_key`: chia đều hơn cho file đơn, nhưng `forget(file)` phải quét cả 16 shard. Chọn theo `file_key` cho `forget` rẻ. Nếu đo thấy file đơn thiếu cache, đổi sang shard theo `(file_key, line / READ_AROUND)`.

### 9. Settings và session

- `workspace_state` thêm `warm_tab_budget_mb: u32`, mặc định 512, `#[serde(default)]`, lưu trong `src/workspace/session.rs`.
- `src/ui/settings.rs` thêm một ô số "Warm Tab Budget (MB)", 0 = tắt, gợi ý ngắn "Tabs you leave stay in memory up to this size; 0 lets them go at once."
- Đổi giá trị thì gọi lại bước cắt của quyết định 4 ngay.
- `ROW_CACHE_BYTES` là hằng số, không đưa vào Settings.

### 10. Tab strip

Warm Tab hiện như tab đã đọc xong, không có dấu Dormant. Requirement "Tab hiện trạng thái đọc" của `dormant-tab` phân biệt Dormant, đang Indexing và đã đọc xong. Warm Tab rơi vào nhóm "đã đọc xong", vì chọn nó thì hiện ngay. Không thêm trạng thái hiển thị mới.

### 12. mimalloc làm allocator, `mi_collect` khi file bị bỏ

- `src/main.rs` đặt `#[global_allocator] static GLOBAL: mimalloc::MiMalloc`. Test và example không có `main.rs` của app, nên vẫn dùng allocator hệ thống, trừ `examples/mimalloc_probe.rs`, vốn chạy như app.
- `live::release_freed_memory` gọi `libmimalloc_sys::mi_collect(true)` trên mọi hệ. Trên macOS nó vẫn gọi thêm `malloc_zone_pressure_relief`, cho phần memory mà framework của hệ thống cấp qua malloc. Lời gọi đi qua crate `libmimalloc-sys` (feature `extended`), không qua `extern "C"` tự khai báo, để linker luôn kéo mimalloc vào, kể cả với binary dùng allocator khác.
- Hàm này được gọi khi `drop_dormant_files` bỏ file, khi trim Live Window, và mới thêm: khi tab đang mở đọc xong (`file_read`, `!is_reading()`). Lời gọi cuối thu dọn các bộ đệm và các bản cũ của `Vec` mà Indexing bỏ lại.
- *Thay thế đã cân nhắc* (số đo ở mục "Đo thử mimalloc"):
  - `MallocLargeCache=0`: memory đứng yên nhưng Indexing chậm gấp 2 lần, và phải khởi động lại tiến trình để đặt biến môi trường.
  - `MIMALLOC_PURGE_DELAY=0`: trả memory ngay ở mọi lần giải phóng, không cần lời gọi tường minh, nhưng trả giá ở mọi lần giải phóng. Trễ 100 ms thì không trả khi app đứng yên.
  - Sửa Indexing để cấp sẵn dung lượng hoặc lưu theo khúc: không thêm dependency nhưng phải sửa sâu, và chưa đo.

### 11. Ghi chú khi triển khai (lệch so với các quyết định trên)

- **Ước lượng memory tên là `IndexedFile::memory_bytes`**, không phải `heap_bytes` (quyết định 5). `heap_bytes` đã có sẵn: `mem_probe` và `index_probe` dùng nó để báo số "Line Index B/dòng", nên giữ nguyên. `memory_bytes` cộng thêm `effective_ts`, `row_ids` và giá trị của các cột. `MetaTable::heap_bytes` giờ tính cả các `Runs` (Generation, Stream).
- **File của Warm Tab không giữ handle** (rủi ro "Handle file của Warm Tab"). File local được index qua `DiskSource::stay_open`, nên handle vẫn mở sau khi rời tab. `LineSource` thêm `park()` và `unpark()`:
  - `park` đóng handle; lần đọc sau mở lại theo path.
  - `ChainSource::park` chỉ đồng ý khi chỉ có một Generation. Generation cũ chỉ đọc được qua handle nó giữ, vì path đã trỏ sang file khác.
  - File không park được thì bị bỏ (Dormant) thay vì giữ Warm. Điều này khớp với glossary của Generation: "stay readable … until its tab goes Dormant".
  - Chọn lại tab thì `unpark`, rồi `catch_up_shown_again` kiểm tra file như quyết định 7.
- **Mỗi file Warm tính ít nhất 1 byte.** Nếu không, file rỗng (Scratch mới) với budget 0 vẫn nằm lại trong Warm Tab mãi mãi.
- **Settings dùng `ButtonGroup`** (Off, 256 MB, 512 MB, 1 GB, 2 GB) như Live Window và Full Read Limit, thay vì ô số tự do.
- **`is_dormant_tab`** trả false cho một tab mà mọi file của nó đang Warm.
- **Đường đọc Archive Entry vào memory trong app đã bị bỏ** (`ensure_live_file_loaded` không còn nhánh `read_source`), vì quyết định 3b đi qua `archive_queue`. Phần này trước đó nằm ngoài phạm vi; nó biến mất tự nhiên khi Archive đi đường nền. `ParsedFile::from_source` và `parser::parse_*` vẫn còn đường đọc vào memory, nhưng app không gọi chúng.
- **Hàng đợi archive** là `AppView::archive_queue` (Archive → các entry), bắt đầu trong `start_queued_loads`. Các entry cùng Archive dùng chung một cờ huỷ. Nếu một lần giải nén bị huỷ vì một entry khác không còn được hiện, các entry vẫn cần được đưa lại vào hàng đợi.

## Risks / Trade-offs

- **[Ước lượng memory sai nhiều]** → Budget chặn sai. → Kiểm tra bằng `mem_probe` (quyết định 5). Ước lượng nghiêng về phía thừa.
- **[Allocator không trả memory khi bỏ Warm Tab]** → RSS không giảm dù đã bỏ. → Gọi `release_freed_memory()` sau mỗi lần bỏ, như đường Dormant hiện tại.
- **[Memory tăng so với hiện tại]**: người dùng có máy ít RAM sẽ thấy RSS cao hơn trước tới 512 MB. → Mặc định 512 MB là một phần nhỏ so với 3,1 GB trước `line-offset-index`; 0 trả lại hành vi cũ. Có thể hạ mặc định sau khi dùng thật.
- **[File static đổi trong lúc Warm mà không ai biết]** → Kiểm tra `changed_on_disk` khi chọn lại (quyết định 7). `changed_on_disk` không đọc gì khi kích thước và mtime không đổi, nên rẻ.
- **[Handle file của Warm Tab]**: giữ Line Index không được giữ handle mở, kẻo chạm `RLIMIT_NOFILE`. → `DiskSource::on_demand` đóng handle sau mỗi lượt đọc. Kiểm tra rằng `close_cached` hay một bước tương đương đóng handle của file live khi tab thành Warm.
- **[Index lại Scratch lớn mỗi lần rời Raw View]** → Chỉ xảy ra khi đã sửa. Chạy ở nền, có tiến độ (quyết định 2). Diff theo dòng để sau.
- **[Tiến độ giải nén với zip]**: zip đọc theo entry, không theo byte của Archive từ đầu tới cuối, nên tiến độ theo byte Archive có thể nhảy. → Với zip, tính tiến độ theo tổng kích thước nén của các entry cần giải nén, lấy từ central directory. Chấp nhận tiến độ thô.
- **[Archive bị đổi trong lúc đang giải nén]** → `dir_of` đã gắn thư mục với kích thước và mtime lúc bắt đầu. Lần sau mở thì giải nén vào thư mục mới, và `sweep` dọn thư mục cũ.
- **[Shard theo file làm file đơn có ít cache hơn]** → Tính toán ở quyết định 8 cho thấy vẫn hơn hiện tại. Có test cuộn nhanh trên file đơn, và đổi cách shard nếu cần.
- **[`Drop` của `IndexedFile` chạy trên luồng nền]** → `forget` chỉ lock một shard, rẻ.

## Số đo (2026-11-01)

Đo bằng `examples/memory_budget_probe.rs`, bản release, page cache nóng, trong harness GPUI (`TestAppContext`). Worktree cách ly không dựng được bản code trước change để đo nền. Vì vậy "trước" được đo bằng budget 0, là đúng hành vi rời tab cũ. Với Archive và Scratch, "trước" là tổng thời gian của phần việc mà code cũ chạy trên UI thread.

**Chuyển lại tab vừa rời** (file 3.000.000 dòng, ~300 MB; mỗi budget chạy một tiến trình riêng):

| | Budget 0 (Dormant, như trước) | Budget 512 MB (Warm) |
|---|---|---|
| UI thread khi bấm tab | 0,55 ms | 64 ms (dựng lại view từ Line Index) |
| Tới khi hiện dòng | 226 ms (index lại ở nền rồi dựng view) | 64 ms |
| `memory_bytes` của file | 135 MB (45 B/dòng) | 135 MB |
| RSS tăng khi mở / sau khi rời | +447 MB / +448 MB | +446 MB / +446 MB |

- Với budget 0, RSS **không giảm** sau khi rời tab. Lý do được điều tra ở mục "RSS không giảm khi rời tab" bên dưới: đó không phải leak trong app.
- **`memory_bytes` khớp với memory thật đang dùng** (task 6.2), đo bằng byte malloc đang dùng (`malloc_zone_statistics`), không đo bằng RSS. Sau khi rời tab, malloc còn dùng +33 MB với budget 0 và +168 MB với budget 512. Hiệu 135 MB đúng bằng `memory_bytes` (135 MB).

### RSS không giảm khi rời tab (điều tra 2026-11-01)

- **File bị bỏ đúng là được giải phóng.** Test `a_file_let_go_is_freed` cho thấy không còn tham chiếu nào tới `IndexedFile` hay `Indexed`. Byte malloc đang dùng giảm từ +301 MB xuống +33 MB.
- **Phần còn lại nằm trong large-cache của malloc macOS.** `vmmap --summary` sau khi rời tab cho thấy `MALLOC_LARGE (empty)` 363 MB, trong đó 315 MB vẫn dirty, trong 60 vùng. Đó là các khối lớn đã giải phóng: Line Index, `effective_ts`, `row_ids` và bộ đệm Indexing, cùng các bản cũ bỏ lại mỗi lần một `Vec` lớn tăng gấp đôi. `malloc_zone_pressure_relief` trả 0 MB, tức không lấy lại được gì; `phys_footprint` (số Activity Monitor hiển thị) cũng không giảm.
- **Chạy lại với `MallocLargeCache=0`:** footprint lúc mở là 241 MB thay vì 467 MB, sau khi rời tab còn 39 MB. Nhưng chuyển lại tab Dormant chậm gấp đôi (480 ms so với 230 ms).
- **Nó tăng dần, không được dùng lại hết.** Mở lần lượt 8 file 3 triệu dòng với budget 0: malloc chỉ dùng khoảng 300 MB ở mọi bước, nhưng footprint tăng khoảng 100 MB mỗi file (462 → 1.145 MB). Với `MallocLargeCache=0` thì đứng yên khoảng 237 MB.
- **Vấn đề này có từ trước change này.** Dormant Tab bỏ Line Index như vậy từ `line-offset-index`. Warm Tab làm nó ít xảy ra hơn, vì tab được giữ thì không phải index lại. Nhưng mỗi tab lớn thành Dormant rồi được index lại vẫn để lại khoảng 100 MB.
- **Hướng sửa (chưa chọn):**
  - (a) Dùng một allocator khác làm global allocator (mimalloc hoặc jemalloc), vì chúng trả memory về hệ điều hành. Thêm một dependency.
  - (b) Giảm số khối lớn bị bỏ lại khi Indexing: cấp sẵn dung lượng theo ước lượng từ kích thước file, hoặc lưu theo khúc cố định thay vì một `Vec` liền tăng gấp đôi.
  - (c) Khởi động lại tiến trình với `MallocLargeCache=0`: đơn giản nhưng Indexing chậm hơn khoảng 2 lần.

### Đo thử mimalloc (2026-11-01)

`examples/mimalloc_probe.rs` chạy cùng `memory_budget_probe` nhưng đặt mimalloc (crate `mimalloc` 0.1.48, chỉ ở dev-dependency) làm global allocator. "Collect" là gọi `mi_collect(true)` sau khi rời tab và sau mỗi lần Indexing xong, đúng chỗ app đang gọi `release_freed_memory`.

| File 3 triệu dòng | malloc macOS | `MallocLargeCache=0` | mimalloc mặc định | mimalloc `PURGE_DELAY=0` | mimalloc + collect |
|---|---|---|---|---|---|
| Footprint lúc mở | 462–474 MB | 241 MB | 541 MB | 288 MB | 544 MB (292 MB nếu collect sau Indexing) |
| Footprint sau khi rời (budget 0) | 467 MB | 39 MB | 541 MB | 52 MB | **54 MB** |
| Footprint sau khi rời (budget 512) | 451 MB | | | 174 MB | **172 MB** |
| Mở lần lượt 8 file (budget 0), file thứ 8 | 1.145 MB, tăng đều | 238 MB | 680–903 MB, dao động | 340 MB | **338 MB, đứng yên** |
| Chuyển lại tab Dormant, tới khi hiện | 230 ms | 480 ms | 174 ms | 177 ms | **173 ms** |
| Chuyển lại tab Warm, UI thread | 64–78 ms | | 89 ms | 81 ms | 80 ms |
| Archive Entry 404 MB, mở lần đầu / mở lại | 734 / 450 ms | | 557 / 263 ms | | |
| Scratch 40 MB, tới khi hiện | 54 ms | | 38 ms | | |

- `MIMALLOC_PURGE_DELAY=100` **không** trả memory khi app đứng yên (vẫn 328 MB sau 3 giây). mimalloc chỉ dọn khi có cấp phát tiếp theo, nên cần một lời gọi tường minh. Đặt delay 0 thì trả ngay, nhưng cái giá trả ở mọi lần giải phóng.
- **Khuyến nghị:** mimalloc làm global allocator của app (`src/main.rs`), cộng với `mi_collect(true)` trong `release_freed_memory`, thay cho `malloc_zone_pressure_relief`. Thêm một lời gọi sau khi Indexing của tab xong. Footprint khi đó đứng yên khoảng 340 MB khi đi qua nhiều tab lớn (so với 1,1 GB và tăng tiếp), và Indexing, giải nén, Scan đều nhanh hơn 20–40%.
- Cái giá: thêm một dependency có mã C (`libmimalloc-sys`, build bằng `cc`), và footprint lúc vừa mở file trước khi collect cao hơn khoảng 15%. RSS theo `ps` vẫn cao vì trang đã purge được đánh dấu MADV_FREE; số Activity Monitor hiển thị là footprint.

**Đã áp dụng (quyết định 12), đo lại qua đường của app** (`mimalloc_probe`, app tự gọi `release_freed_memory`; mimalloc 0.1.52):

| File 3 triệu dòng | malloc macOS (trước) | mimalloc trong app |
|---|---|---|
| Footprint sau khi mở (đã đọc xong) | 462–474 MB | 298 MB |
| Sau khi rời, budget 0 / 512 | 467 / 451 MB | **56 / 182 MB** |
| Mở lần lượt 8 file, file thứ 8 | 1.145 MB | **342 MB** (dao động 299–342) |
| Chuyển lại tab Dormant / Warm | 230 / 64–78 ms | 175 / 87 ms |
| Archive 404 MB, mở lần đầu / mở lại | 734 / 450 ms | 544 / 256 ms |
| Scratch 40 MB | 54 ms | 36 ms |

- Chuyển lại tab Warm chậm hơn một chút (87 ms so với 64–78 ms trên UI thread). Đây là dựng lại view.
- Chưa đo trên Windows và Linux.
- Với file 3 triệu dòng, Warm hiện lại nhanh hơn khoảng 3,5 lần, nhưng 64 ms đó chạy trên UI thread. Dormant chỉ chiếm UI thread chưa tới 1 ms; phần dựng view của nó chạy sau, trong `file_read`, và cũng trên UI thread.

**Row Cache chung** (Log Space 50 file × 20.000 dòng, đọc mọi dòng):
- Cache giữ 69,7 MB: dưới ngưỡng cắt 125% của 64 MB.
- Theo cách cũ (4096 dòng mỗi file), cùng tải giữ khoảng 82 MB, và con số này tăng theo số file.

**Scratch 40 MB** (500.000 dòng, chọn lại tab với budget 0): UI thread 0,97 ms; tới khi hiện dòng 54 ms. Trước change, toàn bộ việc đọc file vào memory và index chạy đồng bộ trên UI thread.

**Archive Entry 404 MB** (trong `.tar.gz` 93 MB):

| | UI thread | Tới khi hiện dòng |
|---|---|---|
| Mở lần đầu (giải nén rồi index) | 0,44 ms | 734 ms |
| Mở lại (đã giải nén) | 0,40 ms | 450 ms |

Trước change, toàn bộ cột "tới khi hiện dòng" chạy trên UI thread.

**Mặc định 512 MB:** giữ nguyên. Một file 3 triệu dòng tính 135 MB, nên 512 MB giữ được khoảng ba tab cỡ đó. Nên xem lại sau khi giải được câu hỏi RSS không giảm ở trên.

## Migration Plan

- Settings mới có mặc định khi thiếu. Không đổi format khác.
- Làm theo thứ tự trong tasks.md: Row Cache chung → Scratch từ đĩa → editor theo vòng đời tab → Archive Entry ở nền → Warm Tab → Settings → đo. Mỗi bước là một commit riêng, test xanh.
- Rollback: đặt Warm Tab Budget về 0 trả lại hành vi rời tab cũ mà không cần revert. Các bước khác revert theo commit.

## Open Questions

- **Memory pressure của macOS** (`DISPATCH_SOURCE_TYPE_MEMORYPRESSURE`): khi hệ thống báo warning hay critical, có nên bỏ hết Warm Tab ngay không? Hợp lý nhưng là một nguồn sự kiện mới. Đề xuất để sau, trừ khi người dùng muốn gộp vào change này.
- Mặc định 512 MB có hợp với máy của người dùng không? Chốt sau khi đo `perf_probe` với 3–5 tab thật.
- Có nên hiện tổng memory của Warm Tab ở đâu đó (Settings, cạnh ô budget) để người dùng biết nên đặt bao nhiêu không?
- Khi `line-offset-index` được archive, chuyển requirement "Rời tab giữ Line Index trong Warm Tab Budget" thành MODIFIED của `dormant-tab` ("Rời tab thì bỏ dòng, giữ kết nối").
