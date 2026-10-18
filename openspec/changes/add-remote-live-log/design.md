## Context

Hiện tại mỗi file được đọc trọn một lần (`std::fs::read` trong `src/parser/mod.rs`, `ParsedFile::from_path` trong `src/space_view.rs`), rồi `build_space_view` gộp thành `all_entries: Vec<LogEntry>` và `FilterIndex::build_merged`. Đã có sẵn một vài điểm nối cho live: `FilterIndex::append`, `on_entries_appended` trong `src/app/focus.rs`, nhưng không có gì gọi chúng; `tail_enabled` chỉ là cờ hiển thị. App chạy trên executor của gpui, không có tokio. Không có code SSH nào.

Các quyết định miền đã chốt nằm trong CONTEXT.md (nhóm Live, L1–L15) và ADR 0009 (SSH client nhúng).

## Goals / Non-Goals

**Goals:**
- Một cơ chế nguồn dòng chung cho file local và Remote File: nạp lần đầu, nạp dần ở nền, nhận dòng mới, báo đứt quãng.
- Remote File đọc qua SSH client nhúng, đi qua Jump Server, không phụ thuộc cấu hình máy.
- View, Filter, Search, Focus Pane cập nhật theo lô, không giật, với Live Window bỏ dòng cũ.

**Non-Goals:**
- ssh-agent, keyboard-interactive/2FA, sudo, đọc `~/.ssh/config`.
- Theo dõi file mới sinh ra sau khi khớp glob, đọc các file đã xoay vòng (`app.log.1`, `.gz`).
- Chỉnh lệch múi giờ giữa Server.
- Giữ lại dòng đã nhận sau khi đóng Remote Log Space.
- Server Windows (chỉ hỗ trợ server POSIX có SFTP).

## Decisions

### 1. `LineSource`: một giao diện nguồn dòng cho local và remote
Mỗi file live có một `LineSource` chạy ở nền, gửi `SourceEvent` qua channel về app:
- `Tail { lines, first_line_no, generation }`: lô dòng cuối đầu tiên (~1.000 dòng).
- `Backfill { lines, first_line_no, generation }`: lô dòng cũ hơn, chèn phía trên.
- `Append { lines }`: dòng mới (chỉ gồm dòng đã có ký tự xuống dòng).
- `Break { kind }` với `kind` là `Rotated | Truncated | Deleted | Disconnected | Reconnected | LeftWindow`.
- `Status(SourceStatus)`: Connecting, Live, Reconnecting, Disconnected, WaitingForFile, Error(msg).

App nhận lệnh điều khiển qua channel ngược: `Pause`, `Resume`, `SetWindow(n)`, `Close`.
Hai bản cài đặt: `LocalFileSource` và `RemoteFileSource`. Phần còn lại của app không biết file ở đâu.

*Thay thế đã cân nhắc:* làm hai luồng xử lý riêng cho local và remote. Bị loại vì Follow, Pause, Break và Generation giống hệt nhau ở hai bên.

### 2. Remote đọc bằng SFTP theo byte offset, không dùng `tail` trên server
`RemoteFileSource` mở phiên SFTP:
- **Nạp lần đầu:** đọc ngược từ cuối file theo khối 64 KB cho đến khi đủ ~1.000 dòng.
- **Nạp dần:** đọc tiếp ngược ở nền cho đến khi đủ Live Window hoặc chạm byte 0.
- **Dòng mới:** cứ ~500 ms gọi `stat` theo đường dẫn và đọc từ offset đã nhớ.
- **Kết nối lại:** nối tiếp từ đúng offset đã nhớ.

Để nhận biết file xoay vòng, app chạy thêm lệnh `stat -c '%i %s'` qua kênh exec (dùng `stat -f '%i %z'` cho BSD) để lấy inode:
- inode đổi: xoay vòng.
- kích thước nhỏ hơn offset: truncate.
- `stat` lỗi ENOENT: file bị xoá, chuyển sang chờ file xuất hiện.

Nếu kênh exec không dùng được, app chỉ dựa vào kích thước file và trạng thái handle đã mở để đoán.

Số dòng thật lấy bằng `wc -l` chạy qua kênh exec lúc mở file. Lệnh này chạy song song với việc nạp lần đầu: các dòng hiện ngay, còn Gutter chỉ có số khi kết quả `wc -l` trả về.

