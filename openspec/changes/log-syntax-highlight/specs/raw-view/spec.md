## ADDED Requirements

### Requirement: Dòng trong Raw View có Syntax Color
Khi Syntax Color bật, Raw View SHALL tô màu chữ mỗi dòng theo các loại token của Syntax Color. Tô màu SHALL NOT thay đổi nội dung mà Raw View hiện (xem "Raw View hiện đúng nội dung file"), và SHALL NOT làm file scratch đang sửa hiện khác nội dung đã gõ.

#### Scenario: Raw View của file log
- **WHEN** người dùng mở Raw View của một file log có timestamp và level
- **THEN** timestamp và level có màu riêng, chữ còn lại giữ màu mặc định

#### Scenario: Sửa file scratch
- **WHEN** người dùng gõ vào một dòng của file scratch trong Raw View
- **THEN** màu token của dòng đó cập nhật theo chữ mới và con trỏ, vùng chọn không bị lệch
