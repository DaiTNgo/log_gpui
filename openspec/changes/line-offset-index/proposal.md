## Why

App lag và tốn rất nhiều memory khi mở file lớn. Số đo trên bản release:

- Một file 376MB (3 triệu dòng) mất 7,5 giây để parse, chạy một luồng trên UI thread, nên cửa sổ đứng suốt thời gian đó.
- RSS lên khoảng 3,1GB, tức khoảng 8 lần kích thước file. Mỗi dòng tốn khoảng 468 byte sau parse, rồi thêm khoảng 389 byte cho bản copy của view.
- Mỗi lần chuyển tab, view lại được dựng lại từ đầu.

Search và Filter thì không chậm: chỉ vài ms trên 1 triệu dòng. Cái giá nằm ở việc parse và giữ **mọi dòng** trong RAM. Các log viewer chuyên dụng (klogg, lnav, less) không giữ text trong RAM: chúng giữ chỉ mục vị trí dòng và đọc phần cần xem từ đĩa. App nên làm theo cách đó.

## What Changes

- **Line Index**: mỗi file local được đọc qua một chỉ mục vị trí dòng có nén, cộng một bản ghi nhỏ cố định cho mỗi dòng (timestamp, level, Parent Line, Generation, cờ) và id giá trị của các cột lọc được.
  - Text của dòng không được giữ trong RAM. Hệ thống chỉ đọc từ đĩa các dòng cần hiển thị, và giữ một bộ đệm nhỏ các dòng đã parse quanh vị trí đang xem.
  - Mục tiêu: không quá khoảng 50 byte RAM mỗi dòng.
- **Indexing chạy ở nền, có tiến độ**: mở file không còn chặn UI.
  - Các dòng đầu tiên hiện ngay khi được index, phần còn lại được index tiếp. View đọc và cuộn được trong lúc đó.
  - Tab hiện tiến độ indexing; người dùng có thể huỷ.
- **Mọi thao tác cần đọc text đều chạy ở nền, có tiến độ, kết quả hiện dần**: Search, Filter Custom Value, Highlight Rule có Custom Value, bật Detected Field mới, đổi Log Format.
  - Không thao tác nào chặn UI. Kết quả không cần tức thì, kể cả với file nhỏ.
  - Thao tác không cần đọc text (tick Filter Value, Time Range, Highlight Rule theo Filter Value, Marker) vẫn tức thì như hiện tại.
- **Một đường cho mọi cỡ file**: không giữ đường cũ "file nhỏ nằm trong RAM". Mọi file local, Archive Entry và Remote Copy đều đi qua Line Index.
- **Archive Entry**: được giải nén ra file tạm trong thư mục dữ liệu của app rồi index như file local, thay vì giữ nội dung giải nén trong RAM.
- **Log Space gộp nhiều file**: thứ tự theo timestamp được tính trên chỉ mục (một danh sách `(file, dòng)` gọn) và được gộp dần khi index hoặc khi có dòng mới, không dựng lại từ đầu.
- **Live tail của file local**: dòng mới được thêm vào cuối Line Index. Rotate và truncate được nhận ra bằng kích thước file cộng hash đoạn đầu và đoạn cuối đã index.
- **Dormant Tab**:
  - Khi mở app, chỉ tab đang chọn được đọc. Các tab khác ở trạng thái Dormant cho đến khi người dùng bấm vào.
  - File View chỉ đọc file của chính nó.
  - Live Glob chỉ được theo dõi cho các tab đang được dùng.
  - Rời một tab thì bỏ dòng của nó khỏi memory, nhưng giữ kết nối SSH của Server.
- **Chuyển tab** không còn clone toàn bộ dòng hay dựng lại chỉ mục Filter.
- Bước đầu tiên là một **spike** để đo Line Index trên file 376MB. Nó có cổng go/no-go; phần còn lại chỉ làm khi số đo đạt mục tiêu (xem design.md).

Ngoài phạm vi của change này, để dành cho change sau:
- Remote File vẫn dùng Live Window như hiện tại. Mirror Remote File về cache local rồi index theo kiểu lnav là change sau.
- Index trực tiếp trên `.gz` (kiểu zran) cũng để sau. Change này chỉ giải nén ra file tạm.

## Domain changes

Change này không sửa CONTEXT.md hay ADR vì các session khác đang làm song song. Khi áp dụng change, cần cập nhật CONTEXT.md như sau.

