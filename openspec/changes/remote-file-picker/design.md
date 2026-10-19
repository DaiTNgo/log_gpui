## Context

Bộ chọn file trên Server (`src/app/remote_picker.rs`, view ở `src/app/view/server_views.rs`) được làm trong change `add-remote-live-log`. Hiện tại:

- `open_remote_picker` luôn chọn Server đầu tiên và `picker_load(None)` mở thư mục home (`sftp.canonicalize(".")`); không có gì được nhớ giữa các lần mở.
- `list_dir` (`src/remote/browse.rs`) trả `DirEntry { name, is_dir, size, modified }` đã sắp cố định: thư mục trước, rồi theo tên. Không có lọc.
- `resolve_input` biến ô đường dẫn thành danh sách đường dẫn: glob (chỉ ở tên file) được khớp một lần; đường dẫn thư mục bị báo lỗi "is a folder".
- `picker.staged` là `Vec<(server_id, path)>` và đã cho trộn nhiều Server, nhưng mỗi lần thêm chỉ trên Server đang chọn.
- Remote File được lưu là `LogFile { remote: Some(RemoteRef { server_id, path }) }` trong Remote Log Space; `add_remote_files` bỏ file trùng theo `file_id`.
- `RemoteFileSource` (`src/live/remote.rs`) đã phát hiện xoay vòng/xoá bằng `stat` inode qua exec (dự phòng bằng kích thước) và chờ file khi nó bị xoá (`WaitingForFile`).

Ràng buộc: L1 (Pause theo Log Space), L4–L5, L11, L13 (Live Window theo từng file), dữ liệu Project cũ phải đọc được, và các change song song (`ssh-connect-ergonomics`, `remote-full-read`, `remote-command-source`) có thể đụng tới Server và Remote File.

## Goals / Non-Goals

**Goals:**
- Giảm số thao tác để mở lại đúng log đã dùng: thư mục cuối và Recent Paths theo Server.
- Tìm file nhanh trong thư mục đông file: lọc khi gõ, sắp xếp, lối tắt.
- Thêm cùng một đường dẫn trên nhiều Server trong một lần.
- Live Glob: file mới khớp glob tự vào Remote Log Space, với hành vi rõ ràng cho xoá, xoay vòng, gỡ, Pause và mở lại.

**Non-Goals:**
- Glob đệ quy (`**`) hoặc glob ở phần thư mục.
- Đánh dấu thư mục yêu thích hay lối tắt do người dùng tự định nghĩa (ngoài Recent Paths).
- Tìm file theo nội dung trên Server, hay theo dõi thư mục bằng inotify.
- Đọc file nén/Archive trên Server.
- Thay đổi Live Window (thuộc `remote-full-read` nếu có).

## Decisions

### D1. Lưu thư mục cuối, Recent Paths và cách sắp xếp ở `picker_state.json` cấp app
Một file JSON mới ở thư mục dữ liệu app, cạnh `servers.json`: `{ sort: { key, descending }, servers: { <server_id>: { last_dir, recent: [ { path, is_glob } ] } } }`. Khoá theo `server_id` (ổn định khi đổi tên Server). Xoá Server thì xoá mục của nó.
- *Thay vì* nhét vào `Server` trong `servers.json`: tách riêng để không trộn cấu hình kết nối với lịch sử dùng, và tránh đụng change `ssh-connect-ergonomics` đang sửa mô hình Server.
- *Thay vì* lưu theo Project: Server dùng chung mọi Project, nên người dùng mong thư mục cuối cũng dùng chung.
- Thư mục cuối được ghi mỗi lần liệt kê thành công (không chỉ khi xác nhận), vì người dùng thường đóng bộ chọn rồi mở lại để tìm tiếp. Recent Paths chỉ được ghi khi `picker_confirm` thật sự thêm (không ghi khi chỉ đưa vào danh sách chờ).
- Ghi file theo kiểu ghi tạm rồi đổi tên, như `servers.json`.

### D2. Lọc và sắp xếp ở phía client
`list_dir` trả danh sách thô; `RemotePicker` giữ `entries` gốc và tính `visible_entries()` = lọc (chuỗi con không phân biệt hoa thường, hoặc `glob_match` khi `is_glob`) rồi sắp. Không gửi lại yêu cầu tới Server khi gõ hay đổi sắp xếp. Thư mục luôn trước file; khi sắp theo kích thước thì thư mục sắp theo tên (SFTP trả kích thước thư mục vô nghĩa). Mục `..` là một hàng cố định của view, không phải `DirEntry`, nên không bị lọc.
- Hàm lọc/sắp là hàm thuần trong `browse.rs` để test không cần Server.
- Thư mục có hàng chục nghìn file: danh sách vẽ bằng `uniform_list` (ảo hoá) nên lọc mỗi lần gõ trên vài chục nghìn tên vẫn đủ nhanh; nếu không, debounce 50 ms.