*Thay thế đã cân nhắc:* chạy `tail -n N -F` qua kênh exec. Bị loại vì 3 lý do: khó đọc tiếp đúng byte sau khi mất kết nối, không nạp ngược dần được, và số dòng phụ thuộc vào output của `tail`.

### 3. SSH chạy trên một tokio runtime riêng
`russh` cần tokio, còn gpui có executor riêng. App tạo một tokio runtime nhiều luồng duy nhất ở `RemoteHub`, khởi tạo lần đầu khi cần.
- Mỗi Server có một `russh` session dùng chung cho mọi Remote File trên Server đó: mỗi file một kênh SFTP hoặc exec.
- Jump Server được nối bằng `direct-tcpip` channel: session của tầng trong đi qua channel của tầng ngoài.
- Sự kiện được gửi về gpui bằng `async-channel`. Phía gpui nhận trong `cx.spawn`.

### 4. File local: polling bằng `std::fs::metadata`
`LocalFileSource` cứ 250 ms lấy metadata của file: inode (`MetadataExt::ino`) và kích thước. Có thay đổi thì đọc từ offset đã nhớ. Đổi inode là xoay vòng, kích thước nhỏ đi là truncate.

*Thay thế đã cân nhắc:* crate `notify` (FSEvents). Bị loại ở bản đầu: FSEvents có độ trễ và gộp sự kiện, đằng nào cũng phải gọi `stat`, nên polling đơn giản hơn và đủ nhanh.

### 5. Gom dòng mới theo frame
Phía app gom mọi `Append` nhận được trong ~100 ms rồi xử lý một lần:
1. parse các dòng mới;
2. chèn theo timestamp vào `all_entries`;
3. gọi `FilterIndex::append`;
4. tính lại Search và Focus;
5. nếu Follow đang bật thì cuộn xuống;
6. gọi `cx.notify()` một lần.

Nhờ vậy UI không vẽ lại theo từng dòng khi log ghi hàng nghìn dòng mỗi giây.

### 6. Định danh dòng gồm Generation
`LineRef` và `StoredMarker` được thêm trường `generation: u32`.
- Bên trong app, mỗi file đếm Generation từ 0 và tăng mỗi khi có Break xoay vòng, truncate hoặc xoá.
- Khi lưu Marker xuống đĩa, Generation lưu là khoảng cách tới Generation hiện tại: 0 là file hiện tại, 1 là file trước đó… Khi mở lại, chỉ Marker có khoảng cách 0 mới được khôi phục, và còn phải khớp `line_hash` như hiện nay.
- File Imported không xoay vòng giữ Generation 0, nên Marker cũ không cần migrate.

### 7. Live Window: bỏ dòng cũ theo lô
Mỗi Remote File được phép vượt Live Window thêm 10%. Khi vượt mức đó, app bỏ một lô dòng cũ nhất của file để quay về đúng Live Window:
- loại các row của file đó khỏi `all_entries` trong một lượt;
- dựng lại `FilterIndex` cho Log Space;
- bỏ Marker và dịch dòng hiện tại của Focus Tab nếu dòng đó bị bỏ.

Dựng lại toàn bộ index xấp xỉ 10.000 lần ít hơn so với cập nhật mỗi khi có dòng mới. Với 100.000 dòng × vài file, mỗi lần dựng lại mất cỡ vài chục ms.

*Thay thế đã cân nhắc:* dùng ring buffer và chỉ mục giảm dần theo từng dòng. Để sau, khi đo thấy chậm.

### 8. Giữ vị trí đọc theo `LineRef`, không theo chỉ số row
Nạp dần chèn dòng lên trên, còn bỏ dòng thì xoá ở phía trên. Vì vậy trước mỗi lô, viewport ghi lại `LineRef` của row đầu đang hiện và độ lệch pixel; sau lô thì cuộn lại đúng row đó. Cách này dùng chung cho yêu cầu "Chèn vào giữa" sẵn có của merged-timeline.

### 9. Pause giữ kết nối, ngừng đọc
Khi Pause, nguồn ngừng đọc nhưng vẫn giữ offset; session SSH gửi keepalive mỗi 30 giây. Khi tiếp tục, nguồn đọc từ offset đã giữ nên nhận được mọi dòng ghi trong lúc Pause. Riêng file local thì chỉ ngừng polling.

