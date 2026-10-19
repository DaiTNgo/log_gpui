## 1. Rotated File

- [ ] 1.1 `src/remote/rotated.rs`: khớp tên Rotated File theo các mẫu `<name>.<N>`, `-<YYYYMMDD>`, `-<YYYYMMDDHHMMSS>`, `-<YYYY-MM-DD>`, `.<YYYY-MM-DD>` kèm đuôi nén; xếp theo `mtime` giảm dần, hoà thì theo số/ngày trong tên; test bảng tên (gồm `app.log.bak`, `app.log.2026`)
- [ ] 1.2 Liệt kê Rotated File qua SFTP `readdir` của thư mục cha, kèm kích thước, `mtime` và kiểu nén đọc từ byte đầu (không nén, gzip, zstd, xz, bzip2, hoặc "not supported")
- [ ] 1.3 Thêm dependency thuần Rust đã pin phiên bản: `ruzstd`, `lzma-rs`, `bzip2` (backend `libbz2-rs-sys`); `src/remote/compress.rs` với `decoder(kind, reader) -> Box<dyn Read>` (gzip dùng `MultiGzDecoder`) và cầu nối `Read` trên SFTP đọc trước theo khối 1 MB, chạy trong `spawn_blocking`
- [ ] 1.4 Đếm số dòng trong lúc giải nén; test mỗi kiểu nén với file nhỏ, file gzip nhiều member, file `.1` thực chất là gzip, đuôi `.gz` nhưng không nén, và byte đầu lz4 (not supported)
- [ ] 1.5 `archive.rs` dùng chung `decoder()` và thêm `ArchiveKind` cho zstd, xz, bzip2 một file, để Rotated File đã Download mở được như Archive

## 2. Generation từ Rotated File khi Live

- [ ] 2.1 Thêm lệnh `LoadEarlier` cho `RemoteFileSource`; phát `Backfill` với Generation cũ hơn và Break "rotated" giữa hai Generation
- [ ] 2.2 Tự nạp: khi nạp dần chạm byte 0 của file hiện tại mà Live Window còn chỗ và chưa chạm Starting Time, liệt kê Rotated File và nếu file mới nhất không nén thì phát Break "rotated" rồi `Backfill` từ cuối nó bằng `BackwardLines`; không tự nạp file nào khác; test ngay sau logrotate, file mới nhất bị nén, file hiện tại đã đủ window
- [ ] 2.3 Nạp theo lệnh `LoadEarlier`: Rotated File thường đọc ngược bằng `BackwardLines`; Rotated File nén thì giải nén từ đầu và giữ K dòng cuối trong ring buffer (K = chỗ còn trống trong Live Window), có tiến độ trên status bar
- [ ] 2.4 Break đầu phần dòng của file hiện nút "Load earlier generation (<tên>, <kích thước>)"; khi Live Window đầy thì hiện "Open in full…"
- [ ] 2.5 Marker trên Generation từ Rotated File lưu theo khoảng cách Generation; test khôi phục sau khi mở lại

## 3. Starting Time

- [ ] 3.1 Đọc ô Starting Time: khoảng tương đối (`30m`, `1h`, `2h30m`, `2d`) và thời điểm tuyệt đối qua `parse_time_bound`; tính theo timestamp mới nhất trong khối cuối, giờ không kèm ngày lùi một ngày khi cần; test qua nửa đêm và khi file không có timestamp
- [ ] 3.2 `BackwardLines` thêm `stop_before: Option<f64>`: dừng ở Parent Line đầu tiên có `ts < T − 1s`, giữ Continuation Line đi cùng Parent Line; test với dòng không timestamp xen giữa và timestamp lệch vài giây
- [ ] 3.3 Tail và Backfill của `RemoteFileSource` chịu Starting Time; Break báo "N lines since … do not fit the Live Window" khi vượt window; view báo "no lines since …" khi không có dòng nào
- [ ] 3.4 Ô Starting Time trong luồng thêm/mở Remote File (phối hợp với change `remote-file-picker` về chỗ đặt ô)

## 4. Open in full

