Thứ tự làm (đổi ngày 2026-10-25):

1. Nhóm 1–3.
2. Phần lõi của 4.1–4.2.
3. Nhóm 11 (Dormant Tab), vì nhóm này không cần `ViewRows`.
4. Task 4.3 làm cùng lúc với nhóm 5–8.
5. Task 7.4 (trước là 4.4).
6. Nhóm 9, 10, rồi 12.

Lý do: `all_entries` còn là trạng thái được sửa tại chỗ. Filter ghi `delta_time_ms`, Marker gắn vào entry, Live và Detected Field sửa entry. Vì vậy chưa bỏ được việc dựng nó mỗi lần chuyển tab cho tới khi xong nhóm 7.

## 1. Spike: đo Line Index (cổng go/no-go)

- [x] 1.1 Tạo dữ liệu đo (`index_probe --gen`):
  - Thêm vào `examples/perf_probe.rs`, hoặc tách thành một hàm dùng chung, cách sinh file 3.000.000 dòng (~376MB), cùng định dạng với số đo nền trong design.md.
  - Ghi số nền của `mem_probe` (thời gian parse, byte/dòng, RSS tối đa) vào design.md để so.
- [x] 1.2 `examples/index_probe.rs`, phần Indexing:
  - Pipeline đọc khối 4 MiB, `memchr` tìm dòng, parse timestamp, level và Parent Line song song theo khối, rồi nối tuần tự.
  - Dựng `LineIndex` theo hai cách để so: khối 128 dòng mã hoá varint, và `Vec<u64>`.
  - Dựng `LineMeta` và `row_ids` cho các cột phát hiện được.
  - Chạy hai lần: có khoá format và timestamp theo file, và không khoá (quyết định 7).
  - Chạy với `row_ids` cho mọi cột (a), và chỉ cho các cột có sẵn trong mọi dòng (b). Với (b), đo thêm thời gian Scan dựng một Detected Field khi cần (quyết định 4).
  - In ra: thời gian lô đầu (1.000 dòng), thời gian Indexing toàn bộ, byte/dòng của từng phần, RSS tăng thêm.
- [x] 1.3 `index_probe`, phần đọc theo nhu cầu: thời gian `read_at` cộng parse một màn hình 60 dòng, ở đầu, giữa và cuối file, và khi nhảy ngẫu nhiên.
- [x] 1.4 `index_probe`, phần Scan: Search Expression `timeout` và `user=42` trên mọi dòng, song song theo khối 10.000 dòng.
  - Đo khi page cache nóng, và khi lạnh (sau `purge` trên macOS).
  - So `Vec<u32>` với Roaring bitmap cho kết quả.
- [x] 1.5 `index_probe`, phần gộp: k-way merge của 3 file × 1.000.000 dòng theo `effective timestamp`.
  - So kết quả với `timeline::materialize` trên cùng dữ liệu (cùng thứ tự).
  - Đo thời gian gộp toàn bộ và thời gian gộp thêm 1.000 dòng.
- [ ] 1.6 Chạy `index_probe` trên một file log thật của người dùng (do người dùng chọn), ghi số.
- [x] 1.7 Ghi mọi số đo vào bảng "Spike" trong design.md, rồi chốt:
  - cách mã hoá Line Index;
  - kiểu tập kết quả;
  - có dùng khoá format không;
  - chọn (a) hay (b) cho `row_ids`.

  Sau đó kết luận **go** hoặc **no-go** theo mục tiêu của quyết định 14.
  - Nếu no-go: dừng ở đây và cập nhật design trước khi làm tiếp.

## 2. LineSource và LineIndex

- [x] 2.1 `src/index/source.rs`: trait `LineSource` (`read_at`, `len`) với các hiện thực:
  - file trên đĩa (`FileExt::read_at` / `seek_read`, mở có chia sẻ đọc, ghi và xoá)
  - bytes trong memory
  - API (`read_at`, `len`, thêm vào cuối) không giả định nguồn nằm trong memory, để change "Remote ghi ra cache local" sau này chỉ phải thay hiện thực (quyết định 2).
  - Test: đọc giữa file, đọc vượt cuối, file đổi kích thước.
