## ADDED Requirements

### Requirement: Value Color tô màu chữ theo giá trị
Trong Parsed View và Focus Pane, hệ thống SHALL tự tô màu chữ của mỗi ô thuộc cột được tô bằng một màu tính từ hash của giá trị ô đã đổi về chữ thường. Cùng giá trị SHALL có cùng màu ở mọi file, mọi Log Space, mọi Project và mọi lần mở ứng dụng. Value Color SHALL chỉ đổi màu chữ, SHALL NOT tô nền, và SHALL giữ chữ đọc rõ ở cả theme sáng lẫn tối.

#### Scenario: Cùng giá trị cùng màu
- **WHEN** cột `thread` có `T-12` ở dòng 3 và dòng 900, ở hai file khác nhau của Log Space
- **THEN** hai ô `T-12` có cùng màu chữ

#### Scenario: Giá trị khác hoa thường
- **WHEN** cột `host` có `API-1` và `api-1`
- **THEN** hai ô có cùng màu chữ

#### Scenario: Ổn định qua phiên
- **WHEN** người dùng thoát rồi mở lại ứng dụng
- **THEN** `T-12` vẫn có đúng màu chữ như trước

### Requirement: Cột nào được tô
Hệ thống SHALL tô Value Color cho mọi cột trừ: cột timestamp, cột message/payload, cột level, và cột có kiểu Number, Boolean hoặc Json. Hệ thống SHALL NOT giới hạn theo số giá trị khác nhau của cột. Ô không có giá trị SHALL NOT được tô. Người dùng SHALL NOT bật, tắt hay đổi màu Value Color theo cột hoặc theo giá trị.

#### Scenario: Cột định danh nhiều giá trị
- **WHEN** cột `request_id` có 20 000 giá trị khác nhau
- **THEN** các ô của `request_id` vẫn có Value Color

#### Scenario: Cột bị loại
- **WHEN** Log Format có cột `duration` kiểu Number và cột `level`
- **THEN** các ô của `duration`, `level`, timestamp và message giữ màu chữ thường

### Requirement: Value Color tránh màu Text Highlight
Hệ thống SHALL đẩy màu của một giá trị ra xa các màu Text Highlight của Project đang mở khi hai màu quá gần nhau, để chữ được app tô không bị nhầm với chỗ người dùng tô.

#### Scenario: Màu hash trùng màu đã dùng
- **WHEN** giá trị `T-12` có màu hash tím và Project có Text Highlight tím
- **THEN** `T-12` hiện bằng một màu chữ khác không gần tím

### Requirement: Công tắc Value Colors
Settings SHALL có một công tắc Value Colors áp cho toàn ứng dụng, bật mặc định và được nhớ qua lần khởi động lại. Khi tắt, hệ thống SHALL NOT vẽ Value Color ở đâu cả; Marker, Text Highlight, Highlight Rule và Search Highlight SHALL NOT bị ảnh hưởng.

#### Scenario: Tắt
- **WHEN** người dùng tắt Value Colors trong Settings
- **THEN** mọi ô hiện màu chữ thường còn Text Highlight vẫn được tô

#### Scenario: Nhớ qua lần mở
- **WHEN** người dùng tắt Value Colors, thoát rồi mở lại ứng dụng
- **THEN** Value Colors vẫn tắt

### Requirement: Value Color nhường nền do người dùng tô
Ở chỗ có nền của Marker hoặc Text Highlight, hệ thống SHALL dùng màu chữ chọn theo độ tương phản với nền đó thay cho Value Color.

#### Scenario: Marker trên ô có Value Color
- **WHEN** ô `T-12` có Value Color xanh và người dùng đặt Marker vàng lên nó
- **THEN** chữ `T-12` hiện màu tối hoặc sáng theo nền vàng, không phải xanh
