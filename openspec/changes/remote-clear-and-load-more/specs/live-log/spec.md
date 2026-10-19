## MODIFIED Requirements

### Requirement: Live Window
Hệ thống SHALL giữ cho mỗi Remote File tối đa số dòng mới nhất bằng Live Window, một giá trị chung cho cả ứng dụng, mặc định 100.000, chỉnh trong Settings, trừ khi file có Extended Window sau Load more; khi đó file SHALL giữ mọi dòng từ dòng cũ nhất đã nạp trở về sau, tối đa Full Read Limit. Khi có dòng mới làm vượt giới hạn của file, dòng cũ nhất của file đó SHALL rời view. Hệ thống SHALL NOT tự nạp dòng vượt quá Live Window; chỉ Load more SHALL làm việc đó. Live Window SHALL NOT áp dụng cho Imported Log Space. Remote Log Space SHALL không giới hạn thời gian kết nối, và các dòng đã nhận, việc Clear và Extended Window SHALL NOT được giữ sau khi đóng Log Space.

#### Scenario: Window đầy
- **WHEN** `web1:/app/app.log` đang giữ 100.000 dòng và nhận thêm 10 dòng
- **THEN** 10 dòng cũ nhất của file đó rời view, và dòng của các file khác không bị ảnh hưởng

#### Scenario: Đổi Live Window
- **WHEN** người dùng giảm Live Window từ 100.000 xuống 20.000
- **THEN** mỗi Remote File đang mở mà không có Extended Window chỉ còn giữ 20.000 dòng mới nhất

#### Scenario: Extended Window không bị cắt theo Live Window
- **WHEN** `web1:/app/app.log` có Extended Window giữ 150.000 dòng và có thêm dòng mới
- **THEN** không dòng nào của file đó rời view cho đến khi file giữ quá Full Read Limit

### Requirement: Dòng rời Live Window
Khi một dòng rời Live Window, hoặc rời Extended Window vì file giữ quá Full Read Limit, hệ thống SHALL bỏ Marker của dòng đó, bỏ dòng khỏi số đếm của Filter, và vẽ một Break ở đầu phần dòng của file đó nói rằng các dòng cũ hơn đã rời, kèm nút Load more. Nếu dòng hiện tại của một Focus Tab rời, dòng hiện tại SHALL chuyển sang dòng cũ nhất còn giữ của Log Space. Dòng bị Clear SHALL được xử lý theo cùng cách, trừ Break là "cleared" và dòng hiện tại SHALL chuyển sang dòng còn giữ ngay sau nó.

#### Scenario: Marker rời window
- **WHEN** dòng 1.900.001 có Marker và rời Live Window
- **THEN** Marker đó bị bỏ và không hiện lại

#### Scenario: Dòng hiện tại rời window
- **WHEN** dòng hiện tại của Focus Tab 1 rời Live Window
- **THEN** dòng hiện tại của Focus Tab 1 là dòng cũ nhất còn giữ

#### Scenario: Break có Load more
- **WHEN** các dòng cũ nhất của `web1:/app/app.log` rời Live Window
- **THEN** Break "web1:/app/app.log older lines left the window" ở đầu phần dòng của file có nút "Load more"
