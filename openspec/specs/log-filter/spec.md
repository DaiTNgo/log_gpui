# log-filter Specification

## Purpose

Filter dòng log theo giá trị của các cột phát hiện được, gắn với view (Log Space hoặc File View).
## Requirements
### Requirement: Phát hiện cột và giá trị khi nạp file
Hệ thống SHALL phát hiện các cột của file log và các giá trị riêng biệt của từng cột khi nạp file, và SHALL dùng chúng làm nguồn cho Filter thay cho danh sách cố định.

#### Scenario: Giá trị lấy từ file
- **WHEN** file log có `module` nhận các giá trị `API`, `DB`, `SEARCH`
- **THEN** Filter của cột `module` liệt kê đúng ba giá trị đó và không liệt kê module nào khác

#### Scenario: Trường JSON
- **WHEN** file log là JSON có trường `region` với các giá trị `us`, `eu`
- **THEN** `region` xuất hiện là một cột để lọc

### Requirement: Cột lọc được
Hệ thống SHALL cho phép lọc theo giá trị trên mọi cột có tối đa 1000 giá trị riêng biệt, trừ `timestamp` và `message`/payload. Hệ thống SHALL NOT cho thêm điều kiện giá trị trên cột vượt 1000 giá trị riêng biệt.

#### Scenario: Cột nằm trong ngưỡng
- **WHEN** cột `module` có 7 giá trị riêng biệt
- **THEN** người dùng có thể thêm Filter cho cột `module`

#### Scenario: Cột có vài trăm giá trị
- **WHEN** cột `thread` có 800 giá trị riêng biệt
- **THEN** người dùng có thể thêm Filter cho cột `thread`

#### Scenario: Cột quá ngưỡng
- **WHEN** cột `request_id` có hơn 1000 giá trị riêng biệt
- **THEN** cột đó hiển thị mờ kèm số giá trị riêng biệt và lý do, và không thể thêm Filter cho nó

#### Scenario: Cột luôn bị loại
- **WHEN** người dùng chọn cột để thêm Filter
- **THEN** `timestamp` và `message`/payload không có trong danh sách chọn cột giá trị

### Requirement: Thẻ Message
Hệ thống SHALL có mục Message ở cuối danh sách chọn cột để thêm Filter. Thẻ Message SHALL NOT liệt kê checkbox giá trị; nó SHALL chỉ nhận Custom Value, khớp các dòng có message chứa chuỗi đã nhập theo cùng quy tắc của Add value, và SHALL có Hide Mode như các thẻ khác.

#### Scenario: Thêm điều kiện trên message
- **WHEN** người dùng chọn Message trong danh sách chọn cột và thêm Custom Value `timeout`
- **THEN** chỉ các dòng có message chứa `timeout`, không phân biệt hoa thường, được hiển thị

#### Scenario: Thẻ Message không có checkbox
- **WHEN** người dùng mở thẻ Message
- **THEN** thẻ chỉ có ô nhập Custom Value, không có danh sách giá trị

### Requirement: Hiển thị số giá trị riêng biệt khi chọn cột
Hệ thống SHALL hiển thị số giá trị riêng biệt (Distinct Count) của từng cột trong danh sách chọn cột để thêm Filter.

#### Scenario: Số đếm bên cạnh cột
- **WHEN** người dùng mở danh sách chọn cột để thêm Filter
- **THEN** mỗi cột hiển thị số giá trị riêng biệt của nó, và cột vượt ngưỡng hiển thị "> 1000"

### Requirement: Checkbox giá trị với số dòng
Hệ thống SHALL hiển thị mỗi giá trị của cột lọc được dưới dạng một checkbox kèm số dòng có giá trị đó. Tích checkbox SHALL khớp các dòng có giá trị bằng đúng giá trị đó.

#### Scenario: Tích một giá trị
- **WHEN** người dùng tích `DB` ở cột `module`
- **THEN** chỉ các dòng có `module` là `DB` được hiển thị

### Requirement: Gộp giá trị chỉ khác hoa thường
Hệ thống SHALL gộp các giá trị chỉ khác nhau về hoa thường thành một checkbox, hiển thị dạng xuất hiện nhiều nhất với số đếm cộng dồn, và SHALL NOT gộp các giá trị khác nhau về dấu.

#### Scenario: Gộp hoa thường
- **WHEN** cột `level` có 40 dòng `ERROR` và 2 dòng `error`
- **THEN** chỉ có một checkbox `ERROR` với số đếm 42, và tích nó khớp cả 42 dòng

#### Scenario: Không gộp theo dấu
- **WHEN** cột `message_type` có `lỗi` và `loi`
- **THEN** có hai checkbox riêng

