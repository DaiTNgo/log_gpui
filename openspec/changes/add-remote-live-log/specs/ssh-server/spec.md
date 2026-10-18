## ADDED Requirements

### Requirement: Server lưu chung cho mọi Project
Hệ thống SHALL lưu danh sách Server ở cấp ứng dụng, dùng chung cho mọi Project. Mỗi Server SHALL có tên hiển thị, địa chỉ, port (mặc định 22), user, cách đăng nhập, và tuỳ chọn một Jump Server. Tên hiển thị SHALL không trống và không trùng.

#### Scenario: Dùng lại Server ở Project khác
- **WHEN** người dùng tạo Server "prod-web1" khi đang ở Project A rồi chuyển sang Project B và tạo Remote Log Space
- **THEN** "prod-web1" có sẵn trong danh sách chọn Server

#### Scenario: Tên trùng
- **WHEN** đã có Server "prod-web1" và người dùng lưu Server mới cũng tên "prod-web1"
- **THEN** nút Save bị tắt và hiện lỗi tên đã tồn tại

### Requirement: Đăng nhập bằng key hoặc mật khẩu
Hệ thống SHALL hỗ trợ đăng nhập bằng file private key (kèm passphrase nếu key có mã hoá) hoặc bằng mật khẩu. Mật khẩu và passphrase SHALL được lưu trong macOS Keychain khi người dùng chọn ghi nhớ, và SHALL NOT được ghi dạng text vào file của ứng dụng. Hệ thống SHALL NOT đọc `~/.ssh/config`, ssh-agent hay lệnh `ssh` của máy.

#### Scenario: Key có passphrase
- **WHEN** Server dùng `~/.ssh/id_ed25519` có passphrase và người dùng nhập passphrase, tích "Remember"
- **THEN** kết nối thành công và lần sau không hỏi lại passphrase

#### Scenario: Không ghi nhớ mật khẩu
- **WHEN** người dùng đăng nhập bằng mật khẩu mà không tích "Remember"
- **THEN** mỗi lần kết nối mới hệ thống hỏi lại mật khẩu

#### Scenario: Sai mật khẩu
- **WHEN** server từ chối mật khẩu
- **THEN** hệ thống báo "Authentication failed" và hỏi lại mật khẩu, không thử lại tự động

### Requirement: Kiểm tra host key
Hệ thống SHALL giữ danh sách host key tin cậy riêng của ứng dụng. Lần đầu kết nối tới một địa chỉ và port, hệ thống SHALL hiện fingerprint và chỉ kết nối khi người dùng xác nhận. Khi host key khác với key đã tin, hệ thống SHALL chặn kết nối và cảnh báo, và SHALL chỉ cho kết nối lại khi người dùng chủ động thay key đã tin.

#### Scenario: Lần đầu kết nối
- **WHEN** người dùng kết nối lần đầu tới `10.0.0.5:22`
- **THEN** dialog hiện fingerprint SHA256 với nút "Trust" và "Cancel", và chưa có dữ liệu nào được đọc

#### Scenario: Host key thay đổi
- **WHEN** `10.0.0.5:22` trả về host key khác với key đã tin
- **THEN** kết nối bị chặn, status bar báo "Host key changed", và không có dòng nào được nạp

### Requirement: Jump Server
Hệ thống SHALL cho một Server đi qua một Server khác làm Jump Server. Vì Jump Server cũng là Server, chuỗi SHALL được lồng nhiều tầng tới tối đa 4 Jump Server. Hệ thống SHALL từ chối lưu một chuỗi có vòng lặp hoặc vượt 4 tầng. Mỗi Server trong chuỗi SHALL được kiểm tra host key và đăng nhập theo cấu hình của chính nó.

#### Scenario: Đi qua bastion
- **WHEN** Server "db" có Jump Server "bastion" và người dùng mở Remote File trên "db"
- **THEN** hệ thống kết nối tới "bastion" trước, rồi tới "db" qua "bastion"

#### Scenario: Vòng lặp
- **WHEN** "a" dùng Jump Server "b" và người dùng đặt Jump Server của "b" là "a"
- **THEN** hệ thống từ chối lưu và báo chuỗi Jump Server bị vòng lặp

#### Scenario: Quá 4 tầng
- **WHEN** người dùng tạo chuỗi a → b → c → d → e → f
- **THEN** hệ thống từ chối lưu và báo vượt quá 4 Jump Server

### Requirement: Không xoá Server đang được dùng
Hệ thống SHALL NOT xoá một Server khi nó đang được một Remote Log Space hoặc một Server khác dùng làm Jump Server. Khi người dùng yêu cầu xoá, hệ thống SHALL liệt kê những gì đang dùng Server đó.

#### Scenario: Xoá Server đang dùng
- **WHEN** Remote Log Space "Payment" có file trên "prod-web1" và người dùng chọn xoá "prod-web1"
- **THEN** Server không bị xoá và hệ thống hiện "Used by: Payment (Project A)"

#### Scenario: Xoá Server không dùng
- **WHEN** không có gì dùng "staging" và người dùng xác nhận xoá
- **THEN** "staging" biến mất khỏi danh sách và mật khẩu của nó bị xoá khỏi Keychain

### Requirement: Sửa Server khi đang kết nối
Khi người dùng lưu thay đổi của một Server đang có kết nối, hệ thống SHALL đóng kết nối cũ và kết nối lại với cấu hình mới, vẽ một Break tại chỗ đứt.

#### Scenario: Đổi port
- **WHEN** Remote File trên "prod-web1" đang live và người dùng đổi port từ 22 sang 2222
- **THEN** hệ thống kết nối lại qua port 2222 và đọc tiếp từ byte đã dừng
