## 1. Trạng thái bộ chọn cấp app

- [ ] 1.1 `PickerState` (sort, `last_dir` và `recent` theo `server_id`) đọc/ghi `picker_state.json` ở thư mục dữ liệu app, ghi tạm rồi đổi tên; file thiếu coi như rỗng; test round-trip
- [ ] 1.2 Recent Paths: thêm lên đầu, bỏ trùng, tối đa 10, xoá một mục, xoá hết; test các scenario của "Recent Paths"
- [ ] 1.3 Xoá Server thì xoá mục của nó trong `PickerState`

## 2. Lọc, sắp xếp, mở thư mục

- [ ] 2.1 Hàm thuần trong `browse.rs`: lọc (chuỗi con không phân biệt hoa thường, glob khi `is_glob`) và sắp theo tên / thời gian sửa / kích thước, thư mục trước, thư mục sắp theo tên khi sắp theo kích thước; unit test
- [ ] 2.2 `resolve_input` trả `Resolved { Dir, Files, Glob { dir, pattern } }`; glob chỉ cần thư mục tồn tại; từ chối glob ở thư mục; cập nhật test
- [ ] 2.3 `RemotePicker`: ô lọc (xoá khi đổi thư mục), `visible_entries()`, tiêu đề cột bấm để sắp/đảo chiều, nhớ cách sắp vào `PickerState`
- [ ] 2.4 Mở bộ chọn ở `last_dir` của Server; thư mục không mở được thì về home và báo "<dir> is no longer available"; ghi `last_dir` mỗi lần liệt kê thành công
- [ ] 2.5 Lối tắt Home và `/var/log`; lỗi thì giữ thư mục đang mở và báo trong bộ chọn
- [ ] 2.6 Ô đường dẫn: `Dir` mở thư mục và xoá ô

## 3. Nhiều Server

- [ ] 3.1 `staged` thành `Vec<Staged { File, Glob }>`; hiện Live Glob trong danh sách chờ khác file
- [ ] 3.2 Checkbox Server ở hàng Add (Server đang chọn luôn tích, giữ lựa chọn trong phiên bộ chọn)
- [ ] 3.3 `picker_add_typed` và bấm Recent Paths chạy `resolve_input` song song trên từng Server đã tích, gom lỗi theo Server dạng "<server>: <message>"
- [ ] 3.4 `picker_confirm` ghi Recent Paths cho từng Server của các mục thật sự được thêm

## 4. Live Glob trong Remote Log Space

- [ ] 4.1 `LiveGlob { glob_id, server_id, dir, pattern, excluded, stopped }` trong `LogSpace.globs` và `LogFile.from_glob`, đều `#[serde(default)]`; test đọc Project cũ
- [ ] 4.2 `add_live_glob` từ chối trùng (cùng Server, cùng mẫu); file đã có giữ nguồn gốc ban đầu
- [ ] 4.3 Gỡ file có `from_glob` thêm đường dẫn vào `excluded`; "Remove Live Glob" gỡ mọi file của nó và giữ file thêm bằng đường dẫn
- [ ] 4.4 Thêm lại đúng đường dẫn đang nằm trong `excluded` của một Live Glob cùng Server: bỏ khỏi `excluded` và tạo lại file với `from_glob` của glob đó; test
- [ ] 4.5 Hàm thuần `plan_glob(glob, files, listing, inodes) -> GlobPlan { add, gone, pending }`: khớp mẫu, bỏ thư mục, bỏ `excluded`, bỏ Rotated File (dùng hàm nhận biết của `remote-full-read`), bỏ inode đã đọc, giới hạn theo setting (file không Gone, chọn theo thời gian sửa mới nhất), lấp chỗ trống, không thêm gì khi `stopped`; unit test cho mọi scenario Live Glob

## 5. Theo dõi thư mục