### 10. Lưu trữ Server và bí mật
- `servers.json` và `known_hosts.json` nằm trong thư mục dữ liệu của app, ghi cùng cơ chế debounce của `src/app/persist.rs`.
- Mật khẩu và passphrase lưu trong Keychain qua crate `keyring`. Service là `log_gpui`; account là `<server-id>:password` hoặc `<server-id>:passphrase`.
- Remote Log Space lưu trong session theo `server_id` + đường dẫn, không lưu nội dung.

### 11. Dependency mới
`russh`, `russh-sftp`, `russh-keys` (hoặc `ssh-key`), `tokio` (rt-multi-thread, chỉ dùng trong `RemoteHub`), `async-channel`, `keyring`. Pin phiên bản cụ thể thay vì dùng `"*"`.

## Risks / Trade-offs

- **[Server tắt SFTP subsystem]**: Remote File không đọc được. → Báo lỗi rõ "SFTP not available on <server>". Cách đọc dự phòng qua exec để ngỏ (Open Questions).
- **[`wc -l` chậm trên file vài GB]**: Gutter trống vài giây. → Dòng vẫn hiện ngay, số dòng điền sau. Chạy quá 30 giây thì huỷ và báo "line numbers unavailable".
- **[Xoay vòng trong lúc Pause hoặc mất kết nối]**: phần đuôi của file cũ, ghi sau offset đã nhớ, có thể mất vì app chỉ mở theo đường dẫn. → Trước khi chuyển sang file mới, đọc nốt handle cũ nếu nó vẫn còn mở; nếu không còn thì nhãn Break ghi "lines may be missing".
- **[Bộ nhớ]**: 100.000 dòng × nhiều Remote File × kích thước `LogEntry`. → Live Window chỉnh được; đo bộ nhớ trong task kiểm thử với 10 file.
- **[Hai executor]**: tokio và gpui chạy song song, khó debug. → Chỉ `RemoteHub` chạm vào tokio, ranh giới duy nhất giữa hai bên là channel.
- **[Host key và Jump Server]**: mỗi tầng có host key riêng. → Kiểm tra host key và hỏi xác nhận lần lượt từng tầng, từ ngoài vào trong.
- **[Bỏ công tắc Tail]**: thay đổi UI. → `PaletteAction::ToggleTail` đổi thành `ToggleFollow` và `TogglePause`.

## Migration Plan

- Session cũ không có Remote Log Space nên không cần migrate.
- Trường `tail_enabled` được bỏ khỏi state; session cũ không lưu trường này.
- `StoredMarker` thêm `generation` với `#[serde(default)]` là 0.
- Rollback: gỡ change; `servers.json` và `known_hosts.json` thừa lại không ảnh hưởng gì.

## Open Questions

- Có cần cách đọc dự phòng qua exec (`tail -c +<offset>` / `dd`) cho server tắt SFTP không? Mặc định là không ở bản đầu.
- ~~Dùng `russh` hay `ssh2`?~~ **Đã chốt `russh` 0.64.1 + `russh-sftp` 3.0.1** sau spike (`examples/ssh_spike.rs`, chạy với `sshd` cục bộ): đăng nhập bằng key OpenSSH ed25519 có passphrase (`load_secret_key`), đi qua Jump Server bằng `channel_open_direct_tcpip` + `client::connect_stream(.., channel.into_stream(), ..)`, mở SFTP (`SftpSession::new(channel.into_stream())`), đọc theo offset (`seek` + `read`) và `exec` `stat` đều chạy. Đăng nhập mật khẩu (`authenticate_password`) chưa chạy được trên máy này vì `sshd` cục bộ không có PAM; API giống publickey.

## Kết quả đo (task 10.2)

Máy phát triển (Apple Silicon, bản release), 10 Remote File × 100.000 dòng qua `sshd` cục bộ (~75 byte/dòng), ghi thêm 1.000 dòng/giây chia đều cho 10 file (`LOG_GPUI_LIVE_TRACE=1` in thời gian mỗi lô):