- [x] 2.1b Quản lý file handle:
  - Mở theo nhu cầu cho file không live, đóng sau khi đọc xong một lượt.
  - Giữ handle mở chỉ cho file live và cho Generation cũ còn dòng trong view.
  - Đếm số handle đang mở, và báo người dùng trước khi chạm `RLIMIT_NOFILE`.
    (`HandleBudget::near_limit` và lỗi "too many files open" đã có; hiện thông báo trên UI khi app dùng `DiskSource`, ở nhóm 4.)
  - Test: mở 300 file nhỏ trong một Log Space với giới hạn 256 handle.
- [x] 2.2 `src/index/line_index.rs`: `LineIndex` theo cách đã chọn ở spike (thêm dòng, tra vị trí và độ dài của dòng `n`, số dòng).
  - Test: dòng rỗng, CRLF, dòng cuối không có `\n`, dòng 1MB, file rỗng.
- [x] 2.3 `LineMeta` (effective timestamp, `has_time`, level, Parent Line, cờ) cùng danh sách khoảng Generation và Stream.
  - Test: so với `LogEntry` do `parse_entries` sinh ra trên các file trong `tests/` (cùng timestamp, level, Parent Line).
- [x] 2.4 Parse lại một dải dòng từ `LineSource` thành `LogEntry` bằng `LineParser` hiện có, cho đúng kết quả như `ParsedFile::from_bytes` trên cùng dòng (test so sánh trên mọi file mẫu).

## 3. Indexing ở nền

- [x] 3.1 Pipeline Indexing (luồng IO, worker theo khối, bước nối tuần tự) chạy trên background executor. Nó gửi lô kèm tiến độ (byte đã đọc / tổng), có token huỷ.
  - Test: Continuation Line vắt qua ranh giới khối; timestamp kế thừa qua ranh giới; huỷ giữa chừng.
- [x] 3.1b Khoá format và timestamp theo file (spike chọn dùng), kèm bộ đọc nhanh không cấp phát cho các dạng phổ biến như trong `index_probe`:
  - Khoá sau khi đủ số dòng liên tiếp khớp; không khớp thì dò lại toàn bộ.
  - Test: `LineMeta` có khoá khớp từng dòng với không khoá, kể cả file trộn hai định dạng thời gian và file có dòng rác xen giữa.
- [x] 3.2 Dựng `ColumnIndex::row_ids`, danh sách giá trị, số đếm và Coverage dần theo lô, giữ `MAX_DISTINCT`. Dựng cho mọi cột (a), hay chỉ cho các cột có sẵn trong mọi dòng (b), theo kết quả spike. Với (b), Detected Field được dựng bằng Scan khi người dùng mở picker hoặc Highlight Rule của cột lần đầu, hoặc khi Filter đã lưu có điều kiện trên cột đó.
  - Test: so `FilterIndex` dựng theo lô với `FilterIndex::build` trên cùng file.
- [x] 3.3 File live: hiện các dòng cuối trước trong lúc Indexing chạy từ đầu; số dòng đúng khi Indexing xong (L3).
  - Nếu spike cho thấy Indexing đủ nhanh, ghi lại trong design và bỏ bước đọc ngược.
  - Indexing không khoá mất 2,28 s trên 376MB, nên vẫn cần bước này. Đã có `index::tail::read_tail` (đọc 64 KiB cuối) kèm test. Còn phải nối vào view, ở nhóm 4 và nhóm 9.
  - Đã làm: khi file của Live Log Space được index, `FileLoad` đọc 64 KiB cuối ngay lúc bắt đầu, rồi đọc lại mỗi 500 ms nếu file lớn lên. Canvas hiện tối đa 500 dòng cuối (nhiều file thì xếp theo thời gian, kèm tên file), dòng mới nhất ở dưới, không có số dòng, với dòng ghi "Reading … (N%)". Khi đọc xong, view mở ở cuối với Follow bật và số dòng đúng. Static Log Space không đọc trước dòng cuối.

## 4. Lớp ViewRows thay cho all_entries

