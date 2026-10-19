## Context

`add-remote-live-log` đã có:
- `RemoteFileSource` (`src/live/remote.rs`): đọc SFTP theo byte offset, nạp ~1.000 dòng cuối rồi nạp ngược theo khối 64 KB (`BackwardLines`, `src/live/backward.rs`) tới khi đầy Live Window, poll 500 ms để lấy dòng mới, nhận biết xoay vòng qua inode, lấy số dòng thật bằng `wc -l` qua exec.
- `SourceEvent` gồm `Tail`, `Backfill`, `Append`, `Break`, `Status`. Generation đếm trong bộ nhớ, Marker lưu Generation dưới dạng khoảng cách tới file hiện tại (0 = hiện tại, 1 = trước đó).
- Download qua SFTP (`src/remote/download.rs`).

`src/archive.rs` đã đọc `.gz` bằng `flate2` (nhận biết bằng byte đầu, giới hạn 2 GB sau giải nén). `read_timestamp` (`src/parser/text.rs`) đọc mọi dạng timestamp parser biết, theo đồng hồ của file. `parse_time_bound` (`src/filter.rs`) đọc thời điểm người dùng gõ cho Time Range.

Đo trong `add-remote-live-log`: mỗi dòng tốn khoảng **1.320 byte** bộ nhớ (cả bản sao của view), 1.000.000 dòng ≈ 1,5–2,2 GB. Vì vậy đọc trọn một file vài GB bằng cách nạp hết vào bộ nhớ là không được.

Ba yêu cầu (đọc Rotated File, đọc trọn file, đọc từ một thời điểm) mâu thuẫn với L4 (chỉ nạp trong Live Window), L10 (Live chỉ là một cửa sổ, muốn đọc trọn thì Download) và L14 (chỉ tải file hiện tại). Design này giải quyết mâu thuẫn bằng cách tách hai cách đọc: **Live** giữ nguyên Live Window, còn đọc trọn đi qua **Remote Copy** tĩnh trong Imported Log Space.

## Goals / Non-Goals

**Goals:**
- Đọc Rotated File (kể cả file nén gzip, zstd, xz, bzip2) như các Generation cũ hơn của cùng Remote File, ở cả Live và Open in full.
- Đọc trọn một Remote File bằng một lệnh, không qua chọn thư mục Download rồi Import.
- Bắt đầu đọc từ một thời điểm, tìm vị trí bằng timestamp chứ không đọc cả file.
- Bộ nhớ có giới hạn rõ ràng, người dùng biết trước khi đọc.

**Non-Goals:**
- Bỏ Live Window, hay cho Remote Log Space giữ dòng không giới hạn.
- Kiểu nén ngoài gzip, zstd, xz và bzip2 (ví dụ `.lz4`, `.Z`).
- Đọc theo trang (paging) phần giữa file mà không nạp vào bộ nhớ, hay chỉ mục trên đĩa cho file nhiều GB.
- Remote Copy tự cập nhật theo file trên Server. Muốn có dữ liệu mới thì dùng Read again hoặc mở Live.
- Theo dõi Rotated File sinh ra sau này trong một Remote Copy.
- Lệnh trên server ngoài những gì đã dùng (`stat`, `wc -l`, `head -c`). Không cần `zcat` hay `grep` trên server.

## Decisions

### 1. Đọc trọn đi qua Remote Copy trong Imported Log Space, không thêm loại Log Space thứ tư
"Open in full" đọc file từ Server vào thư mục dữ liệu của app (`remote-copies/<id>/`) và thêm nó vào một Imported Log Space như một loại file thứ ba, bên cạnh file trên đĩa và Archive Entry.
- Lệnh chạy trên một Remote File thì tạo Imported Log Space mới tên `<space> (full)`, hoặc thêm vào một Imported Log Space có sẵn do người dùng chọn trong dialog.
- Lệnh chạy trên cả Remote Log Space thì tạo một Log Space chứa Remote Copy của mọi file.

Imported Log Space vốn đã là "tĩnh, chỉ đọc, không có Live Window", và Archive Entry đã là tiền lệ cho "file đọc vào từ nguồn khác, không nằm trong thư mục của người dùng". Nhờ vậy Filter, Search, Marker, Focus Pane, persist và mở lại Project dùng được ngay, và L10 vẫn đúng: Remote Log Space vẫn chỉ là một cửa sổ.

