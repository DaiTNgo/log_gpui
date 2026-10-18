## MODIFIED Requirements

### Requirement: Không đổi múi giờ
Hệ thống SHALL so timestamp theo giá trị ghi trong từng file, và SHALL NOT đổi múi giờ hay chỉnh lệch giờ, kể cả giữa các file trên các Server khác nhau.

#### Scenario: Hai file khác múi giờ
- **WHEN** `a.log` ghi `10:00` theo UTC và `b.log` ghi `17:00` theo giờ Việt Nam cho cùng một thời điểm
- **THEN** dòng của `a.log` được xếp trước dòng của `b.log`

#### Scenario: Hai Server khác múi giờ
- **WHEN** `web1:/app/app.log` ghi giờ UTC và `web2:/app/app.log` ghi giờ +07:00 không kèm múi giờ
- **THEN** dòng được xếp theo đúng chữ số ghi trong file, không chỉnh lệch 7 tiếng

### Requirement: Dòng mới khi tail
Khi một file Imported hoặc Remote File được ghi thêm dòng trong lúc Log Space đang mở và không Pause, hệ thống SHALL chèn dòng mới vào đúng vị trí theo timestamp. Khi Follow tắt, hệ thống SHALL NOT làm nhảy vị trí người dùng đang đọc. Khi Follow bật, view SHALL giữ dòng mới nhất trong tầm nhìn.

#### Scenario: Chèn vào giữa
- **WHEN** Follow tắt, người dùng đang đọc quanh 10:05 và `worker.log` ghi thêm một dòng lúc 10:01
- **THEN** dòng mới nằm ở vị trí 10:01, và các dòng đang hiện trên màn hình không bị dịch đi

#### Scenario: Follow bật
- **WHEN** Follow bật và `web1:/app/app.log` ghi thêm một dòng có timestamp mới nhất
- **THEN** view cuộn để dòng đó hiện ở cuối màn hình