- [x] 4.1 `ViewRows`: dòng của view là chỉ số.
  - File View: `0..line_count`.
  - Log Space: `Vec<u64>` đóng gói `(file, line)` từ k-way merge tăng dần.
  - API: số dòng, `meta(row)`, `entry(row)` qua LRU, dải dòng đang hiện.
  - Test thứ tự gộp theo các scenario của `merged-timeline` (trùng timestamp, stack trace, file không có timestamp).
  - Đã có `index::view_rows` (`ViewRows`, `Merger` gộp tăng dần, `pack`/`unpack`) kèm test: gộp cả file và gộp khi file đang lớn dần đều ra đúng thứ tự của `timeline::merge`. Còn nối vào `AppView`.
  - Đã làm: `ViewData::rows` là `ViewRows`. File View là `File { lines }`, không còn mảng một phần tử mỗi dòng (24 MB cho file 3M dòng); Log Space là `Merged`. Đường live của Remote File đổi dòng tại chỗ qua `to_mut()`, khi đó dòng của File View được ghi ra mảng. `ViewTexts` của Scan giữ `ViewRows` thay vì copy mảng.
- [x] 4.2 LRU các dòng đã parse (khoảng 4.096 dòng, khoá `(file, generation, line)`); đọc gom theo khối; canvas vẽ placeholder khi dòng chưa có rồi `cx.notify()` khi có.
  - Đã có `RowCache` (LRU, `missing` cho ra các đoạn cần đọc gom). Còn phần placeholder và `cx.notify()` trong canvas.
  - Đã làm: canvas vẽ từng dòng qua `Rows::cached`, không đọc file trong frame. Dòng chưa có trong cache là placeholder "…"; sau frame, `read_rows_soon` đọc các khối `READ_AROUND` của chúng ở worker thread (mỗi khối một lần) rồi `cx.notify()`, chỉ khi có dòng đọc được. Minimap vẫn đọc entry (4.3 mục 5).
- [x] 4.3 Chuyển lần lượt các chỗ đọc `all_entries` sang `ViewRows`, mỗi nhóm một commit, giữ test xanh. Làm cùng lúc với nhóm 5–8, theo từng tính năng:
  1. canvas và Gutter
  2. Focus Pane
  3. Go to
  4. chọn dòng và copy
  5. Minimap
  6. độ rộng cột (`max_chars` tính trong lúc Indexing)
  - Đã xong lớp truy cập: trait `Rows` (3981fc7). `ViewData` thay `all_entries`, view không còn copy dòng (867c99b). Mọi chỗ trong danh sách trên đọc qua `Rows`; entry vẫn lấy từ `ParsedFile.entries`. Còn lại: chuyển nguồn entry sang Line Index cộng `RowCache` (cần Indexing được nối vào app), và bỏ các lần duyệt toàn bộ dòng (Search, Highlight, `FilterIndex::build`) theo nhóm 5–7.
  - Đã làm (2026-10-28): bỏ các lần đọc mọi dòng trong lúc vẽ và refresh. Canvas (4.2) và Minimap chỉ vẽ dòng có trong cache, dòng còn lại được đọc sau frame. Dòng dài nhất (độ rộng của danh sách) lấy từ độ dài dòng trong Line Index (`LineIndex::longest`), không parse: refresh một file 300k dòng từ 614 ms còn 3 ms. Focus Pane không đọc dòng khi không có matcher. Go to và chọn dòng đã đọc `LineMeta` (4b.4); `max_chars` có từ Indexing (4b.3).
  - Còn lại: Search, Highlight Rule trên MESSAGE và Focus Find vẫn đọc text từng dòng, làm theo nhóm 6–7.
- 4.4 đã chuyển thành 7.4: chuyển tab không clone chỉ làm được khi Filter, Search, Highlight và Marker không còn đọc hay sửa `all_entries`.

### 4b. Thay `ParsedFile.entries` của file local bằng Indexing (kế hoạch 2026-10-25)

Hiện tại (c8bf40d):
- view không copy dòng nữa;
- file được parse song song ở nền (big.log 2,6 s);
- nhưng mọi entry vẫn nằm trong RAM, khoảng 500 B/dòng.

Để giảm memory, file local phải giữ Line Index, `LineMeta` và `FilterIndex` (khoảng 30 B/dòng), còn entry lấy qua `RowCache`. Làm theo thứ tự sau, mỗi bước một commit, giữ test xanh:

