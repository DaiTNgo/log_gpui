## ADDED Requirements

### Requirement: Import Server từ ~/.ssh/config
Hệ thống SHALL cho người dùng mở "Import from ~/.ssh/config…" trong dialog Servers. Hệ thống SHALL đọc `~/.ssh/config` (kèm các file được `Include`) đúng một lần khi mở dialog, và liệt kê mỗi `Host` có tên cụ thể thành một dòng có ô tích, kèm địa chỉ, port, user, cách đăng nhập và Jump Server sẽ được tạo. Chỉ các dòng được tích SHALL được lưu thành Server khi người dùng bấm "Import". Hệ thống SHALL NOT đọc `~/.ssh/config` vào bất kỳ lúc nào khác, kể cả khi kết nối.

#### Scenario: Import hai host
- **WHEN** `~/.ssh/config` có `Host web1` (HostName 10.0.0.5, User deploy, IdentityFile ~/.ssh/id_ed25519) và `Host db` (HostName 10.0.1.9, Port 2222), người dùng tích cả hai và bấm "Import"
- **THEN** danh sách Server có "web1" (`deploy@10.0.0.5:22`, key `~/.ssh/id_ed25519`) và "db" (`10.0.1.9:2222`)

#### Scenario: Sửa config sau khi import
- **WHEN** người dùng đã import "web1" rồi đổi HostName của `Host web1` trong `~/.ssh/config` mà không bấm Refresh
- **THEN** "web1" trong app vẫn kết nối tới địa chỉ cũ

#### Scenario: Không có file config
- **WHEN** `~/.ssh/config` không tồn tại
- **THEN** dialog báo "No ~/.ssh/config found" và không có gì được tạo

### Requirement: Ánh xạ trường của ~/.ssh/config
Hệ thống SHALL lấy giá trị theo luật của OpenSSH: với mỗi trường, giá trị đầu tiên gặp trong các khối `Host` khớp với alias (kể cả `Host *` và pattern) là giá trị dùng. Hệ thống SHALL ánh xạ: alias → tên Server; `HostName` → địa chỉ (mặc định là alias, `%h` thay bằng alias); `User` → user (mặc định là user đăng nhập máy); `Port` → port (mặc định 22); `IdentityFile` → đăng nhập bằng key với file đầu tiên tồn tại; `IdentityAgent` → đăng nhập bằng Agent với socket đó; không có `IdentityFile` và `IdentityAgent` → đăng nhập bằng Agent với `SSH_AUTH_SOCK`. `~` trong đường dẫn SHALL được mở rộng thành thư mục home.

#### Scenario: Giá trị từ Host *
- **WHEN** config có `Host *` với `User ops` ở cuối file và `Host web1` không có `User`
- **THEN** Server "web1" được import với user `ops`

#### Scenario: 1Password agent
- **WHEN** `Host web1` có `IdentityAgent "~/Library/Group Containers/2BUA8C4S2C.com.1password/t/agent.sock"`
- **THEN** "web1" được import với cách đăng nhập Agent dùng đúng socket đó

#### Scenario: IdentityFile không tồn tại
- **WHEN** `Host web1` chỉ có `IdentityFile ~/.ssh/missing_key` và file đó không có trên máy
- **THEN** dòng "web1" hiện cảnh báo "Key file not found" và vẫn import được, với cách đăng nhập Agent

### Requirement: ProxyJump thành Jump Server
Hệ thống SHALL chuyển `ProxyJump` thành chuỗi Jump Server. Mỗi tầng trong `ProxyJump` (dạng `[user@]host[:port]`, nhiều tầng cách nhau bằng dấu phẩy) SHALL dùng Server ứng với alias cùng tên nếu có trong config hoặc đã có trong app, nếu không thì SHALL tạo thêm một Server cho tầng đó. Chuỗi vượt 4 Jump Server hoặc có vòng lặp SHALL bị đánh dấu lỗi và SHALL NOT được import. `ProxyJump none` SHALL nghĩa là không có Jump Server.