### Requirement: Add value
Hệ thống SHALL cho phép người dùng nhập thêm một Custom Value bên dưới danh sách checkbox của cột. Custom Value SHALL khớp các dòng có giá trị ở cột đó chứa chuỗi đã nhập, không phân biệt hoa thường theo Unicode và không phải regex.

#### Scenario: Khớp chứa chuỗi
- **WHEN** người dùng thêm Custom Value `pay` ở cột `module`
- **THEN** các dòng có `module` là `PAYMENT` hoặc `paypal` được hiển thị

#### Scenario: Không regex
- **WHEN** người dùng thêm Custom Value `a.c` ở cột `module`
- **THEN** chỉ các dòng có `module` chứa đúng chuỗi `a.c` được hiển thị

### Requirement: Kết hợp điều kiện
Hệ thống SHALL kết hợp các giá trị đã tích và Custom Value trong cùng một cột bằng OR, và kết hợp các cột khác nhau cùng Time Range bằng AND.

#### Scenario: OR trong một cột
- **WHEN** người dùng tích `API` và `DB` ở cột `module`
- **THEN** các dòng có `module` là `API` hoặc `DB` được hiển thị

#### Scenario: AND giữa các cột
- **WHEN** người dùng tích `API` ở `module` và `ERROR` ở `level`
- **THEN** chỉ các dòng có cả `module` là `API` và `level` là `ERROR` được hiển thị

### Requirement: Hide Mode
Hệ thống SHALL cung cấp công tắc theo cột để đổi "chỉ hiện các giá trị đã chọn" thành "ẩn các giá trị đã chọn", áp dụng cho cả giá trị đã tích và Custom Value của cột đó.

#### Scenario: Ẩn giá trị
- **WHEN** người dùng bật Hide Mode ở cột `level` và tích `DEBUG`
- **THEN** các dòng có `level` là `DEBUG` bị ẩn và các dòng còn lại vẫn hiển thị

#### Scenario: Ẩn theo chuỗi chứa
- **WHEN** người dùng bật Hide Mode ở cột `module` và thêm Custom Value `health`
- **THEN** các dòng có `module` chứa `health` bị ẩn

### Requirement: Dòng thiếu giá trị không bao giờ khớp
Hệ thống SHALL coi dòng không có giá trị ở một cột là không khớp mọi giá trị đã tích và Custom Value của cột đó, và SHALL NOT hiển thị giá trị "(trống)".

#### Scenario: Chỉ hiện giá trị đã chọn
- **WHEN** người dùng tích `API` ở cột `module` và có dòng không có `module`
- **THEN** dòng không có `module` bị ẩn

#### Scenario: Hide Mode
- **WHEN** người dùng bật Hide Mode ở cột `level`, tích `DEBUG`, và có dòng không có `level`
- **THEN** dòng không có `level` vẫn được hiển thị

### Requirement: Số đếm không đổi theo Filter khác
Hệ thống SHALL tính danh sách giá trị và số dòng của mỗi cột khi nạp file, và SHALL NOT thay đổi chúng khi áp dụng điều kiện của cột khác.

#### Scenario: Áp Filter không đổi số đếm
- **WHEN** cột `module` có `DB` với số đếm 120 và người dùng tích `ERROR` ở cột `level`
- **THEN** số đếm của `DB` vẫn là 120

### Requirement: Khi tail
Khi file đang được tail và xuất hiện giá trị mới ở một cột lọc được, hệ thống SHALL thêm giá trị đó vào cuối danh sách chưa tích và cộng số đếm theo dòng mới. Nếu cột vượt 1000 giá trị riêng biệt, hệ thống SHALL giữ các điều kiện đã bật và SHALL NOT cho thêm điều kiện mới.

#### Scenario: Giá trị mới khi tail
- **WHEN** đang tail và xuất hiện dòng đầu tiên có `module` là `SEARCH`
- **THEN** `SEARCH` xuất hiện cuối danh sách `module`, chưa tích, với số đếm 1

#### Scenario: Cột vượt ngưỡng khi tail
- **WHEN** cột `module` có điều kiện đang bật và vượt 1000 giá trị riêng biệt do tail
- **THEN** điều kiện đang bật vẫn có hiệu lực và không thể thêm điều kiện mới

### Requirement: Time Range
Hệ thống SHALL cung cấp Filter khoảng thời gian cho `timestamp` với hai ô Từ và Đến. Mỗi ô SHALL nhận mọi định dạng thời gian mà ứng dụng đọc được từ dòng log, và SHALL có placeholder lấy từ timestamp đầu tiên của view. Ô trống nghĩa là không giới hạn phía đó. Hệ thống SHALL gợi ý giá trị nhỏ nhất và lớn nhất, và SHALL so sánh theo đồng hồ của file, không đổi múi giờ.

