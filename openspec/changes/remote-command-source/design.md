## Context

`add-remote-live-log` đã có: `RemoteHub` (một tokio runtime, một `russh` session dùng chung cho mỗi Server, Jump Server qua `direct-tcpip`, keepalive cố định 30 s × 3), `LineSource` / `SourceEvent` / `SourceCommand` (`src/live/source.rs`), `RemoteFileSource` đọc qua SFTP theo byte offset và tự nối lại (1 s → 30 s, bỏ cuộc sau 5 phút), và status bar hiện trạng thái xấu nhất của các file kèm tooltip liệt kê từng file (`src/app/view/status_bar.rs`). `RemoteSession::exec` hiện chỉ gom toàn bộ stdout đến khi lệnh kết thúc (dùng cho `stat`, `wc -l`). Một thành viên của Remote Log Space là `LogFile` với `remote: Some(RemoteRef { server_id, path })`.

Ràng buộc miền: L1 (Pause cả Log Space), L6 (tự nối lại), L7 (trạng thái live chỉ trên status bar), L9 (không nâng quyền), L10/L13 (Live Window theo từng thành viên), rule 24 (loại Log Space không trộn). Thay đổi miền được mô tả trong mục "Domain changes" của proposal.

## Goals / Non-Goals

**Goals:**
- Output của một lệnh trên Server là một nguồn live ngang hàng với Remote File: cùng `LineSource`, Follow, Pause, Break, Generation, Live Window, Filter, Search, Focus Pane, timeline gộp.
- Preset cho journalctl, Docker, Kubernetes sinh lệnh có timestamp để gộp timeline đúng.
- Người dùng luôn biết chính xác lệnh nào chạy, trên Server nào, bằng user nào; không có đường nào dẫn tới nâng quyền.
- Preset nối lại không mất, không lặp dòng (Resume By Time).
- Biết dòng nào đến từ stderr, lọc và tô màu được theo đó.
- Người dùng thấy được sức khoẻ kết nối của từng Server và từng nguồn mà không phá L7.

**Non-Goals:**
- Lệnh tương tác, PTY, nhập stdin, màu ANSI (mã ANSI hiện nguyên văn như mọi text đã lưu, rule 30).
- Đọc tiếp đúng chỗ dừng cho Remote Command gõ tay hoặc preset đã sửa tay (chỉ preset chưa sửa được Resume By Time).
- `sudo -n` hay bất kỳ dạng nâng quyền nào (L9 tuyệt đối).
- Preset cho dịch vụ khác (Podman, `docker compose logs`, `stern`…); người dùng gõ tay được.
- Lịch sử lỗi, biểu đồ độ trễ, lưu số liệu kết nối qua lần mở app.
- Download output của Remote Command.

## Decisions

### 1. Remote Command là thành viên mới của Remote Log Space, không phải "Remote File đặc biệt"
`RemoteRef` thành enum:
```
enum RemoteTarget { File { path }, Command { command, preset: Option<PresetFields>, restart_on_exit: bool } }
struct RemoteRef { server_id, target: RemoteTarget }
```
`file_id` của lệnh là `remote-cmd:<server_id>:<hash của command>`, tên hiển thị `<server>$ <command>`. Phần còn lại của app (sidebar, Source Column, File Section, File View, Marker theo `file_id`) dùng nguyên như Remote File.

*Thay thế đã cân nhắc:* coi lệnh như một "đường dẫn" kiểu `!journalctl …` trong Remote File. Bị loại: số dòng, nối lại, Download và Pause đều khác nghĩa; trộn vào một thuật ngữ sẽ làm sai L4, L6, L14.

### 2. Chạy lệnh: kênh exec dạng stream, không PTY, stdin đóng
Thêm `RemoteSession::exec_stream(command) -> ExecStream` trả về kênh exec đang chạy, đọc `ChannelMsg::Data` (stdout), `ExtendedData { ext: 1 }` (stderr), `ExitStatus`, `ExitSignal`, `Eof`, `Close`. Ngay sau `exec` gửi `channel.eof()` để lệnh nào đọc stdin thì nhận EOF ngay. Không gọi `request_pty`, nên `sudo` không thể hỏi mật khẩu và không có mã `\r\n`/màu do PTY sinh ra.

