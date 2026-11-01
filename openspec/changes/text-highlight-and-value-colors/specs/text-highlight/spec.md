## ADDED Requirements

### Requirement: Text Highlight tô mọi chỗ có một đoạn text
Hệ thống SHALL cho người dùng tạo Text Highlight: một đoạn text và một màu. Mọi chỗ có đoạn text đó trong text đã lưu của dòng SHALL được tô nền bằng màu đó, ở mọi dòng của mọi Log Space thuộc Project. So khớp SHALL không phân biệt hoa thường, SHALL NOT dùng regex hay cú pháp Search Expression, và khoảng trắng trong đoạn text SHALL được so đúng như đã chọn.

#### Scenario: Tô mọi chỗ trùng
- **WHEN** người dùng tạo Text Highlight tím cho `conn-7f3a`
- **THEN** mọi chỗ có `conn-7f3a` ở mọi dòng đều có nền tím, các dòng không có nó không đổi

#### Scenario: Không phân biệt hoa thường
- **WHEN** có Text Highlight cho `Timeout`
- **THEN** `timeout`, `TIMEOUT` và `Timeout` đều được tô

#### Scenario: Ký tự đặc biệt là ký tự thường
- **WHEN** có Text Highlight cho `a.b`
- **THEN** `a.b` được tô còn `axb` không được tô

### Requirement: Tạo Text Highlight từ thanh màu nổi
Thanh màu nổi SHALL cho chọn phạm vi "Mọi chỗ “<text>”" khi vùng chọn nằm trong một dòng và có ít nhất một ký tự không phải khoảng trắng. Chọn một màu với phạm vi này, hoặc Alt+bấm một màu, SHALL tạo Text Highlight cho đúng đoạn text đã chọn. Khi đã đếm xong mà không làm chậm giao diện, nút phạm vi SHALL ghi thêm số dòng của view đang mở có chứa đoạn text đó ("· N"). Nếu Project đã có Text Highlight cho cùng đoạn text (không phân biệt hoa thường), hệ thống SHALL đổi màu của nó thay vì tạo cái thứ hai.

#### Scenario: Tạo từ vùng chọn
- **WHEN** người dùng chọn `req-91bc` trong một dòng, chọn phạm vi "Mọi chỗ có “req-91bc”" và bấm màu cam
- **THEN** Project có Text Highlight cam cho `req-91bc` và mọi chỗ có `req-91bc` được tô cam

#### Scenario: Số chỗ trùng
- **WHEN** người dùng chọn `conn-7f3a` và view đang mở có 4 dòng chứa nó
- **THEN** nút phạm vi ghi "Mọi chỗ “conn-7f3a” · 4"

#### Scenario: Vùng chọn nhiều dòng
- **WHEN** vùng chọn chạy qua hai dòng
- **THEN** thanh màu nổi không cho chọn phạm vi "Mọi chỗ …", và Alt+bấm một màu tạo Marker như bấm thường

#### Scenario: Tạo lại cùng đoạn text
- **WHEN** đã có Text Highlight cam cho `req-91bc` và người dùng chọn `REQ-91BC` rồi tạo Text Highlight xanh
- **THEN** Project chỉ có một Text Highlight cho đoạn text đó và nó màu xanh

### Requirement: Đổi màu và gỡ Text Highlight bằng chuột phải
Trong Parsed View, menu chuột phải trên một chỗ đang có Text Highlight SHALL có "Change highlight color" và "Remove highlight". Trong Raw View, nơi Marker được đổi màu và gỡ bằng cách bấm lên nó, bấm lên một chỗ đang có Text Highlight SHALL mở thanh màu của nó với nút "Remove highlight". Gỡ SHALL gỡ Text Highlight đó khỏi Project, nên mọi chỗ của nó hết được tô. Hệ thống SHALL NOT có danh sách hay panel Text Highlight ở sidebar.

#### Scenario: Gỡ
- **WHEN** người dùng chuột phải lên chữ `conn-7f3a` đang được tô và chọn "Remove highlight"
- **THEN** không còn chỗ nào có `conn-7f3a` được tô bởi Text Highlight đó

#### Scenario: Bấm trong Raw View
- **WHEN** người dùng bấm lên chữ `conn-7f3a` đang được tô trong Raw View
- **THEN** thanh màu mở để đổi màu Text Highlight đó, có nút "Remove highlight"

#### Scenario: Đổi màu
- **WHEN** người dùng chọn "Change highlight color" trên một chỗ của Text Highlight tím rồi bấm màu đỏ
- **THEN** mọi chỗ của Text Highlight đó chuyển sang đỏ

#### Scenario: Sidebar
- **WHEN** người dùng mở context sidebar
- **THEN** không có mục Text Highlight

### Requirement: Text Highlight chồng nhau
Khi hai Text Highlight phủ cùng ký tự, hệ thống SHALL vẽ màu của Text Highlight tạo hoặc đổi màu sau cùng.

#### Scenario: Đoạn ngắn nằm trong đoạn dài
- **WHEN** có Text Highlight xanh cho `conn` rồi người dùng tạo Text Highlight đỏ cho `conn-7f3a`
- **THEN** trong `conn-7f3a` cả đoạn có màu đỏ, còn `conn` ở chỗ khác vẫn xanh

### Requirement: Text Highlight hiện ở mọi view
Hệ thống SHALL vẽ Text Highlight trong Raw View, Parsed View và Focus Pane. Trong Parsed View, phần nằm ngoài mọi ô SHALL chỉ thấy trong Raw View, giống Marker. Text Highlight SHALL NOT có vạch trên overview ruler và SHALL NOT thay đổi text được copy.

#### Scenario: Tạo trong Raw, xem trong Parsed
- **WHEN** người dùng tạo Text Highlight cho `retry` trong Raw View rồi chuyển sang Parsed View
- **THEN** chữ `retry` trong ô MESSAGE có cùng màu nền

#### Scenario: Overview ruler
- **WHEN** có Text Highlight khớp 300 dòng và không có Marker nào
- **THEN** overview ruler không có vạch nào cho 300 dòng đó

### Requirement: Text Highlight thuộc Project và áp cho dòng mới
Hệ thống SHALL lưu Text Highlight trong Project, khôi phục khi khởi động lại, và áp cho mọi dòng được hiện, kể cả dòng mới tới trong Live Log Space và dòng của file thêm vào sau. Dòng rời Live Window hay file bị gỡ khỏi Log Space SHALL NOT làm mất Text Highlight.

#### Scenario: Dòng live mới
- **WHEN** có Text Highlight cho `conn-7f3a` và một dòng live mới có `conn-7f3a` tới
- **THEN** dòng mới được tô ngay khi hiện

#### Scenario: Khởi động lại
- **WHEN** người dùng tạo hai Text Highlight, thoát rồi mở lại ứng dụng
- **THEN** hai Text Highlight vẫn còn với đúng màu

#### Scenario: Project khác
- **WHEN** người dùng chuyển sang một Project khác
- **THEN** Text Highlight của Project trước không được vẽ
