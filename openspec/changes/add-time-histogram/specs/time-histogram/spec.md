## ADDED Requirements

### Requirement: View đủ điều kiện có Time Histogram
Hệ thống SHALL hiện Time Histogram, một dải ngang phía trên danh sách dòng, ở mọi view mà các dòng có thời gian theo từng dòng và có ít nhất một dòng có giờ: Log Space và File View. Hệ thống SHALL NOT hiện Time Histogram ở Raw View, ở view không có dòng nào có giờ, hay trong Focus Pane và Focus Window. Dải SHALL thu gọn được.

#### Scenario: Log Space có giờ
- **WHEN** người dùng mở Log Space có `api.log` và `db.log`, cả hai đều có timestamp, và Histogram đang bật
- **THEN** Time Histogram hiện phía trên danh sách dòng

#### Scenario: File không có timestamp
- **WHEN** người dùng mở File View của một file không có dòng nào có timestamp
- **THEN** Time Histogram không hiện, kể cả khi Histogram đang bật

#### Scenario: Raw View
- **WHEN** người dùng chuyển view sang Raw View
- **THEN** Time Histogram không hiện

#### Scenario: Focus Pane
- **WHEN** Focus Pane đang mở dưới một Log Space có Time Histogram
- **THEN** Focus Pane không có Time Histogram riêng; những dòng nó liệt kê qua Filter hiện ở dạng đậm trên dải của view

### Requirement: Cột đếm Parent Line theo level
Mỗi cột của Time Histogram SHALL là số Parent Line có thời gian hiệu lực trong khoảng của cột đó, chia theo level: Error và Fatal màu đỏ, Warn màu cam, các level còn lại và dòng không có level màu xám. Continuation Line SHALL NOT được đếm.

#### Scenario: Exception nhiều dòng
- **WHEN** một dòng `ERROR` lúc `10:02:00` có 80 Continuation Line
- **THEN** cột chứa `10:02:00` tính dòng đó là một dòng Error

#### Scenario: File không có level
- **WHEN** view chỉ có các dòng không có level
- **THEN** mọi cột đều màu xám

### Requirement: Trục thời gian và zoom
Trục của Time Histogram SHALL trải từ thời gian hiệu lực nhỏ nhất tới lớn nhất của view. Độ rộng mỗi cột SHALL được chọn trong các bước 1s, 5s, 10s, 30s, 1m, 5m, 15m, 1h, 6h, 1d, sao cho mỗi cột rộng khoảng 4 đến 6 pixel theo chiều rộng dải. ⌘ + cuộn trên dải SHALL zoom quanh vị trí con trỏ, và nút "Xem toàn bộ" SHALL đưa trục về toàn bộ khoảng của view. Zoom SHALL NOT làm đổi Filter, Focus Pane hay danh sách dòng.

#### Scenario: Chọn bước theo khoảng
- **WHEN** view trải 1 giờ và dải rộng 720 pixel
- **THEN** mỗi cột là 30 giây

#### Scenario: Zoom không lọc
- **WHEN** người dùng zoom vào khoảng `10:20`–`10:50`
- **THEN** trục hiện `10:20`–`10:50`, còn Focus Pane và danh sách dòng không đổi

### Requirement: Filter và Search trên histogram
Khi Filter của view đang thu hẹp Focus Pane, mỗi cột SHALL vẽ tổng số dòng ở dạng mờ, và số dòng qua Filter ở dạng đậm, tô theo level. Khi Focus Tab đang mở có Search Expression, những cột có ít nhất một Match SHALL có một vạch đánh dấu dưới cột.

#### Scenario: Filter theo level
- **WHEN** Filter chỉ cho qua `level` = `ERROR`
- **THEN** mỗi cột hiện tổng số dòng ở dạng mờ, và phần Error ở dạng đậm

#### Scenario: Khoảng trống phân biệt được
- **WHEN** từ `10:41` tới `10:44` không có dòng nào
- **THEN** các cột ở khoảng đó trống cả phần mờ lẫn phần đậm

#### Scenario: Đánh dấu Match
- **WHEN** Search Expression là `timeout` và chỉ có Match trong khoảng `10:31`–`10:33`
- **THEN** chỉ các cột trong khoảng đó có vạch đánh dấu

### Requirement: Khung vùng đang xem
Time Histogram của view chính SHALL vẽ một khung chỉ có viền, trải từ thời gian của dòng đầu tới thời gian của dòng cuối mà danh sách đang hiện. Khung SHALL đi theo khi người dùng cuộn danh sách.

