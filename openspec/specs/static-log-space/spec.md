# static-log-space Specification

## Purpose
Static Log Space là bản cố định của file local, Archive Entry và Remote Copy: hiện file như đang có trên đĩa, không có Live/Pause, Follow hay Generation.
## Requirements
### Requirement: Static Log Space không theo dõi
Một Static Log Space SHALL hiện nội dung file như đang có trên đĩa. Nó SHALL NOT có Live/Pause, Follow, Break, Generation, Load more, Live Window hay Clear. Status bar của nó SHALL chỉ hiện số dòng và trạng thái file.

#### Scenario: Status bar của Static
- **WHEN** người dùng mở Static Log Space "Payment" chứa `api.log`
- **THEN** status bar hiện số dòng, không có nút Live, Pause hay Follow

#### Scenario: Không có Break
- **WHEN** `api.log` của một Static Log Space bị thay bằng file khác
- **THEN** view hiện nội dung file mới và không có dòng Break nào

### Requirement: Đọc lại file đã đổi trên đĩa
Khi một file local của Static Log Space đổi trên đĩa, hệ thống SHALL cập nhật view theo nội dung mới ở nền, mà SHALL NOT chặn UI.
- File chỉ được ghi thêm (cùng inode, dài hơn, phần đầu và phần cuối đã đọc không đổi): hệ thống SHALL thêm các dòng mới và SHALL NOT tự cuộn.
- File bị thay bằng inode mới, bị cắt ngắn, hay bị ghi đè cùng inode (phần đầu hoặc phần cuối đã đọc khác đi): hệ thống SHALL đọc lại cả file.

Filter, Search và Highlight Rule SHALL được giữ qua việc đọc lại. Hệ thống SHALL chỉ kiểm tra thay đổi cho tab đang mở. Một tab từ Dormant được mở lại SHALL được kiểm tra một lần trước khi hiện.

#### Scenario: File bị thay bằng mv
- **WHEN** `api.log` có 100 dòng đang mở trong Static Log Space, và người dùng chạy `mv new.log api.log` với `new.log` có 40 dòng
- **THEN** view hiện 40 dòng của `new.log`, Filter đang bật vẫn còn

#### Scenario: File bị ghi đè cùng inode, dài hơn
- **WHEN** `api.log` có 100 dòng, và người dùng chạy `cp bigger.log api.log` với `bigger.log` có 300 dòng khác hẳn
- **THEN** view hiện đúng 300 dòng của `bigger.log`, không còn dòng nào của nội dung cũ

#### Scenario: File được ghi thêm
- **WHEN** người dùng đang xem dòng 10 của `api.log` và có 5 dòng được ghi thêm vào cuối file
- **THEN** 5 dòng đó xuất hiện ở cuối view, và view vẫn ở dòng 10

#### Scenario: Tab Dormant
- **WHEN** `api.log` thuộc một tab Dormant bị thay, rồi người dùng bấm vào tab đó
- **THEN** tab hiện nội dung mới của `api.log`

#### Scenario: File bị xoá
- **WHEN** `api.log` đang mở trong Static Log Space bị xoá
- **THEN** view vẫn hiện nội dung cũ kèm trạng thái "File deleted", và khi một file `api.log` mới xuất hiện thì view đọc file đó

### Requirement: Marker sau khi đọc lại
Sau khi đọc lại một file của Static Log Space, hệ thống SHALL giữ mỗi Marker có dòng cùng số dòng còn khớp `line_hash`, và SHALL bỏ Marker có dòng không còn khớp hoặc không còn. Khi có Marker bị bỏ, hệ thống SHALL hiện notice cho biết số Marker bị bỏ.

#### Scenario: Marker còn khớp
- **WHEN** dòng 5 của `api.log` có Marker, và file được thay bằng một bản có dòng 5 giống hệt
- **THEN** Marker vẫn ở dòng 5

#### Scenario: Marker không còn khớp
- **WHEN** dòng 5 và dòng 9 của `api.log` có Marker, và file được thay bằng một bản có dòng 5 và dòng 9 khác đi
- **THEN** hai Marker bị bỏ và notice "2 Markers no longer match the file" hiện ra

### Requirement: Reload cho Remote Copy
Một Remote Copy SHALL được đọc một lần, và hệ thống SHALL NOT giữ SSH session để theo dõi nó. Menu của Remote Copy SHALL có "Reload". Reload SHALL đọc lại Remote File cùng các Rotated File đã chọn, với cùng Starting Time, và SHALL giữ Marker theo cùng quy tắc như khi đọc lại file local.

#### Scenario: Reload một Remote Copy
- **WHEN** người dùng chọn "Reload" trên Remote Copy `web1:/app/app.log`, sau khi file trên server có thêm 50 dòng
- **THEN** Remote Copy hiện cả 50 dòng mới, và Marker còn khớp vẫn ở chỗ cũ

#### Scenario: Không theo dõi server
- **WHEN** một Static Log Space chỉ có Remote Copy đang mở, và file trên server có thêm dòng mới
- **THEN** view không đổi cho tới khi người dùng chọn "Reload"

