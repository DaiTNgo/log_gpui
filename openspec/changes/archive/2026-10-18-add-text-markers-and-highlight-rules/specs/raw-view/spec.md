## MODIFIED Requirements

### Requirement: Marker trong Raw View
Hệ thống SHALL cho tô Marker lên đoạn text đã chọn trong Raw View, và đổi màu hoặc gỡ Marker từ chính đoạn đã tô, như trong Parsed View. Marker SHALL dùng chung giữa Raw View, Parsed View, Log Space và File View. Sửa nội dung một dòng scratch SHALL làm mất mọi Marker của dòng đó.

#### Scenario: Marker dùng chung
- **WHEN** người dùng tô `timeout` ở dòng 42 của `api.log` trong Raw View rồi chuyển sang Parsed View
- **THEN** chữ `timeout` ở dòng 42 của `api.log` có cùng màu trong Parsed View

#### Scenario: Sửa dòng có Marker
- **WHEN** dòng 10 của `untitled.log` có Marker và người dùng sửa nội dung dòng đó
- **THEN** sau khi lưu, dòng 10 không còn Marker nào

#### Scenario: Không có Highlight Rule
- **WHEN** một dòng có Marker và khớp một Highlight Rule đang bật
- **THEN** Raw View chỉ hiện màu Marker, không có màu của rule
