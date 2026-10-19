## Why

Nhiều log trên server không nằm trong file mà chỉ đọc được qua lệnh: `journalctl` (systemd), `docker logs`, `kubectl logs`. Remote Log Space hiện chỉ nhận Remote File, nên người dùng vẫn phải mở terminal và mất Filter, Search, Marker và timeline gộp. Ngoài ra, khi kết nối chập chờn, status bar chỉ cho biết "Reconnecting", không cho biết Server nào chậm, đã nối lại bao nhiêu lần hay lỗi cuối cùng là gì, nên khó phân biệt lỗi mạng, lỗi Server hay lỗi lệnh.

## What Changes

- Thêm loại thành viên thứ hai của Remote Log Space: **Remote Command**, một lệnh chạy trên một Server mà output được đọc live như một file. Gọi bằng `<server>$ <lệnh>`, ví dụ `web1$ journalctl -f -u nginx`.
- Thêm Remote Command bằng cách gõ lệnh hoặc chọn **Command Preset**: journalctl (unit), Docker (container), Kubernetes (pod, namespace, container). Preset điền sẵn các cờ cần cho log viewer (theo dõi, có timestamp, số dòng ban đầu) và gợi ý unit/container/pod lấy từ Server. Lệnh cuối cùng luôn hiện đầy đủ và sửa được trước khi thêm.
- An toàn: lệnh chạy bằng user của Server qua kênh exec, không PTY, stdin đóng ngay; app từ chối lệnh bắt đầu bằng `sudo` (kể cả `sudo -n`), `su`, `doas`, `pkexec`, `run0` (L9, tuyệt đối). App không bao giờ tự chạy lệnh mà người dùng chưa gõ hoặc chưa xác nhận; preset chỉ chạy lệnh liệt kê (gợi ý) chỉ đọc, đã biết trước.
- Mỗi lần chạy lệnh là một **Run**. Run chạy mới bắt đầu một Generation: số dòng là thứ tự dòng trong output, bắt đầu từ 1. Không có nạp ngược; Live Window áp dụng cho từng Remote Command như Remote File.
- Lệnh kết thúc: hiện "Exited (code N)" và nút Restart. Tuỳ chọn **Restart on exit** (mặc định tắt) tự chạy lại với backoff. Restart và nối lại sau mất kết nối đều chạy lại lệnh và vẽ Break.
- **Resume by time**: Remote Command sinh từ Command Preset và chưa bị sửa tay, khi chạy lại tự động (sau mất kết nối hoặc Restart on exit), được chạy với `--since`/`--since-time` lấy từ timestamp của dòng cuối đã nhận (theo đồng hồ và offset của chính Server, có sẵn trong output), và các dòng trùng ở ranh giới bị bỏ. Sau mất kết nối, Run tiếp tục cùng Generation và số dòng, nên không mất, không lặp dòng. Lệnh gõ tay vẫn chạy lại nguyên văn với Break "lines may be missing or repeated".
- **Stream**: mỗi dòng của Remote Command biết nó đến từ stdout hay stderr. Dòng stderr có số dòng ở Gutter màu stderr; **Stream Column** (ẩn mặc định, bật trong Quick Settings) hiện `stdout`/`stderr`, lọc được bằng Filter và dùng được trong Highlight Rule.
- Pause với Remote Command: lệnh vẫn chạy, dòng được giữ ở nền (tối đa Live Window) và nạp bù khi tiếp tục.
- Thêm **Connection Details**: popover mở từ trạng thái kết nối trên status bar, liệt kê từng Server của Log Space (trạng thái, độ trễ, số lần nối lại, lỗi cuối và thời điểm, keepalive, chuỗi Jump Server) và từng Remote File/Remote Command (trạng thái, số lần nối lại/chạy lại, lỗi cuối, mã thoát, thời điểm nhận dòng cuối), kèm nút Reconnect/Restart. Nhãn trạng thái trên status bar kèm độ trễ, ví dụ "Live · 42 ms".
- Keepalive SSH chỉnh được cho từng Server (khoảng gửi, số lần mất trước khi coi là đứt); độ trễ đo bằng chính keepalive.
- Download all bỏ qua Remote Command (không có file để tải) và nói rõ điều đó.

## Domain changes

