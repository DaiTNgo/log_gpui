## 1. Spike SSH

- [ ] 1.1 Với `russh` 0.64 và `sshd` cục bộ (`examples/ssh_spike.rs`): chạy exec dạng stream đọc riêng stdout/stderr, gửi EOF stdin, nhận `ExitStatus`; gửi `signal TERM` và xác nhận `journalctl -f`/`sleep` dừng; đo thời gian vòng của global request `keepalive@openssh.com` có `want_reply` (nếu không chờ được reply, đo bằng mở/đóng kênh session). Ghi kết quả vào design.md

## 2. Server và RemoteHub

- [ ] 2.1 Thêm `keepalive_interval_secs` (30, 5–300) và `keepalive_max_missed` (3, 1–10) vào `Server` với `serde(default)`; đưa vào `client::Config` của từng tầng trong `connect_chain`; test đọc `servers.json` cũ
- [ ] 2.2 Thêm trường keepalive vào dialog Server, kiểm tra khoảng giá trị
- [ ] 2.3 `ServerHealth` trong `RemoteHub`: trạng thái, đã nối từ, số lần nối lại, lỗi cuối kèm giờ, keepalive, chuỗi Jump Server; cập nhật khi connect/mất session; snapshot đọc được từ gpui và báo thay đổi qua channel
- [ ] 2.4 Task đo độ trễ cho mỗi session theo chu kỳ keepalive (gần nhất, trung bình 5 lần); test với `sshd` cục bộ (env-gated như các test remote sẵn có)
- [ ] 2.5 `RemoteSession::exec_stream`: kênh exec không PTY, EOF stdin ngay, sự kiện stdout/stderr/exit; `ExecStream::terminate()` gửi `signal TERM`, chờ tối đa 2 s, rồi đóng kênh

## 3. Mô hình Remote Command

- [ ] 3.1 `RemoteRef { server_id, target: RemoteTarget::File { path } | Command { command, preset, restart_on_exit } }`; `LogFile::remote_command` với `file_id` `remote-cmd:<server_id>:<hash>` và tên `<server>$ <command>`; đọc session cũ có `path` thành `File`; thành viên không đọc được bị bỏ qua, không làm hỏng session
- [ ] 3.2 Chặn trùng Server + lệnh khi thêm; rule 24: chỉ Remote Log Space nhận Remote Command
- [ ] 3.3 Kiểm tra nâng quyền: tách đoạn theo `|`, `;`, `&&`, `||`, bỏ gán biến và tiền tố `env`/`command`/`exec`/`nohup`/`time`, từ chối `sudo`/`su`/`doas`/`pkexec`/`run0`; unit test cho các dạng như `FOO=1 sudo x`, `/usr/bin/sudo x`, `sudo -n x`, `a | sudo tee`, `echo sudo` (cho phép)
- [ ] 3.4 `PresetFields` (Journal/Docker/Kube) và `render(fields, Start::Tail(n) | Start::Since(ts))` có shell-quote; `preset` chỉ lưu khi lệnh bằng `render(.., Tail)`, mở lại mà khác thì coi là gõ tay; unit test chuỗi lệnh của cả hai dạng Start
- [ ] 3.5 `LogEntry.stream: Option<Stream>` (không tăng kích thước struct), `SourceEvent::Append` mang bitset Stream cho lệnh

## 4. RemoteCommandSource

- [ ] 4.1 `RemoteCommandSource: LineSource`: mỗi Run chạy mới gửi `Tail { lines: [], generation }` rồi `Append`; hai `LineSplitter` cho stdout/stderr, mỗi dòng mang Stream; đẩy dòng dở khi Run kết thúc
- [ ] 4.2 Thêm `BreakKind::Restarted`, `BreakKind::Exited(Option<u32>)` (bắt đầu Generation mới ở Run sau), `SourceStatus::Exited(Option<u32>)` với severity giữa WaitingForFile và Reconnecting, `SourceCommand::Restart`
- [ ] 4.3 Thoát, Restart tay, Restart on exit với backoff 1→30 s, về 1 s sau Run > 60 s, dừng sau 5 lần thoát mã ≠ 0 trong 10 s
- [ ] 4.4 Mất kết nối: backoff và bỏ cuộc như `RemoteFileSource` (tôn trọng Sign-in Prompt của ssh-connect-ergonomics), chạy lại lệnh khi có session: preset chưa sửa thì Resume By Time cùng Generation, lệnh gõ tay thì Run mới với Break "command restarted, lines may be missing or repeated"
- [ ] 4.5 `ResumePoint`: đọc timestamp ở token đầu của dòng theo từng preset (journalctl offset → epoch, Docker/kubectl RFC3339 nguyên văn), giữ `boundary` mỗi Stream (tối đa 10.000 dòng), bộ lọc trùng 3 bước theo multiset; unit test: trùng ở ranh giới giây, hai dòng giống hệt nhau, dòng tiếp nối, đồng hồ lùi (>1.000 dòng cũ hơn mốc), `boundary` tràn
- [ ] 4.6 Resume khi Restart on exit (Generation mới), fallback khi công cụ không nhận cờ since (chạy nguyên văn một lần, "resume not supported", không thử lại đến khi mở lại); test với `sshd` cục bộ bằng script giả lập journalctl/docker in timestamp và nhận `--since`
- [ ] 4.7 Pause: vẫn đọc kênh, hàng đợi tối đa Live Window, gửi khi Resume, Break `LeftWindow` khi tràn; `SetWindow` áp dụng cho hàng đợi
- [ ] 4.8 Gỡ thành viên, đóng Log Space, thoát app: `terminate()` mọi Run đang chạy
- [ ] 4.9 Test nguồn với `sshd` cục bộ: số dòng theo Run, stderr thành dòng, thoát/Restart/Restart on exit, Pause tràn window, ngắt `sshd` rồi nối lại (preset resume không mất/lặp dòng, lệnh gõ tay có Break)