- **Mở:** 1.000.000 dòng hiện đủ sau khoảng 6 giây; 1.000 dòng cuối của mỗi file hiện gần như ngay.
- **Bộ nhớ mỗi dòng** (`cargo run --release --example mem_probe -- <file> --live`, 300.000 dòng ~85 byte, tính cả bản sao của view): **2.338 → 1.320 byte/dòng** (−44%). `LogEntry.spans` là `Spans` (danh sách khoá tĩnh dùng chung, `Arc`) thay cho `HashMap<String, _>` (−~700 byte/dòng cho cả hai bản); `raw_line`, `payload`, `time_str`, `tag`, `level_text` là `SharedString` (chuỗi ngắn nằm ngay trong struct, chuỗi dài dùng chung bằng `Arc`) nên bản sao trong `all_entries` không cấp phát lại chuỗi. `LogEntry` 360 → 328 byte. Chưa làm: `payload`/`time_str` thành khoảng của `raw_line` và bỏ `lines` khi không ở Raw View (mỗi thứ chỉ còn ~5–9%, và đổi kiểu `ParsedFile.lines` chạm cả Raw View).
- **Bộ nhớ của app** (10 × 100.000 dòng, đang ghi): `footprint` dao động 1,5–2,2 GB khi chạy ổn định ở ~1.05 triệu dòng (trước: 2,4 GB), nhưng nhảy lên 3–5 GB trong vài giây mỗi lần view dựng lại (nạp Tail, Live Window cắt bớt dòng cũ) vì bộ cấp phát giữ lại các khối lớn vừa giải phóng. Đã bớt: rebuild giải phóng view cũ trước khi dựng view mới, `visible_indices` giãn tại chỗ thay vì cấp phát lại 8 MB mỗi lô, và `malloc_zone_pressure_relief` sau lô có sự kiện cấu trúc và mỗi 50 lô (macOS). Phần còn lại là việc của một bộ cấp phát khác (jemalloc/mimalloc) hoặc của việc không dựng lại cả view khi Tail/trim.
- **Rảnh:** CPU ~0,3%. Trước khi sửa, mỗi lần vẽ lại sao chép toàn bộ `all_entries` (CPU ~97%, 3,4 GB); nay canvas và Focus Pane đọc dòng trực tiếp từ view khi vẽ từng row nên không sao chép.
- **Đang ghi, các dòng đến đúng thứ tự thời gian:** mỗi lô mất ~1 ms phân tích và ~15 ms cập nhật view (nối thẳng vào cuối, `FilterIndex::append`, tính lại Filter/Search/Focus).
- **Đang ghi, dòng đến lệch thứ tự giữa các file** (file thứ f ghi dòng cũ hơn file 0 `30·f` ms, nên mỗi lô chèn ~130 dòng trước cuối view): `append_to_open_view` gộp các dòng mới với phần view từ vị trí dòng sớm nhất trở đi (cùng khoá `(thời gian, thứ tự file, dòng)` như `timeline::merge`, Continuation Line đi theo cha), rồi `FilterIndex::truncate` + `append` cho phần đó. Ở 1.000.000+ dòng, 284 lô: **trung vị 18 ms, p90 24 ms, p99 49 ms** (trước: ~450 ms mỗi lô lệch thứ tự). Hai lô > 100 ms (~320 ms) là lúc Live Window cắt dòng cũ, vẫn dựng lại view. Đã đối chiếu 21 lần: thứ tự các row và `FilterIndex` (giá trị, đếm, id từng row, `effective_ts`) bằng đúng kết quả dựng lại từ đầu. Dựng lại vẫn xảy ra khi: Tail/Backfill/cắt/đánh số lại, dòng thay thế dòng cuối, file có thời gian đi lùi, hoặc Log Space có file không có timestamp.
- **Giữ vị trí đọc (task 4.2):** khi view dựng lại hoặc có dòng chèn vào phía trên, row ở đầu giữ nguyên cả độ lệch pixel (`logical_scroll_top` lấy chỉ số row và phần đã cuộn vào row, `ScrollHandle::set_offset` đặt lại sau khi tính vị trí mới của `LineRef`). Chưa kiểm thủ công trên cửa sổ thật (không gửi phím giả vào desktop).

### Việc tiếp theo
- Không dựng lại cả view khi nạp Tail của một file hay khi Live Window cắt dòng: chèn/xoá theo khoảng như đã làm với dòng lệch thứ tự. Việc này cũng bỏ các đỉnh bộ nhớ 3–5 GB.
- `payload`/`time_str` thành khoảng của `raw_line` và bỏ `lines` khi không ở Raw View.
