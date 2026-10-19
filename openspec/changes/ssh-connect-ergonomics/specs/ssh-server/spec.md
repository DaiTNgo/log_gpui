## MODIFIED Requirements

### Requirement: Đăng nhập bằng key hoặc mật khẩu
Hệ thống SHALL hỗ trợ bốn cách đăng nhập chính: file private key (kèm passphrase nếu key có mã hoá), mật khẩu, ssh-agent, hoặc keyboard-interactive (Sign-in Prompt). Sau cách chính, nếu server yêu cầu thêm bước keyboard-interactive, hệ thống SHALL tiếp tục bằng Sign-in Prompt. Mật khẩu và passphrase SHALL được lưu trong macOS Keychain khi người dùng chọn ghi nhớ, và SHALL NOT được ghi dạng text vào file của ứng dụng. Hệ thống SHALL NOT chạy lệnh `ssh` của máy và SHALL NOT đọc `~/.ssh/config` khi kết nối; `~/.ssh/config` chỉ được đọc khi người dùng import.

#### Scenario: Key có passphrase
- **WHEN** Server dùng `~/.ssh/id_ed25519` có passphrase và người dùng nhập passphrase, tích "Remember"
- **THEN** kết nối thành công và lần sau không hỏi lại passphrase

#### Scenario: Không ghi nhớ mật khẩu
- **WHEN** người dùng đăng nhập bằng mật khẩu mà không tích "Remember"
- **THEN** mỗi lần kết nối mới hệ thống hỏi lại mật khẩu

#### Scenario: Sai mật khẩu
- **WHEN** server từ chối mật khẩu
- **THEN** hệ thống báo "Authentication failed" và hỏi lại mật khẩu, không thử lại tự động

#### Scenario: Key rồi OTP
- **WHEN** server yêu cầu `publickey,keyboard-interactive` và key được chấp nhận
- **THEN** hệ thống hiện Sign-in Prompt của server thay vì báo "Authentication failed"

## ADDED Requirements

### Requirement: Đăng nhập bằng ssh-agent
Hệ thống SHALL cho chọn cách đăng nhập "Agent" với socket là `SSH_AUTH_SOCK` của hệ thống (mặc định), socket của 1Password, socket của Secretive, hoặc một đường dẫn socket tuỳ chọn. Hệ thống SHALL lấy danh sách key từ agent và thử lần lượt; nếu Server có ghi một public key ưu tiên thì SHALL thử key đó trước và chỉ key đó. Hệ thống SHALL NOT đọc, xuất hay lưu private key từ agent. Trong lúc chờ agent ký (người dùng chạm YubiKey, duyệt trong 1Password hoặc Touch ID), status SHALL hiện "Waiting for agent approval" và hệ thống SHALL chờ tới 60 giây trước khi báo lỗi.

#### Scenario: Agent mặc định
- **WHEN** Server "web1" đăng nhập bằng Agent và `SSH_AUTH_SOCK` có key được `web1` chấp nhận
- **THEN** kết nối thành công mà không hỏi mật khẩu hay passphrase nào

#### Scenario: Không có agent
- **WHEN** Server dùng Agent nhưng socket không tồn tại hoặc không trả lời
- **THEN** hệ thống báo "ssh-agent not available at <đường dẫn>" cho Server đó

#### Scenario: Agent không có key phù hợp
- **WHEN** agent có ba key nhưng server từ chối cả ba
- **THEN** hệ thống báo "Authentication failed: no agent key accepted"

#### Scenario: Chờ chạm YubiKey
- **WHEN** agent chờ người dùng chạm YubiKey
- **THEN** status bar hiện "Waiting for agent approval: web1" cho tới khi agent trả lời hoặc hết 60 giây

### Requirement: Sign-in Prompt (keyboard-interactive)
Hệ thống SHALL hỗ trợ keyboard-interactive, là cách đăng nhập chính hoặc bước sau một cách khác. Mỗi lượt hỏi của server SHALL hiện trong một dialog có tên Server, tên và lời dẫn do server gửi, và từng câu hỏi với ô trả lời bị che khi server đánh dấu không hiện chữ. Câu trả lời SHALL NOT được lưu ở bất kỳ đâu, kể cả Keychain. Huỷ dialog SHALL huỷ kết nối tới Server đó. Với chuỗi Jump Server, mỗi tầng SHALL hỏi riêng theo thứ tự từ ngoài vào trong.

#### Scenario: OTP
- **WHEN** server gửi câu hỏi "Verification code:" không hiện chữ
- **THEN** dialog "Sign in to web1" hiện ô "Verification code:" dạng che, và sau khi người dùng nhập đúng thì kết nối tiếp

#### Scenario: Huỷ
- **WHEN** người dùng bấm Cancel ở Sign-in Prompt
- **THEN** kết nối tới Server đó dừng, các Remote File trên Server đó ở trạng thái "Sign-in required" với nút Connect, và các Server khác không bị ảnh hưởng

