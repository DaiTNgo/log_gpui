## ADDED Requirements

### Requirement: Open in full
Hệ thống SHALL có lệnh "Open in full…" trên một Remote File và trên một Remote Log Space. Lệnh SHALL mở một dialog cho chọn Rotated File cần đọc kèm, Starting Time, mốc To, và nơi đặt: một Imported Log Space mới (mặc định, tên `<space> (full)`) hoặc một Imported Log Space có sẵn. Xác nhận dialog SHALL đọc các file đã chọn từ Server thành Remote Copy và thêm chúng vào Imported Log Space đó, không hỏi thư mục tải về.

#### Scenario: Đọc trọn một file
- **WHEN** người dùng chọn "Open in full…" trên `web1:/app/app.log` trong Remote Log Space "Checkout" và bấm Open
- **THEN** một Imported Log Space "Checkout (full)" được tạo với Remote Copy `web1:/app/app.log` chứa mọi dòng của file, không bị giới hạn bởi Live Window

#### Scenario: Cả Remote Log Space
- **WHEN** người dùng chọn "Open in full…" trên "Checkout" có ba Remote File
- **THEN** Imported Log Space mới chứa ba Remote Copy, xếp xen theo timestamp như mọi Imported Log Space

#### Scenario: Kèm Rotated File
- **WHEN** người dùng tích `app.log.1` và `app.log.2.gz` trong dialog
- **THEN** Remote Copy có ba Generation, cũ nhất là nội dung đã giải nén của `app.log.2.gz`

### Requirement: Remote Copy tĩnh
Remote Copy SHALL là bản tĩnh và chỉ đọc của nội dung đã đọc: nó SHALL NOT nhận dòng ghi sau khi đọc xong, SHALL NOT có Live Window, và SHALL NOT bị ảnh hưởng khi file trên Server xoay vòng hay bị xoá. Remote Copy SHALL có tên `<server>:<path>` kèm biểu tượng phân biệt với file trên đĩa, và tooltip ghi thời điểm đọc và khoảng thời gian đã đọc.

#### Scenario: File trên server được ghi thêm
- **WHEN** Remote Copy `web1:/app/app.log` đã đọc xong và server ghi thêm 100 dòng vào `app.log`
- **THEN** Remote Copy không đổi

### Requirement: Dòng hiện dần khi đọc
Trong lúc đọc Remote Copy, hệ thống SHALL hiện các dòng đã đọc theo thứ tự trong file, cho cuộn, Search, Filter và đặt Marker, và SHALL hiện tiến độ trên status bar dạng "Reading web1:/app/app.log 45% (120 / 260 MB)" kèm nút Cancel. Imported Log Space tạo bởi Open in full SHALL mở với Follow tắt và view ở dòng đầu tiên được đọc.

#### Scenario: Đọc file lớn
- **WHEN** Remote Copy đang đọc một file 260 MB
- **THEN** các dòng đầu hiện ngay khi khối đầu tiên về, view đứng ở dòng đầu, và status bar hiện tiến độ

### Requirement: Huỷ và đọc lại
Hệ thống SHALL cho huỷ việc đọc Remote Copy. Khi huỷ, phần đã đọc SHALL được giữ, kèm một Break "read cancelled" ở cuối. Hệ thống SHALL có lệnh "Read again" trên Remote Copy, đọc lại từ Server với cùng Rotated File, Starting Time và To, rồi thay nội dung cũ. Marker SHALL được khôi phục trên các dòng có cùng Generation, số dòng và nội dung.

#### Scenario: Huỷ giữa chừng
- **WHEN** người dùng bấm Cancel khi Remote Copy đã đọc 40%
- **THEN** các dòng đã đọc vẫn hiện, cuối phần đó có Break "read cancelled", và menu của file có "Read again"

#### Scenario: Read again
- **WHEN** người dùng chọn "Read again" trên một Remote Copy có Marker ở dòng 500 và dòng 500 trên Server không đổi
- **THEN** nội dung được đọc lại và Marker ở dòng 500 vẫn còn

