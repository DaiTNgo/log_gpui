## MODIFIED Requirements

### Requirement: Time Range
Hệ thống SHALL cung cấp Filter khoảng thời gian cho `timestamp` với hai ô Từ và Đến. Mỗi ô SHALL nhận mọi định dạng thời gian mà ứng dụng đọc được từ dòng log, và SHALL có placeholder lấy từ timestamp đầu tiên của view. Ô trống nghĩa là không giới hạn phía đó. Hệ thống SHALL gợi ý giá trị nhỏ nhất và lớn nhất, và SHALL so sánh theo đồng hồ của file, không đổi múi giờ.

#### Scenario: Giới hạn hai phía
- **WHEN** người dùng nhập Từ `10:00:00` và Đến `10:05:00`
- **THEN** chỉ các dòng có timestamp trong khoảng đó được hiển thị

#### Scenario: Một phía
- **WHEN** người dùng chỉ nhập Từ `10:00:00`
- **THEN** mọi dòng từ `10:00:00` trở đi được hiển thị

#### Scenario: Định dạng khác cột hiển thị
- **WHEN** cột TIMESTAMP hiện `2026-10-18T10:00:00+07:00` và người dùng nhập Từ `2026-10-18 10:00:00`
- **THEN** Time Range được chấp nhận và lọc từ thời điểm đó

### Requirement: Không còn các điểm vào cũ
Hệ thống SHALL NOT có panel "View", danh sách module cố định, "Saved View", panel Marker, hay menu nhanh để lọc theo giá trị của ô. Menu chuột phải trên ô SHALL chỉ có các lệnh tô màu ("Mark with color", "Highlight rows where…"). Ghim cột trên Column Header, đổi độ rộng cột và Auto-fit SHALL vẫn hoạt động. Density và công tắc Source Column SHALL nằm trong Quick Settings trên toolbar, và Quick Settings SHALL chỉ chứa các công tắc hiển thị.

#### Scenario: Các mục của context sidebar
- **WHEN** người dùng mở context sidebar
- **THEN** các mục là Filter, Highlights và Inspector, không có View hay Marker

#### Scenario: Ghim cột vẫn dùng được
- **WHEN** người dùng bấm Pin Toggle trên Column Header `LEVEL`
- **THEN** cột `LEVEL` được ghim như trước

#### Scenario: Density trong Quick Settings
- **WHEN** người dùng mở Quick Settings trên toolbar
- **THEN** có lựa chọn density và công tắc "Show source file"
