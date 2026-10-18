## ADDED Requirements

### Requirement: Filter thu hẹp Focus Pane, không ẩn dòng ở view
Hệ thống SHALL áp dụng Filter lên Focus Pane và SHALL NOT ẩn dòng nào trong Parsed View. Ở mọi yêu cầu của Filter, "hiển thị" nghĩa là được liệt kê trong Focus Tab, và "ẩn" nghĩa là không được liệt kê trong Focus Tab. Sidebar Filter SHALL chỉ hiện ở Parsed View, và Filter SHALL NOT áp dụng khi view là Raw View.

#### Scenario: View vẫn đầy đủ
- **WHEN** người dùng tích `ERROR` ở cột `level` trong Parsed View
- **THEN** view vẫn hiện mọi dòng, và Focus Tab chỉ liệt kê các dòng `ERROR`

#### Scenario: Hide Mode
- **WHEN** người dùng bật Hide Mode ở cột `level` và tích `DEBUG`
- **THEN** các dòng `DEBUG` vẫn hiện ở view nhưng không có trong Focus Tab

#### Scenario: Chép ở view
- **WHEN** Filter chỉ hiện `ERROR` và người dùng chọn 10 dòng liền nhau ở view, trong đó có 3 dòng `INFO`, rồi bấm ⌘C
- **THEN** clipboard có cả 10 dòng

## MODIFIED Requirements

### Requirement: Filter và Marker theo view
Hệ thống SHALL giữ một Filter chung cho mỗi Log Space và một Filter riêng cho mỗi File View, tự động lưu và khôi phục khi mở lại hoặc khởi động lại ứng dụng. Các Focus Tab của view, cùng Search Expression, Focus Find và dòng hiện tại của chúng, SHALL được lưu và khôi phục theo cùng cách. Marker SHALL gắn với một dòng của một file, và SHALL hiện ở mọi view và Focus Pane có dòng đó. Hệ thống SHALL NOT có preset Filter theo Project.

#### Scenario: Filter chung của Log Space
- **WHEN** người dùng đặt Filter `level` = `ERROR` trong Log Space "Payment" có `api.log` và `worker.log`
- **THEN** Focus Tab của "Payment" chỉ liệt kê dòng `ERROR` của cả `api.log` lẫn `worker.log`

#### Scenario: File View có Filter riêng
- **WHEN** Log Space "Payment" đang lọc `level` = `ERROR` và người dùng mở File View `api.log`
- **THEN** File View `api.log` không có Filter nào, và đặt Filter trong File View không làm đổi Filter của "Payment"

#### Scenario: Marker dùng chung giữa các view
- **WHEN** người dùng đặt Marker trên dòng 42 của `api.log` trong File View
- **THEN** dòng đó cũng có Marker trong Log Space "Payment" và trong Focus Pane khi dòng đó được liệt kê

#### Scenario: Khôi phục khi mở lại
- **WHEN** người dùng đóng tab "Payment" có Filter, Marker và Focus Tab `timeout` rồi mở lại
- **THEN** Filter, Marker và Focus Tab được khôi phục, và ô Search hiện `timeout`