Trong section Filter, hai ô Từ và Đến SHALL luôn hiện, xếp dọc, mỗi ô có nhãn cố định ("From", "To"). Section SHALL NOT có nhãn nhóm "Time range", preset khoảng thời gian, hay dòng tóm tắt Time Range.

#### Scenario: Giới hạn hai phía
- **WHEN** người dùng nhập Từ `10:00:00` và Đến `10:05:00`
- **THEN** chỉ các dòng có timestamp trong khoảng đó được hiển thị

#### Scenario: Một phía
- **WHEN** người dùng chỉ nhập Từ `10:00:00`
- **THEN** mọi dòng từ `10:00:00` trở đi được hiển thị

#### Scenario: Định dạng khác cột hiển thị
- **WHEN** cột TIMESTAMP hiện `2026-10-18T10:00:00+07:00` và người dùng nhập Từ `2026-10-18 10:00:00`
- **THEN** Time Range được chấp nhận và lọc từ thời điểm đó

#### Scenario: Chỉ có hai ô
- **WHEN** người dùng mở section Filter
- **THEN** có ô From và ô To, không có preset hay dòng tóm tắt thời gian

### Requirement: Dòng không có timestamp kế thừa
Hệ thống SHALL gán cho dòng không có timestamp riêng timestamp của dòng gần nhất phía trên có timestamp, và SHALL luôn hiển thị các dòng đứng trước timestamp đầu tiên của file.

#### Scenario: Stack trace đi cùng sự kiện
- **WHEN** một sự kiện lúc `10:02:00` có ba dòng stack trace không có timestamp và Time Range là `10:00:00`–`10:05:00`
- **THEN** cả sự kiện và ba dòng stack trace được hiển thị

#### Scenario: Dòng đầu không có timestamp
- **WHEN** file bắt đầu bằng các dòng không có timestamp và Time Range chỉ cho phép khoảng sau đó
- **THEN** các dòng đầu vẫn được hiển thị

### Requirement: Không có regex trong Filter
Hệ thống SHALL chỉ khớp theo bằng (Filter Value) hoặc chứa chuỗi (Custom Value) và SHALL NOT hỗ trợ regex trong Filter.

#### Scenario: Ký tự regex là chữ thường
- **WHEN** người dùng thêm Custom Value `.*` ở cột `module`
- **THEN** chỉ các dòng có `module` chứa đúng chuỗi `.*` được hiển thị

### Requirement: So khớp Unicode dùng chung với Search
Hệ thống SHALL so khớp chuỗi trong Filter không phân biệt hoa thường theo Unicode, chuẩn hóa NFC, và SHALL NOT bỏ dấu.

#### Scenario: Hoa thường tiếng Việt
- **WHEN** người dùng thêm Custom Value `đã` ở một cột và có dòng chứa `ĐÃ`
- **THEN** dòng đó khớp

### Requirement: Không còn các điểm vào cũ
Hệ thống SHALL NOT có panel "View", danh sách module cố định, "Saved View" hay panel Marker. Lọc theo giá trị SHALL làm được từ context sidebar, từ Selection Menu và từ Inspector, theo quy tắc của `selection-menu`. Ghim cột trên Column Header, đổi độ rộng cột và Auto-fit SHALL vẫn hoạt động. Density và công tắc Source Column SHALL nằm trong Quick Settings trên toolbar, và Quick Settings SHALL chỉ chứa các công tắc hiển thị.

#### Scenario: Các mục của context sidebar
- **WHEN** người dùng mở context sidebar trên Parsed View
- **THEN** các mục là Filter, Highlights, Columns và Inspector, không có View hay Marker

#### Scenario: Ghim cột vẫn dùng được
- **WHEN** người dùng bấm Pin Toggle trên Column Header `LEVEL`
- **THEN** cột `LEVEL` được ghim như trước

#### Scenario: Density trong Quick Settings
- **WHEN** người dùng mở Quick Settings trên toolbar
- **THEN** có lựa chọn density và công tắc "Show source file"

### Requirement: Filter và Marker theo view
Hệ thống SHALL giữ một Filter chung cho mỗi Log Space và một Filter riêng cho mỗi File View, tự động lưu và khôi phục khi mở lại hoặc khởi động lại ứng dụng. Marker SHALL gắn với một dòng của một file, và SHALL hiện ở mọi view có dòng đó. Hệ thống SHALL NOT lưu Search Expression, và SHALL NOT có preset Filter theo Project.

#### Scenario: Filter chung của Log Space
- **WHEN** người dùng đặt Filter `level` = `ERROR` trong Log Space "Payment" có `api.log` và `worker.log`
- **THEN** cả dòng của `api.log` lẫn `worker.log` đều chỉ còn `ERROR`