stdout và stderr đều thành dòng (Docker gửi stderr của container ra stderr; `kubectl logs` gộp sẵn). Mỗi luồng có một `LineSplitter` riêng để hai luồng không cắt ngang dòng của nhau; dòng của hai luồng xếp theo thứ tự đến, và mỗi dòng mang Stream của nó (mục 13). `SourceEvent::Append` đổi thành `Append { lines, streams: Option<StreamMask> }` (`None` cho file; bitset song song với `lines` cho lệnh) để không tăng bộ nhớ của Remote File. Khi Run kết thúc, phần dòng chưa có ký tự xuống dòng được đẩy ra luôn (không còn byte nào đến nữa).

*Thay thế đã cân nhắc:* chỉ lấy stdout, stderr chỉ để báo lỗi. Bị loại vì mất nửa log của `docker logs`.

### 3. Chặn nâng quyền bằng kiểm tra từ đầu tiên
Trước khi chạy (cả lúc thêm và lúc mở lại), app tách lệnh theo khoảng trắng kiểu shell, bỏ qua các gán biến `NAME=value` và tiền tố `env`, `command`, `exec`, `nohup`, `time`; nếu từ còn lại (tên cuối của đường dẫn) là `sudo`, `su`, `doas`, `pkexec`, `run0` thì từ chối với "Commands run as <user>; elevation is not allowed". Kiểm tra lặp lại cho từng đoạn sau `|`, `;`, `&&`, `||`.

`sudo -n` cũng bị từ chối (L9 tuyệt đối, đã chốt với người dùng). Đây là rào chắn cho thao tác vô ý, không phải ranh giới bảo mật: người dùng có quyền SSH vào Server vẫn chạy được mọi thứ user đó chạy được. Ranh giới thật là: lệnh chạy bằng user của Server, không PTY, không stdin, nên `sudo` (trừ `NOPASSWD`) không thể thành công, và app không thêm gì vào lệnh ngoài việc thay cờ số dòng ban đầu bằng cờ since khi Resume By Time (L28, mục 12).

### 4. Command Preset sinh lệnh, không chạy logic riêng
Preset là một form có vài trường, sinh ra chuỗi lệnh; sau đó Remote Command chỉ là chuỗi lệnh đó. Giá trị trường được shell-quote (`shell_quote` sẵn có trong `src/live/remote.rs`).

| Preset | Trường | Lệnh sinh ra |
|---|---|---|
| journalctl | unit (tuỳ chọn), số dòng đầu (1000) | `journalctl --no-pager -o short-iso-precise -f -n 1000 -u '<unit>'` |
| Docker | container, số dòng đầu | `docker logs -f --timestamps --tail 1000 '<container>'` |
| Kubernetes | pod, namespace (tuỳ chọn), container (tuỳ chọn), context (tuỳ chọn), số dòng đầu | `kubectl logs -f --timestamps --tail=1000 [--context '<c>'] [-n '<ns>'] [-c '<container>'] '<pod>'` |

`PresetFields` là enum `Journal { unit, lines } | Docker { container, lines } | Kube { pod, namespace, container, context, lines }`; `render(fields, Start::Tail(n) | Start::Since(ts))` sinh lệnh. Một Remote Command chỉ lưu `preset = Some(fields)` khi `command == render(fields, Tail(lines))` lúc bấm Add; người dùng sửa tay ô lệnh thì `preset = None`, dialog hiện "Edited: will rerun as typed after a lost connection" và lệnh đó là lệnh gõ tay. Lúc mở lại, nếu `render` của bản app mới cho chuỗi khác với `command` đã lưu thì cũng coi là lệnh gõ tay (lệnh đã lưu là nguồn sự thật).

