## Why

Log cần đọc thường nằm trên server và đang được ghi liên tục, nhưng app chỉ đọc được file local và đọc trọn một lần. Công tắc "Tail" trên status bar chỉ là cờ, chưa có gì đẩy dòng mới vào view. Người dùng phải `ssh` + `tail -f` trong terminal, mất Filter, Search, Marker và timeline gộp nhiều server.

## What Changes

- Thêm loại Log Space thứ ba: **Remote Log Space**, chỉ chứa **Remote File** (Server + đường dẫn), read-only, có thể lấy file từ nhiều Server và gộp theo timestamp.
- Thêm **Server** lưu chung cho mọi Project: địa chỉ, port, user, đăng nhập bằng private key (kèm passphrase) hoặc mật khẩu lưu trong macOS Keychain, tuỳ chọn **Jump Server** lồng nhau tối đa 4 tầng. Kiểm tra host key: xác nhận fingerprint lần đầu, chặn khi key đổi. Không xoá được Server đang được dùng.
- Kết nối bằng SSH client nhúng trong app, không dùng lệnh `ssh` hay `~/.ssh/config` của máy (ADR 0009).
- Chọn file trên server bằng duyệt thư mục SFTP, ô gõ đường dẫn, hoặc glob (khớp một lần lúc thêm). Đường dẫn chưa tồn tại vẫn thêm được và chờ file xuất hiện.
- Đọc live: mở Remote File thì hiện ngay các dòng cuối, rồi nạp dần dòng cũ hơn ở nền cho đến đầy **Live Window** (mặc định 100.000 dòng mỗi Remote File, chỉnh trong Settings). Dòng cũ nhất rời window khi có dòng mới. Số dòng là số dòng thật trên server.
- File local trong Imported Log Space cũng được đọc live khi được ghi thêm, không có Live Window.
- **BREAKING**: thay công tắc "Tail" bằng hai công tắc **Follow** (tự cuộn theo dòng mới, tự tắt khi cuộn lên, tự bật khi về cuối) và **Pause** (cả Log Space ngừng nạp dòng mới; tiếp tục thì nạp bù). Cả hai cùng trạng thái kết nối chỉ nằm trên status bar, không được nhớ khi mở lại.
- **Break**: vạch có nhãn vẽ giữa hai row khi file xoay vòng, bị cắt ngắn, bị xoá, mất kết nối, hoặc dòng cũ rời Live Window. Không phải row.
- **Generation**: sau mỗi lần xoay vòng/cắt ngắn/xoá, số dòng đánh lại từ 1; Marker gắn theo Generation + số dòng.
- Mất kết nối thì tự nối lại (1s, 2s, 4s… tối đa 30s) và đọc tiếp từ byte đã dừng; sau 5 phút thất bại thì chờ người dùng bấm Reconnect.
- **Download** một Remote File hoặc **Download all** cả Log Space về thư mục chọn, rồi mở thành Imported Log Space mới.
- Mở lại Project thì Remote Log Space tự kết nối lại và nạp lại; Server cần mật khẩu không có trong Keychain thì chờ người dùng.

## Capabilities

### New Capabilities
- `ssh-server`: Server và Jump Server đã lưu, cách đăng nhập, lưu bí mật trong Keychain, kiểm tra host key, quy tắc xoá Server đang dùng.
- `remote-log-space`: tạo Remote Log Space, thêm/gỡ Remote File (duyệt, gõ, glob, đường dẫn chưa tồn tại), lỗi quyền đọc, Download / Download all, kết nối lại khi mở Project.
- `live-log`: đọc dòng mới cho file local và Remote File; Follow, Pause, Break, Generation, xoay vòng/cắt ngắn, tự nối lại, Live Window với nạp dần, trạng thái live trên status bar.

### Modified Capabilities
- `log-space`: dialog New Log Space có thêm lựa chọn tạo Remote Log Space; ba loại Log Space không trộn file; Remote File cũng chỉ đọc.
- `merged-timeline`: yêu cầu "Dòng mới khi tail" áp dụng cho cả Remote File và chịu Follow/Pause; quy tắc không đổi múi giờ áp dụng giữa các Server.
- `log-filter`: số đếm Filter Value tính theo các dòng đang giữ, giảm khi dòng rời Live Window; giá trị không còn dòng nào biến mất trừ khi đang tích.
- `text-marker`: Marker gắn theo Generation + số dòng; Marker của dòng rời Live Window bị bỏ.

## Impact

- Code: `src/app/` (status bar, commands, view settings thay `tail_enabled`), `src/workspace/` (loại Log Space mới, Server lưu chung, persist), `src/parser/` (nạp tăng dần thay vì `std::fs::read` một lần), `src/filter_index.rs` (bỏ dòng khỏi chỉ mục), `src/marker.rs` (khoá theo Generation).
- Dependency mới: thư viện SSH/SFTP thuần Rust (dự kiến `russh` + `russh-sftp`), runtime async cho nó, truy cập macOS Keychain (`security-framework` hoặc `keyring`), theo dõi file local (`notify` hoặc polling).
- Dữ liệu lưu: danh sách Server và host key tin cậy ở thư mục dữ liệu app; session cũ chưa có Remote Log Space nên không cần migrate.
- Tài liệu: CONTEXT.md (nhóm Live, L1–L15), ADR 0009.
