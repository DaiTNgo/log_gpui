## ADDED Requirements

### Requirement: Project chứa nhiều Log Space
Hệ thống SHALL cho mỗi Project chứa nhiều Log Space. Mỗi Log Space SHALL có một tên và một danh sách file. Hai Project SHALL NOT dùng chung Log Space.

#### Scenario: Nhiều Log Space trong một Project
- **WHEN** người dùng tạo Log Space "Payment" và "Auth" trong Project "Prod"
- **THEN** sidebar của Project "Prod" hiện cả hai Log Space

#### Scenario: Project khác không thấy
- **WHEN** người dùng chuyển sang Project "Staging"
- **THEN** sidebar không hiện "Payment" hay "Auth"

### Requirement: Mở dialog tạo Log Space
Hệ thống SHALL mở dialog New Log Space khi người dùng bấm nút **+** ở đầu danh sách Log Space trên sidebar. Dialog SHALL gồm ô tên, vùng kéo thả file kèm nút Browse, danh sách file đã chọn, nút Create và nút Cancel.

#### Scenario: Mở dialog
- **WHEN** người dùng bấm **+** trên sidebar
- **THEN** dialog New Log Space hiện ra với ô tên trống, danh sách file trống, và nút Create bị tắt

#### Scenario: Hủy
- **WHEN** người dùng đã nhập tên và thả file rồi bấm Cancel
- **THEN** dialog đóng và không có Log Space nào được tạo

### Requirement: Điều kiện bật nút Create
Hệ thống SHALL chỉ bật nút Create khi tên, sau khi bỏ khoảng trắng hai đầu, không rỗng và không trùng tên Log Space khác trong cùng Project. Hệ thống SHALL hiện lý do ngay dưới ô tên khi tên bị trùng.

#### Scenario: Tên rỗng
- **WHEN** ô tên chỉ chứa khoảng trắng
- **THEN** nút Create bị tắt

#### Scenario: Tên trùng trong Project
- **WHEN** Project đã có Log Space "Payment" và người dùng nhập ` Payment `
- **THEN** nút Create bị tắt và dưới ô tên hiện thông báo tên đã tồn tại

#### Scenario: Tên trùng ở Project khác
- **WHEN** Project "Staging" có Log Space "Payment" và người dùng đang tạo "Payment" trong Project "Prod"
- **THEN** nút Create được bật

### Requirement: Loại Log Space suy ra từ file
Hệ thống SHALL tạo Scratch Log Space khi bấm Create mà danh sách file trống, và SHALL tạo Imported Log Space khi danh sách có ít nhất một file. Dialog SHALL NOT có lựa chọn loại riêng. Nhãn nút Create SHALL cho biết kết quả, ví dụ "Create empty" hoặc "Create with 3 files".

#### Scenario: Tạo Scratch
- **WHEN** người dùng nhập tên "Scratch 1", không thả file nào và bấm "Create empty"
- **THEN** Scratch Log Space "Scratch 1" được tạo với một file rỗng `untitled.log`

#### Scenario: Tạo Imported
- **WHEN** người dùng nhập tên "Payment", thả `api.log` và `worker.log`, rồi bấm "Create with 2 files"
- **THEN** Imported Log Space "Payment" được tạo với hai file đó

### Requirement: Nhận file vào dialog
Hệ thống SHALL nhận file qua kéo thả và qua nút Browse. Thư mục SHALL được mở rộng một cấp và chỉ lấy file text. File binary SHALL hiện trong danh sách kèm cảnh báo và SHALL NOT được thêm khi tạo. Người dùng SHALL bỏ được từng file khỏi danh sách trước khi bấm Create.

#### Scenario: Thả thư mục
- **WHEN** người dùng thả thư mục `logs/` chứa `a.log`, `b.log` và thư mục con `old/`
- **THEN** danh sách có `a.log` và `b.log`, không có file nào trong `old/`

#### Scenario: File binary
- **WHEN** người dùng thả `core.dump` có byte NUL
- **THEN** `core.dump` hiện trong danh sách với cảnh báo "Not a text file" và không được đếm vào nhãn Create

#### Scenario: Bỏ một file
- **WHEN** danh sách có `a.log` và `b.log` và người dùng bấm bỏ `b.log`
- **THEN** chỉ còn `a.log` và nhãn nút là "Create with 1 file"

### Requirement: Imported và Scratch không trộn lẫn
Hệ thống SHALL chỉ thêm file có sẵn trên đĩa vào Imported Log Space, và SHALL chỉ thêm file scratch rỗng vào Scratch Log Space.

#### Scenario: Menu của Imported
- **WHEN** người dùng chuột phải lên một Imported Log Space
- **THEN** menu có "Add files…" và không có "New scratch file"

