## ADDED Requirements

### Requirement: Dòng Remote ghi vào cache trên đĩa
Hệ thống SHALL ghi mọi dòng mà một Remote File hoặc một Remote Command nhận được vào cache của nó trong `<app data>/remote-cache/`, và SHALL đọc các dòng đó qua Line Index như file local. Hệ thống SHALL NOT giữ text của mọi dòng Remote trong memory; chỉ các dòng đang hiện và một bộ đệm nhỏ quanh chúng được giữ.

#### Scenario: Memory không tăng theo text
- **WHEN** một Remote File nhận 1.000.000 dòng, mỗi dòng khoảng 125 byte
- **THEN** memory app dùng cho file đó cỡ Line Index (vài chục byte mỗi dòng), không cỡ text (khoảng 125 MB)

#### Scenario: Search đọc cache
- **WHEN** người dùng gõ Search trên một Live Log Space có Remote File
- **THEN** Match được tìm bằng Scan qua cache trên đĩa, có tiến độ, và Server không được đọc lại

### Requirement: Mở lại từ cache trong phiên app
Khi người dùng chọn lại một Dormant Tab mà Remote File của tab có cache, hệ thống SHALL hiện ngay các dòng trong cache với số dòng thật của chúng trên Server, rồi SHALL chỉ đọc từ Server phần được ghi sau phần đã cache. Cache SHALL chỉ sống trong phiên app: hệ thống SHALL NOT giữ dòng Remote qua lần mở app sau.

#### Scenario: Quay lại tab Remote
- **WHEN** người dùng rời tab có `web1:/app/app.log` (cache có 100.000 dòng tới offset 12 MB), rồi file trên Server được ghi thêm 50 dòng, rồi người dùng chọn lại tab
- **THEN** 100.000 dòng hiện ngay từ cache, chỉ 50 dòng mới được đọc từ Server và hiện bên dưới, và Live Window không được tải lại

#### Scenario: Mở lại app
- **WHEN** người dùng tắt app khi đang xem `web1:/app/app.log`, rồi mở lại app và chọn tab đó
- **THEN** `remote-cache/` không còn gì của lần trước, và tab đọc từ Server như lần đầu

### Requirement: Đồng bộ cache theo hash khối
Trước khi đọc tiếp từ phần đã cache, hệ thống SHALL kiểm tra file trên Server còn là file đã cache. Việc kiểm tra SHALL dùng inode (nếu Server cho biết), kích thước, hash của 4 KiB đầu file, và hash của 4 KiB ngay trước kích thước đã cache, tất cả đọc qua kết nối SSH sẵn có. Hệ thống SHALL NOT cài gì lên Server. Nếu không khớp, dòng trong cache SHALL thành một Generation cũ có Break ghi lý do ("rotated", "replaced" hoặc "truncated"), và file mới SHALL được đọc như lần đầu, trong phạm vi Live Window.

#### Scenario: File được ghi thêm
- **WHEN** file trên Server cùng inode, dài hơn phần đã cache, và cả hai khối 4 KiB có cùng hash
- **THEN** chỉ phần từ offset đã cache trở đi được đọc, và không có Break nào

#### Scenario: File bị rotate trong lúc tab không mở
- **WHEN** file ở đường dẫn trên Server có inode khác, và file cũ nằm trong Rotated File
- **THEN** dòng trong cache hiện ở Generation cũ, có Break "rotated" phía trên Generation mới, và 1000 dòng cuối của file mới hiện ngay rồi được backfill

#### Scenario: File bị ghi đè cùng kích thước
- **WHEN** file trên Server cùng inode và cùng kích thước, nhưng hash 4 KiB đầu khác
- **THEN** dòng trong cache ở lại phía trên một Break "truncated", và file được đọc lại như lần đầu

### Requirement: Cache của Remote Command
Mỗi lần chạy của một Remote Command SHALL được ghi vào cache như một Generation, kèm stream (stdout hay stderr) của từng dòng. Khi chọn lại tab trong cùng phiên app, output của lần chạy trước SHALL hiện ngay từ cache dưới một Break "previous run", rồi lệnh SHALL chạy lại theo luật Resume By Time hoặc Restart hiện có, và output mới SHALL nằm ở Generation tiếp theo.

#### Scenario: Mở lại một Remote Command
- **WHEN** người dùng chọn lại tab có `kubectl logs -f api`, lần chạy trước đã in 2.000 dòng
- **THEN** 2.000 dòng đó hiện ngay, đúng stdout và stderr, rồi lệnh chạy lại và output mới hiện dưới một Break

### Requirement: Dọn cache
Hệ thống SHALL xoá cache của một Remote File hoặc Remote Command khi không còn tab đang mở nào chứa nó (tab bị đóng, file rời Log Space, Log Space bị xoá, Edit… làm nó thành file khác, Server bị xoá). Chuyển sang tab khác SHALL NOT xoá cache. Khi tắt app, hệ thống SHALL xoá mọi cache; lúc mở app, hệ thống SHALL xoá mọi thư mục còn sót trong `remote-cache/`. Cache thiếu hoặc hỏng SHALL được xử lý như chưa có cache.

#### Scenario: Chuyển tab rồi đóng tab
- **WHEN** người dùng chuyển từ tab có `web1:/app/app.log` sang tab khác, rồi đóng tab đó
- **THEN** cache của `web1:/app/app.log` còn trong lúc chỉ chuyển tab, và bị xoá khi tab đóng

#### Scenario: Cùng file ở hai tab
- **WHEN** `web1:/app/app.log` ở trong hai tab đang mở, và người dùng đóng một tab
- **THEN** cache của nó còn, cho tới khi tab còn lại cũng đóng

#### Scenario: Cache bị xoá bằng tay
- **WHEN** người dùng xoá thư mục `remote-cache/` rồi chọn lại tab Remote
- **THEN** tab đọc từ Server như lần đầu, và không có lỗi nào

### Requirement: Ghi cache lỗi
Khi không ghi được vào cache của một file (đĩa đầy, lỗi quyền), hệ thống SHALL báo một notice nêu lý do và SHALL ngừng đọc file đó. Các dòng đã có SHALL vẫn xem được. Hệ thống SHALL NOT giữ dòng Remote trong memory thay cho cache.

#### Scenario: Đĩa đầy
- **WHEN** đĩa đầy trong lúc `web1:/app/app.log` nhận dòng mới
- **THEN** một notice báo không ghi được cache, file dừng đọc, và các dòng đã hiện vẫn còn

### Requirement: Kết nối SSH theo tab đang mở
Hệ thống SHALL giữ kết nối tới một Server khi người dùng chỉ chuyển sang tab khác. Khi không còn tab đang mở nào có file của Server đó (Remote File, Remote Command, Live Glob), hệ thống SHALL đóng kết nối tới Server đó.

#### Scenario: Đóng tab Remote cuối cùng của một Server
- **WHEN** chỉ một tab đang mở có file của `web1`, và người dùng đóng tab đó
- **THEN** kết nối tới `web1` được đóng