### D3. Ô đường dẫn: thư mục thì mở, glob thì thành Live Glob
`resolve_input` đổi kiểu trả về thành `enum Resolved { Dir(String), Files(Vec<String>), Glob { dir, pattern } }`. `Dir` → `picker_load(Some(dir))`. `Glob` chỉ kiểm tra thư mục tồn tại (không bắt buộc có file khớp — L11 mở rộng). Ô lọc của danh sách là cách để "khớp một lần": người dùng lọc bằng glob rồi tích file, nên không cần thêm công tắc "chỉ khớp một lần" ở ô đường dẫn.
- *Thay vì* công tắc "Watch for new files" cạnh glob: hai cách cho cùng một ô dễ gây nhầm; tách theo vị trí (ô đường dẫn = live, ô lọc = chọn tay) rõ hơn.

### D4. Nhiều Server: tích Server ở hàng thêm, kiểm tra song song
Hàng "Add" của ô đường dẫn có một danh sách Server dạng checkbox (Server đang chọn luôn được tích). `picker_add_typed` chạy `resolve_input` cho từng Server đã tích song song trên `RemoteHub` (mỗi Server một session như hiện tại), gom kết quả: thành công vào `staged`, lỗi thành dòng báo `"<server>: <message>"`. Bấm một mục Recent Paths dùng cùng đường đi này. Duyệt thư mục và tích file vẫn chỉ trên Server đang chọn (một thư mục trên nhiều Server không có cùng nội dung).
- `staged` đổi thành `Vec<Staged>` với `enum Staged { File { server_id, path }, Glob { server_id, dir, pattern } }`.

### D5. Live Glob là một mục riêng của Remote Log Space, file nó thêm mang `glob_id`
`LogSpace` thêm `globs: Vec<LiveGlob>` với `LiveGlob { glob_id, server_id, dir, pattern, excluded: Vec<String>, stopped: bool }` (`#[serde(default)]`). `LogFile` thêm `from_glob: Option<String>` (`#[serde(default)]`). File thêm bằng đường dẫn có `from_glob = None`. Trùng được kiểm tra theo `file_id` như hiện tại, nên một đường dẫn đã có thì giữ nguồn gốc ban đầu.
- Trên đĩa, chỉ Live Glob và `excluded` cần bền; danh sách file của Live Glob được dựng lại khi mở (spec "mở lại Project"), nhưng vẫn lưu `LogFile` của chúng để giữ `file_id` ổn định và Marker (Marker của dòng đã rời thì vốn không khôi phục).
- Gỡ một file có `from_glob` thêm đường dẫn của nó vào `excluded`. Gỡ Live Glob gỡ mọi file có `from_glob` trùng.
- Thêm lại một đường dẫn (bộ chọn, Recent Paths) mà đường dẫn đó nằm trong `excluded` của một Live Glob cùng Server trong Log Space đích: `add_remote_files` bỏ nó khỏi `excluded` và tạo `LogFile` với `from_glob` của Live Glob đó, thay vì một file thêm bằng đường dẫn. Không cần nút riêng.

### D6. `GlobWatcher`: liệt kê thư mục định kỳ, gộp theo (Server, thư mục)
Một task trên `RemoteHub` cho mỗi cặp (Server, thư mục) có ít nhất một Live Glob đang mở, liệt kê thư mục qua SFTP theo setting nhịp quét (mặc định 5 s; spec hứa ≤ hai nhịp) và gửi về app `GlobEvent::Listing { server_id, dir, entries }` qua channel sự kiện sẵn có. App so khớp mọi Live Glob của thư mục đó với danh sách và quyết định thêm/Gone. Đặt logic quyết định trong một hàm thuần `plan_glob(glob, files, listing, inodes) -> GlobPlan { add, gone, pending }` để test không cần SSH.
- *Thay vì* `inotifywait` qua exec: không có trên mọi Server, và sẽ cần quyền/gói cài thêm; polling đủ cho log (độ trễ vài giây chấp nhận được).
- *Thay vì* mỗi `RemoteFileSource` tự tìm file: glob là thuộc tính của Log Space, không của file.
- Watcher dừng khi Log Space bị Pause (L1), đóng, hoặc khi mọi Live Glob của thư mục bị gỡ hoặc bị dừng (D12); khi Resume hay Resume watching thì liệt kê ngay một lần.
- Thư mục mất: watcher báo `GlobStatus::WaitingForFolder` và thử lại theo cùng nhịp; các file con tự thành Gone theo D7.

