# merged-timeline Specification

## Purpose
TBD - created by archiving change add-log-space. Update Purpose after archive.
## Requirements
### Requirement: Gộp dòng theo timestamp
Hệ thống SHALL hiện Parsed View của một Log Space bằng cách gộp dòng của mọi file và xếp tăng dần theo timestamp. Hai dòng trùng timestamp SHALL theo thứ tự file của Log Space, rồi theo số dòng.

#### Scenario: Xen kẽ hai file
- **WHEN** `api.log` có dòng lúc 10:00:01 và 10:00:03, còn `worker.log` có dòng lúc 10:00:02
- **THEN** Parsed View hiện theo thứ tự 10:00:01 (`api.log`), 10:00:02 (`worker.log`), 10:00:03 (`api.log`)

#### Scenario: Trùng timestamp
- **WHEN** `api.log` đứng trước `worker.log` trong Log Space và cả hai có một dòng lúc 10:00:05
- **THEN** dòng của `api.log` hiện trước

### Requirement: Dòng không có timestamp riêng
Hệ thống SHALL giữ dòng tiếp nối (ví dụ stack trace) đi liền ngay sau dòng cha. Dòng nằm trước timestamp đầu tiên của một file SHALL lấy timestamp đầu tiên của file đó. File không có timestamp nào SHALL được xếp sau mọi dòng khác và SHALL được gắn cờ "No timestamps".

#### Scenario: Stack trace
- **WHEN** `api.log` có dòng lỗi lúc 10:00:01 kèm 5 dòng stack trace, và `worker.log` có dòng lúc 10:00:01.500
- **THEN** 5 dòng stack trace hiện ngay sau dòng lỗi, trước dòng của `worker.log`

#### Scenario: File không có timestamp
- **WHEN** Log Space có `notes.txt` không parse được timestamp nào
- **THEN** các dòng của `notes.txt` nằm cuối canvas, và `notes.txt` hiện cờ "No timestamps" trên sidebar

### Requirement: Không đổi múi giờ
Hệ thống SHALL so timestamp theo giá trị ghi trong từng file, và SHALL NOT đổi múi giờ.

#### Scenario: Hai file khác múi giờ
- **WHEN** `a.log` ghi `10:00` theo UTC và `b.log` ghi `17:00` theo giờ Việt Nam cho cùng một thời điểm
- **THEN** dòng của `a.log` được xếp trước dòng của `b.log`

### Requirement: Hợp cột theo tên
Hệ thống SHALL lấy các cột của Log Space là hợp các cột phát hiện trong từng file, gộp theo tên cột. Dòng của file không có một cột SHALL để trống ở cột đó.

#### Scenario: Cột chỉ có ở một file
- **WHEN** `api.log` có cột `module` còn `nginx.log` không có
- **THEN** Parsed View có cột `module`, và các dòng của `nginx.log` để trống ở cột đó

### Requirement: Source Column
Hệ thống SHALL thêm Source Column vào Parsed View của Log Space, ghi tên Source File của mỗi dòng. Source Column SHALL ghim được, đổi độ rộng được, và SHALL là Filterable Column. Source Column SHALL mặc định hiện khi Log Space có từ hai file và ẩn khi chỉ có một file.

#### Scenario: Lọc theo file
- **WHEN** người dùng mở Filter cột Source và chỉ tick `api.log`
- **THEN** canvas chỉ còn các dòng của `api.log`

#### Scenario: Một file
- **WHEN** Log Space chỉ có một file
- **THEN** Source Column mặc định bị ẩn

### Requirement: Quick Settings
Hệ thống SHALL có nút Quick Settings trên toolbar, mở popover chứa công tắc hiện/ẩn Source Column và lựa chọn density. Lựa chọn hiện/ẩn Source Column SHALL được nhớ theo từng Log Space.

#### Scenario: Ẩn Source Column
- **WHEN** người dùng tắt "Show source file" trong Quick Settings
- **THEN** Source Column biến mất, canvas rộng thêm phần đó, và Filter đang đặt trên cột Source vẫn giữ hiệu lực

### Requirement: File View
Hệ thống SHALL cho mở riêng một file của Log Space thành File View, chỉ hiện các dòng của file đó và không có Source Column. File View SHALL có nút chuyển Raw/Parsed như Log Space.

#### Scenario: Mở File View
- **WHEN** người dùng bấm `api.log` trong Log Space "Payment"
- **THEN** tab mới chỉ hiện các dòng của `api.log`, không có Source Column

### Requirement: Dòng mới khi tail
Khi một file Imported được ghi thêm dòng trong lúc đang mở, hệ thống SHALL chèn dòng mới vào đúng vị trí theo timestamp, và SHALL NOT làm nhảy vị trí người dùng đang đọc.

#### Scenario: Chèn vào giữa
- **WHEN** người dùng đang đọc quanh 10:05 và `worker.log` ghi thêm một dòng lúc 10:01
- **THEN** dòng mới nằm ở vị trí 10:01, và các dòng đang hiện trên màn hình không bị dịch đi