*Thay thế đã cân nhắc:*
- **Loại Log Space thứ tư ("Remote Snapshot Log Space")**: bị loại. Phải lặp lại gần hết hành vi của Imported Log Space, và rule 24 thêm một nhánh.
- **Chế độ "full" cho từng Remote File trong Remote Log Space**: bị loại. Một Log Space vừa có file live (bị Live Window cắt) vừa có file tĩnh (không cắt) làm Follow, Pause, L12 và số đếm Filter khó hiểu.
- **Chỉ giữ trong bộ nhớ, không lưu đĩa**: bị loại. Mở lại Project sẽ phải tải lại, mà file trên Server có thể đã xoay vòng.

### 2. Rotated File là Generation cũ hơn, không phải Source File riêng
Rotated File được hiện phía trên file hiện tại trong cùng Source File, mỗi cái một Generation, ngăn cách bằng Break "rotated".

Lý do chọn cách này:
- Generation đã có nghĩa đúng như vậy: "đoạn dòng giữa hai Break xoay vòng".
- Khoảng cách Generation đã lưu cho Marker (0 = hiện tại, 1 = trước đó) khớp đúng với `app.log`, `app.log.1`, `app.log.2.gz`.
- Khi file live xoay vòng, Generation trong bộ nhớ trở thành `app.log.1` trên đĩa mà không phải đổi tên gì.
- Sidebar và Source Column không bị phình thêm một dòng cho mỗi file cũ.

*Thay thế đã cân nhắc:* mỗi Rotated File là một file riêng (`web1:/app/app.log.2.gz`). Cách này dễ cài hơn, và xếp xen theo timestamp vẫn đúng. Bị loại vì Marker của dòng đổi chủ mỗi lần logrotate chạy, và người dùng thấy một "file" chỉ là quá khứ của file khác.

### 3. Tìm và xếp Rotated File
`src/remote/rotated.rs`: đọc thư mục cha qua SFTP `readdir`, lấy các tên khớp một trong các mẫu sau với `<name>` là tên của Remote File:
- `<name>.<N>`
- `<name>-<YYYYMMDD>` hoặc `<name>-<YYYYMMDD><HH><MM><SS>` (logrotate `dateext`)
- `<name>-<YYYY-MM-DD>` hoặc `<name>.<YYYY-MM-DD>`
- mỗi mẫu có thể kèm `.gz`, `.bz2`, `.xz` hoặc `.zst`

Xếp theo `mtime` giảm dần; hai file cùng `mtime` thì xếp theo số hoặc ngày trong tên. Khoảng cách Generation là vị trí trong danh sách này (1, 2, …).

Kiểu nén được xác định bằng byte đầu, không theo đuôi tên, giống Archive. Đuôi tên chỉ dùng để nhận ra Rotated File.

| Kiểu | Byte đầu | Crate | Ghi chú |
|---|---|---|---|
| gzip | `1f 8b` | `flate2` (đã có, backend `miniz_oxide`) | `MultiGzDecoder` để đọc file nhiều member |
| zstd | `28 b5 2f fd` | `ruzstd` | thuần Rust, chỉ giải nén; chậm hơn `zstd` (bản C) khoảng 2–3 lần nhưng vẫn nhanh hơn mạng |
| xz | `fd 37 7a 58 5a 00` | `lzma-rs` (`xz_decompress`) | thuần Rust; chậm hơn `xz2`/`liblzma` (bản C), chấp nhận được vì chỉ đọc |
| bzip2 | `42 5a 68` (`BZh`) | `bzip2` ≥ 0.5 với backend mặc định `libbz2-rs-sys` | thuần Rust, cùng API với bản C |

Chọn crate thuần Rust để không phải build thư viện C và giữ một toolchain cho mọi máy. Bộ giải mã nằm sau một hàm `decoder(kind, reader) -> Box<dyn Read>` trong `src/remote/compress.rs`; `archive.rs` có thể dùng chung sau này. File có đuôi nén mà byte đầu không khớp kiểu nào được đọc như file thường. Byte đầu khớp một kiểu khác (ví dụ `.lz4`) thì hiện nhãn "compression not supported" và không chọn được.

Mọi lần giải nén chạy trên luồng `spawn_blocking` của tokio runtime trong `RemoteHub`, đọc dữ liệu nén qua một cầu nối `Read` trên SFTP (đọc trước theo khối 1 MB), để không chặn các file khác.

