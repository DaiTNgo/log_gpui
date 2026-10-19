## ADDED Requirements

### Requirement: Clear một Remote File
Hệ thống SHALL cho người dùng Clear từng Remote File của một Remote Log Space, từ menu của file ở sidebar, header File Section trong Raw View, Command Palette và phím tắt `Cmd+Shift+K` cho file của dòng đang chọn. Clear SHALL bỏ khỏi bộ nhớ mọi dòng file đó đang giữ, ở mọi Generation, và SHALL NOT thay đổi gì trên Server. Imported Log Space và Scratch Log Space SHALL NOT có Clear.

#### Scenario: Clear một file
- **WHEN** `web1:/app/app.log` đang giữ 100.000 dòng và người dùng chọn "Clear" trên file đó
- **THEN** không còn dòng nào của `web1:/app/app.log` trong view, và file `/app/app.log` trên `web1` không đổi

#### Scenario: Chỉ file được chọn
- **WHEN** Remote Log Space "Checkout" có `web1:/app/app.log` và `web2:/app/app.log`, và người dùng Clear `web1:/app/app.log`
- **THEN** các dòng của `web2:/app/app.log` vẫn hiện nguyên, cùng thứ tự

#### Scenario: Clear cả Generation cũ
- **WHEN** file đang giữ dòng của Generation hiện tại và của `app.log.1` nạp phía trên
- **THEN** sau Clear không còn dòng nào của cả hai Generation

#### Scenario: Không có Clear trong Imported Log Space
- **WHEN** người dùng chuột phải lên một file của Imported Log Space, kể cả Remote Copy
- **THEN** menu không có "Clear"

### Requirement: Clear all files
Menu của Remote Log Space SHALL có "Clear all files", có tác dụng như Clear từng Remote File và Remote Command của Log Space đó trong cùng một lần.

#### Scenario: Clear all
- **WHEN** người dùng chọn "Clear all files" trên "Checkout" có ba Remote File
- **THEN** cả ba file không còn dòng nào, và mỗi file có Break "cleared" riêng chờ dòng mới

### Requirement: Clear một Remote Command
Hệ thống SHALL cho người dùng Clear một Remote Command của Remote Log Space theo cùng cách và với cùng hệ quả như Clear một Remote File. Clear SHALL NOT dừng, gửi signal hay chạy lại Run đang chạy; các dòng output đến sau Clear Point SHALL hiện dưới Break "cleared". Restart, Restart on exit và nối lại sau đó SHALL bắt đầu Run như không có Clear. Resume By Time SHALL dùng timestamp của dòng cuối đã nhận, kể cả khi dòng đó đã bị Clear. Remote Command SHALL NOT có Load more hay Back to Live Window.

#### Scenario: Lệnh vẫn chạy sau Clear
- **WHEN** `web1$ journalctl -f -u nginx` đang chạy và người dùng Clear nó
- **THEN** không còn dòng nào của lệnh trong view, Run vẫn chạy, và dòng output tiếp theo hiện dưới Break "web1$ journalctl -f -u nginx cleared"

#### Scenario: Không có Load more cho Remote Command
- **WHEN** người dùng Clear một Remote Command rồi chuột phải lên nó
- **THEN** menu không có "Load more", và Break "cleared" không có nút nào

#### Scenario: Resume By Time sau Clear
- **WHEN** một Remote Command từ Command Preset chưa sửa đã bị Clear, rồi mất kết nối và nối lại
- **THEN** lệnh được chạy lại với thời điểm của dòng cuối đã nhận trước khi mất kết nối, và không dòng nào trước Clear Point hiện lại

### Requirement: Đọc tiếp sau Clear
Sau Clear, hệ thống SHALL tiếp tục đọc file. Các dòng sau Clear Point SHALL hiện dưới một Break "`<file>` cleared `<thời điểm>`". Clear Point SHALL nằm ngay sau dòng trọn vẹn cuối cùng file đã nhận; một dòng chưa trọn lúc Clear SHALL hiện lại trọn vẹn dưới Break. Khi file chưa có dòng mới, File Section của nó trong Raw View SHALL hiện Break đó cùng nút Load more.

#### Scenario: Dòng mới sau Clear
- **WHEN** người dùng Clear `web1:/app/app.log` lúc 10:42:11 rồi server ghi thêm 3 dòng
- **THEN** view hiện Break "web1:/app/app.log cleared 10:42:11" rồi đúng 3 dòng đó, với số dòng thật của chúng trong Generation

#### Scenario: Dòng đang ghi dở
- **WHEN** lúc Clear, dòng cuối cùng nhận được chưa có ký tự xuống dòng
- **THEN** dòng đó bị bỏ cùng các dòng khác, và khi server ghi xong nó hiện trọn vẹn dưới Break "cleared"

#### Scenario: Chưa có dòng mới
- **WHEN** người dùng Clear một file và chưa có dòng mới
- **THEN** File Section của file trong Raw View chỉ có Break "cleared", dòng "Waiting for new lines" và nút "Load more"

