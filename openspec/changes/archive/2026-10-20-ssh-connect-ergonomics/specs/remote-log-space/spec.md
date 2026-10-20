## ADDED Requirements

### Requirement: Quick Connect
Bộ chọn file trên Server SHALL có ô Quick Connect nhận chuỗi dạng `[user@]host[:port]:<path>`, `ssh://[user@]host[:port]/<path>` hoặc `<tên Server>:<path>`, với `host` có thể là IPv6 trong ngoặc vuông và `<path>` là đường dẫn tuyệt đối, đường dẫn bắt đầu bằng `~/`, hoặc glob. Thiếu user SHALL dùng user đăng nhập máy; thiếu port SHALL dùng 22. Hệ thống SHALL chọn Server theo thứ tự: (1) Server có tên đúng bằng phần trước dấu `:` khi chuỗi không có `@` và không có port; (2) Server có cùng địa chỉ, port và user; (3) nếu không có, một Server mới tên `user@host` (thêm `:port` khi khác 22) với cách đăng nhập Agent. Đường dẫn SHALL được xử lý như đường dẫn gõ tay hoặc glob trong bộ chọn file (kể cả đường dẫn chưa tồn tại).

#### Scenario: Dùng Server có sẵn theo tên
- **WHEN** người dùng gõ `web1:/var/log/app.log` và đã có Server "web1"
- **THEN** Remote File `web1:/var/log/app.log` được thêm và không có Server mới nào được tạo

#### Scenario: Dùng Server có sẵn theo địa chỉ
- **WHEN** người dùng gõ `deploy@10.0.0.5:/app/app.log` và Server "web1" là `deploy@10.0.0.5:22`
- **THEN** Remote File `web1:/app/app.log` được thêm

#### Scenario: Server mới
- **WHEN** người dùng gõ `ops@10.0.2.7:2222:/var/log/syslog` và không có Server nào khớp
- **THEN** hệ thống kết nối tới `10.0.2.7:2222` với user `ops` bằng Agent, và khi file được thêm thì danh sách Server có thêm "ops@10.0.2.7:2222"

#### Scenario: Chuỗi sai dạng
- **WHEN** người dùng gõ `10.0.2.7/var/log`
- **THEN** ô báo "Expected user@host[:port]:/path" và không có gì được thêm

### Requirement: Server tạo bởi Quick Connect
Server mới của Quick Connect SHALL chỉ được lưu khi có ít nhất một Remote File được thêm bằng nó; nếu người dùng huỷ hoặc kết nối thất bại thì SHALL NOT có Server nào được lưu. Server này SHALL là Server bình thường: hiện trong danh sách Servers, sửa, đổi tên, đặt group và xoá được theo cùng quy tắc. Khi đăng nhập Agent bị từ chối, hệ thống SHALL hỏi người dùng chọn đăng nhập bằng mật khẩu hoặc đường dẫn file key (gõ tay) ngay tại đó, và Server SHALL được lưu với cách đăng nhập thành công.

#### Scenario: Huỷ trước khi thêm file
- **WHEN** người dùng gõ một chuỗi Quick Connect tới host mới, kết nối được, rồi đóng bộ chọn file mà không thêm file nào
- **THEN** danh sách Server không đổi

#### Scenario: Agent bị từ chối
- **WHEN** Quick Connect tới host mới mà agent không có key được chấp nhận
- **THEN** hệ thống hỏi "Sign in to ops@10.0.2.7:2222 with password or key file", và khi người dùng đăng nhập bằng mật khẩu có "Remember" thì Server được lưu với cách đăng nhập mật khẩu

### Requirement: Mở lại Project với Server cần Sign-in Prompt
Khi mở lại Project hoặc khởi động lại ứng dụng, Remote File trên Server (hoặc chuỗi Jump Server) đã cần Sign-in Prompt ở lần kết nối gần nhất SHALL ở trạng thái "Sign-in required" với nút Connect và SHALL NOT tự hiện Sign-in Prompt; các Remote File khác SHALL vẫn tự kết nối.

#### Scenario: Mở lại với Server có OTP
- **WHEN** người dùng mở lại Project có Remote Log Space với file trên "web1" (có OTP) và "web2" (chỉ key)
- **THEN** file trên "web2" tự live, còn file trên "web1" chờ với "Sign-in required: web1" và nút Connect