### D7. Gone: trạng thái mới của nguồn, chỉ cho file thuộc Live Glob
`RemoteFileSource` nhận cờ `from_glob`. Khi file bị xoá: file thường giữ `WaitingForFile` như cũ (L5); file thuộc glob phát `Break(Deleted)` rồi `Status(Gone)` và tiếp tục kiểm tra đường dẫn với nhịp chậm (cùng nhịp watcher). `SourceStatus::Gone` có trọng số thấp nhất (ngang Live) trong `worst()`, nên không làm status bar đỏ; vẫn được liệt kê khi hover. Nếu đường dẫn có lại file → Generation mới như L5.
- Gone file không chiếm chỗ trong giới hạn số file (D9) và bị bỏ qua khi "Download all".
- Khi mở lại Project, file Gone không có trong danh sách khớp nên không được thêm lại (D5).

### D8. Không đọc một file hai lần: Rotated File theo tên, rồi inode
Bước đầu là quy tắc nhận biết Rotated File của change `remote-full-read` (`<name>.<N>`, `<name>-<YYYYMMDD>`, `<name>-<YYYY-MM-DD>`, có thể kèm đuôi nén): trong một lần quét, mọi ứng viên là Rotated File của một file khác cũng khớp (hoặc của một Remote File đã có trong Log Space) bị loại khỏi `add`, vì nó là Generation cũ hơn của file đó (đọc qua "Load earlier generation"/Open in full của `remote-full-read`), không phải Source File riêng. Hàm nhận biết được dùng chung, không viết lại. Nếu `remote-full-read` chưa được áp dụng, bước này bị bỏ qua và chỉ còn inode.

Sau đó là inode, để bắt các kiểu đổi tên quy tắc tên không thấy. App giữ, cho mỗi Live Glob đang mở, tập `inode` của mọi file nó đã đọc (mọi Generation), lấy từ sự kiện mà `RemoteFileSource` đã có khi `stat`. Watcher lấy inode của ứng viên mới bằng cùng lệnh `stat` qua exec, một lệnh cho cả lô tên. Quy tắc:
1. Ứng viên mới có inode trong tập → không thêm (file cũ đã xoay vòng).
2. Một file của Live Glob phát hiện xoay vòng mà inode mới thuộc tập (file ở đường dẫn này giờ là file đã đọc dưới tên khác, ví dụ `app.log.1` nhận nội dung cũ của `app.log`) → chuyển Gone thay vì mở Generation mới.
3. Không có inode (exec bị tắt, `stat` lạ) → coi là file mới; ghi rõ trong Risks.
- *Thay vì* chỉ dùng tên hoặc chỉ dùng inode: tên khớp với cách `remote-full-read` hiện Rotated File như Generation cũ (một nguồn sự thật cho khái niệm Rotated File), còn inode bắt các kiểu đổi tên tuỳ ý.
- Vì Rotated File không bao giờ thành Remote File của glob, quy tắc 2 hầu như chỉ xảy ra với kiểu đổi tên tuỳ ý.

### D9. Giới hạn số file và nhịp quét là setting cấp app
Mỗi file có Live Window riêng (L13), nên glob `*` trên thư mục lớn có thể làm bộ nhớ vượt kiểm soát (50 × 100.000 dòng đã là 5 triệu dòng). Hai setting mới cạnh Live Window trong Settings: "Live Glob file limit" (mặc định 50, khoảng 1–500) và "Live Glob scan interval" (mặc định 5 s, khoảng 2–300 s), lưu cùng chỗ với Live Window. Giới hạn đếm file không Gone; chọn theo thời gian sửa mới nhất; phần còn lại là `pending` và được báo trên status bar.
- Tăng giới hạn: lần quét kế tiếp lấp chỗ trống từ `pending`. Giảm giới hạn: không gỡ file đang đọc (gỡ sẽ làm mất dòng mà người dùng không yêu cầu), chỉ ngừng thêm cho đến khi xuống dưới giới hạn.
- Đổi nhịp: gửi lệnh tới mọi `GlobWatcher` đang chạy; thư mục > 10.000 mục vẫn dùng ít nhất 30 s.