- [x] 4b.1 `Rows::get` trả về `RowRef` (mượn từ `Vec`, hoặc `Arc<LogEntry>` từ cache) thay cho `&LogEntry`. Chỉ đổi kiểu, chưa đổi hành vi.
- [x] 4b.2 `index::file::IndexedFile`: một file local đọc qua Indexing. Nó giữ Line Index, `LineMeta` và `FilterIndex`; entry được đọc lại từ file theo khối 256 dòng và giữ trong `RowCache`. Có test so với `ParsedFile::from_bytes`.
  - Số đo trên big.log (3M dòng): mở 0,21 s, index 29 B/dòng, RSS +237 MB; 60 entry từ đĩa mất 0,6 ms, từ cache 1 µs. So với hiện tại (`ParsedFile`): mở 2,6 s, RSS +1,58 GB.
- [x] 4b.3 `ParsedFile` có thêm dạng đã index (`indexed: Option<Arc<IndexedFile>>`, khi đó `entries` rỗng). Các phần đọc qua API theo file: `BoundRows::get` (lấy từ cache), gộp timeline bằng `Merger` trên `LineMeta`, `max_chars`, độ dài message dài nhất.
- [x] 4b.4 `FilterIndex` của view:
  - File View dùng thẳng `FilterIndex` của file;
  - Log Space gộp `FilterIndex` của các file theo thứ tự dòng đã gộp.
  Go to và `LineRef` đọc `effective_timestamp` cùng `LineMeta` thay vì entry.
- [x] 4b.5 Highlight Rule theo Filter Value tô bằng `row_ids` (7.1); rule có Custom Value tô bằng Scan (7.2). `run_search` cũng chạy qua Scan, đọc từ Line Index.
  - Đã làm: Highlight Rule (kể cả Custom Value) trên cột có `row_ids` được tô bằng bảng tra theo giá trị. `run_search` chạy `scan_all` trên text của view, đọc từ Line Index cho file đã index.
  - Còn lại: rule trên MESSAGE hoặc trên cột tràn vẫn đọc text từng dòng. Với file đã index, nghĩa là đọc từ đĩa qua cache, cần chuyển sang Scan ở nền (7.2).
- [x] 4b.6 Raw View và xem trước Log Format đọc dòng qua Line Index (8.1).
  - Đã làm: `RawSection.lines` thành `RawLines`. Một dòng lẻ của file đã index đọc qua cache; khi cần toàn bộ text (editor của Raw View, Raw Search), file được đọc một lần và giữ cho tới khi rời tab. Xem trước Log Format chỉ đọc tới dòng đầu tiên không khớp.
  - Còn lại (8.1): Raw View vẫn đưa toàn bộ text vào một editor, nên mở Raw View một file lớn vẫn tốn memory bằng cỡ file. Muốn bỏ phải chuyển sang danh sách ảo.
- [x] 4b.7 Mở file local bằng `IndexedFile` khi đọc ở nền (`app::loading`). Live tail của file đó index phần byte mới (9.1); rotate và truncate mở Generation mới (9.2).
  - Status bar của file local chỉ còn Live/Pause, Follow và trạng thái file. Không có Load more, Open in full, Live Window, Back to Live Window, "N lines held" hay Clear (design quyết định 12).
  - Viết lại test live của file local: ghi byte thật vào file. Bỏ các test Live Window của file local.
  - Đã làm: file local mở qua `IndexedFile` ở nền (`index_into`). Live/Pause và Follow index phần dòng trọn vẹn mới (`IndexedFile::extend`); dòng cuối chưa có line break được đọc lại khi nó đủ dòng. big.log (376 MB, 3M dòng): đọc xong trong 1,28 s, RSS 470 MB (trước 2,6 GB).
  - Còn lại (9.2): rotate và truncate (file ngắn đi, hoặc Break mở Generation) chỉ đọc lại file từ đầu. Dòng của Generation cũ bị bỏ, chưa có Break.
  - Change `static-live-log-spaces` (chốt 2026-10-27): Imported thành Static, không còn Live/Pause và Follow. Live/Pause và Follow của file local chuyển sang Live Log Space (task 4.1 của change đó).
