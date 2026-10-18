## ADDED Requirements

### Requirement: Filter và Marker theo view
Hệ thống SHALL giữ một Filter chung cho mỗi Log Space và một Filter riêng cho mỗi File View, tự động lưu và khôi phục khi mở lại hoặc khởi động lại ứng dụng. Marker SHALL gắn với một dòng của một file, và SHALL hiện ở mọi view có dòng đó. Hệ thống SHALL NOT lưu Search Expression, và SHALL NOT có preset Filter theo Project.

#### Scenario: Filter chung của Log Space
- **WHEN** người dùng đặt Filter `level` = `ERROR` trong Log Space "Payment" có `api.log` và `worker.log`
- **THEN** cả dòng của `api.log` lẫn `worker.log` đều chỉ còn `ERROR`

#### Scenario: File View có Filter riêng
- **WHEN** Log Space "Payment" đang lọc `level` = `ERROR` và người dùng mở File View `api.log`
- **THEN** File View `api.log` không có Filter nào, và đặt Filter trong File View không làm đổi Filter của "Payment"

#### Scenario: Marker dùng chung giữa các view
- **WHEN** người dùng đặt Marker trên dòng 42 của `api.log` trong File View
- **THEN** dòng đó cũng có Marker trong Log Space "Payment"

#### Scenario: Khôi phục khi mở lại
- **WHEN** người dùng đóng tab "Payment" có Filter và Marker rồi mở lại
- **THEN** Filter và Marker được khôi phục và ô Search trống

### Requirement: Chỉ mục Filter trên tập dòng đã gộp
Hệ thống SHALL tính các cột lọc được, Filter Value, số dòng và Distinct Count của Log Space trên tập dòng gộp từ mọi file. File View SHALL tính trên các dòng của riêng file đó.

#### Scenario: Giá trị từ nhiều file
- **WHEN** `api.log` có `module` = `API` và `worker.log` có `module` = `JOB`
- **THEN** Filter cột `module` của Log Space liệt kê cả `API` và `JOB`

## MODIFIED Requirements

### Requirement: Không còn các điểm vào cũ
Hệ thống SHALL NOT có panel "View", danh sách module cố định, menu nhanh trên giá trị level/module, hay "Saved View". Ghim cột trên Column Header, đổi độ rộng cột và Auto-fit SHALL vẫn hoạt động. Density và công tắc Source Column SHALL nằm trong Quick Settings trên toolbar, và Quick Settings SHALL chỉ chứa các công tắc hiển thị.

#### Scenario: Không có panel View
- **WHEN** người dùng mở context sidebar
- **THEN** các mục là Filter, Marker và Inspector, không có View

#### Scenario: Ghim cột vẫn dùng được
- **WHEN** người dùng bấm Pin Toggle trên Column Header `LEVEL`
- **THEN** cột `LEVEL` được ghim như trước

#### Scenario: Density trong Quick Settings
- **WHEN** người dùng mở Quick Settings trên toolbar
- **THEN** có lựa chọn density và công tắc "Show source file"

## REMOVED Requirements

### Requirement: Filter và Marker riêng cho từng file
**Reason**: Log Space gộp nhiều file vào một canvas, nên Filter gắn với view (Log Space hoặc File View) thay vì với file.
**Migration**: Dùng requirement "Filter và Marker theo view". Session cũ không được chuyển đổi.