Gợi ý giá trị lấy bằng một lệnh liệt kê cố định, chỉ đọc, chạy qua `exec` một lần khi mở form (bấm làm mới để chạy lại): `systemctl list-units --type=service --all --no-legend --plain`, `docker ps --format '{{.Names}}'`, `kubectl get pods [-n ns] -o name`. Lỗi (chưa cài, không có quyền với socket Docker) thì form hiện lỗi đó ở dưới trường và người dùng vẫn gõ tay được. Lệnh cuối hiện trong một ô sửa được.

Preset luôn thêm cờ timestamp, vì không có timestamp thì nguồn rơi xuống cuối timeline (rule 22). Cần kiểm tra parser nhận được `2026-10-19T10:02:03.123456+0000` (journalctl `short-iso-precise`) và tiền tố RFC3339Nano của Docker/kubectl; nếu chưa, đó là task của change này.

### 5. Run và Generation; số dòng đếm theo output
`RemoteCommandSource` gửi `SourceEvent::Tail { lines: [], generation }` khi một Run chạy mới bắt đầu (để app biết Generation mới), rồi chỉ `Append`. Không có `Backfill`, không `TotalLines` (số dòng = thứ tự trong Generation, đếm ở nguồn). Run chạy mới (lần đầu, Restart tay, Restart on exit, lệnh gõ tay nối lại) tăng Generation và có Break trước nó. Run nối lại bằng Resume By Time sau mất kết nối KHÔNG tăng Generation: số dòng nối tiếp, Break là `Disconnected`/`Reconnected` như Remote File (mục 12). Thêm vào `BreakKind`: `Restarted` (Restart tay hoặc Restart on exit), `Exited { code }` (vẽ ngay sau dòng cuối của Run). Mất kết nối dùng `Disconnected` sẵn có. Khi nối lại: preset chưa sửa dùng `Reconnected` với nhãn "reconnected, resumed from <giờ của dòng cuối>"; lệnh gõ tay dùng `Restarted { after_disconnect: true }` với nhãn "reconnected — command restarted, lines may be missing or repeated" và bắt đầu Generation mới.

Live Window áp dụng như Remote File, tính trên mọi dòng của Remote Command qua các Run (L13), bỏ dòng cũ theo lô như sẵn có.

*Liên quan:* change `remote-clear-and-load-more` (L32–L35) sở hữu Clear theo từng thành viên và áp dụng cả cho Remote Command: Clear bỏ các dòng đang giữ của Remote Command còn lệnh vẫn chạy (không Restart, không đổi Run hay Generation, số dòng đếm tiếp); Load more không áp dụng cho Remote Command vì không có dòng cũ để nạp (L4). Change này không thêm requirement nào về Clear.

### 6. Vòng đời và Restart
Trạng thái nguồn: `Connecting → Live → (Exited(code) | Reconnecting | Error)`. Thêm `SourceStatus::Exited(Option<u32>)` với severity giữa `WaitingForFile` và `Reconnecting`; `Exited(Some(0))` không tính là lỗi nhưng vẫn hiện (lệnh không còn chạy).

- **Restart** (lệnh `SourceCommand::Restart` mới): nếu Run đang chạy thì kết thúc nó (mục 7), vẽ Break `Restarted`, chạy Run mới.
- **Restart on exit** (tắt mặc định, đổi trong context menu của Remote Command): khi lệnh thoát, chạy lại (preset chưa sửa: Resume By Time, để `docker logs --tail 1000` không in lại dòng của container cũ; Generation mới vì lệnh đã thoát) sau backoff 1 s, 2 s, 4 s… tối đa 30 s; backoff về 1 s khi một Run sống quá 60 s. Nếu 5 Run liên tiếp thoát mã khác 0 trong vòng 10 s thì dừng ở `Exited(code)` và chờ Restart tay, để `command not found` không lặp mãi.
- **Mất kết nối**: như Remote File (backoff, bỏ cuộc sau 5 phút, Reconnect; Server cần Sign-in Prompt theo ssh-connect-ergonomics thì chờ người dùng), nhưng khi session có lại thì chạy lại lệnh, kể cả khi Restart on exit tắt (lệnh đã không thoát vì chính nó): preset chưa sửa thì Resume By Time, lệnh gõ tay thì chạy nguyên văn thành Run mới.
- **Restart tay** luôn chạy lệnh nguyên văn đã lưu (số dòng ban đầu của preset), thành Generation mới: người dùng muốn bắt đầu lại.

