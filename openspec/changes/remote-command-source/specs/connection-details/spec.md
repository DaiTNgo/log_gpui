## ADDED Requirements

### Requirement: Nhãn kết nối kèm độ trễ
Với Remote Log Space, trạng thái kết nối trên status bar SHALL hiện trạng thái xấu nhất của các thành viên như hiện nay, và khi mọi Server của Log Space đã nối SHALL thêm độ trễ của Server chậm nhất, dạng "Live · 42 ms". Remote Command đã thoát SHALL được tính vào trạng thái xấu nhất với nhãn "Exited".

#### Scenario: Ba Server đều live
- **WHEN** Log Space có file trên web1 (12 ms), web2 (42 ms) và db (30 ms), đều đang live
- **THEN** status bar hiện "Live · 42 ms"

#### Scenario: Một lệnh đã thoát
- **WHEN** một trong ba thành viên là Remote Command đã thoát
- **THEN** status bar hiện "Exited (1 of 3)"

### Requirement: Connection Details
Bấm vào trạng thái kết nối trên status bar SHALL mở popover Connection Details, neo vào status bar, cho Remote Log Space đang mở. Popover SHALL nhóm theo Server và hiện cho mỗi Server: tên, `user@host:port`, các Jump Server từ ngoài vào trong, trạng thái, độ trễ gần nhất và trung bình, thời điểm đã nối, số lần nối lại, khoảng keepalive và số lần mất cho phép, lỗi cuối kèm thời điểm. Dưới mỗi Server, popover SHALL liệt kê từng Remote File và Remote Command của Log Space trên Server đó với trạng thái, thời điểm nhận dòng cuối, số lần nối lại (Remote File) hoặc số Run (Remote Command), lỗi cuối, và mã thoát nếu có.

#### Scenario: Xem chi tiết khi chập chờn
- **WHEN** web2 đã mất kết nối hai lần trong buổi, lần cuối lúc 10:02 với "Connection reset by peer", và người dùng bấm "Reconnecting (1 of 3)"
- **THEN** popover hiện web2 với "Reconnects: 2" và "Last error 10:02: Connection reset by peer", cùng `web2:/app/app.log` ở trạng thái Reconnecting

#### Scenario: Server qua Jump Server
- **WHEN** Server "db" đi qua Jump Server "bastion"
- **THEN** popover hiện "via bastion" dưới "db" và độ trễ của "db" là độ trễ tới "db" qua "bastion"

### Requirement: Thao tác trong Connection Details
Popover Connection Details SHALL có nút Reconnect cho mỗi Server và mỗi Remote File không live, và nút Restart cho mỗi Remote Command. Reconnect một Server SHALL nối lại mọi thành viên của Log Space trên Server đó.

#### Scenario: Reconnect Server sau khi bỏ cuộc
- **WHEN** web2 ở trạng thái Disconnected sau 5 phút thử và người dùng bấm Reconnect cạnh web2 trong popover
- **THEN** mọi Remote File và Remote Command trên web2 của Log Space bắt đầu nối lại

### Requirement: Đo độ trễ
Hệ thống SHALL đo độ trễ của mỗi Server đang nối bằng một vòng hỏi–đáp SSH theo chu kỳ keepalive của Server đó, kể cả khi Log Space đang Pause. Trung bình SHALL là trung bình của 5 lần đo gần nhất. Khi chưa có lần đo nào, độ trễ SHALL hiện "—".

#### Scenario: Vừa nối
- **WHEN** web1 vừa nối và chưa có lần đo nào
- **THEN** popover hiện độ trễ "—" cho web1 và nhãn status bar chưa kèm độ trễ

### Requirement: Số liệu không lưu
Số lần nối lại, số Run, lỗi cuối và độ trễ SHALL được đếm từ khi ứng dụng khởi động và SHALL NOT được lưu khi đóng Log Space hay thoát ứng dụng. Số liệu của một Server SHALL là chung cho mọi Log Space dùng Server đó trong lần chạy này.

#### Scenario: Khởi động lại app
- **WHEN** web2 đã nối lại 3 lần và người dùng khởi động lại ứng dụng
- **THEN** popover hiện "Reconnects: 0" cho web2

### Requirement: Trạng thái kết nối chỉ ở status bar
Trạng thái kết nối, độ trễ, số lần nối lại và lỗi cuối SHALL chỉ hiện trên status bar và trong popover Connection Details mở từ status bar. Sidebar, tab, dialog Server và dialog thêm thành viên SHALL NOT hiện trạng thái kết nối live.

#### Scenario: Sidebar
- **WHEN** web2 đang Reconnecting
- **THEN** sidebar hiện `web2:/app/app.log` như bình thường, không có biểu tượng hay màu trạng thái kết nối
