## MODIFIED Requirements

### Requirement: Mở Remote File và nạp dần
Khi mở một Remote File, hệ thống SHALL hiện ngay khoảng 1.000 dòng cuối, hoặc chỉ các dòng không cũ hơn Starting Time nếu người dùng chọn Starting Time. Sau đó hệ thống SHALL nạp dần các dòng cũ hơn ở nền cho đến khi đầy Live Window, chạm Starting Time, hoặc hết file. Khi đã nạp hết file hiện tại mà Live Window còn chỗ, hệ thống SHALL tự nạp tiếp từ cuối Rotated File mới nhất nếu nó không nén; các Rotated File khác chỉ được nạp khi người dùng bấm "Load earlier generation". Mọi lần nạp vẫn trong Live Window. Trong lúc nạp dần, view SHALL đọc và cuộn được, dòng mới vẫn được nhận, và vị trí người dùng đang đọc SHALL NOT bị nhảy. Số dòng SHALL là số dòng thật trong Generation của dòng đó trên Server. Hệ thống SHALL NOT nạp dòng vượt ngoài Live Window.

#### Scenario: File lớn
- **WHEN** người dùng mở `web1:/app/app.log` có 2.000.000 dòng
- **THEN** dòng 1.999.001–2.000.000 hiện gần như ngay, sau đó các dòng phía trên được nạp dần tới dòng 1.900.001, và Gutter hiện đúng số dòng đó

#### Scenario: File nhỏ
- **WHEN** file chỉ có 500 dòng
- **THEN** cả 500 dòng hiện với số dòng 1–500

#### Scenario: Có Starting Time
- **WHEN** người dùng mở `web1:/app/app.log` với Starting Time `30m` và 30 phút cuối có 4.000 dòng
- **THEN** việc nạp dần dừng sau 4.000 dòng đó dù Live Window còn chỗ

#### Scenario: Hết file hiện tại khi có Rotated File
- **WHEN** `app.log` có 500 dòng, Live Window là 100.000, và thư mục có `app.log.1` không nén
- **THEN** 500 dòng của `app.log` hiện với số dòng 1–500, rồi các dòng cuối của `app.log.1` tự được nạp phía trên với Break "rotated" cho đến khi file giữ đủ 100.000 dòng

### Requirement: Trạng thái live khi mở
Hệ thống SHALL mở mọi Imported Log Space và Remote Log Space với Follow bật và không Pause, và SHALL NOT nhớ Follow hay Pause khi đóng hay mở lại. Riêng Imported Log Space vừa được tạo bởi Open in full SHALL mở với Follow tắt, không Pause, và view ở dòng đầu tiên được đọc. Scratch Log Space SHALL NOT hiện Follow và Pause.

#### Scenario: Mở lại khi đang Pause
- **WHEN** người dùng đóng một Log Space đang Pause rồi mở lại
- **THEN** Log Space mở với Follow bật và không Pause

#### Scenario: Mở bằng Open in full
- **WHEN** người dùng chọn Open in full trên `web1:/app/app.log` với Starting Time `10:00`
- **THEN** Imported Log Space mới mở với Follow tắt và view ở dòng đầu tiên từ 10:00
