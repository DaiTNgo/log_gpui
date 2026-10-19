## ADDED Requirements

### Requirement: Nhập Starting Time
Hệ thống SHALL cho nhập Starting Time khi thêm hoặc mở lại một Remote File live và trong dialog Open in full. Starting Time SHALL nhận một khoảng tương đối (`30m`, `1h`, `2h30m`, `2d`) hoặc một thời điểm tuyệt đối ở mọi dạng mà Time Range nhận. Ô trống SHALL nghĩa là đọc như mặc định: các dòng cuối khi Live, cả file khi Open in full.

#### Scenario: Ô không đọc được
- **WHEN** người dùng gõ `một giờ`
- **THEN** ô hiện lỗi và nút xác nhận bị tắt

### Requirement: Starting Time theo đồng hồ của file
Hệ thống SHALL so Starting Time với timestamp như ghi trong file, không đổi múi giờ. Một khoảng tương đối SHALL được tính lùi từ timestamp mới nhất trong phần cuối của file hiện tại, không từ đồng hồ máy người dùng hay đồng hồ của Server. Giờ không kèm ngày SHALL lấy theo ngày của timestamp mới nhất đó, lùi một ngày nếu kết quả muộn hơn timestamp đó. Khi phần cuối của file không có timestamp nào, ô Starting Time SHALL bị tắt kèm lý do "no timestamps found".

#### Scenario: Server ghi giờ UTC
- **WHEN** máy người dùng ở UTC+7 lúc 17:00, dòng mới nhất của `web1:/app/app.log` ghi `2026-10-19 10:00:00Z`, và người dùng nhập `1h`
- **THEN** Starting Time là `2026-10-19 09:00:00` theo đồng hồ của file

#### Scenario: Giờ không kèm ngày qua nửa đêm
- **WHEN** dòng mới nhất ghi `2026-10-19 00:20:00` và người dùng nhập `23:30`
- **THEN** Starting Time là `2026-10-18 23:30:00`

### Requirement: Starting Time khi Live
Với Remote File live có Starting Time, hệ thống SHALL nạp ngược từ cuối file và SHALL dừng ở Parent Line đầu tiên cũ hơn Starting Time, ở khi đầy Live Window, hoặc ở khi hết file, tuỳ điều nào đến trước. Continuation Line SHALL đi cùng Parent Line của nó. Khi số dòng kể từ Starting Time không vừa Live Window, Break đầu phần dòng của file SHALL ghi điều đó và đề nghị "Open in full…".

#### Scenario: Một giờ gần nhất
- **WHEN** người dùng mở `web1:/app/app.log` với Starting Time `1h` và trong 1 giờ cuối file ghi 12.000 dòng
- **THEN** đúng 12.000 dòng đó được nạp, không có dòng nào cũ hơn, và các dòng mới vẫn tiếp tục đến

#### Scenario: Không vừa Live Window
- **WHEN** trong 1 giờ cuối file ghi 450.000 dòng và Live Window là 100.000
- **THEN** 100.000 dòng mới nhất được nạp, và Break đầu file ghi "450,000 lines since 09:00 do not fit the Live Window — Open in full…"

#### Scenario: Không có dòng nào kể từ Starting Time
- **WHEN** dòng mới nhất của file cũ hơn Starting Time tuyệt đối người dùng nhập
- **THEN** file không có dòng nào, view báo "no lines since <Starting Time>", và dòng mới vẫn được nhận khi đến

#### Scenario: Starting Time trước dòng đầu của file hiện tại
- **WHEN** Starting Time cũ hơn dòng đầu tiên của `app.log` và có `app.log.1` không nén
- **THEN** cả file hiện tại được nạp, rồi `app.log.1` tự được nạp từ cuối lên và dừng ở Starting Time

### Requirement: Starting Time và To khi Open in full
Trong Open in full, hệ thống SHALL tìm vị trí byte của Starting Time và của mốc To trong mỗi file không nén bằng cách đọc timestamp ở các vị trí trong file, không đọc cả file. Hệ thống SHALL chỉ đọc các dòng từ Parent Line đầu tiên không cũ hơn Starting Time tới Parent Line cuối cùng không muộn hơn To. Với file nén, hệ thống SHALL giải nén từ đầu và chỉ giữ các dòng trong khoảng đó. Rotated File có thời gian sửa cũ hơn Starting Time SHALL được bỏ tích sẵn trong dialog.

#### Scenario: Một đoạn giữa file lớn
- **WHEN** người dùng chọn Open in full trên file 8 GB với Starting Time `2026-10-19 10:00` và To `2026-10-19 10:30`
- **THEN** chỉ các byte của đoạn 10:00–10:30 được tải về, và Remote Copy chứa đúng các dòng trong đoạn đó

#### Scenario: Bỏ Rotated File ngoài khoảng
- **WHEN** Starting Time là `1h` và `app.log.1` được sửa lần cuối 3 giờ trước
- **THEN** `app.log.1` hiện trong dialog nhưng không được tích

### Requirement: Số dòng thật khi bắt đầu giữa file
Khi Starting Time làm việc đọc bắt đầu giữa một file, Gutter SHALL hiện số dòng thật của dòng đó trong Generation của nó trên Server. Trong lúc chưa biết số dòng, Gutter SHALL để trống; nếu không lấy được số dòng thật, Gutter SHALL đánh số từ 1 kèm dấu `~` và status bar SHALL ghi "line numbers unavailable".

#### Scenario: Bắt đầu ở dòng 1.500.001
- **WHEN** Starting Time rơi vào dòng 1.500.001 của `app.log`
- **THEN** dòng đầu tiên hiện có số dòng 1.500.001