### 7. Kết thúc một Run trên Server
Không PTY thì sshd không gửi SIGHUP khi đóng kênh; tiến trình chỉ chết vì SIGPIPE ở lần ghi tiếp theo. Để không để lại tiến trình mồ côi (`journalctl -f` trên unit im lặng có thể chờ rất lâu), app gửi channel request `signal TERM` (OpenSSH ≥ 7.9 hỗ trợ), chờ tối đa 2 s cho `ExitStatus`/`Close`, rồi đóng kênh. Khi gỡ Remote Command, đóng Log Space hay thoát app, làm như vậy cho mọi Run đang chạy.

*Thay thế đã cân nhắc:* bọc lệnh thành `sh -c 'echo $$; exec <cmd>'` để biết pid rồi `kill` qua exec khác. Bị loại vì app sẽ thêm chữ vào lệnh của người dùng (trái L28) và phụ thuộc shell. Ghi lại thành rủi ro.

### 8. Pause: lệnh vẫn chạy, dòng giữ ở nguồn
Ngừng đọc kênh SSH sẽ làm đầy cửa sổ SSH, chặn lệnh trên Server, và với `russh` có thể chặn cả session dùng chung với Remote File khác. Vì vậy khi Pause, `RemoteCommandSource` vẫn đọc và giữ dòng trong một hàng đợi ở nền, tối đa Live Window dòng; khi tiếp tục thì gửi cả hàng đợi một lần. Nếu trong lúc Pause có nhiều hơn Live Window dòng, các dòng cũ nhất bị bỏ và có Break `LeftWindow` — đúng như khi chúng đã vào view rồi rời window (L10, L12). Số dòng vẫn đếm cả dòng bị bỏ.

### 9. Số liệu kết nối ở `RemoteHub`
`RemoteHub` giữ `ServerHealth` cho mỗi Server, cập nhật từ tokio và đọc từ gpui qua `Arc<Mutex<…>>` (snapshot khi status bar vẽ, đẩy `cx.notify()` khi thay đổi qua channel sẵn có):
```
struct ServerHealth {
    state: Connecting | Connected | Reconnecting | Disconnected | Waiting,
    connected_since: Option<Instant>,
    latency: Option<Duration>,      // lần đo gần nhất
    latency_avg: Option<Duration>,  // trung bình trượt 5 lần
    reconnects: u32,                // session mới sau khi session cũ mất, trong lần chạy app này
    last_error: Option<(SystemTime, String)>,
    keepalive: (interval, max_missed),
    hops: Vec<String>,              // tên Jump Server, ngoài vào trong
}
```
Mỗi nguồn giữ `SourceHealth { status, reconnects_or_restarts, last_error, last_line_at, exit_code }` ở phía app (từ `SourceEvent`).

**Đo độ trễ:** một task cho mỗi session gửi global request `keepalive@openssh.com` với `want_reply = true` theo chu kỳ keepalive và đo thời gian tới khi có reply (thành công hay `REQUEST_FAILURE` đều được tính, vì cả hai đều là một vòng). Với Server sau Jump Server, độ trễ là tới Server đích (đi qua mọi tầng). Nếu API `russh` 0.64 không cho chờ reply của global request, dùng phương án dự phòng: mở rồi đóng một kênh session (`channel_open_session` có confirm) mỗi chu kỳ. Spike trong task 1.

