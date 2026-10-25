## MODIFIED Requirements

### Requirement: Search khớp trên toàn bộ dòng log
Hệ thống SHALL khớp Search Expression trên toàn bộ `raw_line` của từng dòng log, không theo trường hay theo cột nào. Việc tìm SHALL chạy như một Scan ở nền: Match SHALL xuất hiện dần, ô Search SHALL hiện tổng tạm thời và tiến độ cho tới khi quét xong, và ô Search SHALL luôn gõ được.

#### Scenario: Từ khóa nằm ở bất kỳ vị trí nào trong dòng
- **WHEN** người dùng gõ `error` và một dòng có `ERROR` ở cột level, một dòng khác có `error` trong message
- **THEN** cả hai dòng đều là Match

#### Scenario: Search không đổi tập dòng
- **WHEN** người dùng gõ một Search Expression
- **THEN** danh sách dòng đang hiển thị không đổi, chỉ có Match được đánh dấu và có thể điều hướng

#### Scenario: Match hiện dần
- **WHEN** người dùng gõ `timeout` trên file 3.000.000 dòng
- **THEN** ô Search hiện tổng tạm thời tăng dần kèm tiến độ, rồi tổng chính thức khi quét xong