#### Scenario: File View có Filter riêng
- **WHEN** Log Space "Payment" đang lọc `level` = `ERROR` và người dùng mở File View `api.log`
- **THEN** File View `api.log` không có Filter nào, và đặt Filter trong File View không làm đổi Filter của "Payment"

#### Scenario: Marker dùng chung giữa các view
- **WHEN** người dùng đặt Marker trên dòng 42 của `api.log` trong File View
- **THEN** dòng đó cũng có Marker trong Log Space "Payment"

#### Scenario: Khôi phục khi mở lại
- **WHEN** người dùng đóng tab "Payment" có Filter và Marker rồi mở lại
- **THEN** Filter và Marker được khôi phục và ô Search trống

### Requirement: Chỉ mục Filter trên tập dòng đã gộp
Hệ thống SHALL tính các cột lọc được, Filter Value, số dòng và Distinct Count của Log Space trên tập dòng gộp từ mọi file. File View SHALL tính trên các dòng của riêng file đó.

#### Scenario: Giá trị từ nhiều file
- **WHEN** `api.log` có `module` = `API` và `worker.log` có `module` = `JOB`
- **THEN** Filter cột `module` của Log Space liệt kê cả `API` và `JOB`

### Requirement: Tạm tắt Filter
Filter chỉ nằm ở section Filter của context sidebar; ngoài sidebar SHALL NOT có thanh hay chip nào hiện Filter. Header của section Filter SHALL có nút tạm tắt khi Filter khác rỗng. Khi Filter đang tạm tắt, Focus Pane SHALL liệt kê mọi dòng như khi Filter rỗng (bảng ở trên vẫn luôn hiện toàn bộ log), nhưng các điều kiện SHALL vẫn còn và hiện gạch ngang. Bấm lại, hoặc sửa bất kỳ điều kiện nào, SHALL bật lại Filter. Trạng thái tạm tắt SHALL NOT được lưu.

#### Scenario: Tạm tắt rồi bật lại
- **WHEN** Filter đang ẩn dòng `DEBUG`, người dùng bấm tạm tắt rồi bấm lại
- **THEN** lúc tạm tắt Focus Pane có dòng `DEBUG`, sau khi bấm lại thì `DEBUG` lại bị ẩn khỏi Focus Pane

#### Scenario: Không lưu khi tạm tắt
- **WHEN** người dùng tạm tắt Filter rồi đóng và mở lại view
- **THEN** Filter đang bật

### Requirement: Thẻ Filter dạng tóm tắt
Mỗi cột có điều kiện SHALL hiện là một dòng tóm tắt (tên cột, các giá trị, `not` khi ở Hide Mode, nút ×). Bấm vào dòng SHALL mở khối chọn giá trị của cột. Khối này có một ô nhập: gõ chữ thì lọc danh sách giá trị, Enter thêm chữ đã gõ làm Custom Value. Mỗi giá trị hiện số dòng căn phải. Ở cuối khối có lựa chọn chuyển giữa hiện và Hide Mode.

#### Scenario: Tìm rồi thêm
- **WHEN** người dùng mở thẻ `module`, gõ `pay` và nhấn Enter
- **THEN** danh sách chỉ còn các giá trị chứa `pay`, và `pay` được thêm làm Custom Value

### Requirement: Time Range từ Time Histogram
Time Range SHALL đặt, đổi và xoá được từ Time Histogram (xem `time-histogram`), và kết quả SHALL giống hệt nhập Từ và Đến. Hai ô Từ và Đến SHALL hiện giá trị do Time Histogram đặt: chỉ giờ (`HH:MM:SS`, thêm phần lẻ giây khi cột nhỏ hơn 1 giây) khi khoảng nằm trong một ngày, và ngày giờ đầy đủ khi khoảng trải qua nhiều ngày. Sửa hai ô SHALL cập nhật dải tô nền trên Time Histogram.

#### Scenario: Kéo chọn điền hai ô
- **WHEN** người dùng kéo chọn `10:30:00`–`10:35:00` của cùng một ngày trên Time Histogram
- **THEN** ô Từ hiện `10:30:00` và ô Đến hiện `10:35:00`

#### Scenario: Khoảng qua nhiều ngày
- **WHEN** người dùng kéo chọn từ `2026-10-28 23:50:00` tới `2026-10-29 00:10:00`
- **THEN** hai ô hiện ngày giờ đầy đủ

#### Scenario: Gõ vào ô thì dải đổi theo
- **WHEN** Time Range là `10:30:00`–`10:35:00` và người dùng sửa ô Đến thành `10:40:00`
- **THEN** dải tô nền trên Time Histogram trải tới `10:40:00`