- [ ] 4.1 `src/remote/full_read.rs`: tìm nhị phân vị trí byte theo timestamp (khối 64 KB, mở rộng tới 1 MB khi khối không có timestamp, quét xuôi trong khối cuối) cho Starting Time và To; test bằng file giả lập qua trait `ReadAt` với log tăng đều, log lệch vài giây và đoạn không có timestamp
- [ ] 4.2 Đọc xuôi theo khối 1 MB từ byte bắt đầu tới byte kết thúc (hoặc kích thước lúc bắt đầu), đọc các Generation từ cũ tới mới, ghi `gen-<k>.log` và phát `Append` cho view
- [ ] 4.3 Số dòng thật khi bắt đầu giữa file: `head -c <offset> | wc -l` qua exec song song, timeout 30 giây; không có thì đánh số `~` và báo "line numbers unavailable"
- [ ] 4.4 Ước lượng số dòng và số byte (độ dài dòng trung bình của khối cuối, kích thước sau giải nén từ ISIZE của gzip, Frame Content Size của zstd, Index của xz, đoán ×8 cho bzip2) và kiểm tra dung lượng đĩa trống trước khi đọc
- [ ] 4.5 Full Read Limit: setting `full_read_limit_lines` (mặc định 2.000.000) trong Settings; chặn cứng khi đọc với Break "stopped at Full Read Limit"; lựa chọn "Read newest <limit> lines" tính byte bắt đầu và bỏ Generation không còn chỗ
- [ ] 4.6 Tiến độ trên status bar, Cancel giữ phần đã đọc với Break "read cancelled", Pause dừng và tiếp tục đọc, mất kết nối thì thử lại và đọc tiếp từ offset, xoay vòng khi đang đọc thì dừng với Break "file rotated while reading"

## 5. Remote Copy trong Imported Log Space

- [ ] 5.1 Thư mục `remote-copies/<uuid>/` với `meta.json` và `gen-*.log`; biến thể `FileSource::RemoteCopy { dir }` trong workspace và persist
- [ ] 5.2 Nạp Remote Copy khi mở lại Project: ghép các Generation từ cũ tới mới với Break "rotated", đánh số từ `first_line_no`, Break "read cancelled"/"stopped at Full Read Limit" cho Generation chưa đủ
- [ ] 5.3 Gỡ Remote Copy hoặc xoá Log Space thì xoá thư mục; khi khởi động thì dọn thư mục không còn được tham chiếu
- [ ] 5.4 Sidebar: tên `<server>:<path>` kèm biểu tượng Remote Copy, tooltip ghi thời điểm đọc và khoảng thời gian; menu có "Read again"
- [ ] 5.5 Read again: đọc vào thư mục mới với cùng lựa chọn, thay thư mục cũ, khôi phục Marker theo khoảng cách Generation, số dòng và `line_hash`
- [ ] 5.6 Imported Log Space tạo bởi Open in full mở với Follow tắt, view ở dòng đầu tiên được đọc

## 6. Dialog và lối vào

- [ ] 6.1 Dialog Open in full: danh sách Rotated File có ô tích (bỏ tích sẵn file ngoài khoảng thời gian, tắt file nén không hỗ trợ), Starting Time, To, nơi đặt (Log Space mới `<space> (full)` hoặc Imported Log Space có sẵn), ước lượng và ba lựa chọn khi vượt giới hạn
- [ ] 6.2 Lệnh "Open in full…" trên menu của Remote File, menu của Remote Log Space và command palette
- [ ] 6.3 Dialog Download / Download all thêm danh sách Rotated File (mặc định không tích), tải nguyên dạng nén, đặt tên phân biệt theo Server khi trùng

## 7. Kiểm thử và tài liệu

- [ ] 7.1 Test tích hợp với `sshd` cục bộ (`LOG_GPUI_REMOTE_TEST`): Rotated File thường và cả bốn kiểu nén, tự nạp `app.log.1` sau logrotate, Load earlier generation, Starting Time khi Live, Open in full một đoạn giữa file lớn, Cancel, Pause/Resume, mất kết nối khi đang đọc
- [ ] 7.2 Đo: tốc độ giải nén của `ruzstd`, `lzma-rs`, `bzip2` so với tốc độ tải qua SSH; số lần đọc SFTP khi tìm nhị phân trên file 1 GB; thời gian và bộ nhớ của Open in full 2.000.000 dòng; ghi kết quả vào design.md
- [ ] 7.3 Cập nhật CONTEXT.md theo mục "Domain changes" của proposal (Rotated File, Remote Copy, Starting Time, Full Read Limit; L3, L4, L10, L14, 24, 25; L24–L26, L27 để trống) và thêm ADR 0011
- [ ] 7.4 `cargo test` và `cargo clippy` sạch; `openspec validate remote-full-read` qua
