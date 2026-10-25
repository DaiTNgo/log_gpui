## ADDED Requirements

### Requirement: Thao tác cần text chạy như Scan
Hệ thống SHALL chạy như một Scan ở nền mọi thao tác cần đọc lại text của các dòng: Search, Filter Custom Value trên cột cần text, Highlight Rule có Custom Value, bật một Detected Field mới, và đổi Log Format. Scan SHALL NOT chặn UI, và áp dụng cho mọi cỡ file. Kết quả SHALL xuất hiện dần, kèm tiến độ.

#### Scenario: Search trên file lớn
- **WHEN** người dùng gõ `timeout` trong Search trên một file 3.000.000 dòng
- **THEN** ô Search vẫn gõ được ngay, Match hiện dần, và có tiến độ cho tới khi quét hết

#### Scenario: File nhỏ
- **WHEN** người dùng gõ `timeout` trên một file 1.000 dòng
- **THEN** Match hiện sau Scan, thường trong lô đầu tiên

### Requirement: Kết quả gần vị trí đang xem có trước
Scan SHALL quét các dòng quanh vị trí đang xem trước, rồi mới tới phần còn lại.

#### Scenario: Match ở màn hình hiện tại
- **WHEN** người dùng đang xem dòng 2.000.000 và gõ một Search Expression khớp dòng 2.000.010
- **THEN** Match ở dòng 2.000.010 được tô trước khi Scan tới đầu hay cuối file

### Requirement: Tổng tạm thời và tiến độ
Trong lúc Scan chạy, hệ thống SHALL hiện tổng số kết quả đã tìm thấy như một tổng tạm thời, cùng tiến độ. Hệ thống SHALL hiện tổng chính thức khi Scan xong. Điều hướng giữa các Match SHALL dùng được trên các Match đã tìm thấy.

#### Scenario: Đang quét
- **WHEN** Scan Search mới quét 30% và đã thấy 120 Match
- **THEN** ô Search hiện 120 cùng dấu hiệu đang quét, và Next nhảy được tới các Match đó

### Requirement: Scan mới huỷ Scan cũ
Khi người dùng đổi truy vấn của một thao tác đang Scan, hệ thống SHALL huỷ Scan cũ, SHALL NOT trộn kết quả cũ vào kết quả mới, và SHALL bắt đầu Scan mới.

#### Scenario: Gõ tiếp
- **WHEN** Scan cho `time` đang chạy và người dùng gõ thêm thành `timeout`
- **THEN** Scan cho `time` dừng, và chỉ Match của `timeout` được hiện

### Requirement: Thao tác không cần text vẫn tức thì
Tick hay bỏ tick Filter Value, Time Range, Highlight Rule chỉ dựa trên Filter Value, Marker, và Go to SHALL áp dụng ngay, không qua Scan.

#### Scenario: Tick level
- **WHEN** người dùng tick `ERROR` ở cột `level` trên một file 3.000.000 dòng đã index
- **THEN** Focus Pane đổi ngay, không có tiến độ Scan

### Requirement: Dòng mới khi live
Khi có dòng mới đến trong lúc một kết quả Scan đang được dùng, hệ thống SHALL chỉ quét các dòng mới để cập nhật kết quả, không quét lại toàn bộ.

#### Scenario: Search khi tail
- **WHEN** Search `timeout` đã quét xong và 1.000 dòng mới đến, trong đó có 3 dòng chứa `timeout`
- **THEN** tổng Match tăng thêm 3 mà không có Scan toàn bộ

### Requirement: Đổi Log Format hoặc Detected Field
Khi người dùng đổi Log Format của một file, hoặc bật một Detected Field mới, hệ thống SHALL đọc lại các dòng ở nền với tiến độ, rồi cập nhật cột, Filter Value và số đếm khi có kết quả.

#### Scenario: Bật Detected Field
- **WHEN** người dùng bật Detected Field `user` trên file 3.000.000 dòng
- **THEN** cột `user` hiện ngay cho các dòng đang xem, còn danh sách Filter Value của `user` hiện dần kèm tiến độ