Phần này mô tả thay đổi cho CONTEXT.md; chưa sửa CONTEXT.md vì các change khác đang viết song song. Áp dụng khi change được archive.

**Thuật ngữ mới (nhóm Spaces):**
- **Remote Command**: A command the user typed or picked from a Command Preset, run on one Server as a member of a Remote Log Space, whose output is read live as lines. Named `<server>$ <command>`. _Avoid_: Remote stream, exec source, command tail, remote process.
- **Command Preset**: A ready-made Remote Command form for a common log command (journalctl, Docker, Kubernetes) that fills in the flags a log viewer needs and suggests values found on the Server. The user always sees and can edit the final command. _Avoid_: Template, shortcut, integration.
- **Run** (nhóm Live): One execution of a Remote Command, from start to exit, restart or lost connection. A fresh Run starts a new Generation whose lines are numbered from 1 in the order they were output; a Run resumed by time after a lost connection continues the Generation and numbering of the Run before it. _Avoid_: Session, process, instance.
- **Resume By Time** (nhóm Live): Running an unedited Command Preset again from the timestamp of the last line already received, written in the Server's own clock and offset, and dropping the lines at that boundary that were already received. _Avoid_: Catch-up, replay, since mode.
- **Stream** (nhóm Lines): Whether a line of a Remote Command came from the command's standard output or its standard error. Lines of files have no Stream. _Avoid_: Channel, fd, output type.
- **Stream Column**: The column in a Log Space's Parsed View that shows each Remote Command line's Stream (`stdout` or `stderr`). Hidden by default and shown from Quick Settings; filterable, like the Source Column. _Avoid_: stderr column, fd column.
- **Connection Details** (nhóm Live): The popover opened from the connection state in the status bar that lists, for the open Remote Log Space, each Server's connection health and each Remote File's and Remote Command's state. _Avoid_: Connection panel, network inspector, diagnostics.

**Thuật ngữ sửa:**
- **Remote Log Space**: "…whose files live on servers…" → "whose members are Remote Files and Remote Commands on servers reached over SSH. Its members are read-only, can come from several servers, and keep receiving new lines while the Log Space is open."
- **Source File**: "The file inside a Log Space that a row came from" → "The file, or Remote Command, inside a Log Space that a row came from." (Source Column, File Section, File View dùng chung, hiện tên `<server>$ <command>`.)
- **Generation**: thêm câu "For a Remote Command, a fresh Run starts a Generation; a Run resumed by time after a lost connection continues it."
- **Gutter**: thêm "The line number of a stderr line is drawn in the stderr color."
- **Quick Settings**: "such as showing the Source Column and density" → "such as showing the Source Column, the Stream Column and density".
- **Live Window**: "The newest lines a Remote Log Space holds" giữ nguyên; áp dụng cho từng Remote File và từng Remote Command.

**Luật sửa:**
- **L4 Remote Opens At The End**: thêm "A Remote Command has no older lines to fill in: it shows what its command outputs, and its line numbers count the lines of the current Run."
- **L6 Reconnect Without Loss**: thêm (bổ sung, không thay câu nào, tương thích với phần ssh-connect-ergonomics thêm cho Server cần Sign-in Prompt) "A Remote Command from an unedited Command Preset is resumed by time (L30) and continues its Generation. A typed or edited Remote Command cannot resume where it stopped: it is run again as a new Run, and the Break says lines may be missing or repeated."
- **L7 Live Status In The Status Bar** → "The connection state of a Remote Log Space, its Connection Details, and the Follow and Pause switches are shown in the status bar only (Connection Details opens from it). The sidebar, tabs and dialogs never show live connection state."
- **L9 Readable Without Elevation**: thêm "A Remote Command runs as the Server's own user, without a terminal and with no input; a command starting with `sudo` (even `sudo -n`), `su`, `doas`, `pkexec` or `run0` is refused."
- **L10 Live Is A Window, Not An Archive**: thêm "A Remote Command has nothing to download; to keep its lines, the user runs a command that writes a file and adds that file."
- **L13 Window Per Remote File**: "applied to each Remote File separately" → "applied to each Remote File and each Remote Command separately".
- **L14 Download For Full Reading**: thêm "Download all skips Remote Commands."
- **Rule 17 Quick Settings Only**: "Quick Settings holds only display switches (Source Column, density)" → "(Source Column, Stream Column, density)".
- **Rule 19 Continuation Lines Inherit**: thêm "Stream and Source File are a line's own and are never inherited."
- **Rule 24 Space Kinds Never Mix**: "a Remote Log Space only gains Remote Files" → "a Remote Log Space only gains Remote Files and Remote Commands".

