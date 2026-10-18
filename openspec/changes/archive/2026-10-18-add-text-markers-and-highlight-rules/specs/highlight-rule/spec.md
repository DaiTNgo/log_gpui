## ADDED Requirements

### Requirement: Highlight Rule trên một cột
Hệ thống SHALL cho tạo Highlight Rule gồm một cột, một tập điều kiện giá trị và một màu. Điều kiện SHALL viết giống Filter: Filter Value được tick (so bằng nhau, không phân biệt hoa thường) hoặc Custom Value (chứa chuỗi). Các điều kiện trong một rule SHALL nối bằng OR.

#### Scenario: Nhiều giá trị
- **WHEN** rule màu cam có cột `module` với giá trị `DB` và `CACHE`
- **THEN** mọi hàng có `module` là `DB` hoặc `CACHE` được tô cam

#### Scenario: Chứa chuỗi trên MESSAGE
- **WHEN** rule màu đỏ có cột MESSAGE với Custom Value `timeout`
- **THEN** mọi hàng có MESSAGE chứa `timeout` được tô đỏ

### Requirement: Cột dùng được cho Highlight Rule
Hệ thống SHALL cho tạo Highlight Rule trên mọi Filterable Column, kể cả Source Column. Trên cột MESSAGE, hệ thống SHALL chỉ cho dùng Custom Value. Hệ thống SHALL NOT cho tạo Highlight Rule trên TIMESTAMP. Hàng không có giá trị ở cột của rule SHALL NOT khớp rule đó.

#### Scenario: Tô theo file
- **WHEN** rule màu tím có Source Column với giá trị `worker.log`
- **THEN** mọi hàng của `worker.log` trong Log Space được tô tím

#### Scenario: Không có TIMESTAMP
- **WHEN** người dùng mở form "+ Add rule"
- **THEN** danh sách cột không có TIMESTAMP

### Requirement: Cách tô của Highlight Rule
Hệ thống SHALL tô nền cả hàng khớp bằng sắc độ nhạt của màu rule, và tô ô thỏa điều kiện bằng sắc độ đậm hơn.

#### Scenario: Hàng khớp
- **WHEN** hàng có `module` = `DB` khớp rule màu cam
- **THEN** nền cả hàng là cam nhạt và ô `module` là cam đậm hơn

### Requirement: Highlight Rule chỉ trong Parsed View
Hệ thống SHALL áp dụng Highlight Rule trong Parsed View của Log Space và File View, và SHALL NOT áp dụng trong Raw View. Highlight Rule SHALL NOT ẩn hay hiện hàng nào.

#### Scenario: Raw View
- **WHEN** rule `module` = `DB` đang bật và người dùng chuyển sang Raw View
- **THEN** các dòng `DB` không có màu của rule

### Requirement: Thứ tự ưu tiên
Khi một hàng khớp nhiều rule, hệ thống SHALL tô theo rule đứng trên cùng trong danh sách. Người dùng SHALL kéo thả để đổi thứ tự.

#### Scenario: Hai rule cùng khớp
- **WHEN** rule cam `module` = `DB` đứng trên rule xanh MESSAGE chứa `timeout`, và một hàng khớp cả hai
- **THEN** hàng đó được tô cam

#### Scenario: Đổi thứ tự
- **WHEN** người dùng kéo rule xanh lên trên rule cam
- **THEN** hàng đó được tô xanh

### Requirement: Highlight Rule thuộc Project
Hệ thống SHALL dùng chung Highlight Rule cho mọi Log Space trong một Project, tự lưu và khôi phục khi khởi động lại. Mỗi rule SHALL có công tắc bật/tắt; tắt rule SHALL giữ rule trong danh sách nhưng không tô.

#### Scenario: Log Space khác
- **WHEN** rule `module` = `DB` được tạo khi đang mở Log Space "Payment", rồi người dùng mở Log Space "Auth" cùng Project
- **THEN** các hàng `DB` trong "Auth" cũng được tô

#### Scenario: Tắt rule
- **WHEN** người dùng tắt rule `module` = `DB`
- **THEN** các hàng `DB` không còn được tô và rule vẫn nằm trong danh sách

### Requirement: Mục Highlights
Context sidebar SHALL có mục Highlights liệt kê các rule theo thứ tự ưu tiên. Mỗi rule SHALL hiện ô màu, tên cột, các giá trị, số hàng khớp trong view đang mở, công tắc bật/tắt, nút xóa và tay kéo. Mục SHALL có nút "+ Add rule" mở form chọn cột, giá trị và màu.

#### Scenario: Thêm rule
- **WHEN** người dùng bấm "+ Add rule", chọn cột `module`, tick `DB`, chọn màu cam rồi xác nhận
- **THEN** rule mới nằm cuối danh sách và các hàng `DB` được tô cam

#### Scenario: Đổi màu
- **WHEN** người dùng bấm ô màu của một rule và chọn màu khác
- **THEN** các hàng khớp đổi sang màu mới

### Requirement: Tạo nhanh từ ô
Menu chuột phải trên một ô SHALL có "Highlight rows where <cột> = <giá trị>", tạo một Highlight Rule bình thường với màu kế tiếp trong palette. Trên MESSAGE, mục này SHALL chỉ có khi đang có vùng chọn trong ô, và dùng text đã chọn làm Custom Value. Mục này SHALL NOT có trên TIMESTAMP. Menu chuột phải SHALL NOT có lệnh lọc.

#### Scenario: Tạo nhanh
- **WHEN** người dùng chuột phải lên ô `module` có giá trị `DB` và chọn "Highlight rows where module = DB"
- **THEN** rule `module` = `DB` xuất hiện trong mục Highlights và các hàng `DB` được tô

#### Scenario: Không có lệnh lọc
- **WHEN** người dùng chuột phải lên một ô
- **THEN** menu không có lệnh "Filter" hay "Show only"
