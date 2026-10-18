## ADDED Requirements

### Requirement: Định dạng Archive được nhận
Hệ thống SHALL nhận file `.zip`, `.tar`, `.tar.gz`, `.tgz` và `.gz` làm Archive. Hệ thống SHALL nhận dạng Archive theo nội dung đầu file (magic bytes), không chỉ theo đuôi tên. Một file `.gz` mà nội dung giải nén là tar SHALL được coi là `.tar.gz`. File định dạng khác SHALL đi theo quy tắc text/binary như file thường.

#### Scenario: Zip đổi đuôi
- **WHEN** người dùng thả `bundle.dat` có nội dung là zip
- **THEN** `bundle.dat` được mở như Archive và các entry bên trong hiện trong danh sách

#### Scenario: Định dạng chưa hỗ trợ
- **WHEN** người dùng thả `logs.7z`
- **THEN** `logs.7z` hiện trong danh sách với cảnh báo "Not a text file" và không được thêm

### Requirement: Liệt kê Archive Entry
Khi người dùng thả hoặc chọn một Archive, hệ thống SHALL thay Archive trong danh sách chờ bằng các Archive Entry là file bên trong nó, ở mọi cấp thư mục, theo thứ tự trong Archive. Hệ thống SHALL bỏ qua thư mục, symlink, mọi entry dưới `__MACOSX/` và entry tên `.DS_Store`. Entry text SHALL ở trạng thái Ok; entry binary SHALL hiện kèm cảnh báo "Not a text file" và SHALL NOT được thêm. Một file `.gz` không chứa tar SHALL cho đúng một Archive Entry.

#### Scenario: Zip nhiều cấp
- **WHEN** người dùng thả `bundle.zip` chứa `app.log`, `nginx/access.log`, `nginx/` và `__MACOSX/._app.log`
- **THEN** danh sách có `bundle.zip › app.log` và `bundle.zip › nginx/access.log`, không có mục nào khác từ `bundle.zip`

#### Scenario: Entry binary
- **WHEN** `bundle.zip` chứa `heap.hprof` có byte NUL
- **THEN** `bundle.zip › heap.hprof` hiện với cảnh báo "Not a text file" và không được đếm vào nhãn Create

#### Scenario: Gz đơn lẻ
- **WHEN** người dùng thả `app.log.1.gz` chứa một file text
- **THEN** danh sách có đúng một mục `app.log.1`

#### Scenario: Archive lồng nhau
- **WHEN** `bundle.zip` chứa `old.zip`
- **THEN** `bundle.zip › old.zip` hiện với cảnh báo "Not a text file" và các file trong `old.zip` không được liệt kê

### Requirement: Tên hiển thị của Archive Entry
Hệ thống SHALL đặt tên hiển thị của Archive Entry là `<tên file Archive> › <đường dẫn trong Archive>`. Với `.gz` đơn lẻ, tên hiển thị SHALL là tên file Archive bỏ đuôi `.gz`. Tên này SHALL dùng ở danh sách chờ, sidebar, tab và Source Column.

#### Scenario: Tên trong sidebar
- **WHEN** người dùng tạo Log Space từ `bundle.zip › nginx/access.log`
- **THEN** sidebar và Source Column hiện `bundle.zip › nginx/access.log`

### Requirement: Đọc Archive Entry không giải nén ra đĩa
Hệ thống SHALL đọc nội dung Archive Entry thẳng từ Archive vào bộ nhớ khi mở Log Space. Hệ thống SHALL NOT ghi entry ra đĩa và SHALL NOT sửa Archive. Archive Entry SHALL được parse, lọc, tìm kiếm, gắn Marker và gộp timeline như file Imported thường.

#### Scenario: Không để lại file tạm
- **WHEN** người dùng mở Log Space có `bundle.zip › app.log`
- **THEN** các dòng của `app.log` hiện trên canvas và không có file nào được tạo trên đĩa

#### Scenario: Gộp với file thường
- **WHEN** một Log Space có `bundle.zip › app.log` và `/var/log/worker.log`
- **THEN** dòng của cả hai file được xen kẽ theo timestamp

