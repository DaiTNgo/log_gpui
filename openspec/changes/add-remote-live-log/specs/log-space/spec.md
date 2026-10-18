## MODIFIED Requirements

### Requirement: Mở dialog tạo Log Space
Hệ thống SHALL mở dialog New Log Space khi người dùng bấm nút **+** ở đầu danh sách Log Space trên sidebar. Dialog SHALL gồm ô tên, vùng kéo thả file kèm nút Browse, danh sách file đã chọn, nút "Add remote files…", nút Create và nút Cancel. Khi danh sách có Remote File, vùng kéo thả và nút Browse SHALL bị tắt; khi danh sách có file local, nút "Add remote files…" SHALL bị tắt. Có Remote File thì nút Create SHALL tạo Remote Log Space với nhãn ví dụ "Create remote with 3 files".

#### Scenario: Mở dialog
- **WHEN** người dùng bấm **+** trên sidebar
- **THEN** dialog New Log Space hiện ra với ô tên trống, danh sách file trống, và nút Create bị tắt

#### Scenario: Hủy
- **WHEN** người dùng đã nhập tên và thả file rồi bấm Cancel
- **THEN** dialog đóng và không có Log Space nào được tạo

#### Scenario: Tạo Remote Log Space
- **WHEN** người dùng nhập tên "Checkout", bấm "Add remote files…", chọn `web1:/app/app.log` và `web2:/app/app.log`, rồi bấm "Create remote with 2 files"
- **THEN** Remote Log Space "Checkout" được tạo với hai Remote File đó, và vùng kéo thả bị tắt trong lúc danh sách có Remote File

### Requirement: Imported và Scratch không trộn lẫn
Hệ thống SHALL chỉ thêm file có sẵn trên đĩa vào Imported Log Space, chỉ thêm file scratch rỗng vào Scratch Log Space, và chỉ thêm Remote File vào Remote Log Space.

#### Scenario: Menu của Imported
- **WHEN** người dùng chuột phải lên một Imported Log Space
- **THEN** menu có "Add files…" và không có "New scratch file" hay "Add remote files…"

#### Scenario: Menu của Scratch
- **WHEN** người dùng chuột phải lên một Scratch Log Space
- **THEN** menu có "New scratch file" và không có "Add files…" hay "Add remote files…"

#### Scenario: Menu của Remote
- **WHEN** người dùng chuột phải lên một Remote Log Space
- **THEN** menu có "Add remote files…" và "Download all", và không có "Add files…" hay "New scratch file"

### Requirement: File Imported chỉ đọc
Hệ thống SHALL NOT sửa, đổi tên hay xóa file Imported trên đĩa hay Remote File trên Server. Gỡ file khỏi Log Space SHALL chỉ bỏ nó khỏi danh sách.

#### Scenario: Gỡ file
- **WHEN** người dùng chọn "Remove from Log Space" trên `api.log`
- **THEN** `api.log` biến mất khỏi Log Space, file trên đĩa vẫn còn nguyên, và các Marker của nó bị bỏ

#### Scenario: Gỡ file cuối cùng
- **WHEN** người dùng gỡ file duy nhất của một Imported Log Space
- **THEN** Log Space vẫn còn, và canvas hiện trạng thái rỗng kèm nút "Add files"

#### Scenario: Remote File chỉ đọc
- **WHEN** người dùng mở Raw View của `web1:/app/app.log`
- **THEN** text không sửa được, và hệ thống không bao giờ ghi vào file đó trên Server
