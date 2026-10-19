## ADDED Requirements

### Requirement: Keepalive theo Server
Mỗi Server SHALL có khoảng gửi keepalive (mặc định 30 giây, từ 5 đến 300) và số keepalive không được trả lời trước khi coi là mất kết nối (mặc định 3, từ 1 đến 10), chỉnh trong dialog Server. Giá trị mới SHALL áp dụng từ lần kết nối sau tới Server đó. Server đã lưu từ trước SHALL dùng giá trị mặc định.

#### Scenario: Mạng hay rớt NAT
- **WHEN** người dùng đặt keepalive của "web1" là 10 giây và 3 lần
- **THEN** sau lần kết nối tiếp theo, app gửi keepalive tới web1 mỗi 10 giây và coi là mất kết nối khi 3 keepalive liên tiếp không được trả lời

#### Scenario: Server cũ
- **WHEN** app mở `servers.json` lưu từ bản trước, không có trường keepalive
- **THEN** mọi Server dùng keepalive 30 giây và 3 lần
