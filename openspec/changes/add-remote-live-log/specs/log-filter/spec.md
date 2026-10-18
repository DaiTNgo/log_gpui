## MODIFIED Requirements

### Requirement: Số đếm không đổi theo Filter khác
Hệ thống SHALL tính danh sách giá trị và số dòng của mỗi cột theo các dòng view đang giữ: tính khi nạp file, cộng thêm khi có dòng mới, và trừ đi khi dòng rời Live Window. Hệ thống SHALL NOT thay đổi chúng khi áp dụng điều kiện của cột khác.

#### Scenario: Áp Filter không đổi số đếm
- **WHEN** cột `module` có `DB` với số đếm 120 và người dùng tích `ERROR` ở cột `level`
- **THEN** số đếm của `DB` vẫn là 120

#### Scenario: Dòng rời Live Window
- **WHEN** `DB` có số đếm 120 và 20 dòng `DB` rời Live Window
- **THEN** số đếm của `DB` là 100

### Requirement: Khi tail
Khi file đang nhận dòng mới và xuất hiện giá trị mới ở một cột lọc được, hệ thống SHALL thêm giá trị đó vào cuối danh sách chưa tích và cộng số đếm theo dòng mới. Nếu cột vượt 500 giá trị riêng biệt, hệ thống SHALL giữ các điều kiện đã bật và SHALL NOT cho thêm điều kiện mới. Khi số đếm của một giá trị về 0 vì dòng rời Live Window, giá trị đó SHALL biến mất khỏi danh sách trừ khi đang được tích.

#### Scenario: Giá trị mới khi tail
- **WHEN** đang tail và xuất hiện dòng đầu tiên có `module` là `SEARCH`
- **THEN** `SEARCH` xuất hiện cuối danh sách `module`, chưa tích, với số đếm 1

#### Scenario: Cột vượt ngưỡng khi tail
- **WHEN** cột `module` có điều kiện đang bật và vượt 500 giá trị riêng biệt do tail
- **THEN** điều kiện đang bật vẫn có hiệu lực và không thể thêm điều kiện mới

#### Scenario: Giá trị hết dòng
- **WHEN** dòng `module = CRON` cuối cùng rời Live Window và `CRON` chưa được tích
- **THEN** `CRON` biến mất khỏi danh sách `module`

#### Scenario: Giá trị đang tích hết dòng
- **WHEN** dòng `module = CRON` cuối cùng rời Live Window và `CRON` đang được tích
- **THEN** `CRON` vẫn còn trong danh sách, vẫn tích, với số đếm 0