- [x] 4b.8 Detected Field hoặc Log Format đổi thì index lại file (6.3).
  - Đã làm: file đã index nào có Line Config hoặc Log Format khác cái đang dùng được đọc lại ở nền. Session được lưu trước, nên Filter và Marker giữ nguyên.
  - Còn lại (6.3): đọc lại cả file chứ chưa chỉ dựng `row_ids` của cột mới; cột mới chưa hiện ngay cho dòng đang xem.

## 5. Filter

- [x] 5.1 `FilterState::evaluate` chạy trên `row_ids` và `LineMeta` (Filter Value, Time Range, dòng thiếu giá trị, Hide Mode, Continuation Line theo Parent Line).
  - Test: chạy lại toàn bộ test hiện có của `log-filter`.
  - `evaluate` không còn sửa entry. Trường `delta_time_ms` đã bỏ, vì UI không hiện nó (chỉ `ui/table_view.rs` đọc, mà file này không được biên dịch). Text chỉ đọc qua `RowValues` cho cột không có `row_ids` (cột tràn, sẽ thành Scan ở 5.3).
- [x] 5.2 Picker hiện giá trị và số đếm tăng dần trong lúc Indexing, kèm dấu "chưa đọc hết" (spec `log-filter`, scenario "Giá trị trong lúc Indexing").
  - Trong lúc file của tab còn đang index và view chưa có dòng, Filter lấy giá trị và số đếm từ phần đã index (`FilterIndex::values_so_far`, không chép `row_ids`). Danh sách được cập nhật theo nhịp tiến độ đọc. Log Space nhiều file thì cộng giá trị các file theo thứ tự file; cột Source là tên file.
  - Filter ghi "Still reading (N%): values and counts so far." cho tới khi đọc xong.
  - Giá trị tick trong lúc đọc được lưu vào tab trước khi dựng view, nên lọc ngay khi file đọc xong; picker đang mở vẫn mở.
- [x] 5.3 Custom Value:
  - Cột còn danh sách giá trị: so trên danh sách đó, không cần text.
  - Cột tràn `MAX_DISTINCT` hoặc cột `message`: chạy Scan (nhóm 6). Focus Pane nhận dòng dần.
  - Đã làm: `FilterState::evaluate_split` tính ngay các điều kiện đọc từ `row_ids` và Time Range, và trả về các điều kiện cần text (`TextCondition`). Dòng của file giữ trong RAM được so ngay. Dòng của file đã index được so bằng Scan ở nền (`scan_blocks`, khối quanh dòng đang xem trước, đọc entry qua `IndexedFile::each_entry_at`, không qua cache). Focus Pane nhận dòng dần, Filter ghi tiến độ. Filter mới huỷ Scan cũ. Dòng live đến trong lúc Scan được lọc ngay như trước.
  - Thẻ Message: mục Message ở cuối danh sách chọn cột, chỉ nhận Custom Value. Spec chính `log-filter` có thêm Requirement "Thẻ Message".

## 6. Scan

- [x] 6.1 Bộ chạy Scan chung (`index::scan`; nối vào Search ở 6.2):
  - Chia khối 10.000 dòng, quét khối quanh vị trí đang xem trước.
  - Kết quả là tập dòng đã sắp xếp; có tổng tạm thời và tiến độ.
  - Scan mới huỷ Scan cũ; dòng live chỉ quét phần mới.
  - Test cho từng hành vi trong spec `progressive-scan`.
- [x] 6.2 Search chạy qua Scan, dùng lại `search_expr` và so khớp Unicode hiện có.
  - Đã làm: khi gõ Search, Match được tìm bằng `scan_in_steps` qua `ViewData::texts`. Các khối quanh dòng đang xem được quét trước. Match hiện dần, ô đếm có tiến độ (`3/120 · 30%`), và query mới bỏ Scan cũ. `SearchLines` (bản sao text của mọi dòng) đã bỏ. Text của file đã index đọc từ đĩa qua `FileTexts`. `run_search` (Enter), dựng lại rows, và đổi Focus Tab cũng tìm bằng Scan (`find_matches`) khi view có file đã index; chỉ Enter đi tới Match khi Scan xong. View chỉ có file giữ trong RAM thì vẫn tìm ngay. Đổi Filter không tìm lại Match (Match tính trên mọi dòng, Focus Pane lọc theo Filter).
  - Parsed View chỉ đếm dòng Filter đang hiện; Raw View đếm mọi dòng.
  - Ô Search hiện tổng tạm thời và tiến độ; Next/Previous dùng được trên Match đã tìm thấy.
  - Test: chạy lại toàn bộ test của `log-search`.
