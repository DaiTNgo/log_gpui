## ADDED Requirements

### Requirement: Nhận dòng mới
Hệ thống SHALL nhận dòng mới được ghi vào file của Imported Log Space và Remote Log Space trong lúc Log Space đang mở và không bị Pause. Dòng mới SHALL được parse, chèn theo timestamp, và cập nhật Filter, Search và Focus Pane. Scratch Log Space SHALL NOT có tính năng này.

#### Scenario: File local được ghi thêm
- **WHEN** Imported Log Space có `/tmp/app.log` và một tiến trình khác ghi thêm 3 dòng vào file
- **THEN** 3 dòng mới hiện trong view và Focus Pane nếu chúng qua Filter và Search Expression

#### Scenario: Dòng viết dở
- **WHEN** file được ghi thêm `ERROR db` nhưng chưa có ký tự xuống dòng
- **THEN** hệ thống chưa hiện dòng đó cho đến khi có ký tự xuống dòng

### Requirement: Follow
Hệ thống SHALL có công tắc Follow trên status bar. Khi Follow bật, view SHALL cuộn để dòng mới nhất luôn hiện mỗi khi có dòng mới. Khi người dùng cuộn khỏi dòng mới nhất, Follow SHALL tự tắt; khi người dùng cuộn về dòng mới nhất, Follow SHALL tự bật. Khi Follow tắt, dòng mới vẫn được nhận và view SHALL NOT dịch chuyển, và status bar SHALL hiện số dòng mới chưa xem.

#### Scenario: Cuộn lên để đọc
- **WHEN** Follow đang bật và người dùng cuộn lên 200 dòng
- **THEN** Follow tắt, view đứng yên khi có dòng mới, và status bar hiện "+N new"

#### Scenario: Quay lại cuối
- **WHEN** Follow đang tắt và người dùng cuộn xuống dòng mới nhất hoặc bấm Follow
- **THEN** Follow bật và view tiếp tục cuộn theo dòng mới

### Requirement: Pause
Hệ thống SHALL có công tắc Pause trên status bar, áp dụng cho cả Log Space. Khi Pause, Log Space SHALL NOT nhận dòng mới từ bất kỳ file nào, và view, Filter, Focus Pane SHALL NOT thay đổi vì file. Kết nối tới Server SHALL được giữ. Khi tiếp tục, hệ thống SHALL nhận mọi dòng được ghi trong lúc Pause, không bỏ dòng nào.

#### Scenario: Pause rồi tiếp tục
- **WHEN** người dùng Pause lúc 10:00, server ghi thêm 5.000 dòng, và người dùng tiếp tục lúc 10:10
- **THEN** trong lúc Pause view không đổi, và sau khi tiếp tục cả 5.000 dòng được nạp

#### Scenario: Không Pause riêng từng file
- **WHEN** Log Space có ba file và người dùng bấm Pause
- **THEN** cả ba file cùng ngừng nhận dòng mới

### Requirement: Trạng thái live khi mở
Hệ thống SHALL mở mọi Imported Log Space và Remote Log Space với Follow bật và không Pause, và SHALL NOT nhớ Follow hay Pause khi đóng hay mở lại. Scratch Log Space SHALL NOT hiện Follow và Pause.

#### Scenario: Mở lại khi đang Pause
- **WHEN** người dùng đóng một Log Space đang Pause rồi mở lại
- **THEN** Log Space mở với Follow bật và không Pause

### Requirement: Trạng thái live trên status bar
Hệ thống SHALL hiện trạng thái live của Log Space chỉ trên status bar, gồm công tắc Follow, công tắc Pause, và với Remote Log Space là trạng thái kết nối: Connecting, Live, Reconnecting, Disconnected, Waiting for file hoặc lỗi. Khi các file ở trạng thái khác nhau, status bar SHALL hiện trạng thái xấu nhất và liệt kê từng file khi hover hoặc bấm vào. Sidebar SHALL NOT hiện trạng thái kết nối. Công tắc "Tail" SHALL bị bỏ.

#### Scenario: Một file mất kết nối
- **WHEN** `web2:/app/app.log` mất kết nối còn hai file khác vẫn live
- **THEN** status bar hiện "Reconnecting (1 of 3)" và hover thấy `web2:/app/app.log` đang Reconnecting

### Requirement: Break
Hệ thống SHALL vẽ một Break giữa hai row ở chỗ dòng của một file không liên tục: file bị xoay vòng, bị cắt ngắn, bị xoá, mất rồi có lại kết nối, hoặc dòng cũ đã rời Live Window. Break SHALL có nhãn nêu lý do, tên file và thời điểm. Break SHALL NOT là row: nó không có số dòng, không bao giờ là Match, không chịu Filter, và không được copy. Break SHALL hiện trong cả Parsed View, Raw View và Focus Pane.

#### Scenario: Mất kết nối
- **WHEN** `web1:/app/app.log` mất kết nối lúc 10:02 và nối lại lúc 10:03
- **THEN** giữa dòng cuối trước 10:02 và dòng đầu tiên đọc sau khi nối lại có Break "web1:/app/app.log disconnected 10:02 – reconnected 10:03"