## 5. App và UI

- [ ] 5.1 `src/app/live.rs`: khởi động `RemoteCommandSource` cho thành viên lệnh; Marker của dòng Remote Command không khôi phục khi mở lại; Live Window theo từng Remote Command
- [ ] 5.2 Dialog thêm vào Remote Log Space: tab "Command" với chọn Server, ô lệnh, danh sách Command Preset, dòng "Runs on <server> as <user> each time this Log Space opens", lỗi nâng quyền khoá nút Add
- [ ] 5.3 Command Preset journalctl/Docker/Kubernetes: form trường, sinh lệnh có shell-quote, gợi ý từ lệnh liệt kê cố định chạy qua `exec`, hiện lỗi dưới trường, nút làm mới
- [ ] 5.4 Context menu của Remote Command: Restart, Restart on exit, Remove from Log Space; không có Download
- [ ] 5.5 Download all bỏ qua Remote Command và báo số lệnh bị bỏ qua
- [ ] 5.6 Kiểm tra parser nhận timestamp của `journalctl -o short-iso-precise`, `docker logs --timestamps`, `kubectl logs --timestamps`; bổ sung định dạng nếu thiếu, kèm test

## 6. Stream

- [ ] 6.1 Gutter: số dòng stderr màu stderr, tooltip "stderr" trong Parsed View, Raw View, Focus Pane; không đổi màu Highlight Rule/Marker, không vào nội dung copy
- [ ] 6.2 Stream Column như Source Column: ẩn mặc định, công tắc trong Quick Settings, không pin, rỗng cho dòng file
- [ ] 6.3 `FilterIndex`: Stream là Filterable Column khi Log Space có Remote Command (giá trị `stdout`/`stderr`, đếm, giảm khi rời Live Window), không thừa kế qua Continuation Line, dòng file không có giá trị; Highlight Rule dùng được Stream; test Filter, Hide Mode, Highlight

## 7. Connection Details

- [ ] 7.1 Nhãn trạng thái trên status bar: thêm "Exited", thêm " · <ms>" của Server chậm nhất khi mọi Server đã nối
- [ ] 7.2 Popover Connection Details neo vào status bar: nhóm theo Server (địa chỉ, Jump Server, trạng thái, độ trễ, đã nối từ, số lần nối lại, keepalive, lỗi cuối), từng thành viên (trạng thái, dòng cuối lúc, số lần nối lại/Run, lỗi cuối, mã thoát)
- [ ] 7.3 Nút Reconnect cho Server (nối lại mọi thành viên trên Server đó) và Remote File, Restart cho Remote Command
- [ ] 7.4 Lệnh của Run hiện tại (kể cả dạng resume) hiện trong Connection Details
- [ ] 7.5 `SourceHealth` phía app từ `SourceEvent`: số lần nối lại/Run, lỗi cuối, giờ dòng cuối; không lưu

## 8. Kiểm thử và tài liệu

- [ ] 8.1 Kiểm thử thủ công với `sshd` cục bộ: journalctl (nếu có) hoặc lệnh giả lập `while true; do date -Iseconds; sleep 1; done`, gộp với Remote File, Pause, Restart, rút mạng, dòng stderr (Gutter, Stream Column, Filter)
- [ ] 8.2 Sau khi `add-remote-live-log` đã archive: cập nhật CONTEXT.md theo mục "Domain changes" của proposal