#### Scenario: Cuộn danh sách
- **WHEN** người dùng cuộn danh sách từ các dòng lúc `10:00` tới các dòng lúc `10:30`
- **THEN** khung dời tới vị trí `10:30` trên trục

### Requirement: Bấm cột để nhảy
Bấm vào một cột SHALL đặt dòng hiện tại của view thành dòng đầu tiên có thời gian hiệu lực trong khoảng của cột đó, và SHALL NOT thu hẹp gì. Cột trống SHALL nhảy tới dòng đầu tiên có thời gian lớn hơn hoặc bằng thời điểm bắt đầu của cột.

#### Scenario: Nhảy tới đợt lỗi
- **WHEN** người dùng bấm vào cột `10:31`
- **THEN** danh sách cuộn tới và chọn dòng đầu tiên lúc `10:31`, còn Filter và Focus Pane không đổi

### Requirement: Kéo chọn là Time Range
Kéo chọn một khoảng trên Time Histogram của view chính SHALL đặt Time Range của view thành khoảng đó, và do đó thu hẹp Focus Pane. Khi Time Range được đặt, dải SHALL tô nền xanh nhạt cho khoảng đó. Kéo mép của dải SHALL đổi Từ hoặc Đến, kéo phần giữa SHALL dời cả khoảng, và bấm vào phần trống của dải SHALL xoá Time Range. Khi không có Time Range, dải SHALL NOT tô nền.

#### Scenario: Kéo chọn
- **WHEN** người dùng kéo từ `10:30` tới `10:35` trên Time Histogram
- **THEN** Time Range là `10:30`–`10:35`, Focus Pane chỉ liệt kê dòng trong khoảng đó, và khoảng đó được tô nền xanh nhạt

#### Scenario: Kéo mép
- **WHEN** Time Range là `10:30`–`10:35` và người dùng kéo mép phải tới `10:40`
- **THEN** Time Range thành `10:30`–`10:40`

#### Scenario: Xoá bằng cách bấm chỗ trống
- **WHEN** Time Range đang được đặt và người dùng bấm vào phần dải nằm ngoài khoảng tô nền mà không kéo
- **THEN** Time Range bị xoá và nền xanh biến mất

### Requirement: Dòng không có thời gian
Dòng không có thời gian hiệu lực SHALL NOT được vẽ thành cột. Khi view có dòng như vậy, Time Histogram SHALL hiện một nhãn ghi số dòng đó, và bấm vào nhãn SHALL đặt dòng hiện tại thành dòng đầu tiên không có thời gian.

#### Scenario: Log Space có một file không có giờ
- **WHEN** Log Space có `api.log` có timestamp và `notes.txt` có 1.204 dòng không có timestamp
- **THEN** nhãn ghi 1.204 dòng không có thời gian, và bấm vào nhãn thì nhảy tới dòng đầu tiên của `notes.txt`

### Requirement: Live và Indexing
Khi view nhận dòng mới, Time Histogram SHALL cộng các dòng đó vào cột tương ứng mà không đếm lại các dòng cũ, trừ khi trục phải giãn ra. Hệ thống SHALL vẽ lại histogram không quá hai lần mỗi giây khi dòng đang tới. Khi Indexing chưa xong, phần trục chưa có số đếm SHALL được vẽ mờ. Việc đếm SHALL NOT chạy trên UI thread và SHALL NOT đọc lại file.

#### Scenario: Tail
- **WHEN** Live Log Space nhận 200 dòng mới trong khoảng thời gian của cột cuối
- **THEN** cột cuối tăng thêm 200 dòng, chia theo level

#### Scenario: Đang Indexing
- **WHEN** file 5 GB mới đọc được một nửa
- **THEN** histogram vẽ phần đã đếm, và phần còn lại vẽ mờ

### Requirement: Nút Histogram và Minimap
Thanh công cụ của view SHALL có nút Histogram và nút Minimap để bật/tắt Time Histogram và minimap. Command Palette SHALL có Command cho mỗi nút để gán được Shortcut. Trạng thái của hai nút SHALL được nhớ toàn cục, cho mọi view, và qua các lần khởi động lại. Histogram SHALL mặc định bật. Settings SHALL NOT còn mục "Show minimap".

#### Scenario: Tắt ở một tab
- **WHEN** người dùng tắt Histogram trong Log Space "Payment" rồi mở Log Space "Auth"
- **THEN** "Auth" cũng không có Time Histogram

#### Scenario: Giữ trạng thái minimap cũ
- **WHEN** người dùng trước đó đã tắt "Show minimap" trong Settings, rồi nâng cấp ứng dụng
- **THEN** nút Minimap ở trạng thái tắt