- [x] 6.3 Bật Detected Field mới và đổi Log Format chạy qua Scan: dựng `row_ids` của cột mới; đổi Log Format thì index lại `LineMeta`. Cột hiện ngay cho dòng đang xem.
  - Đã làm: file được Indexing đọc lại ở nền như trước, nhưng dòng của view ở lại. `IndexedFile::read_with` giữ Line Index cũ và đọc dòng đang xem theo Log Format và Detected Field mới (cache mới), nên cột hiện ngay. Khi Indexing xong, view dựng lại với `row_ids` và `LineMeta` mới, giữ vị trí. Đọc lại lần nữa trong lúc đang đọc thì lần trước bị huỷ (`FileLoad.again`, `file_read` so token).
  - Không dùng Scan riêng: Indexing lại cả file đã dựng `row_ids` và `LineMeta` cùng lúc, ở nền.

## 7. Highlight Rule và Marker

- [x] 7.1 Paint của Highlight Rule lưu 1 byte mỗi dòng của view (`u8` số thứ tự rule), tính từ `row_ids` cho rule theo Filter Value; Minimap đọc mảng này.
  - Test: chạy lại test của `highlight-rule`.
  - Đã làm: `RowPaints` giữ danh sách paint của các rule và một `u8` mỗi dòng (0 là không có rule), thay cho `Option<Arc<RowPaint>>` (8 byte). Rule theo Filter Value vẫn được so bằng `row_ids`. Tối đa 255 rule đang bật được tô. Minimap tô dòng có rule bằng màu của rule, sau Marker và Match.
- [x] 7.2 Rule có Custom Value: tính cho dòng đang hiện lúc parse vào LRU, phần còn lại lấp bằng Scan.
  - Đã làm: rule nào có cột không có `row_ids` (Custom Value trên `message`, cột quá nhiều giá trị) là rule cần text. `paint_rows_split` tô ngay các dòng của file giữ trong RAM và khoảng 400 dòng quanh dòng trên cùng. Các dòng khác được tô bằng rule theo `row_ids` nếu rule đó đứng trước mọi rule cần text, không thì chờ (byte 0). Scan ở nền (`start_paint_scan`, khối quanh dòng trên cùng trước) đọc các dòng chờ từ đĩa, ghi byte của chúng và cộng số đếm của rule cần text. Đổi rule hoặc dựng lại rows thì Scan cũ bị huỷ; rows dời chỗ trong lúc Scan chạy thì tô lại từ đầu.
  - Chưa làm: cuộn tới vùng Scan chưa tới thì dòng chưa có màu cho tới khi Scan tới đó.
- [x] 7.3 Marker: bảng tra đã sắp xếp `(generation, line) → markers` cho mỗi file; vẽ từ bảng cho dòng đang hiện; kiểm `line_hash` khi dòng được đọc ra; Next/Previous Marker trên bảng.
  - Test: chạy lại test của `text-marker`.
  - Đã làm: `ViewFile` giữ Marker trong `BTreeMap<(Generations ago, line)>`. `markers(row)` tra bằng số dòng và Generation từ `LineMeta`, không đọc dòng. Next/Previous Marker dùng `BoundRows::marked_rows`: vị trí của các dòng có Marker tính từ bảng (`FileEntries::position_of`), rồi đi qua rows của view một lượt, không đọc dòng nào.
  - `line_hash` vẫn được kiểm khi file được đọc xong (`drop_markers_that_no_longer_match`, đọc đúng các dòng có Marker), không kiểm lại mỗi lần dòng vào cache.

