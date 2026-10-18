## ADDED Requirements

### Requirement: Chuyển giữa Raw View và Parsed View
Hệ thống SHALL có nút chuyển Raw/Parsed trên toolbar cho mọi Log Space và File View. Imported Log Space SHALL mở ở Parsed View. Scratch Log Space SHALL mở ở Raw View khi mọi file đều rỗng; nếu không, SHALL mở ở chế độ dùng lần cuối.

#### Scenario: Mở Imported
- **WHEN** người dùng mở Imported Log Space "Payment" lần đầu
- **THEN** Parsed View được hiện

#### Scenario: Mở Scratch rỗng
- **WHEN** người dùng mở Scratch Log Space vừa tạo
- **THEN** Raw View được hiện và con trỏ nằm trong `untitled.log`

#### Scenario: Nhớ chế độ
- **WHEN** người dùng chuyển một Scratch Log Space có nội dung sang Parsed, đóng tab rồi mở lại
- **THEN** Parsed View được hiện

### Requirement: File Section
Hệ thống SHALL hiện Raw View thành các File Section xếp chồng, mỗi file của view một section, theo thứ tự file của Log Space. Mỗi section SHALL có header ghi tên file và thư mục, kèm chevron để thu gọn hoặc mở rộng, và SHALL hiện toàn bộ nội dung file với số dòng. Trạng thái thu gọn SHALL được nhớ theo từng file.

#### Scenario: Ba file
- **WHEN** Log Space có `api.log`, `worker.log`, `db.log` và người dùng chuyển sang Raw View
- **THEN** Raw View có ba File Section theo đúng thứ tự đó, đều đang mở rộng

#### Scenario: Thu gọn
- **WHEN** người dùng bấm chevron của `worker.log`
- **THEN** chỉ còn header của `worker.log`, các section khác giữ nguyên

#### Scenario: File View
- **WHEN** người dùng chuyển File View `api.log` sang Raw View
- **THEN** Raw View chỉ có một File Section `api.log`

### Requirement: Raw View hiện đúng nội dung file
Hệ thống SHALL hiện trong Raw View mọi dòng của file đúng như lưu trữ, và SHALL NOT áp dụng Filter. Chỉ Search và Marker SHALL áp dụng trong Raw View.

#### Scenario: Filter không ẩn dòng
- **WHEN** Filter của Log Space đang ẩn dòng `DEBUG` và người dùng chuyển sang Raw View
- **THEN** các dòng `DEBUG` vẫn hiện trong File Section của chúng

### Requirement: Search trong Raw View chỉ tô sáng
Hệ thống SHALL tô sáng Match trong mọi File Section và SHALL cho nút trước/sau đi qua Match xuyên suốt các section. Hệ thống SHALL NOT ẩn dòng hay File Section nào vì Search. Ô Search SHALL NOT có nút match case, whole word hay regex.

#### Scenario: Section không có Match
- **WHEN** người dùng tìm `timeout`, chỉ `api.log` có dòng khớp
- **THEN** `worker.log` và `db.log` vẫn hiện đầy đủ, chỉ dòng khớp trong `api.log` được tô sáng

#### Scenario: Nhảy sang section khác
- **WHEN** đang ở Match cuối của `api.log` và người dùng bấm "next"
- **THEN** selection nhảy tới Match đầu tiên ở section kế tiếp có Match, và section đó tự mở rộng nếu đang thu gọn

### Requirement: Imported chỉ đọc trong Raw View
Hệ thống SHALL hiện file Imported trong Raw View ở chế độ chỉ đọc. Người dùng SHALL chọn và sao chép được text nhưng SHALL NOT sửa được.

#### Scenario: Gõ phím
- **WHEN** người dùng gõ chữ trong File Section của một file Imported
- **THEN** nội dung không đổi

### Requirement: Sửa file scratch trong Raw View
Hệ thống SHALL cho sửa file scratch trong Raw View. Parsed View của file scratch SHALL chỉ đọc, và SHALL được parse lại mỗi lần chuyển từ Raw View sang.

#### Scenario: Dán log rồi parse
- **WHEN** người dùng dán 100 dòng log vào `untitled.log` rồi chuyển sang Parsed View
- **THEN** Parsed View hiện 100 dòng đã tách cột

#### Scenario: Parsed không sửa được
- **WHEN** người dùng đang ở Parsed View của một Scratch Log Space
- **THEN** không có cách nào sửa nội dung từ Parsed View

### Requirement: Đồng bộ giữa các tab
Khi cùng một file scratch đang mở ở tab Log Space và tab File View, hệ thống SHALL hiện nội dung mới ở tab kia sau khi file được lưu.

#### Scenario: Sửa ở File View
- **WHEN** người dùng sửa `untitled.log` trong File View và tab Log Space chứa nó cũng đang mở
- **THEN** sau khi lưu, Raw View của tab Log Space hiện nội dung mới

### Requirement: Tự lưu file scratch
Hệ thống SHALL tự lưu file scratch sau khoảng 500 ms người dùng ngừng gõ, và SHALL lưu ngay khi người dùng bấm ⌘S. Hệ thống SHALL NOT có nút Save riêng.

#### Scenario: Ngừng gõ
- **WHEN** người dùng gõ vào `untitled.log` rồi dừng 1 giây và thoát ứng dụng
- **THEN** mở lại ứng dụng thấy đúng nội dung vừa gõ

### Requirement: Save As tạo bản sao
Hệ thống SHALL cho Save As một file scratch ra vị trí người dùng chọn. Save As SHALL ghi một bản sao, và file scratch SHALL vẫn nằm trong thư mục dữ liệu của ứng dụng.

#### Scenario: Sửa sau Save As
- **WHEN** người dùng Save As `untitled.log` ra `~/Desktop/incident.log` rồi gõ thêm
- **THEN** nội dung mới vào `untitled.log` trong ứng dụng, còn `~/Desktop/incident.log` giữ nguyên nội dung lúc Save As

### Requirement: Marker trong Raw View
Hệ thống SHALL cho đặt và gỡ Marker trên dòng trong Raw View. Marker SHALL dùng chung giữa Raw View, Parsed View, Log Space và File View. Sửa nội dung một dòng scratch SHALL làm mất Marker của dòng đó.

#### Scenario: Marker dùng chung
- **WHEN** người dùng đặt Marker trên dòng 42 của `api.log` trong Raw View rồi chuyển sang Parsed View
- **THEN** dòng 42 của `api.log` có Marker trong Parsed View

#### Scenario: Sửa dòng có Marker
- **WHEN** dòng 10 của `untitled.log` có Marker và người dùng sửa nội dung dòng đó
- **THEN** sau khi lưu, dòng 10 không còn Marker