Danh sách được đọc lúc mở dialog Open in full hoặc Download, khi Live nạp hết file hiện tại, và lúc bấm "Load earlier generation". Nó không được theo dõi liên tục.

### 4. Live: tự nạp Rotated File mới nhất không nén, các file khác khi người dùng chọn
Khi nạp dần của một Remote File live chạm byte 0 của file hiện tại mà Live Window của file còn chỗ (và chưa chạm Starting Time), `RemoteFileSource` liệt kê Rotated File:
- **Tự nạp:** nếu Rotated File mới nhất (khoảng cách 1, ví dụ `app.log.1` hay `app.log-20261018`) không nén theo byte đầu, nguồn tự đi tiếp: phát `Break { kind: Rotated }` rồi `Backfill { generation: g-1, … }` từ cuối file đó bằng `BackwardLines`, cho tới khi đầy Live Window, chạm Starting Time hoặc hết file. Đây là trường hợp ngay sau logrotate: `app.log` mới chỉ có vài trăm dòng.
- **Không tự nạp tiếp nữa:** chỉ một Rotated File được tự nạp. Rotated File cũ hơn, hoặc Rotated File mới nhất nếu nó nén, chỉ được nạp khi người dùng bấm **Load earlier generation** (`app.log.2.gz, 38 MB`) ở Break đầu phần dòng của file. Lý do: file nén phải giải nén cả file mới lấy được dòng cuối (tốn băng thông và CPU), còn file cũ hơn hiếm khi cần.
- Bấm nút thì `RemoteFileSource` nhận lệnh `LoadEarlier` và nạp Rotated File kế tiếp theo cùng cách.
- **File nén** không đọc ngược được, nên được giải nén theo luồng từ đầu, chỉ giữ K dòng cuối trong một ring buffer (K = chỗ còn trống trong Live Window), có tiến độ trên status bar. Tốn băng thông bằng cả file nén nhưng bộ nhớ có giới hạn.
- Khi Live Window đã đầy thì nút đổi thành **Open in full…**.
- Tự nạp chỉ xảy ra trong lần nạp dần lúc mở file. Khi file live bị xoay vòng trong lúc mở thì không cần, vì Generation cũ đã nằm trong bộ nhớ. Tự nạp không bao giờ làm dòng của Generation hiện tại rời window, vì nó chỉ lấp phần còn trống.

Số dòng của Rotated File thường lấy bằng `wc -l` qua exec như file hiện tại. Với file nén, số dòng đếm được trong lúc giải nén, nên luôn chính xác.

### 5. Starting Time: định nghĩa và cách đọc ô nhập
Ô "Start from" chấp nhận:
- **Khoảng tương đối**: `30m`, `1h`, `2h30m`, `2d`. Được tính bằng `T = ts_newest − khoảng`, trong đó `ts_newest` là timestamp của dòng có timestamp mới nhất trong khối cuối của file hiện tại (đọc khối 64 KB cuối, như Tail).
- **Thời điểm tuyệt đối**: mọi dạng `parse_time_bound` chấp nhận. Giờ không kèm ngày (`10:42`) được lấy theo ngày của `ts_newest`; nếu kết quả lớn hơn `ts_newest` thì lùi một ngày.

Tính theo đồng hồ của file (L25, L8). Vì vậy log ghi giờ UTC trên server ở múi giờ khác vẫn cho đúng "1 giờ cuối của log".

File không có timestamp nào trong khối cuối: ô Starting Time bị tắt kèm lý do "no timestamps found".

### 6. Starting Time khi Live: nạp ngược dừng theo timestamp
Không cần tìm nhị phân: Tail + Backfill đã đọc ngược.
- `BackwardLines` thêm điều kiện dừng `stop_before: Option<f64>`. Mỗi lô, dòng được đọc timestamp (chỉ Parent Line). Khi gặp Parent Line có `ts < T` thì cắt lô ở ngay sau dòng đó và báo xong.
- Continuation Line của Parent Line cuối cùng được giữ cùng nó. Dòng không có timestamp nằm giữa hai dòng `≥ T` vẫn được giữ.
- Tail đầu tiên (~1.000 dòng) cũng chịu `stop_before`: nếu 1.000 dòng cuối đã cũ hơn T thì chỉ hiện các dòng `≥ T`, có thể là 0 dòng kèm thông báo "no lines since …".
- Nếu Starting Time cũ hơn dòng đầu của file hiện tại, nạp dần đi tiếp vào Rotated File như quyết định 4 (tự động nếu là file mới nhất không nén, còn lại qua nút Load earlier generation), và lần nạp đó cũng dừng ở T.
- Live Window vẫn thắng: nếu số dòng kể từ T vượt Live Window thì nạp dừng ở Live Window, và Break đầu file ghi "N lines since <T> do not fit the Live Window — Open in full…".