**Thuật ngữ mới (nhóm Views / Spaces):**
- **Line Index**: thứ app giữ cho một file thay vì nội dung của nó. Nó gồm vị trí của từng dòng trong file, và với mỗi dòng: timestamp, level, Parent Line, Generation và giá trị của các cột lọc được. Text của dòng được đọc lại từ file khi cần hiện. _Avoid_: cache, buffer, preload.
- **Indexing**: việc đọc một file lần đầu để dựng Line Index. Nó chạy ở nền, có tiến độ và huỷ được; các dòng đã index hiện ngay. _Avoid_: loading, parsing, import.
- **Scan**: việc đọc lại text của mọi dòng ở nền cho một thao tác cần text (Search, Custom Value, Detected Field mới, đổi Log Format). Kết quả hiện dần, có tiến độ. _Avoid_: search job, background search.
- **Dormant Tab**: một tab mà dòng của nó chưa được đọc, hoặc đã được bỏ khỏi memory khi người dùng rời tab. Nó giữ Filter, Focus Tab, vị trí cuộn và Marker. Bấm vào thì tab được đọc lại. _Avoid_: unloaded tab, sleeping tab, lazy tab.

**Thuật ngữ đổi nghĩa:**
- **Remote Log Space**: "keep receiving new lines while the Log Space is open" đổi thành "while one of its tabs is the open tab". Rời tab thì dừng nhận dòng, nhưng giữ kết nối tới Server.
- **Live Window**: chỉ còn áp cho Remote File và Remote Command. File local được follow live không còn Live Window, vì text của nó nằm trên đĩa và Line Index chỉ tốn vài chục byte mỗi dòng.

**Quy tắc mới (số sẽ được cấp khi áp dụng):**
- **Reading Never Blocks**: mở file, Search, Filter Custom Value, Highlight Rule có Custom Value, Detected Field mới và đổi Log Format đều chạy ở nền, có tiến độ. Kết quả hiện dần; view luôn đọc và cuộn được.
- **Text Stays On Disk**: app không giữ text của mọi dòng trong memory. Chỉ các dòng đang hiện và một bộ đệm nhỏ quanh chúng được giữ.
- **Only The Open Tab Reads**: khi app mở, chỉ tab đang chọn được đọc. Rời một tab thì dòng của nó được bỏ khỏi memory; quay lại thì đọc lại.

## Capabilities

### New Capabilities
- `line-index`: đọc file qua Line Index; Indexing ở nền, có tiến độ và huỷ; dòng đã index hiện ngay; giới hạn memory mỗi dòng; live tail thêm vào chỉ mục; nhận ra rotate và truncate; Archive Entry giải nén ra file tạm.
- `progressive-scan`: Search, Filter Custom Value, Highlight Rule có Custom Value, Detected Field mới và đổi Log Format chạy như Scan ở nền với tiến độ và kết quả hiện dần; huỷ Scan cũ khi người dùng đổi truy vấn.
- `dormant-tab`: chỉ đọc tab đang chọn khi mở app; File View chỉ đọc file của nó; Live Glob chỉ theo dõi cho tab đang dùng; rời tab thì bỏ dòng nhưng giữ kết nối SSH; tab hiện trạng thái Dormant và Indexing.

### Modified Capabilities
- `log-filter`: "Phát hiện cột và giá trị khi nạp file" đổi thành phát hiện dần trong lúc Indexing, nên danh sách giá trị và số đếm tăng dần; Custom Value chạy như một Scan.
- `log-search`: "Search khớp trên toàn bộ dòng log" thêm việc Match hiện dần trong lúc Scan, kèm tiến độ và tổng số Match tạm thời.
- `log-space`: "Tab theo view" thêm Dormant Tab. Một tab có thể nằm trên tab strip mà dòng của nó chưa được đọc.

## Impact

- **Mã chịu ảnh hưởng chính:**
  - `src/space_view.rs`: `ParsedFile`, `build_space_view`, `build_file_view`
  - `src/parser/text.rs`
  - `src/filter_index.rs`, `src/filter.rs`
  - `src/search.rs`, `src/search_lines.rs`
  - `src/timeline.rs`
  - `src/highlight.rs`
  - `src/live/buffer.rs`, `src/live/local.rs`
  - `src/archive.rs`
  - `src/app/tabs.rs`, `src/app/live.rs`, `src/app/globs.rs`, `src/app/raw.rs`, `src/app/highlights.rs`
  - `src/ui/canvas.rs` (minimap, vẽ dòng)
- **Thứ thay đổi:** `LogEntry` không còn là dạng lưu của mọi dòng. Nó chỉ còn là dạng của các dòng đang hiện, nên mọi chỗ duyệt `all_entries` phải đổi.
- **Phụ thuộc mới có thể có:** `memchr`, một crate bitmap (`roaring`), một crate nén số nguyên (`stream-vbyte` hoặc `bitpacking`). Chọn trong spike.
- **Test:** các test dựa trên `all_entries` hoặc `ParsedFile::entries` phải viết lại theo truy vấn trên Line Index. `examples/mem_probe.rs` và `examples/perf_probe.rs` được dùng làm số đo trước và sau.
- **Dữ liệu trên đĩa:** thêm file tạm cho Archive Entry trong thư mục dữ liệu của app, dọn khi Log Space đóng hoặc khi app khởi động. Session JSON không đổi format.
