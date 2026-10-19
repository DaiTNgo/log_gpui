## MODIFIED Requirements

### Requirement: Download Remote File
Hệ thống SHALL cho tải một Remote File, hoặc mọi Remote File của một Remote Log Space bằng "Download all", về thư mục người dùng chọn, có hiện tiến độ và cho huỷ. Mặc định chỉ file hiện tại SHALL được tải; dialog SHALL liệt kê Rotated File của mỗi Remote File để người dùng tích tải kèm, và Rotated File SHALL được tải nguyên dạng, kể cả khi nén. Tải xong, hệ thống SHALL đề nghị mở các file đã tải thành một Imported Log Space mới.

#### Scenario: Download all
- **WHEN** người dùng chọn "Download all" trên "Checkout" và chọn thư mục `~/incident/`
- **THEN** ba file được tải vào `~/incident/`, rồi hệ thống hỏi "Open in a new Imported Log Space?" và nếu đồng ý thì tạo Imported Log Space chứa ba file đó

#### Scenario: Trùng tên khi tải
- **WHEN** `web1:/app/app.log` và `web2:/app/app.log` cùng được tải vào một thư mục
- **THEN** hai file được lưu với tên phân biệt theo Server, ví dụ `web1-app.log` và `web2-app.log`

#### Scenario: Tải kèm Rotated File
- **WHEN** người dùng tích `app.log.2.gz` trong dialog Download của `web1:/app/app.log`
- **THEN** `app.log.2.gz` được tải nguyên dạng nén bên cạnh `app.log`, và khi mở thành Imported Log Space thì nó được đọc như một Archive