Timestamp không tăng đều (lệch vài giây giữa các luồng ghi): dừng ở Parent Line đầu tiên có `ts < T − 1s`, chấp nhận vài dòng cũ hơn T một chút thay vì dừng sớm.

### 7. Open in full: tìm vị trí byte theo timestamp, rồi đọc xuôi
`src/remote/full_read.rs`, chạy trên tokio runtime của `RemoteHub`. Với mỗi Generation đã chọn (file hiện tại và Rotated File):

1. **File thường có Starting Time:** tìm nhị phân trên khoảng byte `[lo, hi]`.
   - Đọc khối 64 KB tại `mid`, bỏ dòng dở đầu khối, lấy Parent Line đầu tiên có timestamp.
   - Khối không có timestamp nào thì mở rộng khối (tối đa 1 MB), rồi coi như "chưa biết" và dịch `mid` lên.
   - Dừng khi `hi − lo ≤ 64 KB`, rồi quét xuôi trong khối đó để tìm Parent Line đầu tiên có `ts ≥ T`, lùi lại để lấy cả Continuation Line phía trước nếu nó thuộc Parent Line đó.
   - Khoảng 15–20 lần đọc cho file 1 GB.
   - Mốc "To" cũng tìm theo cách đó để có byte kết thúc.
2. **File nén:** giải nén theo luồng từ đầu, bỏ dòng có `ts < T`, dừng khi gặp `ts > To`.
3. **Chọn Generation theo thời gian:** Rotated File có `mtime < T` nằm hẳn trước Starting Time nên bị bỏ chọn sẵn trong dialog (người dùng vẫn tích lại được). File hiện tại có dòng đầu `> To` cũng vậy.
4. **Đọc xuôi** từ byte bắt đầu tới byte kết thúc (hoặc tới kích thước file đo lúc bắt đầu, vì Remote Copy là tĩnh), khối 1 MB. Mỗi khối được:
   - ghi vào `remote-copies/<id>/gen-<k>.log` (đã giải nén);
   - gửi `Append` cho view qua cùng kênh `SourceEvent` như file live, để dòng hiện dần.

   Các Generation được đọc từ cũ tới mới, nên dòng đến đúng thứ tự thời gian và đi được nhánh nối thẳng vào cuối của `append_to_open_view`.
5. **Số dòng thật khi bắt đầu giữa file:** chạy `head -c <offset> <path> | wc -l` qua exec song song với việc đọc. Trong lúc chờ, Gutter để trống như cách xử lý `wc -l` hiện nay. Kết quả lưu vào `meta.json` thành `first_line_no` của Generation. Không có exec thì Gutter ghi "line numbers unavailable" và đánh số từ 1 kèm dấu `~`. Với file nén thì số dòng đếm được khi giải nén.

*Thay thế đã cân nhắc:* đọc ngược từ cuối tới T như Live. Bị loại cho Open in full: nội dung phải ghi ra đĩa và vào view theo thứ tự xuôi, mà đọc ngược thì phải đệm cả đoạn rồi đảo, và không có mốc "To".

