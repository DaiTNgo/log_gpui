## MODIFIED Requirements

### Requirement: Nhận file vào dialog
Hệ thống SHALL nhận file qua kéo thả và qua nút Browse. Thư mục SHALL được mở rộng một cấp và chỉ lấy file text và Archive. Archive SHALL được mở rộng thành các Archive Entry theo capability `archive-import`. File binary SHALL hiện trong danh sách kèm cảnh báo và SHALL NOT được thêm khi tạo. Người dùng SHALL bỏ được từng file hoặc Archive Entry khỏi danh sách trước khi bấm Create.

#### Scenario: Thả thư mục
- **WHEN** người dùng thả thư mục `logs/` chứa `a.log`, `b.log` và thư mục con `old/`
- **THEN** danh sách có `a.log` và `b.log`, không có file nào trong `old/`

#### Scenario: File binary
- **WHEN** người dùng thả `core.dump` có byte NUL
- **THEN** `core.dump` hiện trong danh sách với cảnh báo "Not a text file" và không được đếm vào nhãn Create

#### Scenario: Bỏ một file
- **WHEN** danh sách có `a.log` và `b.log` và người dùng bấm bỏ `b.log`
- **THEN** chỉ còn `a.log` và nhãn nút là "Create with 1 file"

#### Scenario: Thư mục có Archive
- **WHEN** người dùng thả thư mục `logs/` chứa `a.log` và `bundle.zip` (bên trong có `app.log`)
- **THEN** danh sách có `a.log` và `bundle.zip › app.log`

#### Scenario: Bỏ một Archive Entry
- **WHEN** danh sách có `bundle.zip › app.log` và `bundle.zip › db.log` và người dùng bấm bỏ `bundle.zip › db.log`
- **THEN** chỉ còn `bundle.zip › app.log` và nhãn nút là "Create with 1 file"

### Requirement: Thêm file vào Imported Log Space
Hệ thống SHALL cho thêm file vào Imported Log Space qua "Add files…", nhận file và Archive theo cùng quy tắc như dialog. File đã có trong Log Space SHALL bị bỏ qua kèm thông báo nhẹ. Hai Archive Entry SHALL được coi là trùng khi cùng đường dẫn Archive và cùng đường dẫn trong Archive.

#### Scenario: Thêm file trùng
- **WHEN** Log Space đã có `/var/log/api.log` và người dùng thêm lại file đó
- **THEN** danh sách file không đổi và hiện thông báo file đã có

#### Scenario: Thêm lại Archive
- **WHEN** Log Space đã có `bundle.zip › app.log` và người dùng thêm lại `bundle.zip` có `app.log` và `db.log`
- **THEN** chỉ `bundle.zip › db.log` được thêm và hiện thông báo một file đã có

#### Scenario: Cùng tên entry ở Archive khác
- **WHEN** Log Space đã có `a.zip › app.log` và người dùng thêm `b.zip` chứa `app.log`
- **THEN** `b.zip › app.log` được thêm