#### Scenario: Qua bastion
- **WHEN** `Host db` có `ProxyJump bastion` và config có `Host bastion`
- **THEN** import "db" tạo cả "bastion" (nếu chưa có) và "db" dùng Jump Server "bastion"

#### Scenario: Nhiều tầng không có alias
- **WHEN** `Host db` có `ProxyJump ops@gw1.example.com,gw2:2200`
- **THEN** hệ thống tạo Server cho `ops@gw1.example.com` và `gw2:2200`, và chuỗi của "db" là gw1 → gw2 → db

### Requirement: Dòng không hỗ trợ được báo rõ
Hệ thống SHALL bỏ qua và liệt kê trong dialog import: `Host` chỉ có pattern (chứa `*`, `?` hoặc `!`), khối `Match`, và các tuỳ chọn ngoài danh sách ánh xạ có ảnh hưởng tới việc kết nối (như `ProxyCommand`, `LocalForward`, `CertificateFile`). Một `Host` có `ProxyCommand` SHALL được hiện nhưng không tích sẵn, kèm cảnh báo "ProxyCommand is not supported".

#### Scenario: ProxyCommand
- **WHEN** `Host legacy` có `ProxyCommand ssh -W %h:%p gw`
- **THEN** dòng "legacy" không được tích sẵn và hiện "ProxyCommand is not supported"

#### Scenario: Khối Match
- **WHEN** config có khối `Match host *.corp exec "..."`
- **THEN** phần "Skipped" của dialog liệt kê khối đó, và không Server nào nhận giá trị từ khối đó

### Requirement: Trùng tên khi import
Khi alias trùng tên một Server đã có mà Server đó không được import từ chính alias này, hệ thống SHALL đánh dấu dòng là trùng, bỏ tích sẵn, và SHALL cho người dùng sửa tên ngay trên dòng trước khi import. Hệ thống SHALL NOT ghi đè một Server có sẵn bằng import.

#### Scenario: Tên đã dùng
- **WHEN** app đã có Server "web1" tạo bằng tay và config có `Host web1`
- **THEN** dòng "web1" hiện "Name already used", không tích sẵn, và người dùng đổi tên thành "web1-cfg" thì import được

### Requirement: Refresh từ ~/.ssh/config
Hệ thống SHALL ghi nhớ alias nguồn của mỗi Server đã import. "Refresh from ~/.ssh/config" SHALL đọc lại file và hiện, cho từng Server đã import, các trường khác nhau giữa app và config, để người dùng tích những thay đổi muốn áp dụng; alias mới SHALL hiện như lần import đầu. Server có alias không còn trong config SHALL được đánh dấu "No longer in ~/.ssh/config" và SHALL NOT bị xoá. Áp dụng thay đổi cho Server đang kết nối SHALL theo yêu cầu "Sửa Server khi đang kết nối".

#### Scenario: Đổi port trong config
- **WHEN** "db" được import với port 22, config đổi thành `Port 2222` và người dùng bấm Refresh
- **THEN** dialog hiện "db: port 22 → 2222" có ô tích, và chỉ khi tích rồi bấm Apply thì "db" mới đổi port

#### Scenario: Alias bị xoá khỏi config
- **WHEN** `Host old` bị xoá khỏi config và người dùng bấm Refresh
- **THEN** Server "old" vẫn còn và hiện "No longer in ~/.ssh/config"

### Requirement: Đặt Server Group khi import
Dialog import SHALL có ô "Server Group" tuỳ chọn; mọi Server được tạo trong lần import đó SHALL thuộc group này.

#### Scenario: Import vào group
- **WHEN** người dùng gõ "prod" vào ô Server Group và import "web1", "db"
- **THEN** "web1" và "db" được liệt kê dưới group "prod"