### Requirement: Archive Entry là tĩnh
Hệ thống SHALL NOT đọc live Archive Entry. Archive Entry SHALL NOT có Generation mới, Break hay dòng mới khi Archive trên đĩa thay đổi trong lúc Log Space đang mở. Một Log Space chỉ có Archive Entry SHALL NOT hiện Follow và Pause.

#### Scenario: Archive bị ghi đè lúc đang mở
- **WHEN** Log Space đang mở `bundle.zip › app.log` và `bundle.zip` bị thay bằng bản khác trên đĩa
- **THEN** canvas giữ nguyên các dòng đã đọc và không có Break

#### Scenario: Log Space chỉ có Archive Entry
- **WHEN** người dùng mở Log Space chỉ gồm các entry của `bundle.zip`
- **THEN** status bar không có công tắc Follow và Pause

### Requirement: Mở lại Archive Entry
Hệ thống SHALL lưu Archive Entry bằng đường dẫn Archive và đường dẫn trong Archive, và SHALL đọc lại entry từ Archive khi mở lại Project. Marker của Archive Entry SHALL được nhớ như file Imported. Khi Archive không còn trên đĩa, không đọc được, hoặc không còn entry đó, hệ thống SHALL hiện thông báo lỗi nêu tên entry và giữ file trong Log Space với nội dung rỗng.

#### Scenario: Marker còn sau khi mở lại
- **WHEN** người dùng tô màu một đoạn trên dòng 12 của `bundle.zip › app.log`, đóng rồi mở lại Project
- **THEN** đoạn đó trên dòng 12 vẫn được tô màu

#### Scenario: Archive bị xoá
- **WHEN** `bundle.zip` bị xoá khỏi đĩa và người dùng mở lại Log Space
- **THEN** hiện thông báo không tìm thấy `bundle.zip › app.log` và file vẫn nằm trong Log Space

### Requirement: Giới hạn dung lượng giải nén
Hệ thống SHALL cảnh báo "Large file" trên Archive Entry có dung lượng giải nén lớn hơn 200 MB nhưng vẫn cho chọn. Hệ thống SHALL tắt nút Create (hoặc nút thêm) và hiện lý do khi tổng dung lượng giải nén của các Archive Entry đang được thêm vượt 2 GB. Khi đọc, hệ thống SHALL dừng giải nén một entry ngay khi số byte vượt dung lượng mà Archive khai báo cho entry đó hoặc vượt 2 GB, và SHALL coi entry đó là lỗi.

#### Scenario: Entry lớn
- **WHEN** `bundle.zip` chứa `big.log` 500 MB sau giải nén
- **THEN** `bundle.zip › big.log` hiện cảnh báo "Large file" và vẫn được đếm vào nhãn Create

#### Scenario: Vượt tổng
- **WHEN** các entry đang chọn có tổng 3 GB sau giải nén
- **THEN** nút Create bị tắt và dialog hiện lý do vượt giới hạn 2 GB

#### Scenario: Khai báo sai dung lượng
- **WHEN** một entry khai báo 1 KB nhưng giải nén ra nhiều hơn 1 KB
- **THEN** việc đọc dừng lại và entry đó hiện lỗi thay vì nội dung

### Requirement: Archive hỏng hoặc có mật khẩu
Hệ thống SHALL hiện Archive hỏng, không đọc được, hoặc có mật khẩu thành một mục trong danh sách chờ kèm cảnh báo nêu lý do ("Damaged archive" hoặc "Password-protected archive"). Mục đó SHALL NOT được thêm và SHALL NOT được đếm vào nhãn Create.

#### Scenario: Zip có mật khẩu
- **WHEN** người dùng thả `secret.zip` có mật khẩu
- **THEN** `secret.zip` hiện với cảnh báo "Password-protected archive" và không entry nào được thêm

#### Scenario: Tar.gz bị cắt
- **WHEN** người dùng thả `logs.tar.gz` bị cắt giữa chừng
- **THEN** `logs.tar.gz` hiện với cảnh báo "Damaged archive"
