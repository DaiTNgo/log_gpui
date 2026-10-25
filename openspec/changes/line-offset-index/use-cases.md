# Use case và cách tối ưu performance, memory

Tài liệu này đối chiếu từng use case của app giữa cách làm hiện tại và cách làm sau change `line-offset-index`. Change chưa được hiện thực. Các con số ở cột "sau" là mục tiêu và sẽ được spike (nhóm task 1) đo lại; xem quyết định 14 trong design.md.

## Ý tưởng chung

Hiện tại mỗi dòng được parse thành một `LogEntry` (240 byte) và giữ trong RAM. Theo số đo, mỗi dòng tốn khoảng 468 byte sau khi parse, cộng thêm khoảng 389 byte cho bản clone của view.

Sau change, RAM chỉ giữ chỉ mục và metadata:

```
Line Index   : vị trí dòng, nén theo khối 128 dòng   ~2–4 B/dòng
LineMeta     : ts, level, Parent Line, cờ            ~16–24 B/dòng
row_ids      : id giá trị Filter (u16 mỗi cột)       ~2 B/cột/dòng
──────────────────────────────────────────────────────────────
Text dòng    : nằm trên đĩa, đọc bằng pread khi cần hiện
LRU          : ~4096 dòng đã parse quanh vị trí đang xem
```

Mục tiêu là không quá 50 byte mỗi dòng. Với file 3 triệu dòng, RSS giảm từ khoảng 3,1GB xuống khoảng 150MB.

## A. Mở app và tab

| Use case | Hiện tại | Sau change |
|---|---|---|
| Mở lại app có nhiều tab | Đọc file của mọi tab. Glob watcher kết nối SSH cho mọi tab | Chỉ tab đang chọn được đọc; các tab khác là Dormant Tab. `needed_files` và `sync_glob_watchers` chỉ xét các tab đang dùng |
| Bấm vào một Dormant Tab | Chưa có khái niệm Dormant Tab | Indexing chạy ở nền. Các dòng đầu hiện ngay (mục tiêu ≤ 300ms), tab hiện tiến độ |
| Chuyển qua lại giữa các tab | `build_view_for` clone mọi dòng và dựng lại `FilterIndex` ở mỗi lần chuyển | Không clone. File View là khoảng `0..n`; Log Space là một `Vec<u64>` đã gộp sẵn |
| Rời một tab | Dòng ở lại trong RAM | Bỏ dòng và đóng file handle. Filter, vị trí cuộn, Marker và kết nối SSH của Server được giữ |
| Mở nhiều tab, nhiều file | Memory tăng theo tổng kích thước các file | Chỉ tab đang mở tốn RAM. File handle được mở khi cần; app báo người dùng trước khi chạm giới hạn file descriptor (`RLIMIT_NOFILE`, mặc định 256 trên macOS) |

## B. Đọc file

| Use case | Hiện tại | Sau change |
|---|---|---|
| Mở file local 376MB | Parse một luồng trên UI thread: 7,5s, cửa sổ đứng | Indexing ở nền theo khối 4MiB, tìm ký tự xuống dòng bằng `memchr`, worker song song, gửi lô mỗi 50ms, huỷ được. Mục tiêu ≤ 2s và UI không đứng |
| Nhận dạng format từng dòng | Thử mọi mẫu ở mỗi dòng | Khoá format: sau khoảng 64 dòng liên tiếp khớp một mẫu thì chỉ thử mẫu đó, gặp dòng không khớp thì dò lại từ đầu (học từ lnav) |
| Cuộn và vẽ dòng | Đọc từ `Vec<LogEntry>` | Đọc các dòng đang hiện bằng `pread`, parse rồi giữ trong LRU. Mục tiêu một màn hình 60 dòng ≤ 2ms |
| Log Space gộp nhiều file theo thời gian | `timeline::materialize` dựng lại toàn bộ | Gộp k-way tăng dần trên các cặp `(file, dòng)` khi có dòng mới được index. Mục tiêu 3 file ≤ 300ms |
| Archive (.gz, .zip) | Giải nén vào RAM, tối đa 2GB | Giải nén ra `<app data>/archive-cache/` rồi index như file local |
| Scratch | Giữ trong RAM | Vẫn giữ trong RAM, nhưng đi qua một `LineSource` dạng bytes, cùng đường index với mọi loại file khác |

## C. Thao tác không cần đọc text: vẫn tức thì

| Use case | Kỹ thuật |
|---|---|
| Tick hoặc bỏ tick Filter Value | So `row_ids` (u16) của dòng với tập id được chọn, không đọc text. Hiện đo được 4–7ms |
| Time Range | So timestamp có sẵn trong `LineMeta` (i64, đơn vị µs) |
| Highlight Rule theo Filter Value | Dựa trên `row_ids`. Màu tô lưu một u8 cho mỗi dòng |
| Marker | Lưu `StoredMarker` theo `(dòng, Generation)`. Chỉ tra cho các dòng đang hiện và kiểm lại bằng `line_hash` khi đọc |

## D. Thao tác cần đọc text: chạy ở nền, có tiến độ

Mỗi thao tác dưới đây là một Scan. Scan đọc lại text từ đĩa theo chunk 10k dòng và quét vùng gần vị trí đang xem trước. Kết quả hiện dần kèm tổng tạm thời. Khi người dùng đổi truy vấn, Scan cũ bị huỷ.

| Use case | Kết quả lưu ở |
|---|---|
| Search | Tập dòng khớp: danh sách đã sắp xếp hoặc Roaring bitmap. Mục tiêu ≤ 1s khi file đã nằm trong page cache |
| Filter Custom Value | Bitmap các dòng khớp |
| Highlight Rule có Custom Value | Tô ngay các dòng đang hiện; phần còn lại được tính bằng Scan ở nền |
| Bật Detected Field mới | `row_ids` của cột đó, dựng bằng Scan. Áp dụng nếu spike chọn phương án (b) của quyết định 4 |
| Đổi Log Format | `LineMeta` và `row_ids` được dựng lại bằng Scan |

## E. Live và Remote

| Use case | Hiện tại | Sau change |
|---|---|---|
| Follow live file local | Live Window; một lần trim mất 86ms | Không còn Live Window: dòng mới được thêm vào cuối Line Index |
| Rotate, truncate | Xử lý theo Generation | Nhận ra bằng kích thước file cộng hash của 4KiB đầu và 4KiB cuối đã index. Mỗi Generation giữ file handle riêng |
| Live Glob | Theo dõi cho mọi tab | Chỉ theo dõi cho tab đang dùng |
| Remote File, Remote Command | Live Window trong RAM; `RemoteHub` giữ một session SSH cho mỗi Server | Trong change này vẫn dùng Live Window. Rời tab thì dừng nhận dòng nhưng giữ session SSH. Change sau (task 12.4) ghi dữ liệu ra `<app data>/remote-cache/` rồi index như file local, theo cách của lnav |

## Chưa giải quyết trong change này

- Remote File vẫn giữ text trong RAM, nhưng chỉ trong giới hạn của Live Window.
- File `.gz` phải được giải nén ra đĩa trước khi index. Index trực tiếp trên `.gz` (kiểu zran) để cho change sau.
- Scan trên file chưa có trong page cache chậm hơn và phụ thuộc tốc độ ổ đĩa. Spike đo cả trường hợp page cache đã có và chưa có file.
