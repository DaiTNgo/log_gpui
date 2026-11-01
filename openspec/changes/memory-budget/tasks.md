## 1. Đo trước

- [x] 1.1 Ghi số đo nền vào design.md bằng `perf_probe` và `mem_probe`: chuyển tab (Dormant đọc lại) với file 3.000.000 dòng; RSS khi cuộn hết một Log Space 50 file; RSS và thời gian mở một Scratch Log Space có file 50 MB.

## 2. Row Cache chung

- [x] 2.1 Cấp `file_key: u64` duy nhất cho mỗi `IndexedFile` (`AtomicU64`). Test: hai file khác nhau có key khác nhau; `read_with` tạo key mới.
- [x] 2.2 `SharedRowCache`: 16 shard theo `file_key`, LRU theo byte ước lượng (`size_of::<LogEntry>() + độ dài dòng + 64`), cắt khi vượt 125% về 100%, `forget(file_key)`. Test đơn vị: giới hạn byte được giữ; `forget` chỉ bỏ dòng của file đó; `missing` trả đúng các đoạn thiếu.
- [x] 2.3 Chuyển `IndexedFile` sang cache chung: bỏ field `cache`, đổi mọi `forget_file(0)` thành `forget(self.file_key)`, thêm `Drop` gọi `forget`. Bỏ `ROW_CACHE`. Test hiện có của `index::file` và `view_rows` xanh.
- [x] 2.4 Test: mở Log Space nhiều file (dùng `BytesSource` cho 50 file), đọc entry của mọi dòng, tổng byte của cache không vượt `ROW_CACHE_BYTES`.

## 3. Scratch đọc từ đĩa

- [x] 3.1 `ensure_scratch_loaded` gọi `queue_file_load` thay vì `read_source` + `BytesSource`. Bỏ `scratch_parsed`. Test: mở Scratch Log Space, file được đọc qua `DiskSource`, không có `BytesSource` nào giữ text.
- [x] 3.2 `reparse_edited_scratch_files`: `flush_scratch_saves` trước; file lưu được thì bỏ `parsed_files` và `queue_file_load`; file lưu thất bại thì giữ `raw.edited` và Line Index cũ. Test: sửa rồi chuyển sang Parsed View thì thấy dòng mới; lưu thất bại (thư mục chỉ đọc) thì Parsed View giữ dòng cũ và có notice.
- [x] 3.3 Test: file scratch rỗng mới tạo hiện ngay, không lỗi; file scratch bị xoá ngoài app đọc thành rỗng như `read_text`.

## 4. Editor của Scratch theo vòng đời tab

- [x] 4.1 Tách việc bỏ state Raw View của một file (`raw.editors`, `raw.texts`, `raw.lines_cache`, `raw.edited`, `raw.save_gen`) thành một hàm, dùng chung với `forget_file`.
- [x] 4.2 Khi một file scratch thành Dormant: lưu rồi bỏ state Raw View; lưu thất bại thì giữ. Test: sửa, rời tab với Warm Tab Budget 0, text nằm trên đĩa và `raw.texts` không còn file; lưu thất bại thì `raw.texts` vẫn còn.

## 5. Archive Entry ở nền

- [x] 5.1 `load_queue` mang `read_path` (`QueuedLoad`); `start_queued_loads` index `read_path`. File local không đổi hành vi. Test hiện có của loading xanh.
- [x] 5.2 `archive_cache::unpack` nhận `cancel` và một bộ đếm byte đã đọc; huỷ thì trả `Cancelled` và xoá `.part`. Test đơn vị: huỷ giữa chừng không để lại `.part`; entry đã có thì không đọc Archive; bộ đếm tăng tới kích thước Archive.
- [x] 5.3 `ensure_archive_entries_loaded` chạy `unpack` trên `background_executor`, một việc mỗi Archive; xong thì đưa entry vào `load_queue` với path đã giải nén; lỗi thì `ParsedFile::failed` kèm notice; huỷ thì không notice. Bỏ `ParsedFile::from_path` khỏi đường này.
- [x] 5.4 `Progress::Unpacking` và hiển thị trên tab strip, rồi chuyển sang tiến độ Indexing. `drop_dormant_loads` huỷ việc giải nén của file không còn được đọc.
- [x] 5.5 Test: mở tab có ba entry của cùng `.tar.gz`, Archive được đọc một lần; rời tab giữa lúc giải nén thì việc dừng và không còn `.part`; mở lại tab với entry đã có thì không đọc Archive; Archive hỏng thì có notice và file rỗng; gọi `ensure_files_loaded` hai lần liền không tạo hai việc giải nén.
- [x] 5.6 Đo bằng `perf_probe` với một `.tar.gz` có entry ~500 MB: thời gian UI thread bị chiếm khi chọn tab, trước và sau. Ghi vào design.md.

## 6. Warm Tab