### Requirement: Không tự nạp lại sau Clear
Sau Clear, hệ thống SHALL NOT tự nạp dòng cũ hơn Clear Point của file đó: việc nạp dần đang chạy SHALL dừng, và hệ thống SHALL NOT tự nạp Rotated File, kể cả khi Live Window còn chỗ hay sau khi kết nối lại. Chỉ Load more SHALL nạp dòng cũ hơn Clear Point.

#### Scenario: Clear khi đang nạp dần
- **WHEN** người dùng mở `web1:/app/app.log` và Clear khi file mới nạp được 20.000 trên 100.000 dòng
- **THEN** việc nạp dần dừng, và không có dòng nào cũ hơn Clear Point được nạp thêm

#### Scenario: Kết nối lại sau Clear
- **WHEN** file đã Clear mất kết nối rồi nối lại
- **THEN** chỉ các dòng sau chỗ đã đọc tới được nạp, có Break đánh dấu mất kết nối, và không có dòng nào trước Clear Point

### Requirement: Clear giải phóng bộ nhớ
Clear SHALL bỏ thật các dòng khỏi bộ nhớ của app, gồm cả bản sao của view, chỉ mục Filter, kết quả Search và Focus Tab, chứ không chỉ ẩn chúng. Tooltip của file SHALL hiện số dòng còn giữ và bộ nhớ ước lượng.

#### Scenario: Số dòng giữ sau Clear
- **WHEN** người dùng Clear một file đang giữ 100.000 dòng
- **THEN** tooltip của file ở sidebar ghi "0 lines held"

### Requirement: Hệ quả của Clear
Dòng bị Clear SHALL được xử lý như dòng rời Live Window: mất Marker, và thôi được đếm cho Filter Value. Một Filter Value không còn dòng nào SHALL biến mất trừ khi đang được tích. Một cột đã vượt 500 giá trị riêng biệt mà sau Clear còn không quá 500 SHALL nhận điều kiện mới trở lại. Match trong dòng bị Clear SHALL ra khỏi Match count, overview ruler và Focus Pane. Dòng hiện tại của một Focus Tab, Match hiện tại hay dòng đang chọn nằm trong dòng bị Clear SHALL chuyển sang dòng còn giữ ngay sau nó trong timeline gộp; nếu không có thì sang dòng cuối cùng còn giữ, hoặc không có dòng nào nếu Log Space rỗng. Row của các file khác SHALL giữ nguyên thứ tự trong timeline gộp. Nếu phần bị Clear có Marker, hệ thống SHALL hỏi xác nhận trước khi Clear, nêu số Marker sẽ mất.

#### Scenario: Marker bị Clear
- **WHEN** dòng 1.950.000 của `web1:/app/app.log` có Marker và người dùng xác nhận Clear file đó
- **THEN** Marker bị bỏ, và không hiện lại kể cả khi dòng đó được Load more đọc lại

#### Scenario: Số đếm Filter sau Clear
- **WHEN** cột `module` có `DB` với 120 dòng, 100 dòng thuộc `web1:/app/app.log`, và người dùng Clear file đó
- **THEN** số đếm của `DB` là 20

#### Scenario: Giá trị hết dòng sau Clear
- **WHEN** mọi dòng `module = CRON` thuộc file bị Clear và `CRON` chưa được tích
- **THEN** `CRON` biến mất khỏi danh sách `module`

#### Scenario: Focus Tab sau Clear
- **WHEN** dòng hiện tại của Focus Tab 1 thuộc `web1:/app/app.log`, và dòng ngay sau nó trong timeline gộp thuộc `web2:/app/app.log`
- **THEN** sau khi Clear `web1:/app/app.log`, dòng hiện tại của Focus Tab 1 là dòng đó của `web2:/app/app.log`

#### Scenario: Match count sau Clear
- **WHEN** Search có 40 Match, 30 Match thuộc file bị Clear
- **THEN** Match count là 10

### Requirement: Clear khi Pause và khi mất kết nối
Clear SHALL chạy được khi Log Space đang Pause và khi mất kết nối tới Server. Khi tiếp tục sau Pause, các dòng ghi trong lúc Pause SHALL hiện dưới Break "cleared".

#### Scenario: Clear khi Pause
- **WHEN** Log Space đang Pause, người dùng Clear `web1:/app/app.log`, server ghi 50 dòng, rồi người dùng tiếp tục
- **THEN** 50 dòng đó hiện dưới Break "cleared"

### Requirement: Clear không được nhớ
Hệ thống SHALL NOT nhớ việc Clear khi đóng hay mở lại Log Space hoặc Project. Mở lại SHALL mở file như lần đầu.

#### Scenario: Mở lại sau Clear
- **WHEN** người dùng Clear một file, đóng Project rồi mở lại
- **THEN** file mở với các dòng cuối và nạp dần đầy Live Window như lần đầu, không có Break "cleared"