- [x] 7.4 (trước là 4.4) Chọn lại một Dormant Tab đọc lại file của nó nhanh nhất có thể. Không giữ Line Index của tab đã rời trong cache: quyết định 13 vẫn giữ (rời tab thì bỏ), người dùng chốt 2026-10-30. Mục tiêu cũ "chuyển tab dưới 50 ms nhờ cache" bỏ.
  - Đo bằng `perf_probe` (bản release, hai Log Space, mỗi cái một file 3.000.000 dòng): chuyển tab trước 7,1 s (Indexing 0,6 s, dựng view 6,45 s).
  - Đã làm: lúc dựng view, độ rộng cột lấy từ `max_chars` mà Indexing đã có (`recompute_widths`). Trước đó `update_from_entries` đọc lại mọi dòng từ đĩa. Sau: chuyển tab 0,69 s, gần hết là Indexing. Mở Log Space mới từ 5,15 s còn 0,71 s; dựng lại view đang mở từ 1,10 s còn 58 ms.

## 8. Raw View và Scratch

- [ ] 8.1 Raw View đọc dòng qua `ViewRows` và LRU, bỏ `ParsedFile::lines`.
  - Test: chạy lại test của `raw-view` (File Section, Search, Marker).
- [x] 8.2 Scratch: Line Index dựng trên bytes của `raw.texts` (`LineSource` trong memory); sửa thì index lại file đó.
  - Test: gõ, dán, xoá dòng giữ đúng Marker và số dòng.
  - Đã làm: file Scratch được đọc bằng `IndexedFile::index_now` trên `BytesSource` (`ensure_scratch_loaded`, `scratch_parsed`), với Log Format và Detected Field của nó. Sửa thì `reparse_edited_scratch_files` index lại text trong memory. Raw View của Scratch vẫn sửa trên `raw.texts`. Marker vẫn theo `line_hash` như trước: dòng đổi text thì Marker bị bỏ khi lưu, Marker không dời theo dòng được dán thêm.

## 9. Live

- [x] 9.1 File local live: bytes mới từ `LocalFileSource` được index thành lô nhỏ, rồi thêm vào Line Index, `row_ids`, `ViewRows`, kết quả Scan đang dùng, và paint của Highlight Rule.
  - Bỏ Live Window cho file local.
  - Đã làm: phần lớn có từ 4b.7 (`IndexedFile::extend`, dòng mới vào Line Index, `row_ids`, view, Filter, Match và paint như dòng live). Thêm: mỗi lần `extend` chỉ index tối đa `EXTEND_BYTES` (4 MiB) dòng trọn vẹn. File lớn thêm hơn một lô thì lô sau được index sau khi UI vẽ (`index_more_soon`, 16 ms), dừng khi Pause và chạy tiếp khi Live lại. Scan đang chạy nhận dòng mới như dòng live: Filter lọc chúng ngay, Search tìm chúng khi Scan xong, paint tô chúng khi thêm vào. Live Window của file local đã bỏ trong change `static-live-log-spaces`.
- [x] 9.2 Phát hiện rotate và truncate bằng kích thước cộng hash 4 KiB đầu và 4 KiB cuối đã index; Generation mới kèm Break. Làm trong change `static-live-log-spaces` (task 2.1, 4.3 và 4.4).
  - Giữ file handle của Generation cũ; dòng có text không còn trên đĩa (sau truncate hoặc ghi đè) bị bỏ, thay cho "no longer on disk" (quyết định của change `static-live-log-spaces`).
  - Test: đổi tên, xoá, `copytruncate`.
  - Đã làm trong `static-live-log-spaces` 4.3 (`Replaced`, `Rewritten`) và 4.4 (`ChainSource`, Generation của file local), handle đóng khi rời tab ở 4.5.
- [x] 9.3 Phát hiện file Imported bị ghi đè (không phải append): index lại và báo người dùng.
  - Đã làm trong `static-live-log-spaces` 2.1 và 2.2: file của Static Log Space bị ghi đè được đọc lại ở nền, giữ Filter; không có notice riêng, chỉ có notice khi Marker bị bỏ (2.4).
- [ ] 9.4 Remote File và Remote Command: Line Index trên bytes của Live Window; Backfill, Load More, Open in full và cắt Live Window cập nhật Line Index.
  - Test: chạy lại test của `live-log` và `remote-log-space`.

