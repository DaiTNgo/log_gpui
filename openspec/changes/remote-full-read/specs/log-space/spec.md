## MODIFIED Requirements

### Requirement: Imported và Scratch không trộn lẫn
Hệ thống SHALL chỉ thêm file có sẵn trên đĩa, Archive Entry và Remote Copy vào Imported Log Space, chỉ thêm file scratch rỗng vào Scratch Log Space, và chỉ thêm Remote File vào Remote Log Space. Remote Copy SHALL chỉ được thêm qua Open in full.

#### Scenario: Menu của Imported
- **WHEN** người dùng chuột phải lên một Imported Log Space
- **THEN** menu có "Add files…" và không có "New scratch file" hay "Add remote files…"

#### Scenario: Menu của Scratch
- **WHEN** người dùng chuột phải lên một Scratch Log Space
- **THEN** menu có "New scratch file" và không có "Add files…" hay "Add remote files…"

#### Scenario: Menu của Remote
- **WHEN** người dùng chuột phải lên một Remote Log Space
- **THEN** menu có "Add remote files…", "Open in full…" và "Download all", và không có "Add files…" hay "New scratch file"

#### Scenario: Open in full vào Log Space có sẵn
- **WHEN** người dùng chọn Open in full trên `web1:/app/app.log` và chọn đặt vào Imported Log Space "Incident"
- **THEN** "Incident" có thêm Remote Copy `web1:/app/app.log` bên cạnh các file trên đĩa của nó

### Requirement: File Imported chỉ đọc
Hệ thống SHALL NOT sửa, đổi tên hay xóa file Imported trên đĩa hay Remote File trên Server. Gỡ file khỏi Log Space SHALL chỉ bỏ nó khỏi danh sách, trừ Remote Copy: gỡ Remote Copy SHALL xoá nội dung đã lưu của nó trong thư mục dữ liệu của app.

#### Scenario: Gỡ file
- **WHEN** người dùng chọn "Remove from Log Space" trên `api.log`
- **THEN** `api.log` biến mất khỏi Log Space, file trên đĩa vẫn còn nguyên, và các Marker của nó bị bỏ

#### Scenario: Gỡ file cuối cùng
- **WHEN** người dùng gỡ file duy nhất của một Imported Log Space
- **THEN** Log Space vẫn còn, và canvas hiện trạng thái rỗng kèm nút "Add files"

#### Scenario: Remote File chỉ đọc
- **WHEN** người dùng mở Raw View của `web1:/app/app.log`
- **THEN** text không sửa được, và hệ thống không bao giờ ghi vào file đó trên Server

#### Scenario: Remote Copy chỉ đọc
- **WHEN** người dùng mở Raw View của Remote Copy `web1:/app/app.log`
- **THEN** text không sửa được
