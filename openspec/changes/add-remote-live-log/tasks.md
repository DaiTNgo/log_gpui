## 1. Spike và dependency

- [x] 1.1 Spike `russh`: kết nối bằng key OpenSSH có passphrase, bằng mật khẩu, qua một Jump Server (`direct-tcpip`), mở SFTP và đọc file theo offset; ghi kết quả vào design.md (Open Questions)
- [x] 1.2 Thêm dependency đã pin phiên bản: `russh`, `russh-sftp`, `ssh-key`/`russh-keys`, `tokio` (rt-multi-thread), `async-channel`, `keyring`

## 2. Nguồn dòng chung

- [x] 2.1 Định nghĩa `LineSource`, `SourceEvent` (Tail, Backfill, Append, Break, Status), `SourceStatus`, `BreakKind` và lệnh điều khiển (Pause, Resume, SetWindow, Close) trong module mới `src/live/`
- [x] 2.2 Bộ tách dòng giữ lại phần dòng viết dở cho đến khi có ký tự xuống dòng; test với lô cắt giữa dòng và giữa ký tự UTF-8
- [x] 2.3 Đọc ngược từ cuối theo khối để lấy N dòng cuối và các lô Backfill; test với file nhỏ hơn N, file không có xuống dòng cuối, dòng dài hơn một khối
- [x] 2.4 `LocalFileSource`: polling metadata 250 ms, đọc từ offset, phát hiện xoay vòng (inode đổi), truncate (kích thước nhỏ đi), xoá rồi chờ file; test bằng file tạm

## 3. Generation và định danh dòng

- [x] 3.1 Thêm `generation` vào `LineRef`, `LogEntry` (hoặc ánh xạ row → file/gen/line) và `StoredMarker` với `#[serde(default)]`
- [x] 3.2 Lưu Marker theo khoảng cách Generation; chỉ khôi phục khoảng cách 0 và khớp `line_hash`; test hai dòng 1 khác Generation

## 4. Tích hợp live vào view

- [x] 4.1 Bộ gom sự kiện theo frame (~100 ms) trong app: parse, chèn theo timestamp vào `all_entries`, `FilterIndex::append`, tính lại Search/Focus, `cx.notify()` một lần
- [x] 4.2 Nạp Tail rồi Backfill chèn lên trên, giữ vị trí đọc theo `LineRef` và độ lệch pixel
- [x] 4.3 Nối `LocalFileSource` cho mọi file của Imported Log Space khi mở; đóng nguồn khi đóng Log Space hoặc gỡ file
- [x] 4.4 Vẽ Break giữa hai row trong Parsed View, Raw View và Focus Pane; loại Break khỏi Search, Filter, copy và số dòng
- [x] 4.5 Thay `tail_enabled` bằng `follow` và `paused`; `PaletteAction::ToggleTail` thành `ToggleFollow` và `TogglePause`; Follow tự tắt khi cuộn khỏi cuối và tự bật khi về cuối; status bar hiện "+N new"
- [x] 4.6 Pause dừng mọi nguồn của Log Space và tiếp tục đọc từ offset; mở Log Space luôn ở Follow bật, không Pause; Scratch Log Space không hiện hai công tắc

## 5. Live Window

- [x] 5.1 Setting Live Window cấp app (mặc định 100.000), chỉnh trong Settings, gửi `SetWindow` tới mọi Remote File đang mở
- [x] 5.2 Bỏ dòng cũ theo lô khi vượt 10%: loại row, dựng lại `FilterIndex`, bỏ Marker, dịch dòng hiện tại của Focus Tab, vẽ Break "older lines left"
- [x] 5.3 Số đếm Filter Value giảm theo; giá trị về 0 biến mất trừ khi đang tích; test theo các scenario của log-filter

## 6. Server và SSH

- [x] 6.1 Mô hình `Server` (tên, địa chỉ, port, user, cách đăng nhập, jump_server_id) và lưu `servers.json` ở thư mục dữ liệu app; kiểm tra tên trùng, vòng lặp Jump Server, tối đa 4 tầng
- [x] 6.2 Lưu và xoá mật khẩu/passphrase trong Keychain qua `keyring`
- [x] 6.3 `known_hosts.json` riêng của app: dialog xác nhận fingerprint lần đầu, chặn khi key đổi, cho thay key đã tin
- [x] 6.4 `RemoteHub`: tokio runtime riêng, một session cho mỗi Server dùng chung giữa các file, nối chuỗi Jump Server qua `direct-tcpip`, keepalive 30 s
- [x] 6.5 UI quản lý Server: thêm, sửa (kết nối lại kèm Break), xoá (từ chối và liệt kê nơi đang dùng)

## 7. Remote File

- [x] 7.1 `RemoteFileSource` qua SFTP: Tail ~1.000 dòng cuối, Backfill tới Live Window, polling 500 ms đọc từ offset
- [x] 7.2 Số dòng thật bằng `wc -l` qua exec, chạy song song; huỷ sau 30 s và báo "line numbers unavailable"
- [x] 7.3 Phát hiện xoay vòng, truncate, xoá bằng `stat` inode qua exec (GNU và BSD), dự phòng bằng kích thước; đọc nốt handle cũ trước khi sang Generation mới
- [x] 7.4 Tự kết nối lại 1s, 2s, 4s… tối đa 30s, đọc tiếp từ offset, Break mất/có lại kết nối; ngừng sau 5 phút và hiện Reconnect
- [x] 7.5 Lỗi theo từng file: Permission denied, SFTP not available, Authentication failed, Host key changed; các file khác vẫn live

## 8. Remote Log Space

- [x] 8.1 Thêm loại `Remote` cho Log Space trong `src/workspace/`, lưu Remote File theo `server_id` + đường dẫn, tên hiển thị `<server>:<path>`
- [x] 8.2 Dialog New Log Space: nút "Add remote files…", khoá trộn local và remote, nhãn "Create remote with N files"
- [x] 8.3 Bộ chọn file trên Server: duyệt thư mục SFTP, ô gõ đường dẫn, glob khớp một lần; cho thêm đường dẫn chưa tồn tại khi thư mục cha có; từ chối Server không kết nối được hoặc thư mục không tồn tại
- [x] 8.4 Menu Remote Log Space: "Add remote files…", "Download all", "Remove from Log Space"; bỏ file trùng kèm thông báo
- [x] 8.5 Download / Download all qua SFTP có tiến độ và huỷ, đặt tên phân biệt theo Server khi trùng, đề nghị mở thành Imported Log Space mới
- [x] 8.6 Mở lại Project: tự kết nối lại Remote Log Space đang mở, chờ Connect khi thiếu mật khẩu trong Keychain

## 9. Status bar

- [x] 9.1 Status bar hiện Follow, Pause và trạng thái xấu nhất của các file (Connecting, Live, Reconnecting, Disconnected, Waiting for file, lỗi) kèm "(k of n)"; hover hoặc bấm thì liệt kê từng file; nút Connect/Reconnect

## 10. Kiểm thử và tài liệu

- [x] 10.1 Test tích hợp với SSH server cục bộ (container `openssh-server`, có Jump Server): xoay vòng, truncate, mất mạng, Pause/Resume không mất dòng
- [x] 10.2 Đo bộ nhớ và độ trễ UI với 10 Remote File × 100.000 dòng và tốc độ ghi 1.000 dòng/giây
- [ ] 10.3 `cargo test` và `cargo clippy` sạch; `openspec validate add-remote-live-log` qua