#### Scenario: Copy qua Break
- **WHEN** người dùng chọn từ dòng trên Break tới dòng dưới Break rồi copy
- **THEN** đoạn copy chỉ có text của các dòng, không có nhãn Break

### Requirement: Generation khi xoay vòng, cắt ngắn, xoá
Khi file đang đọc bị xoay vòng, bị cắt ngắn hoặc bị xoá, hệ thống SHALL giữ các dòng đã đọc, vẽ Break, và đọc file mới ở cùng đường dẫn như một Generation mới, chờ file xuất hiện nếu cần. Số dòng SHALL bắt đầu lại từ 1 trong mỗi Generation để khớp với file trên đĩa hoặc trên Server.

#### Scenario: Logrotate
- **WHEN** `app.log` đang ở dòng 50.000 thì bị đổi tên thành `app.log.1` và `app.log` mới được tạo
- **THEN** dòng 1–50.000 vẫn hiện, một Break "app.log rotated" xuất hiện, và dòng đầu của file mới hiện với số dòng 1

#### Scenario: Truncate
- **WHEN** `app.log` bị cắt về 0 byte rồi được ghi tiếp
- **THEN** một Break "app.log truncated" xuất hiện và các dòng mới bắt đầu từ dòng 1

### Requirement: Tự kết nối lại
Khi mất kết nối tới một Server, hệ thống SHALL tự thử kết nối lại sau 1s, 2s, 4s… tối đa 30s giữa hai lần, giữ các dòng đã nhận, và đọc tiếp từ byte đã dừng khi kết nối lại để không mất dòng. Nếu file đã nhỏ hơn byte đã dừng hoặc là file khác, hệ thống SHALL xử lý như xoay vòng. Sau 5 phút thử không thành công, hệ thống SHALL ngừng thử, hiện Disconnected và chờ người dùng bấm Reconnect.

#### Scenario: Mạng chập chờn
- **WHEN** Wi-Fi mất 20 giây rồi có lại, trong lúc đó server ghi 300 dòng
- **THEN** sau khi kết nối lại cả 300 dòng hiện đúng thứ tự, có Break đánh dấu khoảng mất kết nối

#### Scenario: Mất hẳn
- **WHEN** server không kết nối lại được trong 5 phút
- **THEN** status bar hiện "Disconnected" với nút Reconnect và hệ thống ngừng thử

### Requirement: Live Window
Hệ thống SHALL giữ cho mỗi Remote File tối đa số dòng mới nhất bằng Live Window, một giá trị chung cho cả ứng dụng, mặc định 100.000, chỉnh trong Settings. Khi có dòng mới làm vượt Live Window, dòng cũ nhất của file đó SHALL rời view. Live Window SHALL NOT áp dụng cho Imported Log Space. Remote Log Space SHALL không giới hạn thời gian kết nối, và các dòng đã nhận SHALL NOT được giữ sau khi đóng Log Space.

#### Scenario: Window đầy
- **WHEN** `web1:/app/app.log` đang giữ 100.000 dòng và nhận thêm 10 dòng
- **THEN** 10 dòng cũ nhất của file đó rời view, và dòng của các file khác không bị ảnh hưởng

#### Scenario: Đổi Live Window
- **WHEN** người dùng giảm Live Window từ 100.000 xuống 20.000
- **THEN** mỗi Remote File đang mở chỉ còn giữ 20.000 dòng mới nhất

### Requirement: Mở Remote File và nạp dần
Khi mở một Remote File, hệ thống SHALL hiện ngay khoảng 1.000 dòng cuối, rồi nạp dần các dòng cũ hơn ở nền cho đến khi đầy Live Window hoặc hết file. Trong lúc nạp dần, view SHALL đọc và cuộn được, dòng mới vẫn được nhận, và vị trí người dùng đang đọc SHALL NOT bị nhảy. Số dòng SHALL là số dòng thật trong Generation hiện tại trên Server. Hệ thống SHALL NOT nạp dòng vượt ngoài Live Window.

#### Scenario: File lớn
- **WHEN** người dùng mở `web1:/app/app.log` có 2.000.000 dòng
- **THEN** dòng 1.999.001–2.000.000 hiện gần như ngay, sau đó các dòng phía trên được nạp dần tới dòng 1.900.001, và Gutter hiện đúng số dòng đó

#### Scenario: File nhỏ
- **WHEN** file chỉ có 500 dòng
- **THEN** cả 500 dòng hiện với số dòng 1–500

### Requirement: Dòng rời Live Window
Khi một dòng rời Live Window, hệ thống SHALL bỏ Marker của dòng đó, bỏ dòng khỏi số đếm của Filter, và vẽ một Break ở đầu phần dòng của file đó nói rằng các dòng cũ hơn đã rời. Nếu dòng hiện tại của một Focus Tab rời, dòng hiện tại SHALL chuyển sang dòng cũ nhất còn giữ của Log Space.

#### Scenario: Marker rời window
- **WHEN** dòng 1.900.001 có Marker và rời Live Window
- **THEN** Marker đó bị bỏ và không hiện lại

#### Scenario: Dòng hiện tại rời window
- **WHEN** dòng hiện tại của Focus Tab 1 rời Live Window
- **THEN** dòng hiện tại của Focus Tab 1 là dòng cũ nhất còn giữ