### 8. Remote Copy lưu trên đĩa
```
remote-copies/<uuid>/
  meta.json       # server_id, path, read_at, generations: [{distance, source_name, compressed, start_offset, end_offset, first_line_no, from, to, complete}]
  gen-0.log       # file hiện tại (hoặc đoạn đã chọn), text đã giải nén
  gen-1.log       # app.log.1
  gen-2.log       # app.log.2.gz đã giải nén
```
- Mở lại Project thì `ParsedFile` đọc các `gen-*.log` từ cũ tới mới, ghép lại với Break "rotated" ở giữa, và đánh số dòng từ `first_line_no`.
- Không cần kết nối, và không chịu L6 hay trạng thái kết nối trên status bar.
- Gỡ Remote Copy khỏi Log Space, hay xoá Log Space chứa nó, thì xoá thư mục. Khi app khởi động, thư mục không còn được session nào tham chiếu sẽ bị dọn.
- **Read again**: đọc lại vào một thư mục mới với cùng lựa chọn (Generation, Starting Time, To). Xong thì thay thư mục cũ. Marker được khôi phục theo `(khoảng cách Generation, số dòng, line_hash)` như hiện nay, nên Marker của dòng đã đổi thì bị bỏ.
- Tên hiện của Remote Copy là `<server>:<path>`, cùng tên với Remote File, kèm biểu tượng "copy" ở sidebar. Tooltip ghi thời điểm đọc và khoảng thời gian đã đọc.
- Persist: `FileSource` của workspace thêm biến thể `RemoteCopy { dir }`.

### 9. Full Read Limit và ước lượng trước khi đọc
- Setting `full_read_limit_lines`, mặc định **2.000.000** (≈ 2,6 GB với 1.320 byte/dòng đã đo). Áp dụng cho một lần Open in full, cộng mọi Generation và mọi file của lần đó.
- **Ước lượng** trước khi đọc, ghi trong dialog:
  - độ dài trung bình một dòng lấy từ khối cuối (đã đọc cho Starting Time);
  - số byte cần đọc của từng Generation, sau khi đã tìm nhị phân;
  - với file nén, kích thước sau giải nén lấy từ chính file khi có:
    - gzip: 4 byte cuối (ISIZE, mod 4 GB);
    - zstd: trường Frame Content Size ở đầu frame, nếu được ghi;
    - xz: đọc Stream Footer và Index ở cuối file (vài KB), cộng uncompressed size của các block;
    - bzip2: không có, đoán gấp 8 lần kích thước nén.

    Giá trị đọc được nhỏ hơn kích thước nén thì cũng dùng cách đoán gấp 8 lần.

  Tổng được hiện thành "≈ 3,4M lines (290 MB)".
- **Vượt giới hạn:** nút Open bị thay bằng ba lựa chọn:
  - **Narrow time…**: con trỏ nhảy về ô Starting Time/To.
  - **Read newest 2M lines**: Starting Time được thay bằng byte bắt đầu ước lượng sao cho phần mới nhất vừa giới hạn, rồi bỏ các Generation cũ không còn chỗ.
  - **Cancel**.
- **Chặn cứng khi đọc:** ước lượng có thể sai, nên `full_read.rs` đếm dòng thật. Khi chạm giới hạn thì dừng, đánh dấu Generation `complete: false`, vẽ Break "stopped at Full Read Limit (2,000,000 lines)" ở cuối phần đã đọc, và gợi ý Narrow time. Không cắt mà không báo (L26).
- **Đĩa:** trước khi đọc, kiểm tra dung lượng trống của ổ chứa thư mục dữ liệu app. Nếu nhỏ hơn 1,2 lần số byte ước lượng thì báo lỗi.
- Giới hạn 2 GB sau giải nén của `archive.rs` không dùng ở đây; Full Read Limit thay thế nó cho Remote Copy.

### 10. Tiến độ, huỷ, Pause
- Status bar của Imported Log Space đang có Remote Copy đọc dở hiện "Reading web1:/app/app.log 45% (120 / 260 MB)" kèm nút Cancel. Trạng thái kết nối dùng chung mã của L7.
- **Cancel** giữ phần đã đọc, đánh dấu `complete: false`, vẽ Break "read cancelled", và có Read again.
- **Pause** của Log Space dừng việc đọc Remote Copy và giữ offset. Tiếp tục thì đọc tiếp, giống L1.
- **Mất kết nối** khi đang đọc: thử lại như L6 và đọc tiếp từ offset. File đã xoay vòng trong lúc đó (inode đổi) thì đọc nốt bằng handle cũ nếu còn mở. Nếu handle đã mất thì dừng với Break "file rotated while reading".
- Log Space tạo bởi Open in full mở với Follow tắt, và view ở dòng đầu tiên được đọc (đổi L3). Người dùng thường đọc từ Starting Time trở đi, không phải từ cuối.