#### Scenario: Bastion có 2FA
- **WHEN** "db" đi qua "bastion" và chỉ "bastion" yêu cầu OTP
- **THEN** hệ thống hiện Sign-in Prompt "Sign in to bastion" một lần rồi kết nối tới "db" không hỏi thêm

### Requirement: Không tự kết nối lại khi cần Sign-in Prompt
Khi một Server trong chuỗi đã cần Sign-in Prompt ở lần kết nối gần nhất, hệ thống SHALL NOT tự hiện Sign-in Prompt khi tự kết nối lại sau mất kết nối. Thay vào đó các Remote File liên quan SHALL chuyển sang "Sign-in required" với nút Reconnect, và kết nối lại chỉ bắt đầu khi người dùng bấm. Đọc tiếp từ byte đã dừng và Break SHALL giữ như khi tự kết nối lại.

#### Scenario: Mất mạng với Server có OTP
- **WHEN** "web1" (đăng nhập có OTP) mất kết nối
- **THEN** status bar hiện "Sign-in required: web1" với nút Reconnect, không có dialog tự bật lên, và sau khi người dùng bấm Reconnect và nhập OTP thì file đọc tiếp từ byte đã dừng với một Break

### Requirement: Test connection
Form Server SHALL có nút "Test connection" chạy với giá trị đang có trong form, kể cả khi chưa lưu và kể cả Jump Server mới tạo trong form. Hệ thống SHALL kết nối qua cả chuỗi bằng một kết nối riêng, không dùng chung và không ảnh hưởng kết nối đang live, rồi đóng lại. Kết quả SHALL hiện theo từng tầng và từng bước: kết nối mạng, host key, đăng nhập, SFTP; bước đầu tiên lỗi SHALL được đánh dấu kèm thông báo lỗi. Host key prompt và Sign-in Prompt SHALL hiện như khi kết nối thật. Test SHALL có nút huỷ và SHALL NOT lưu Server; secret nhập trong lúc test SHALL chỉ được lưu vào Keychain khi người dùng lưu Server với "Remember" được tích.

#### Scenario: Test thành công
- **WHEN** người dùng điền form "db" qua "bastion" rồi bấm "Test connection"
- **THEN** form hiện "bastion: connected, host key trusted, signed in" và "db: connected, host key trusted, signed in, SFTP available", và Server chưa được lưu

#### Scenario: Sai port
- **WHEN** port trong form là 2223 mà server không nghe ở port đó
- **THEN** kết quả đánh dấu bước kết nối mạng của Server đó lỗi với "Connection refused" và không thử các bước sau

#### Scenario: Server tắt SFTP
- **WHEN** đăng nhập thành công nhưng server không có SFTP subsystem
- **THEN** kết quả hiện "signed in" và đánh dấu lỗi "SFTP not available"

### Requirement: Server Group và tag
Mỗi Server SHALL có thể thuộc tối đa một Server Group và có không hoặc nhiều tag, đặt trong form Server. Server Group SHALL NOT lồng nhau. Danh sách Servers và ô chọn Server SHALL liệt kê Server theo Server Group (tên group theo thứ tự chữ cái, Server không có group ở cuối dưới "Ungrouped"), mỗi group có thể thu gọn. Group và tag SHALL NOT ảnh hưởng tới việc kết nối. Group không còn Server nào SHALL tự biến mất.

#### Scenario: Nhóm theo group
- **WHEN** "web1", "web2" thuộc group "prod", "stg1" thuộc "staging" và "lab" không có group
- **THEN** danh sách hiện "prod" (web1, web2), "staging" (stg1), rồi "Ungrouped" (lab)

#### Scenario: Group rỗng
- **WHEN** Server cuối cùng của group "staging" được chuyển sang group "prod"
- **THEN** heading "staging" không còn trong danh sách

### Requirement: Tìm Server
Danh sách Servers, ô chọn Server trong bộ chọn file và ô chọn Jump Server trong form Server SHALL có ô tìm. Ô tìm SHALL so khớp không phân biệt hoa thường theo Unicode, dạng "chứa", trên tên, địa chỉ, user, Server Group và tag; nhiều từ cách nhau bằng khoảng trắng SHALL đều phải khớp. Khi đang tìm, group SHALL chỉ hiện nếu còn Server khớp và SHALL tự mở. Enter SHALL chọn Server khớp đầu tiên.

#### Scenario: Tìm theo group và tên
- **WHEN** người dùng gõ "prod web" vào ô tìm
- **THEN** chỉ còn "web1" và "web2" của group "prod"

#### Scenario: Tìm theo địa chỉ
- **WHEN** người dùng gõ "10.0.1"
- **THEN** các Server có địa chỉ chứa "10.0.1" được hiện, kể cả khi tên không chứa chuỗi đó

#### Scenario: Không có kết quả
- **WHEN** không Server nào khớp
- **THEN** danh sách hiện "No matching servers"