**Luật mới:**
- **L28 Commands Are Explicit**: A Remote Command runs only the text the user typed or confirmed from a Command Preset, as shown. The one change the app makes is Resume By Time on an unedited Command Preset, which swaps its initial-lines flag for a since flag; Connection Details shows the command of the current Run. The app runs nothing else on the user's behalf except the fixed read-only commands it needs (`stat`, `wc -l`, and a Command Preset's listing of units, containers or pods).
- **L29 Commands Restart Only When Asked**: When a Remote Command exits, its lines stay and its state shows the exit code. It runs again only when the user presses Restart, when Restart on exit is on, or after a lost connection; every new Run starts after a Break.
- **L30 Presets Resume By Time**: When an unedited Command Preset runs again by itself (after a lost connection or with Restart on exit), it starts from the timestamp of the last line received, as the Server wrote it, and lines at that boundary already received are dropped. Times are never converted between Servers or to the local clock (L8). A typed or edited command, or one with no timestamped line received yet, runs again as it is.
- **L31 Stream Is The Line's Own**: Each line of a Remote Command keeps the Stream it came from. A stderr line shows its line number in the stderr color; the Stream Column, Filter and Highlight Rules can use its Stream; Search and copy ignore it. Lines of files have no Stream, so a Filter on Stream never matches them (rule 18).

**ADR:** không cần ADR mới; ADR 0009 vẫn đúng (lệnh chạy qua SSH client nhúng, kênh exec).

## Capabilities

### New Capabilities
- `remote-command`: Remote Command trong Remote Log Space: thêm bằng gõ hoặc Command Preset, tên hiển thị, an toàn (không nâng quyền, không PTY, không stdin), Run và số dòng, thoát/Restart/Restart on exit, nối lại, Resume By Time, Pause, Live Window, Marker, mở lại Project.
- `command-stream`: Stream của dòng Remote Command: màu số dòng stderr, Stream Column, Filter và Highlight Rule theo Stream.
- `connection-details`: độ trễ, số lần nối lại, lỗi cuối theo Server và theo nguồn; nhãn trên status bar; popover Connection Details với Reconnect/Restart.

### Modified Capabilities
Các capability dưới đây chưa có trong `openspec/specs/` vì change `add-remote-live-log` chưa archive; delta chỉ dùng ADDED và phải archive sau `add-remote-live-log`.
- `remote-log-space`: Remote Log Space nhận cả Remote Command; Download all bỏ qua Remote Command.
- `ssh-server`: keepalive chỉnh được cho từng Server (thuộc change này, không thuộc ssh-connect-ergonomics).
- `live-log`: Break của Remote Command (restarted, exited) và trạng thái live riêng của Remote Command.

## Impact

- Code: `src/live/` (nguồn mới `RemoteCommandSource` dùng `LineSource`/`SourceEvent`, thêm trạng thái `Exited`, Break `Restarted`/`Exited`), `src/remote/hub.rs` (exec dạng stream, signal, đo độ trễ, đếm nối lại, keepalive theo Server), `src/remote/server.rs` (trường keepalive), `src/workspace/log_space.rs` (`RemoteRef` thành file hoặc lệnh, persist), `src/app/live.rs`, `src/app/view/status_bar.rs` (nhãn kèm độ trễ, popover Connection Details), dialog thêm Remote File (tab Command), `src/remote/download.rs` (bỏ qua lệnh).
- `src/filter_index.rs`, Parsed View và Gutter (Stream Column, màu stderr), Quick Settings.
- Không thêm dependency.
- Dữ liệu lưu: session lưu thêm Remote Command (`server_id`, lệnh, các trường preset nếu lệnh chưa bị sửa, Restart on exit); `servers.json` thêm keepalive với giá trị mặc định qua `serde(default)`. Số liệu Connection Details không được lưu.
- Tài liệu: CONTEXT.md theo mục "Domain changes" ở trên.