- [x] 6.1 `IndexedFile::heap_bytes()` ước lượng heap của Line Index, `LineMeta`, `row_ids` và giá trị cột, Generation và Break. Test đơn vị: tăng gần tuyến tính theo số dòng.
- [x] 6.2 Kiểm tra `heap_bytes` bằng `mem_probe` trên file 3.000.000 dòng: chênh lệch so với RAM tăng đo được dưới 20%. Ghi vào design.md.
- [x] 6.3 `WarmFiles` trong `AppView`: `file_id → (left_at, bytes)`, tổng byte, lấy ra file rời lâu nhất. Test đơn vị: thêm, cập nhật `left_at`, cắt theo budget, bỏ file của tab đang mở.
- [x] 6.4 `drop_dormant_files` dùng `WarmFiles`: file rời được giữ nếu vừa budget; Remote File và Remote Command vẫn bỏ như hiện tại; bỏ file thì `close_cached`, `forget` Row Cache, bỏ state Raw View (4.2) và `release_freed_memory`. Test: rời tab trong budget, `parsed_files` còn file; vượt budget, tab rời lâu nhất bị bỏ trước; một tab lớn hơn budget bị bỏ ngay; budget 0 giống hành vi cũ.
- [x] 6.5 Test: Live Log Space có `app.log` local và một Remote File, rời tab, `app.log` được giữ, Remote File bị bỏ và mở lại từ Remote Cache khi chọn lại.
- [x] 6.6 Scratch còn editor: cộng độ dài text × 3 vào `bytes` của file. Test: quay lại Warm Tab scratch thì editor còn nguyên text và con trỏ.
- [x] 6.7 Chọn lại Warm Tab của Static Log Space: `changed_on_disk`: `Same` dùng nguyên; `Appended` hay `Replaced` xử lý như `watch_static_files`; `Gone` như file biến mất khi đang mở. Test cho từng trường hợp.
- [x] 6.8 Chọn lại Warm Tab của Live Log Space: kiểm tra `start_missing_sources` đi tiếp từ phần đã index. Test: ghi thêm 5.000 dòng khi Warm thì chỉ 5.000 dòng mới được đọc; rotate khi Warm thì có Break và file mới đọc từ đầu.
- [x] 6.9 Kiểm tra handle: file của Warm Tab không giữ handle mở. Test với `HandleBudget`: rời 10 tab có file live, số handle đang mở không tăng.
- [x] 6.10 Test: chọn lại Warm Tab thì không có Indexing; Filter, Focus Tab, vị trí và Marker được giữ.

## 7. Settings

- [x] 7.1 `warm_tab_budget_mb` trong `workspace_state`, mặc định 512, `#[serde(default)]`, lưu trong session. Test: session cũ thiếu giá trị đọc ra 512.
- [x] 7.2 Ô "Warm Tab Budget (MB)" trong `src/ui/settings.rs`; đổi giá trị thì cắt Warm Tab ngay. Test: giảm budget thì Warm Tab rời lâu nhất bị bỏ trước.

## 8. mimalloc làm allocator (thêm sau khi đo)

- [x] 8.1 Điều tra RSS không giảm khi rời tab: `vmmap`, `malloc_zone_statistics`, `MallocLargeCache=0`, mở lần lượt 8 file (`memory_budget_probe --cycle`). Ghi vào design.md.
- [x] 8.2 Đo mimalloc bằng `examples/mimalloc_probe.rs`: mặc định, `PURGE_DELAY`, `mi_collect`. Ghi bảng vào design.md.
- [x] 8.3 `mimalloc` và `libmimalloc-sys` (`extended`) trong `[dependencies]`; global allocator trong `src/main.rs`; `release_freed_memory` gọi `mi_collect(true)` trên mọi hệ; thêm một lời gọi khi tab đọc xong.
- [x] 8.4 Đo lại qua đường của app: footprint sau khi rời tab với budget 0 và 512, 8 file lần lượt, tốc độ. Test liên quan xanh.
- [ ] 8.5 Build và chạy thử trên Windows (và Linux): app mở, rời tab trả memory.

## 9. Đo, tài liệu, dọn

- [x] 9.1 Đo lại như 1.1, thêm chuyển tab Warm. Ghi bảng trước/sau vào design.md. Chốt hoặc đổi mặc định 512 MB.
- [x] 9.2 Cập nhật CONTEXT.md theo phần Domain changes của proposal (Warm Tab, Warm Tab Budget, Dormant Tab, Scratch Log Space, quy tắc 43 và 44).
- [ ] 9.3 Chạy test của các module đã đổi (`index`, `app` liên quan tới tab, Raw View, Scratch), `cargo clippy` sạch, `openspec validate memory-budget` qua. Kiểm tra bằng tay trong app thật: rời và quay lại tab file 376 MB, sửa Scratch rồi chuyển view, đổi budget.