### Requirement: Lưu Remote Copy
Hệ thống SHALL lưu nội dung đã giải nén của Remote Copy trong thư mục dữ liệu của app, để mở lại Project thì Remote Copy hiện mà không cần kết nối tới Server. Gỡ Remote Copy khỏi Log Space hoặc xoá Log Space chứa nó SHALL xoá nội dung đã lưu. Khi khởi động, hệ thống SHALL xoá nội dung đã lưu mà không còn Log Space nào dùng.

#### Scenario: Mở lại khi không có mạng
- **WHEN** người dùng thoát app rồi mở lại khi không kết nối được `web1`
- **THEN** Remote Copy `web1:/app/app.log` vẫn hiện đủ các dòng đã đọc, và status bar không hiện trạng thái kết nối

#### Scenario: Gỡ Remote Copy
- **WHEN** người dùng chọn "Remove from Log Space" trên Remote Copy `web1:/app/app.log`
- **THEN** nội dung đã lưu của nó bị xoá khỏi thư mục dữ liệu của app, và `app.log` trên Server không đổi

### Requirement: Full Read Limit
Hệ thống SHALL có Full Read Limit: một giá trị chung cho cả app, mặc định 2.000.000 dòng, chỉnh trong Settings, là số dòng tối đa một lần Open in full được nạp (cộng mọi file và mọi Generation). Trước khi đọc, dialog SHALL hiện số dòng và số byte ước lượng. Khi ước lượng vượt giới hạn, dialog SHALL thay nút Open bằng "Narrow time…", "Read newest <limit> lines" và Cancel.

#### Scenario: File quá lớn
- **WHEN** người dùng chọn Open in full trên một file ước lượng 4.200.000 dòng
- **THEN** dialog hiện "≈ 4.2M lines (380 MB), above the Full Read Limit of 2M" cùng ba lựa chọn, và không đọc gì cho đến khi người dùng chọn

#### Scenario: Đọc phần mới nhất
- **WHEN** người dùng chọn "Read newest 2M lines"
- **THEN** hệ thống đọc khoảng 2.000.000 dòng cuối, bỏ các Rotated File không còn chỗ, và Break đầu Remote Copy ghi rằng các dòng cũ hơn không được đọc

### Requirement: Không cắt mà không báo
Nếu trong lúc đọc số dòng thật chạm Full Read Limit, hệ thống SHALL dừng đọc, giữ phần đã đọc, và vẽ Break "stopped at Full Read Limit" ở chỗ dừng. Hệ thống SHALL kiểm tra dung lượng trống trước khi đọc, và SHALL từ chối kèm thông báo khi không đủ chỗ cho nội dung ước lượng.

#### Scenario: Ước lượng thấp hơn thực tế
- **WHEN** một file `.gz` được ước lượng 1.500.000 dòng nhưng thực tế có 2.300.000 dòng
- **THEN** việc đọc dừng ở dòng thứ 2.000.000 với Break "stopped at Full Read Limit (2,000,000 lines)"

#### Scenario: Không đủ đĩa
- **WHEN** ước lượng là 3 GB và ổ đĩa chỉ còn 2 GB trống
- **THEN** dialog báo không đủ dung lượng và không bắt đầu đọc

### Requirement: Kết nối khi đọc Remote Copy
Khi mất kết nối trong lúc đọc Remote Copy, hệ thống SHALL thử kết nối lại như với Remote File live và SHALL đọc tiếp từ byte đã dừng. Pause của Log Space SHALL tạm dừng việc đọc, và tiếp tục SHALL đọc tiếp mà không bỏ byte nào. Nếu file trên Server bị xoay vòng trong lúc đọc mà không đọc tiếp được, hệ thống SHALL dừng với Break "file rotated while reading".

#### Scenario: Mạng chập chờn khi đọc
- **WHEN** mạng mất 10 giây lúc Remote Copy đọc được 60%
- **THEN** sau khi kết nối lại việc đọc tiếp tục từ 60% và Remote Copy cuối cùng không thiếu dòng nào