### 11. Download kèm Rotated File
Dialog Download / Download all thêm danh sách Rotated File (cùng `rotated.rs`), mặc định không tích file nào. File tải về giữ nguyên tên và dạng nén. Nếu tên trùng thì dùng tiền tố Server như hiện nay (`web1-app.log.2.gz`). Khi mở thành Imported Log Space, file `.gz` đi qua `archive.rs` như Archive một file; file zstd, xz và bzip2 cũng được nhận như Archive một file, nhờ `archive.rs` dùng chung `decoder()` của quyết định 3 (thêm `ArchiveKind` cho ba kiểu này). Download không đổi theo Generation: nó là việc của người dùng muốn giữ file.

## Risks / Trade-offs

- **[Ước lượng sai với file nén]**: ISIZE mod 4 GB sai với gzip giải nén lớn hơn 4 GB; zstd có thể không ghi Frame Content Size; bzip2 chỉ đoán. → Chặn cứng khi đọc (quyết định 9) vẫn giữ giới hạn. Dialog ghi "estimate" cho file nén.
- **[Crate giải nén thuần Rust chậm hơn bản C]**: `lzma-rs` và `ruzstd` chậm hơn `liblzma`/`zstd`. → Thường vẫn nhanh hơn tốc độ tải qua SSH; đo trong task 7.2. Nếu chậm thật thì đổi sang bản C sau `decoder()` mà không đổi gì khác.
- **[Tự nạp Rotated File tốn băng thông]**: `app.log.1` có thể vài GB. → Chỉ đọc ngược phần vừa Live Window, như file hiện tại; không bao giờ tự giải nén file nén.
- **[Tìm nhị phân trên timestamp không tăng đều]**: log nhiều luồng lệch vài giây, hay đồng hồ bị chỉnh lùi. → Quét xuôi trong khối cuối và lấy Parent Line đầu tiên `≥ T`, chấp nhận lệch vài dòng. Đồng hồ bị chỉnh lùi lớn thì có thể bỏ sót đoạn: ghi trong tooltip "Starting Time assumes lines are in time order".
- **[Bộ nhớ của Remote Copy lớn]**: 2.000.000 dòng ≈ 2,6 GB, cộng đỉnh khi view dựng lại (ghi trong `add-remote-live-log`). → Giới hạn mặc định thận trọng và chỉnh được. Ghi rõ trong Settings "≈ 1,3 KB of memory per line".
- **[Đĩa]**: Remote Copy chiếm chỗ trong thư mục dữ liệu app. → Kiểm tra dung lượng trước khi đọc. Gỡ file thì xoá thư mục. Dọn thư mục mồ côi khi khởi động. Settings hiện tổng dung lượng Remote Copy.
- **[Nhận nhầm Rotated File]**: `app.log.bak`, `app.log.old` không được nhận; `app.log.2026` có thể bị nhận nhầm là `<N>`. → Chỉ Rotated File mới nhất không nén được tự đọc (L24), và chỉ phần vừa Live Window; các file khác luôn hiện cho người dùng tích.
- **[`head -c | wc -l` chậm trên file nhiều GB]**: dùng cùng timeout 30 giây như `wc -l`. Hết thời gian thì đánh số `~`.
- **[Phối hợp với `remote-file-picker`]**: picker có thể muốn ẩn hoặc nhóm Rotated File. Quy tắc nhận biết nằm ở `rotated.rs` để hai bên dùng chung.

## Migration Plan

- Session cũ không có Remote Copy. Biến thể `FileSource::RemoteCopy` thêm mới với serde tag, không đổi biến thể cũ.
- Setting `full_read_limit_lines` có `#[serde(default)]` là 2.000.000.
- Rollback: gỡ change. Session có Remote Copy sẽ không mở được các file đó; thư mục `remote-copies/` còn lại thì xoá tay.
- Archive change này sau `add-remote-live-log`, vì delta `live-log` và `remote-log-space` sửa requirement của change đó.

## Open Questions

Không còn câu hỏi mở. Đã chốt với người dùng:
- Live tự nạp Rotated File mới nhất nếu nó không nén, khi file hiện tại ít dòng hơn Live Window (quyết định 4, L24).
- Hỗ trợ gzip, zstd, xz và bzip2 ngay bản đầu, bằng crate thuần Rust (quyết định 3).
- Full Read Limit mặc định 2.000.000 dòng, chỉnh trong Settings (quyết định 9).
- Remote Copy không có hạn sống; chỉ bị xoá khi gỡ khỏi Log Space hoặc khi xoá Log Space (quyết định 8).
