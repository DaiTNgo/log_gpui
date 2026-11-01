## MODIFIED Requirements

### Requirement: Marker trong Raw View
Hệ thống SHALL cho tô Marker lên đoạn text đã chọn trong Raw View, và đổi màu hoặc gỡ Marker từ chính đoạn đã tô, như trong Parsed View. Trong Raw View đang sửa được, thanh màu để tô SHALL chỉ mở qua lệnh "Mark: Color Selection…"; đặt con trỏ lên một Marker hoặc Text Highlight có sẵn SHALL vẫn mở thanh màu để đổi màu hoặc gỡ. Marker SHALL dùng chung giữa Raw View, Parsed View, Log Space và File View. Sửa nội dung một dòng scratch SHALL làm mất mọi Marker của dòng đó.

#### Scenario: Marker dùng chung
- **WHEN** người dùng tô `timeout` ở dòng 42 của `api.log` trong Raw View rồi chuyển sang Parsed View
- **THEN** chữ `timeout` ở dòng 42 của `api.log` có cùng màu trong Parsed View

#### Scenario: Sửa dòng có Marker
- **WHEN** dòng 10 của `untitled.log` có Marker và người dùng sửa nội dung dòng đó
- **THEN** sau khi lưu, dòng 10 không còn Marker nào

#### Scenario: Không có Highlight Rule
- **WHEN** một dòng có Marker và khớp một Highlight Rule đang bật
- **THEN** Raw View chỉ hiện màu Marker, không có màu của rule

#### Scenario: Đổi màu Marker trong editor scratch
- **WHEN** dòng 5 của `untitled.log` đang sửa được có Marker xanh trên `error` và người dùng bấm vào giữa chữ `error`
- **THEN** thanh màu mở để đổi màu hoặc gỡ Marker đó

#### Scenario: Kéo chọn trong editor scratch không tô
- **WHEN** người dùng kéo chọn dòng 2 tới dòng 7 của `untitled.log` đang sửa được rồi nhấn Delete
- **THEN** các dòng đó bị xóa, không có thanh màu nào hiện trong lúc chọn
