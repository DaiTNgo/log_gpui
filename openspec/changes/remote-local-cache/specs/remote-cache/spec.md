## ADDED Requirements

### Requirement: Dòng Remote ghi vào cache trên đĩa
Hệ thống SHALL ghi mọi dòng mà một Remote File hoặc một Remote Command nhận được vào cache của nó trong `<app data>/remote-cache/`, và SHALL đọc các dòng đó qua Line Index như file local. Hệ thống SHALL NOT giữ text của mọi dòng Remote trong memory; chỉ các dòng đang hiện và một bộ đệm nhỏ quanh chúng được giữ.

#### Scenario: Memory không tăng theo text
- **WHEN** một Remote File nhận 1.000.000 dòng, mỗi dòng khoảng 125 byte
- **THEN** memory app dùng cho file đó cỡ Line Index (vài chục byte mỗi dòng), không cỡ text (khoảng 125 MB)

#### Scenario: Search đọc cache
- **WHEN** người dùng gõ Search trên một Live Log Space có Remote File
- **THEN** Match được tìm bằng Scan qua cache trên đĩa, có tiến độ, và Server không được đọc lại

### Requirement: Mở lại từ cache
Khi người dùng chọn lại một Dormant Tab, hoặc mở lại app, mà Remote File của tab có cache, hệ thống SHALL hiện ngay các dòng trong cache với số dòng thật của chúng trên Server, rồi SHALL chỉ đọc từ Server phần được ghi sau phần đã cache.

#### Scenario: Quay lại tab Remote
- **WHEN** người dùng rời tab có `web1:/app/app.log` (cache có 100.000 dòng tới offset 12 MB), rồi file trên Server được ghi thêm 50 dòng, rồi người dùng chọn lại tab
- **THEN** 100.000 dòng hiện ngay từ cache, chỉ 50 dòng mới được đọc từ Server và hiện bên dưới, và Live Window không được tải lại

#### Scenario: Mở lại app
- **WHEN** người dùng đóng app khi đang xem `web1:/app/app.log`, rồi mở lại app
- **THEN** các dòng đã cache hiện ngay, cùng Marker của chúng, trước khi kết nối tới `web1` xong

### Requirement: Đồng bộ cache theo hash khối
Trước khi đọc tiếp từ phần đã cache, hệ thống SHALL kiểm tra file trên Server còn là file đã cache. Việc kiểm tra SHALL dùng inode (nếu Server cho biết), kích thước, hash của 4 KiB đầu file, và hash của 4 KiB ngay trước kích thước đã cache, tất cả đọc qua kết nối SSH sẵn có. Hệ thống SHALL NOT cài gì lên Server. Nếu không khớp, dòng trong cache SHALL thành một Generation cũ có Break ghi lý do ("rotated", "replaced" hoặc "truncated"), và file mới SHALL được đọc như lần đầu, trong phạm vi Live Window.

#### Scenario: File được ghi thêm
- **WHEN** file trên Server cùng inode, dài hơn phần đã cache, và cả hai khối 4 KiB có cùng hash
- **THEN** chỉ phần từ offset đã cache trở đi được đọc, và không có Break nào

#### Scenario: File bị rotate trong lúc app đóng
- **WHEN** file ở đường dẫn trên Server có inode khác, và file cũ nằm trong Rotated File
- **THEN** dòng trong cache hiện ở Generation cũ, có Break "rotated" phía trên Generation mới, và 1000 dòng cuối của file mới hiện ngay rồi được backfill

#### Scenario: File bị ghi đè cùng kích thước
- **WHEN** file trên Server cùng inode và cùng kích thước, nhưng hash 4 KiB đầu khác
- **THEN** dòng trong cache ở lại phía trên một Break "truncated", và file được đọc lại như lần đầu

### Requirement: Cache của Remote Command
Mỗi lần chạy của một Remote Command SHALL được ghi vào cache như một Generation, kèm stream (stdout hay stderr) của từng dòng. Khi mở lại, output của lần chạy trước SHALL hiện ngay từ cache, rồi lệnh SHALL chạy lại theo luật Resume By Time hoặc Restart hiện có, và output mới SHALL nằm ở Generation tiếp theo.

#### Scenario: Mở lại một Remote Command
- **WHEN** người dùng chọn lại tab có `kubectl logs -f api`, lần chạy trước đã in 2.000 dòng
- **THEN** 2.000 dòng đó hiện ngay, đúng stdout và stderr, rồi lệnh chạy lại và output mới hiện dưới một Break

### Requirement: Dọn cache
Hệ thống SHALL xoá cache của một Remote File hoặc Remote Command khi:
- nó rời Log Space;
- Log Space của nó bị xoá;
- Edit… làm nó thành file khác;
- Server của nó bị xoá.

Lúc khởi động, hệ thống SHALL xoá các cache không còn file nào trỏ tới, và các cache không được mở trong 14 ngày. Tổng dung lượng cache SHALL có giới hạn chung cho cả app, mặc định 2 GB, chỉnh được trong Settings. Khi vượt giới hạn, hệ thống SHALL xoá cache được mở lâu nhất trước, và SHALL NOT xoá cache của file thuộc tab đang mở. Cache thiếu hoặc hỏng SHALL được xử lý như chưa có cache.

#### Scenario: Vượt giới hạn dung lượng
- **WHEN** tổng cache vượt 2 GB trong lúc tab của `web1:/app/app.log` đang mở
- **THEN** cache được mở lâu nhất của các file khác bị xoá cho tới khi dưới giới hạn, còn cache của `web1:/app/app.log` vẫn còn

#### Scenario: Cache bị xoá bằng tay
- **WHEN** người dùng xoá thư mục `remote-cache/` rồi mở lại tab Remote
- **THEN** tab đọc từ Server như lần đầu, và không có lỗi nào
