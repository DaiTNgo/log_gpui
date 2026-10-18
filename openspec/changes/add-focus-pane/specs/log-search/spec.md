## MODIFIED Requirements

### Requirement: Search khớp trên toàn bộ dòng log
Hệ thống SHALL khớp Search Expression trên toàn bộ `raw_line` của từng dòng log, không theo trường hay theo cột nào.

#### Scenario: Từ khóa nằm ở bất kỳ vị trí nào trong dòng
- **WHEN** người dùng gõ `error` và một dòng có `ERROR` ở cột level, một dòng khác có `error` trong message
- **THEN** cả hai dòng đều là Match

#### Scenario: Search không đổi tập dòng của view
- **WHEN** người dùng gõ một Search Expression
- **THEN** danh sách dòng của view không đổi, Match được đánh dấu và điều hướng được, và Focus Tab đang mở chỉ liệt kê các dòng là Match

### Requirement: Search chỉ tìm trên dòng Filter đang hiển thị
Trong Parsed View, hệ thống SHALL chỉ coi là Match các dòng qua Filter, và SHALL NOT đếm dòng khớp nhưng không qua Filter. Bộ đếm Match, next/prev, overview ruler, phần tô ở view và Focus Tab SHALL dùng cùng một tập Match này. View vẫn hiện mọi dòng, kể cả dòng không qua Filter. Trong Raw View, Filter không áp dụng, nên hệ thống SHALL tìm và đếm Match trên mọi dòng của mọi File Section.

#### Scenario: Dòng không qua Filter
- **WHEN** đang ở Parsed View, Filter ẩn mọi dòng `DEBUG` và người dùng gõ `timeout`, trong đó 5 dòng `DEBUG` có `timeout` và 12 dòng khác có `timeout`
- **THEN** tổng số Match là 12, 5 dòng `DEBUG` vẫn hiện ở view nhưng không được tô, và Focus Tab liệt kê 12 dòng

#### Scenario: Raw View đếm mọi dòng
- **WHEN** cùng Filter và Search Expression như trên nhưng người dùng chuyển sang Raw View
- **THEN** tổng số Match là 17

### Requirement: Biểu thức không hợp lệ giữ Match gần nhất
Khi Search Expression đang gõ dở hoặc sai cú pháp, hệ thống SHALL giữ danh sách Match của biểu thức hợp lệ gần nhất, và Focus Tab SHALL giữ các dòng của biểu thức đó. Hệ thống SHALL hiển thị chỉ báo lỗi bên cạnh ô Search.

#### Scenario: Gõ dở không làm danh sách về 0
- **WHEN** biểu thức hợp lệ `timeout` đang có 12 Match và người dùng gõ tiếp thành `timeout ||`
- **THEN** danh sách vẫn là 12 Match, Focus Tab vẫn liệt kê 12 dòng, và chỉ báo lỗi được hiển thị

#### Scenario: Biểu thức trở lại hợp lệ
- **WHEN** người dùng gõ tiếp thành `timeout || refused`
- **THEN** danh sách Match và Focus Tab được tính lại theo biểu thức mới và chỉ báo lỗi biến mất

#### Scenario: Biểu thức rỗng
- **WHEN** người dùng xóa hết nội dung ô Search
- **THEN** không có Match, không có chỉ báo lỗi, và Focus Tab chỉ còn giới hạn theo Filter

## ADDED Requirements

### Requirement: Search Expression thuộc Focus Tab
Mỗi Search Expression SHALL thuộc một Focus Tab. Ô Search SHALL hiện và sửa Search Expression của Focus Tab đang mở. Search Expression SHALL được nhớ cùng Focus Tab theo view.

#### Scenario: Mở lại view
- **WHEN** người dùng gõ `timeout` ở tab 1 của "Payment", đóng tab "Payment" rồi mở lại
- **THEN** ô Search hiện `timeout` và Match của `timeout` được tô
