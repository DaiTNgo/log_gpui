## ADDED Requirements

### Requirement: Dòng log được tô màu chữ theo loại token
Hệ thống SHALL tô màu chữ (foreground) cho từng token trong một dòng log theo loại của nó. Syntax Color SHALL NOT đổi nền, nội dung hay vị trí của chữ.

#### Scenario: Dòng log có timestamp và level
- **WHEN** Raw View hiện dòng `2024-05-01 10:22:03.512 ERROR payment failed`
- **THEN** timestamp có màu timestamp (mờ), `ERROR` có màu của level Error, phần `payment failed` giữ màu chữ mặc định

#### Scenario: Dòng không có token nào nhận ra
- **WHEN** Raw View hiện dòng `hello world`
- **THEN** cả dòng giữ màu chữ mặc định

### Requirement: Mức 1 gồm timestamp, level và Continuation Line
Hệ thống SHALL tô timestamp ở đầu dòng bằng màu timestamp, từ khóa level (FATAL, ERROR, WARN, INFO, DEBUG, TRACE, đúng chữ hoa) bằng màu của level đó như ở Level Marker, và một Continuation Line bằng màu mờ. Dòng bắt đầu bằng `Caused by:` SHALL được tô bằng màu đỏ nhạt thay cho màu mờ.

#### Scenario: Stack trace
- **WHEN** Raw View hiện `java.lang.IllegalStateException: x` rồi các dòng `\tat com.foo.Bar.run(Bar.java:42)` và `Caused by: java.io.IOException`
- **THEN** các dòng `\tat …` có màu mờ và dòng `Caused by:` có màu đỏ nhạt

#### Scenario: Level viết thường
- **WHEN** một dòng chứa `error` viết thường trong câu
- **THEN** từ đó không được tô như level

### Requirement: Mức 2 gồm module, số, chuỗi, định danh và key=value
Hệ thống SHALL tô thêm: tên module/logger, số và duration đứng riêng (`42`, `12ms`, `1.5s`), chuỗi trong dấu nháy kép, định danh (UUID, địa chỉ IPv4, URL `http(s)://`), và key của `key=value` (value giữ màu mặc định). Một số nằm giữa chữ (`abc123`, `v2`) SHALL NOT bị tô như số.

#### Scenario: key=value
- **WHEN** dòng chứa `user=alice took=12ms`
- **THEN** `user` và `took` có màu key, `alice` giữ màu mặc định, `12ms` có màu số

#### Scenario: Số nằm giữa chữ
- **WHEN** dòng chứa `abc123 v2`
- **THEN** cả hai giữ màu mặc định

#### Scenario: UUID và IP
- **WHEN** dòng chứa `req 3f2b8c1e-6a4d-4e0b-9a52-1c7d0e5f8a90 from 10.0.0.12`
- **THEN** UUID và địa chỉ IP cùng có màu định danh

### Requirement: Bảng màu cố định theo theme
Hệ thống SHALL dùng một bảng màu cố định, tối đa 8 màu cho các loại token ngoài level, với giá trị riêng cho Dark, Dark Bright và Light. Mọi màu SHALL đủ tương phản (tối thiểu 4.5:1) với nền của theme đang dùng. Người dùng SHALL NOT chọn được màu của Syntax Color.

#### Scenario: Đổi theme
- **WHEN** người dùng đổi từ Dark sang Light
- **THEN** các token đổi sang bảng màu của Light ngay, không cần mở lại tab

### Requirement: Công tắc Syntax Color
Hệ thống SHALL có một công tắc Syntax Color trong Settings và một lệnh Keymap, là một cài đặt cho cả app, mặc định bật và nhớ qua các lần chạy. Khi tắt, mọi dòng SHALL được vẽ như khi chưa có Syntax Color. Một workspace state cũ không có cài đặt này SHALL được đọc như đang bật.

#### Scenario: Tắt công tắc
- **WHEN** người dùng tắt Syntax Color trong Settings
- **THEN** Raw View, Focus Pane và cell Message mọi tab vẽ lại chỉ với màu mặc định và màu level hiện có

#### Scenario: Mở bản lưu cũ
- **WHEN** app mở một workspace state được lưu trước khi có tính năng này
- **THEN** Syntax Color đang bật

### Requirement: Chỉ tô các dòng đang hiển thị
Hệ thống SHALL chỉ tô các dòng đang nằm trong viewport và SHALL NOT đọc hay xử lý dòng ngoài viewport cho mục đích này. Một dòng dài hơn 4096 byte SHALL chỉ được tô trong 4096 byte đầu.

#### Scenario: Cuộn file lớn
- **WHEN** người dùng cuộn nhanh một file 2 GB trong Raw View với Syntax Color bật
- **THEN** cuộn vẫn mượt và memory không tăng theo kích thước file

#### Scenario: Dòng rất dài
- **WHEN** một dòng dài 1 MB được hiện
- **THEN** chỉ phần 4096 byte đầu có màu token, phần sau giữ màu mặc định và dòng vẫn hiện đúng nội dung

### Requirement: Range token đúng theo byte của dòng đã lưu
Mỗi token SHALL có range theo byte của dòng đã lưu, nằm trên ranh giới ký tự, không chồng lên token khác.

#### Scenario: Tiếng Việt và emoji
- **WHEN** dòng là `10:22:03 INFO đã thanh toán 🎉 took=5ms`
- **THEN** các token vẫn rơi đúng chỗ và không cắt giữa một ký tự