#### Scenario: Menu của Scratch
- **WHEN** người dùng chuột phải lên một Scratch Log Space
- **THEN** menu có "New scratch file" và không có "Add files…"

### Requirement: File Imported chỉ đọc
Hệ thống SHALL NOT sửa, đổi tên hay xóa file Imported trên đĩa. Gỡ file khỏi Log Space SHALL chỉ bỏ nó khỏi danh sách.

#### Scenario: Gỡ file
- **WHEN** người dùng chọn "Remove from Log Space" trên `api.log`
- **THEN** `api.log` biến mất khỏi Log Space, file trên đĩa vẫn còn nguyên, và các Marker của nó bị bỏ

#### Scenario: Gỡ file cuối cùng
- **WHEN** người dùng gỡ file duy nhất của một Imported Log Space
- **THEN** Log Space vẫn còn, và canvas hiện trạng thái rỗng kèm nút "Add files"

### Requirement: Thêm file vào Imported Log Space
Hệ thống SHALL cho thêm file vào Imported Log Space qua "Add files…", nhận file theo cùng quy tắc như dialog. File đã có trong Log Space SHALL bị bỏ qua kèm thông báo nhẹ.

#### Scenario: Thêm file trùng
- **WHEN** Log Space đã có `/var/log/api.log` và người dùng thêm lại file đó
- **THEN** danh sách file không đổi và hiện thông báo file đã có

### Requirement: File scratch
Hệ thống SHALL lưu file scratch trong thư mục dữ liệu của ứng dụng. File đầu tiên SHALL tên `untitled.log`; "New scratch file" SHALL tạo `untitled-2.log`, `untitled-3.log`… theo số nhỏ nhất chưa dùng. Người dùng SHALL đổi tên và xóa được file scratch. Xóa file SHALL cần xác nhận, và SHALL NOT xóa được file scratch cuối cùng của Log Space.

#### Scenario: Thêm file scratch
- **WHEN** Scratch Log Space có `untitled.log` và người dùng chọn "New scratch file"
- **THEN** `untitled-2.log` rỗng được thêm vào

#### Scenario: Không xóa file cuối
- **WHEN** Scratch Log Space chỉ còn một file
- **THEN** mục Delete của file đó bị tắt

### Requirement: Xóa Log Space
Hệ thống SHALL xóa Log Space sau khi người dùng xác nhận. Xóa Scratch Log Space SHALL xóa các file scratch của nó. Xóa Imported Log Space SHALL NOT đụng tới file trên đĩa.

#### Scenario: Xóa Imported
- **WHEN** người dùng xác nhận xóa Imported Log Space "Payment"
- **THEN** "Payment" và các tab của nó biến mất, file log trên đĩa vẫn còn

### Requirement: Thứ tự file trong Log Space
Hệ thống SHALL sắp file trong mỗi Log Space theo thứ tự thêm vào (mặc định) hoặc theo tên, chọn qua "Sort files by" trong menu chuột phải. Thứ tự này SHALL được dùng cho sidebar, File Section, và khi hai dòng trùng timestamp.

#### Scenario: Đổi sang theo tên
- **WHEN** Log Space thêm `worker.log` trước rồi `api.log`, và người dùng chọn "Sort files by → Name"
- **THEN** sidebar và Raw View đều liệt kê `api.log` trước `worker.log`

### Requirement: Sidebar Log Space
Hệ thống SHALL hiện mỗi Log Space trên sidebar thành một dòng có chevron để mở danh sách file, cùng biểu tượng phân biệt Imported và Scratch. Bấm tên Log Space SHALL mở Log Space; bấm tên file SHALL mở File View của file đó.

#### Scenario: Mở danh sách file
- **WHEN** người dùng bấm chevron của "Payment"
- **THEN** các file của "Payment" hiện bên dưới theo thứ tự file của Log Space

### Requirement: Tab theo view
Hệ thống SHALL mở mỗi Log Space và mỗi File View trong một tab riêng. Mở một Log Space hoặc File View đã có tab SHALL kích hoạt tab đó thay vì mở tab mới.

#### Scenario: Mở lại
- **WHEN** tab "Payment" đang mở và người dùng bấm "Payment" trên sidebar
- **THEN** tab "Payment" được kích hoạt và số tab không đổi

#### Scenario: Log Space và File View là hai tab
- **WHEN** tab "Payment" đang mở và người dùng bấm `api.log` của "Payment"
- **THEN** một tab mới cho File View `api.log` được mở

### Requirement: Đổi tên Log Space
Hệ thống SHALL cho đổi tên Log Space qua menu chuột phải, theo cùng điều kiện tên của dialog.

#### Scenario: Đổi sang tên trùng
- **WHEN** người dùng đổi "Auth" thành "Payment" trong khi "Payment" đã có
- **THEN** tên không đổi và hiện thông báo tên đã tồn tại
