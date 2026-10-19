## Why

Bộ chọn file trên Server hiện bắt người dùng làm lại từ đầu mỗi lần: luôn mở ở thư mục home, không nhớ đường dẫn đã dùng, không lọc hay sắp xếp được danh sách, phải chọn cùng một đường dẫn lặp lại cho từng Server, và glob chỉ khớp một lần nên file log mới sinh ra (log theo ngày, log của instance mới) bị bỏ sót. So với các công cụ đọc log từ xa khác, đây là chỗ bất tiện nhất khi dùng Remote Log Space hằng ngày.

## What Changes

- Bộ chọn nhớ **thư mục cuối** của từng Server và mở lại ở đó; giữ danh sách **đường dẫn gần đây** (file, glob) theo từng Server để thêm lại bằng một cú bấm. Dữ liệu này ở cấp ứng dụng như danh sách Server.
- Danh sách thư mục có ô **lọc khi gõ** (chuỗi con không phân biệt hoa thường, hoặc glob khi có ký tự glob), **sắp xếp** theo tên / thời gian sửa / kích thước (tăng hoặc giảm, thư mục luôn ở trên), và **lối tắt** tới Home và `/var/log`. Gõ đường dẫn một thư mục vào ô đường dẫn sẽ mở thư mục đó thay vì báo lỗi.
- **Cùng đường dẫn trên nhiều Server**: khi thêm một đường dẫn hoặc glob, người dùng tích nhiều Server và đường dẫn đó được thêm trên từng Server đã tích, mỗi Server thành một Remote File (hoặc Live Glob) riêng.
- **BREAKING (hành vi)** — **Live Glob**: glob gõ vào ô đường dẫn không còn khớp một lần nữa mà được gắn với Remote Log Space. Mọi file khớp, kể cả file sinh ra sau, tự được thêm làm Remote File. File khớp bị xoá trên Server chuyển sang trạng thái Gone thay vì chờ mãi; Live Glob không bao giờ thêm một Rotated File (theo change `remote-full-read`) làm Remote File riêng, và không đọc lại một file đã đọc dưới tên khác (nhận biết qua inode); mỗi file vẫn có Live Window riêng; số file đang đọc của một Live Glob và nhịp quét thư mục là hai setting cấp app (mặc định 50 file, 5 giây).
- **Stop watching**: người dùng dừng được một Live Glob — các file hiện có vẫn đọc live nhưng không thêm file khớp mới — và cho chạy lại được ("Resume watching"). Trạng thái này được nhớ khi mở lại Project.
- Gỡ một file khỏi Live Glob loại nó khỏi glob; thêm lại đúng đường dẫn đó trong bộ chọn sẽ bỏ loại trừ và đưa file về lại nhóm của Live Glob. Muốn chọn một lần theo mẫu thì dùng ô lọc của danh sách rồi tích file.

## Domain changes

Các thay đổi dưới đây cần được đưa vào `CONTEXT.md` khi change này được áp dụng (không sửa trực tiếp lúc này vì các change khác đang viết song song):

- **Thuật ngữ mới — Live Glob**: *A glob on one Server, kept by a Remote Log Space, whose matching files are the space's Remote Files, including files that appear after it was added. The glob may only be in the file name.* _Avoid_: Wildcard, watch, pattern, auto-add.
- **Thuật ngữ mới — Gone**: *The state of a Remote File that came from a Live Glob and no longer exists on its Server. Its lines stay until the Log Space is closed; it is not brought back when the Project is reopened.* _Avoid_: Deleted, missing, stale.
- **Thuật ngữ mới — Stopped Live Glob**: *A Live Glob the user has stopped watching: the files it already brought keep reading live, but it adds no new matching files until the user resumes it. Stopping is not Pause: lines keep arriving.* _Avoid_: Paused glob, frozen glob, disabled glob.
- **Thuật ngữ mới — Recent Paths**: *The paths and Live Globs last added from one Server, offered again in the file picker. Kept per Server for the whole app.* _Avoid_: History, bookmarks, favorites.
- **Sửa Remote File**: thêm câu "A Remote File is either added by its path or brought in by a Live Glob."
- **Sửa L11** thành: *L11. **Paths May Not Exist Yet**: A Remote File or Live Glob may be added before any file exists; it waits and starts reading when a file appears. A Server that cannot be reached or a folder that does not exist is refused when adding.*
- **Luật mới L20. Globs Stay Live**: *A Live Glob keeps adding the files that come to match it while the Log Space is open and when it is reopened. A matching file is never added twice; a Rotated File is never added as its own Remote File, since it is an earlier Generation of the file it belongs to (see Rotated File); a file already read under another name is never read again; and a file removed by the user is not added again by its Live Glob unless the user adds that exact path again. How many files one Live Glob reads at once and how often its folder is checked are app-wide settings. Each file it brings has its own Live Window (L13).*
- **Luật mới L21. Gone Is Not An Error**: *A file of a Live Glob that disappears from its Server becomes Gone: its lines stay, a Break is drawn, and it does not count toward the worst state in the status bar. If a file appears again at the same path it continues as a new Generation (L5).*
- **Luật mới L22. One Pick, Many Servers**: *A path or Live Glob picked once can be added on several Servers at once; each Server gets its own Remote File or Live Glob, named `<server>:<path>`.*
- **Luật mới L23. Stopping A Glob Keeps Its Files**: *A Live Glob can be stopped and resumed. While stopped, its files keep reading live and can still become Gone, but no new matching file is added. Resuming checks the folder at once and reads files that appeared meanwhile from their start, within the Live Window. A stopped Live Glob stays stopped when the Project is reopened, and reopens only the files it already had that still exist.*

## Capabilities

### New Capabilities
- `remote-file-picker`: bộ chọn file trên Server — nhớ thư mục cuối và Recent Paths theo Server, lọc khi gõ, sắp xếp, lối tắt thư mục, mở thư mục từ ô đường dẫn, thêm cùng đường dẫn trên nhiều Server.

### Modified Capabilities
- `remote-log-space`: requirement "Chọn file trên Server" đổi glob từ khớp một lần sang Live Glob; thêm requirement cho Live Glob (thêm file mới, file Gone, Rotated File và xoay vòng, gỡ và thêm lại file, Stop watching, setting giới hạn số file và nhịp quét, Pause, mở lại Project). Capability này đang nằm trong change `add-remote-live-log` chưa archive, nên change này phải được archive sau.

## Impact

- Code: `src/remote/browse.rs` (lọc, sắp xếp, mở thư mục từ đường dẫn), `src/app/remote_picker.rs` và `src/app/view/server_views.rs` (UI bộ chọn, chọn nhiều Server), `src/workspace/log_space.rs` (lưu Live Glob và danh sách loại trừ trong Remote Log Space), module mới theo dõi Live Glob trong `src/live/` hoặc `src/remote/`, status bar (trạng thái Gone và giới hạn glob), Settings (hai setting của Live Glob).
- Dữ liệu: file mới ở thư mục dữ liệu app cho thư mục cuối và Recent Paths theo Server; dữ liệu Project thêm trường cho Live Glob, đọc được Project cũ (`#[serde(default)]`).
- Tải Server: mỗi thư mục có Live Glob được liệt kê qua SFTP định kỳ (một lần cho mọi Live Glob cùng thư mục trên cùng Server).
- Phụ thuộc change `remote-full-read`: dùng quy tắc nhận biết Rotated File của change đó. Nếu `remote-full-read` chưa được áp dụng, Live Glob chỉ dựa vào inode.
- Không có dependency mới.