- [ ] 5.1 `GlobWatcher` trên `RemoteHub`: một task cho mỗi (Server, thư mục) có Live Glob không dừng, liệt kê theo setting nhịp quét (ít nhất 30 s khi > 10.000 mục), nhận lệnh đổi nhịp, gửi `GlobEvent::Listing` qua channel sự kiện
- [ ] 5.2 Lấy inode của ứng viên mới bằng một lệnh `stat` qua exec cho cả lô (GNU và BSD); không có inode thì coi là file mới
- [ ] 5.3 Bỏ qua file nén/Archive không phải Rotated File bằng 512 byte đầu, dùng chung bộ nhận diện của Archive import; file rỗng được thêm
- [ ] 5.4 Dừng watcher khi Pause, đóng Log Space hoặc gỡ Live Glob cuối của thư mục; Resume thì liệt kê ngay và đọc file mới từ đầu trong Live Window
- [ ] 5.5 Thư mục mất: trạng thái chờ thư mục, thử lại theo nhịp; khi có lại thì tiếp tục thêm file
- [ ] 5.6 Mở lại Project: khớp lại mọi Live Glob không dừng, thêm file đang khớp, không thêm lại file Gone hay không còn khớp; Live Glob dừng chỉ mở lại file đã có còn tồn tại
- [ ] 5.7 Stop watching / Resume watching: cờ `stopped`, dừng/khởi động watcher, Resume liệt kê ngay và đọc file mới từ đầu trong Live Window; file đã có vẫn live và vẫn có thể thành Gone

## 6. Gone và xoay vòng

- [ ] 6.1 `SourceStatus::Gone` có trọng số như Live trong `worst()`, hiện trong danh sách hover của status bar
- [ ] 6.2 `RemoteFileSource` với `from_glob`: file bị xoá thì `Break(Deleted)` rồi Gone; đường dẫn có lại file thì Generation mới; file thường giữ `WaitingForFile`
- [ ] 6.3 Theo dõi tập inode đã đọc theo Live Glob từ sự kiện của nguồn; file thuộc glob xoay vòng sang inode đã đọc thì chuyển Gone thay vì Generation mới
- [ ] 6.4 "Download all" bỏ qua file Gone
- [ ] 6.5 Status bar báo "<server>:<glob>: N matching files not read (limit M)" (không báo khi Live Glob bị dừng)
- [ ] 6.6 Settings: "Live Glob file limit" (mặc định 50) và "Live Glob scan interval" (mặc định 5 s) cạnh Live Window; tăng giới hạn lấp chỗ trống, giảm giới hạn không gỡ file, đổi nhịp có hiệu lực ngay

## 7. UI

- [ ] 7.1 Sidebar: nút gom Live Glob thu gọn được, file của nó bên dưới, file Gone hiện mờ, nhãn "stopped", menu "Stop watching"/"Resume watching" và "Remove Live Glob"
- [ ] 7.2 Bộ chọn: khối Recent Paths (bấm để thêm, xoá mục, "Clear"), ô lọc, tiêu đề cột sắp xếp, lối tắt, checkbox Server

## 8. Kiểm thử và tài liệu

- [ ] 8.1 Test tích hợp với SSH server cục bộ (script sshd sẵn có): Live Glob thêm file mới trong 10 s, logrotate với glob `app.log*` chỉ thêm `app.log` (Rotated File không thành Remote File), đổi tên tuỳ ý không đọc trùng (inode), Stop/Resume watching, xoá file thành Gone, Pause/Resume không mất dòng của file mới
- [ ] 8.2 Smoke test GUI bộ chọn: mở lại ở thư mục cuối, lọc, sắp xếp, `/var/log`, thêm một đường dẫn trên hai Server
- [ ] 8.3 Cập nhật `CONTEXT.md` theo mục "Domain changes" của proposal (Live Glob, Stopped Live Glob, Gone, Recent Paths, sửa L11, thêm L20–L23)
- [ ] 8.4 `cargo test` và `cargo clippy` sạch; `openspec validate remote-file-picker` qua