**Keepalive theo Server** (thuộc change này; ssh-connect-ergonomics không làm phần này): `Server.keepalive_interval_secs` (mặc định 30, 5–300) và `Server.keepalive_max_missed` (mặc định 3, 1–10), đưa vào `client::Config` của tầng đó. Đổi giá trị áp dụng cho lần kết nối sau, như các trường khác của Server.

### 10. Connection Details trong status bar (L7)
Nút trạng thái kết nối trên status bar (đang có tooltip) đổi thành nút mở popover **Connection Details**, neo vào status bar:
- Nhãn nút: trạng thái xấu nhất như hiện nay, thêm ` · <độ trễ>` khi mọi Server đã nối (lấy Server chậm nhất), ví dụ `Live · 42 ms`, `Reconnecting (1 of 3)`.
- Popover nhóm theo Server: tên, `user@host:port`, chuỗi Jump Server, trạng thái, độ trễ (gần nhất / trung bình), đã nối từ, số lần nối lại, keepalive, lỗi cuối kèm giờ; nút Reconnect cho Server.
- Dưới mỗi Server: từng Remote File / Remote Command với trạng thái, giờ nhận dòng cuối, số lần nối lại hoặc Run, lỗi cuối, mã thoát; nút Reconnect (file) hoặc Restart (lệnh).
- Chỉ hiện các Server và nguồn của Log Space đang mở.

Popover là một phần của status bar (mở từ đó, đóng khi click ra ngoài), nên L7 chỉ cần nói rõ thêm (proposal, Domain changes). Sidebar, tab, dialog Server không hiện trạng thái kết nối; dialog Server chỉ có trường cấu hình keepalive.

*Thay thế đã cân nhắc:* hiện độ trễ/lỗi trong sidebar cạnh từng Server hoặc trong dialog Servers. Bị loại vì phá L7 và vì Server dùng chung cho mọi Project, còn status bar chỉ nói về Log Space đang mở.

### 11. Lưu và mở lại
Session lưu `RemoteTarget::Command { command, preset, restart_on_exit }`. Mở lại Project chạy lại lệnh như Remote File được nối lại (đây là lệnh người dùng đã thêm, L28). Marker của dòng Remote Command không được khôi phục khi mở lại: Run mới có output khác, dòng 1 của Run mới không phải dòng 1 của Run cũ.

### 12. Resume By Time cho Command Preset
**Lấy giờ của Server.** Không hỏi múi giờ hay đồng hồ của Server, và không dùng đồng hồ máy local: preset luôn yêu cầu output có timestamp kèm offset, và giờ để resume là chính timestamp đó.

| Preset | Timestamp trong output | Cờ khi resume (thay cho cờ số dòng ban đầu) |
|---|---|---|
| journalctl | `-o short-iso-precise`: `2026-10-19T10:02:03.123456+07:00` (offset của Server) | `--since @<giây epoch>` (đổi timestamp có offset sang epoch là phép tính chính xác, không phải chỉnh giờ) |
| Docker | `--timestamps`: `2026-10-19T03:02:03.123456789Z` | `--since 2026-10-19T03:02:03.123456789Z` (nguyên văn), bỏ `--tail` |
| Kubernetes | `--timestamps`: `2026-10-19T03:02:03.123456789Z` | `--since-time=2026-10-19T03:02:03.123456789Z` (nguyên văn), bỏ `--tail` |

Timestamp này được đọc ở nguồn (tokio), không qua parser của app: mỗi preset biết token đầu của dòng là timestamp. Nguồn giữ `ResumePoint { ts: (epoch_nanos, text), boundary: Vec<String> }`: timestamp của dòng có timestamp cuối cùng đã nhận, và văn bản mọi dòng đã nhận từ dòng có timestamp bằng `floor(ts, độ chính xác của cờ)` trở đi (gồm cả dòng tiếp nối không có timestamp của journalctl). `boundary` giới hạn 10.000 dòng; vượt thì bỏ resume, chạy lại nguyên văn như lệnh gõ tay. Giờ chỉ dùng cho lệnh của chính Server đó, không so với Server khác (L8).

