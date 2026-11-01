## MODIFIED Requirements

### Requirement: Cột lọc được
Hệ thống SHALL cho phép lọc theo giá trị trên mọi cột có tối đa 1000 giá trị riêng biệt, trừ `timestamp` và `message`/payload. Hệ thống SHALL NOT cho thêm điều kiện giá trị trên cột vượt 1000 giá trị riêng biệt.

#### Scenario: Cột nằm trong ngưỡng
- **WHEN** cột `module` có 7 giá trị riêng biệt
- **THEN** người dùng có thể thêm Filter cho cột `module`

#### Scenario: Cột có vài trăm giá trị
- **WHEN** cột `thread` có 800 giá trị riêng biệt
- **THEN** người dùng có thể thêm Filter cho cột `thread`

#### Scenario: Cột quá ngưỡng
- **WHEN** cột `request_id` có hơn 1000 giá trị riêng biệt
- **THEN** cột đó hiển thị mờ kèm số giá trị riêng biệt và lý do, và không thể thêm Filter cho nó

#### Scenario: Cột luôn bị loại
- **WHEN** người dùng chọn cột để thêm Filter
- **THEN** `timestamp` và `message`/payload không có trong danh sách chọn cột giá trị

### Requirement: Hiển thị số giá trị riêng biệt khi chọn cột
Hệ thống SHALL hiển thị số giá trị riêng biệt (Distinct Count) của từng cột trong danh sách chọn cột để thêm Filter.

#### Scenario: Số đếm bên cạnh cột
- **WHEN** người dùng mở danh sách chọn cột để thêm Filter
- **THEN** mỗi cột hiển thị số giá trị riêng biệt của nó, và cột vượt ngưỡng hiển thị "> 1000"

### Requirement: Khi tail
Khi file đang được tail và xuất hiện giá trị mới ở một cột lọc được, hệ thống SHALL thêm giá trị đó vào cuối danh sách chưa tích và cộng số đếm theo dòng mới. Nếu cột vượt 1000 giá trị riêng biệt, hệ thống SHALL giữ các điều kiện đã bật và SHALL NOT cho thêm điều kiện mới.

#### Scenario: Giá trị mới khi tail
- **WHEN** đang tail và xuất hiện dòng đầu tiên có `module` là `SEARCH`
- **THEN** `SEARCH` xuất hiện cuối danh sách `module`, chưa tích, với số đếm 1

#### Scenario: Cột vượt ngưỡng khi tail
- **WHEN** cột `module` có điều kiện đang bật và vượt 1000 giá trị riêng biệt do tail
- **THEN** điều kiện đang bật vẫn có hiệu lực và không thể thêm điều kiện mới