### D10. Bỏ qua file nén không phải Rotated File
File nén là Rotated File đã bị loại ở D8 (và `remote-full-read` đọc chúng như Generation cũ). Với ứng viên còn lại, trước khi thêm, watcher đọc tối đa 512 byte đầu qua SFTP và bỏ qua nếu là magic của gzip, zip, bzip2, xz, zstd hoặc tar (dùng chung bộ nhận diện của Archive import). File rỗng được thêm (log mới tạo thường rỗng).

### D11. Sidebar
Live Glob hiện như một nút gom `web1:/var/log/app/*.log` có thể thu gọn, các Remote File nó thêm nằm bên dưới; menu của nút: "Stop watching"/"Resume watching" và "Remove Live Glob". Live Glob bị dừng có nhãn "stopped" cạnh tên. File Gone hiện mờ. Source Column và File Section vẫn dùng `<server>:<path>` như mọi Remote File.

### D12. Stop watching là cờ của Live Glob, không dính tới Pause
`LiveGlob.stopped` được lưu trong Project. Khi `stopped`, `plan_glob` trả `add` và `pending` rỗng; watcher của thư mục chỉ chạy nếu còn Live Glob khác không dừng cùng thư mục. Các `RemoteFileSource` của file đã có không bị đụng tới nên vẫn live, vẫn có thể thành Gone (D7 nằm ở nguồn, không ở watcher). "Resume watching" xoá cờ, khởi động watcher nếu cần, và liệt kê ngay; file sinh ra trong lúc dừng được mở từ đầu trong Live Window như sau Pause. Khi mở lại Project, Live Glob bị dừng không khớp lại: chỉ các `LogFile` có `from_glob` của nó được mở lại, và file không còn trên Server bị bỏ (cùng quy tắc với Gone).
- *Thay vì* "chuyển các file thành file thêm bằng đường dẫn rồi xoá glob": giữ glob cho phép Resume, giữ `excluded`, và giữ nhóm trên sidebar.
- *Thay vì* dùng Pause: Pause dừng dòng của mọi file (L1); Stop watching chỉ dừng việc thêm file.

## Risks / Trade-offs

- [Polling thư mục tạo tải SFTP] → gộp theo (Server, thư mục), chỉ chạy khi Log Space mở, không Pause và còn Live Glob không dừng; nhịp chỉnh được trong Settings; thư mục > 10.000 mục giãn nhịp lên ít nhất 30 s.
- [Không có inode thì file đổi tên kiểu tuỳ ý có thể bị đọc lại, gây dòng trùng] → Rotated File đã bị loại theo tên (D8), nên chỉ còn các kiểu đổi tên lạ trên Server tắt exec; ghi chú trong UI khi exec không có ("rotation detection limited"); gợi ý mẫu hẹp hơn (`*.log`).
- [Đổi hành vi glob là BREAKING với người đã quen glob khớp một lần] → Project cũ không có Live Glob (glob cũ đã được lưu thành file thường), nên không có gì đổi với dữ liệu cũ; ô lọc giữ cách chọn một lần.
- [Live Glob tự thêm file làm view đột ngột có dòng mới từ file lạ] → file mới vào nhóm của Live Glob trong sidebar và chèn theo timestamp; Follow vẫn theo L2.
- [Recent Paths lộ đường dẫn của Server khác Project] → chấp nhận: Server vốn dùng chung giữa Project; cho xoá từng mục và cả danh sách.
- [Quy tắc tên Rotated File của `remote-full-read` thay đổi hoặc chưa có] → Live Glob gọi chung một hàm nhận biết; khi chưa có thì dựa vào inode.
- [Change song song sửa cùng requirement] → change này chỉ MODIFY "Chọn file trên Server" của `remote-log-space`; mọi thứ khác là ADDED.

## Migration Plan

- `picker_state.json` chưa có → coi như rỗng (home dir, không Recent Paths, sắp theo tên).
- Project cũ: `globs` và `from_glob` mặc định rỗng/`None`.
- Change này phải được archive sau `add-remote-live-log` (capability `remote-log-space` chưa có trong `openspec/specs/`), và nên được áp dụng sau `remote-full-read` để dùng chung nhận biết Rotated File.
- Khi áp dụng, cập nhật `CONTEXT.md` theo mục "Domain changes" của proposal (luật L20–L23).

## Open Questions

Không còn câu hỏi mở. Đã chốt với người dùng: có Stop watching (D12); thêm lại đúng đường dẫn để bỏ loại trừ (D5); giới hạn số file và nhịp quét là setting cấp app (D9); file Gone ở lại tới khi đóng Log Space (D7); lối tắt chỉ Home và `/var/log`, Recent Paths lo phần còn lại.
