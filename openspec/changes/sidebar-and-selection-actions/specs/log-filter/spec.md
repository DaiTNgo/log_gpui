## ADDED Requirements

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

## MODIFIED Requirements

### Requirement: Time Range
Hệ thống SHALL cung cấp Filter khoảng thời gian cho `timestamp` với hai ô Từ và Đến. Mỗi ô SHALL nhận mọi định dạng thời gian mà ứng dụng đọc được từ dòng log, và SHALL có placeholder lấy từ timestamp đầu tiên của view. Ô trống nghĩa là không giới hạn phía đó. Hệ thống SHALL gợi ý giá trị nhỏ nhất và lớn nhất, và SHALL so sánh theo đồng hồ của file, không đổi múi giờ.

Section Filter SHALL có các preset 5m, 1h, 24h và All:
- 5m, 1h và 24h SHALL đặt Từ bằng timestamp mới nhất của view trừ đi khoảng đó, và để trống Đến. Giá trị được tính một lần lúc bấm.
- All SHALL xoá Time Range.

Dưới các preset là một dòng tóm tắt Time Range đang đặt. Bấm vào dòng đó SHALL mở hai ô Từ và Đến xếp dọc, mỗi ô có nhãn cố định.

#### Scenario: Giới hạn hai phía
- **WHEN** người dùng nhập Từ `10:00:00` và Đến `10:05:00`
- **THEN** chỉ các dòng có timestamp trong khoảng đó được hiển thị

#### Scenario: Một phía
- **WHEN** người dùng chỉ nhập Từ `10:00:00`
- **THEN** mọi dòng từ `10:00:00` trở đi được hiển thị

#### Scenario: Định dạng khác cột hiển thị
- **WHEN** cột TIMESTAMP hiện `2026-10-18T10:00:00+07:00` và người dùng nhập Từ `2026-10-18 10:00:00`
- **THEN** Time Range được chấp nhận và lọc từ thời điểm đó

#### Scenario: Preset neo theo log
- **WHEN** dòng mới nhất của view có timestamp `2026-10-18 08:40:00` và người dùng bấm 1h
- **THEN** Từ là `2026-10-18 07:40:00`, Đến trống, dù đồng hồ máy đang là ngày khác

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