## 10. Archive Entry

- [x] 10.1 Giải nén Archive Entry theo luồng vào `<app data>/archive-cache/<hash>/`, mỗi Archive một lượt, giữ `MAX_UNCOMPRESSED_BYTES`; index như file local.
- [x] 10.2 Xoá thư mục khi Log Space bị xoá; quét dọn khi khởi động như `remote::copy::sweep`.
  - Test: tắt ngang giữa chừng.
  - Entry được ghi ra `.part` trước, đủ rồi mới đổi tên; sweep xoá các `.part` và thư mục không còn Log Space nào dùng. Thư mục đặt tên theo đường dẫn, kích thước và thời điểm sửa của Archive, nên Archive đổi thì được giải nén lại. Hiện vẫn parse entry vào `ParsedFile` như file local; đọc qua Line Index sau khi app chuyển sang Indexing.

## 11. Dormant Tab

- [x] 11.1 `needed_files`, `open_space_ids` và `sync_glob_watchers` chỉ tính tab đang chọn.
  - Test: mở app với nhiều tab Remote có Live Glob thì không kết nối Server của tab không được chọn.
    - `sync_glob_watchers` dùng `open_space_ids`, hàm này giờ chỉ trả Log Space của tab đang chọn. Test (`dormant_tab_test`) mới kiểm với file local; chưa có test với Server thật cho Live Glob.
- [x] 11.2 File View chỉ đọc file của nó (`build_view_for` không gọi `ensure_space_loaded` cho cả Log Space).
- [x] 11.3 Rời tab bỏ Line Index, LRU và source live của các file không còn tab đang chọn nào dùng, và đóng mọi file handle của chúng; lưu Scratch trước; giữ session SSH trong `RemoteHub`.
  - Test: memory trả lại sau khi rời tab; quay lại tab Remote không đăng nhập lại.
- [x] 11.4 Quay lại Dormant Tab thì index lại với tiến độ, rồi khôi phục Filter, Focus Tab, Search Expression, vị trí đang xem và Marker.
  - Quay lại tab thì đọc lại file và khôi phục session của tab (test: dòng đang chọn). File local được đọc ở nền (`app::loading`): canvas hiện "Reading the log · 40%" và tab hiện %, rồi view được dựng theo session của tab. Rời tab giữa chừng thì việc đọc dừng. Chưa qua Indexing: vẫn parse toàn bộ vào `ParsedFile`, chỉ là không còn chặn UI.
- [x] 11.5 Tab strip hiện trạng thái Dormant và Indexing (tiến độ).
  - Dormant Tab đã có: chữ nghiêng kèm tooltip, qua `is_dormant_tab`. Tab đang đọc hiện % đọc, từ `AppView::reading`.

## 12. Dọn đường cũ và đo lại

- [ ] 12.1 Xoá `ParsedFile::entries`, `ParsedFile::lines`, `AppView::all_entries` và các đường dựng view cũ (`build_space_view`, `build_file_view`, `timeline::materialize` nếu không còn dùng).
- [ ] 12.2 Cập nhật `examples/mem_probe.rs` và `examples/perf_probe.rs` sang đường mới. Ghi số trước và sau vào design.md:
  - thời gian mở
  - byte/dòng
  - RSS
  - chuyển tab
  - Search
- [ ] 12.3 Chạy toàn bộ `cargo test`, rồi kiểm tra bằng tay trên app thật: mở file 376MB, Search, Filter, Marker, Highlight, rotate, chuyển tab, mở lại app với nhiều tab.
- [ ] 12.4 Viết proposal cho change tiếp theo: "Remote ghi ra cache local".
  - Nội dung: Remote File và Remote Command ghi vào `<app data>/remote-cache/`, rồi được index như file local; Dormant Tab Remote mở lại từ cache.
  - Hai câu hỏi mở cần trả lời: quy tắc dọn cache, và cách đồng bộ theo hash khối qua SFTP.
- [ ] 12.5 Cập nhật CONTEXT.md theo mục "Domain changes" của proposal: thêm Line Index, Indexing, Scan và Dormant Tab; sửa Remote Log Space và Live Window; thêm các quy tắc mới.