**Bỏ dòng trùng ở ranh giới.** Vì cờ `--since` có độ chính xác thấp hơn (journalctl theo giây; kubelet có thể làm tròn về giây) và bao gồm cả mốc, Run mới in lại một số dòng đã có. Run resume đi qua chế độ lọc:
1. dòng có timestamp nhỏ hơn mốc `floor` của `boundary`: bỏ;
2. dòng (kể cả dòng tiếp nối ngay sau nó) trùng văn bản với một dòng còn lại trong `boundary`, theo thứ tự: bỏ và gạch dòng đó khỏi `boundary` (đếm theo multiset, nên hai dòng giống hệt nhau cùng giây chỉ bị bỏ đúng hai lần);
3. dòng đầu tiên có timestamp lớn hơn `ts`, hoặc không khớp dòng nào trong `boundary`: tắt chế độ lọc, mọi dòng từ đó được giữ.

Hai luồng stdout/stderr lọc riêng, mỗi luồng một `boundary`.

**Khi không resume được** (chưa nhận dòng có timestamp, timestamp không có offset, lệnh đã sửa tay, `boundary` tràn): chạy lại nguyên văn với Break "lines may be missing or repeated", Generation mới.

*Thay thế đã cân nhắc:* (a) dùng cursor của journalctl (`--after-cursor`) — chính xác hơn nhưng cần `--show-cursor`/`-o json`, làm đổi output; (b) hỏi `date +%z` của Server rồi đổi giờ — thừa vì output đã có offset, và sai khi Server đổi múi giờ giữa chừng; (c) không lọc trùng, chỉ báo Break — người dùng đã chọn làm ngay.

### 13. Stream: đánh dấu dòng stderr
Mỗi dòng của Remote Command có `stream: Stdout | Stderr` lưu trong `LogEntry` (một byte cờ; dòng file là `None`).
- **Gutter:** số dòng của dòng stderr vẽ bằng màu `stderr` của theme (đỏ nhạt, khác màu Level Marker), tooltip "stderr", trong Parsed View, Raw View và Focus Pane. Chọn Gutter vì nó không đụng màu nền của Highlight Rule và màu chữ của Marker (rule 32), hiện ở cả ba nơi, và không đổi text đã lưu (rule 30).
- **Stream Column:** cột của app như Source Column, hiện `stdout`/`stderr` (rỗng cho dòng file), ẩn mặc định, bật trong Quick Settings, không pin được.
- **Filter:** Stream là Filterable Column khi Log Space có ít nhất một Remote Command, dù Stream Column đang ẩn hay hiện, với Filter Value `stdout` và `stderr`, đếm như mọi cột. Dòng file không có Stream nên không khớp (rule 18), ở Hide Mode thì vẫn hiện.
- **Highlight Rule:** điều kiện trên cột Stream dùng được như mọi cột, ví dụ "stream = stderr" tô đỏ nhạt.
- **Search, copy:** bỏ qua Stream (Search khớp raw line, rule 7).
- **Continuation Line:** Stream là của chính dòng, không thừa kế từ Parent Line (như Source File); mọi giá trị khác vẫn thừa kế theo rule 19.

*Thay thế đã cân nhắc:* tô màu cả row stderr — đụng Highlight Rule và Marker; thêm tiền tố `[stderr]` vào text — phá rule 30.

## Risks / Trade-offs

