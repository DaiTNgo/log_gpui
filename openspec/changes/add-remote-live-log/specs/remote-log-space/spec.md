## ADDED Requirements

### Requirement: Tạo Remote Log Space
Hệ thống SHALL cho tạo Remote Log Space từ dialog New Log Space bằng một tên và ít nhất một Remote File. Remote Log Space SHALL chỉ chứa Remote File, và các Remote File SHALL có thể đến từ nhiều Server khác nhau.

#### Scenario: Gộp ba server
- **WHEN** người dùng tạo Remote Log Space "Checkout" với `web1:/app/app.log`, `web2:/app/app.log` và `db:/var/log/pg.log`
- **THEN** Log Space mở ra với dòng của cả ba file xếp xen theo timestamp, và Source Column hiện `web1:/app/app.log`…

### Requirement: Tên Remote File
Hệ thống SHALL gọi một Remote File bằng tên Server và đường dẫn trên Server đó, dạng `<server>:<path>`, ở sidebar, Source Column, File Section và Focus Pane. Một Remote Log Space SHALL NOT có hai Remote File cùng Server và cùng đường dẫn.

#### Scenario: Thêm trùng
- **WHEN** Log Space đã có `web1:/app/app.log` và người dùng thêm lại đúng file đó
- **THEN** danh sách file không đổi và hiện thông báo file đã có

### Requirement: Chọn file trên Server
Hệ thống SHALL cho chọn Remote File bằng ba cách: duyệt thư mục qua SFTP, gõ đường dẫn tuyệt đối, hoặc gõ glob như `/var/log/app/*.log`. Glob SHALL chỉ được khớp một lần lúc thêm, và SHALL NOT tự thêm file sinh ra sau đó.

#### Scenario: Duyệt thư mục
- **WHEN** người dùng chọn Server "web1" và mở `/var/log/app/`
- **THEN** hệ thống liệt kê file và thư mục con kèm kích thước và thời gian sửa, và người dùng tích được nhiều file

#### Scenario: Glob
- **WHEN** người dùng gõ `/var/log/app/*.log` và thư mục có `a.log`, `b.log`
- **THEN** hai Remote File `web1:/var/log/app/a.log` và `web1:/var/log/app/b.log` được thêm, và `c.log` tạo sau đó không được thêm

### Requirement: Đường dẫn chưa tồn tại
Hệ thống SHALL cho thêm một đường dẫn file chưa tồn tại khi thư mục cha của nó tồn tại. File đó SHALL ở trạng thái chờ và SHALL bắt đầu được đọc khi file xuất hiện. Hệ thống SHALL từ chối khi không kết nối được Server hoặc thư mục cha không tồn tại.

#### Scenario: File chưa được tạo
- **WHEN** người dùng thêm `web1:/app/logs/today.log` khi file chưa có nhưng `/app/logs/` có
- **THEN** file được thêm, status bar hiện "Waiting for file", và khi file được tạo thì các dòng của nó bắt đầu hiện

#### Scenario: Thư mục không tồn tại
- **WHEN** người dùng thêm `web1:/no/such/dir/app.log`
- **THEN** hệ thống từ chối và báo thư mục không tồn tại

### Requirement: Không có quyền đọc
Hệ thống SHALL đọc Remote File bằng user của Server và SHALL NOT dùng sudo hay quyền nâng cao. Khi user đó không đọc được file, hệ thống SHALL báo lỗi quyền cho file đó và vẫn đọc các file khác của Log Space.

#### Scenario: Permission denied
- **WHEN** Log Space có `web1:/var/log/syslog` mà user `deploy` không đọc được
- **THEN** status bar báo "Permission denied: web1:/var/log/syslog" và các file khác vẫn live

### Requirement: Thêm và gỡ Remote File
Hệ thống SHALL cho thêm Remote File vào Remote Log Space qua "Add remote files…" và gỡ Remote File qua "Remove from Log Space". Gỡ file SHALL đóng việc đọc file đó, bỏ các dòng và Marker của nó khỏi Log Space, và SHALL NOT thay đổi file trên Server.

#### Scenario: Gỡ một file
- **WHEN** người dùng gỡ `db:/var/log/pg.log` khỏi "Checkout"
- **THEN** các dòng của `pg.log` biến mất khỏi view, các file khác vẫn live, và `pg.log` trên server không đổi

### Requirement: Download Remote File
Hệ thống SHALL cho tải một Remote File, hoặc mọi Remote File của một Remote Log Space bằng "Download all", về thư mục người dùng chọn, có hiện tiến độ và cho huỷ. Chỉ file hiện tại SHALL được tải, không kèm các file đã xoay vòng. Tải xong, hệ thống SHALL đề nghị mở các file đã tải thành một Imported Log Space mới.

#### Scenario: Download all
- **WHEN** người dùng chọn "Download all" trên "Checkout" và chọn thư mục `~/incident/`
- **THEN** ba file được tải vào `~/incident/`, rồi hệ thống hỏi "Open in a new Imported Log Space?" và nếu đồng ý thì tạo Imported Log Space chứa ba file đó

#### Scenario: Trùng tên khi tải
- **WHEN** `web1:/app/app.log` và `web2:/app/app.log` cùng được tải vào một thư mục
- **THEN** hai file được lưu với tên phân biệt theo Server, ví dụ `web1-app.log` và `web2-app.log`

### Requirement: Mở lại Project
Khi mở lại Project hoặc khởi động lại ứng dụng, hệ thống SHALL tự kết nối lại mọi Remote Log Space được mở và nạp lại theo Live Window. Nếu một Server cần mật khẩu hay passphrase không có trong Keychain, hệ thống SHALL chờ người dùng bấm Connect cho Server đó. Các dòng đã nhận trước khi đóng SHALL NOT được giữ.

#### Scenario: Khởi động lại
- **WHEN** người dùng thoát app khi "Checkout" đang mở rồi mở lại
- **THEN** "Checkout" tự kết nối lại và hiện các dòng cuối hiện tại của từng file

#### Scenario: Thiếu mật khẩu
- **WHEN** Server "db" đăng nhập bằng mật khẩu không được ghi nhớ
- **THEN** các file trên "db" ở trạng thái chờ với nút Connect trên status bar, còn các file khác vẫn tự kết nối
