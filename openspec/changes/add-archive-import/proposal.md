## Why

Log thường đến tay người dùng dưới dạng nén: support bundle `.zip`, artifact CI `.tar.gz`, hay file xoay vòng `app.log.1.gz` của logrotate. Hiện app chỉ nhận file text, file nén bị coi là binary và bị từ chối, nên người dùng phải giải nén bằng tay ra một thư mục rồi mới import, và phải tự dọn thư mục đó sau.

## What Changes

- Dialog New Log Space và "Add files…" nhận thêm **Archive**: `.zip`, `.tar`, `.tar.gz`/`.tgz`, và `.gz` chứa một file.
- Thả hoặc chọn một Archive thì danh sách file chờ hiện từng **Archive Entry** bên trong (mọi cấp thư mục). Entry text được tích sẵn; entry binary hiện kèm cảnh báo như file thường; `__MACOSX/`, `.DS_Store` và thư mục bị bỏ qua. Người dùng bỏ được từng entry trước khi Create.
- Archive Entry được đọc thẳng từ Archive vào bộ nhớ, không giải nén ra đĩa. Archive trên đĩa không bao giờ bị sửa.
- Tên hiển thị của Archive Entry là `<tên archive> › <đường dẫn trong archive>`, ví dụ `bundle.zip › var/log/app.log`. File `.gz` đơn lẻ hiện tên đã bỏ đuôi `.gz`.
- Archive Entry là tĩnh: không đọc live, không có Follow/Pause. Archive bị đổi trên đĩa thì không tự nạp lại.
- Marker của Archive Entry được nhớ như file Imported thường; mở lại Project thì đọc lại entry từ Archive. Archive biến mất hoặc entry không còn thì hiện lỗi như file Imported bị mất.
- Giới hạn chống zip bomb: tổng dung lượng giải nén của các entry được chọn trong một lần import có trần (mặc định 2 GB); một entry lớn hơn 200 MB hiện cảnh báo nhưng vẫn chọn được. Archive hỏng hoặc có mật khẩu hiện lỗi và không thêm entry nào.
- Ngoài phạm vi: Archive lồng nhau (zip trong zip được coi là entry binary), Archive trên Server từ xa, Archive có mật khẩu, định dạng khác (`.7z`, `.rar`, `.zst`, `.bz2`, `.xz`).

## Capabilities

### New Capabilities
- `archive-import`: định dạng Archive được nhận, liệt kê và chọn Archive Entry, đọc entry trong bộ nhớ, tên hiển thị, entry là tĩnh, giới hạn dung lượng, lỗi Archive hỏng/mật khẩu/bị mất.

### Modified Capabilities
- `log-space`: "Nhận file vào dialog" và "Thêm file vào Imported Log Space" nhận Archive và mở rộng thành Archive Entry; trùng lặp được xét theo Archive + đường dẫn entry; Archive Entry chỉ đọc như file Imported.

## Impact

- Code: `src/workspace/new_space.rs` (mở rộng Archive thành entry trong danh sách chờ), `src/workspace/log_space.rs` (`LogFile` biết nó là Archive Entry), `src/parser/mod.rs`, `src/space_view.rs`, `src/app/tabs.rs` (đọc qua một hàm đọc chung thay vì `std::fs::read`), `src/app/live.rs` / `src/live/` (bỏ qua Archive Entry khi bật đọc live), dialog New Log Space trong `src/ui/`.
- Dependency mới: `zip`, `tar`, `flate2`.
- Dữ liệu lưu: session lưu thêm đường dẫn entry trong Archive cho `LogFile`; session cũ không có trường này nên đọc như file thường, không cần migrate.
- Tài liệu: CONTEXT.md thêm Archive và Archive Entry.