- **[Tiến trình mồ côi trên Server]** khi `signal` bị sshd bỏ qua (OpenSSH cũ, server không phải OpenSSH). → Đóng kênh vẫn gây SIGPIPE ở lần ghi sau; Connection Details ghi "may still be running on <server>" khi không nhận được `ExitStatus` sau 2 s.
- **[Lệnh nguy hiểm]** người dùng gõ `rm …` thành Remote Command. → App không phân tích ý nghĩa lệnh; dialog thêm hiện "Runs on web1 as deploy each time this Log Space opens" và yêu cầu bấm Add; chặn nâng quyền như mục 3. Chấp nhận: người dùng đã có quyền SSH.
- **[Mất/lặp dòng khi nối lại lệnh gõ tay]** → Break nói rõ; muốn không mất dòng thì dùng preset.
- **[Đồng hồ Server nhảy lùi]** (NTP chỉnh giờ) trong lúc mất kết nối: dòng mới có timestamp nhỏ hơn mốc bị bỏ ở bước 1 của bộ lọc trùng. → Chấp nhận; nếu Run resume bỏ hơn 1.000 dòng ở bước 1 mà chưa thấy dòng mới hơn mốc, tắt lọc và vẽ Break "server clock went back, lines may be repeated".
- **[Cờ `--since` không được hỗ trợ]** (journalctl rất cũ không nhận `@epoch`, docker/kubectl cũ). → Run resume thoát ngay với mã khác 0 và stderr chứa lỗi cờ: chạy lại nguyên văn một lần, ghi "resume not supported" vào lỗi cuối của nguồn trong Connection Details, và không thử resume cho Remote Command đó đến khi mở lại.
- **[Hai dòng giống hệt nhau cùng mốc ghi trong lúc mất kết nối]** có thể bị bỏ nhầm nếu `boundary` cũng có một dòng giống hệt. → Đếm theo multiset giới hạn sai sót ở số dòng giống hệt đã có; ghi rõ trong design.
- **[Lượng output lớn]** (`kubectl logs` của pod ồn) làm đầy bộ nhớ khi Pause. → Hàng đợi giới hạn ở Live Window.
- **[Parser không nhận timestamp của preset]** → task kiểm tra và bổ sung định dạng; nếu không được, Remote Command rơi xuống cuối timeline và bị gắn cờ như rule 22.
- **[Đo độ trễ thêm lưu lượng]** một gói mỗi 30 s mỗi Server: không đáng kể.
- **[Bộ nhớ của Stream]** → một byte trong `LogEntry` (vào phần đệm sẵn có), bitset trong `SourceEvent`.
- **[Archive phụ thuộc]** delta của `remote-log-space`, `ssh-server`, `live-log` chỉ ADDED vì các spec này chưa có trong `openspec/specs/`; change này phải archive sau `add-remote-live-log`.

## Migration Plan

- `RemoteRef { server_id, path }` cũ đọc thành `RemoteTarget::File { path }` (serde `untagged`/`default` cho trường `target`).
- `Server` thêm hai trường keepalive với `serde(default)`.
- Rollback: gỡ change; session có Remote Command lưu bởi bản mới sẽ bị bản cũ bỏ qua (cần serde bỏ qua thành viên không đọc được thay vì lỗi cả session — kiểm tra trong task).

## Spike results (task 1.1, `examples/exec_spike.rs`, russh 0.64.1 against macOS OpenSSH)

- Exec channel: `exec(true, cmd)` then `eof()` works. `ChannelMsg::Data` is stdout, `ChannelMsg::ExtendedData { ext: 1 }` is stderr; they arrive as separate messages, so one `LineSplitter` per stream is enough. Order after the command: `Eof`, `ExitStatus`, `Close`. A command that reads stdin (`cat`) sees EOF at once and ends.
- `channel.signal(Sig::TERM)` stops `sleep 30` within 2 ms: the channel reports `ExitSignal { TERM }` then `Close`, and `pgrep` finds no leftover process.
- Latency: `Handle::send_global_request("keepalive@openssh.com", &[], true)` waits for the reply. OpenSSH answers `REQUEST_FAILURE`, which russh returns as `Err(Error::RequestDenied)`; that is still one round trip, so both `Ok` and `RequestDenied` count as a measurement and any other error means the session is lost. `Handle::send_ping()` also exists but cannot tell a lost session from a reply, so the global request is used. Fallback (open and close a session channel) is not needed.

## Open Questions

- Màu `stderr` của Gutter: dùng một màu mới trong theme hay tái dùng màu cảnh báo nhạt? Chốt khi làm UI.
