## MODIFIED Requirements

### Requirement: Phát hiện cột và giá trị khi nạp file
Hệ thống SHALL phát hiện các cột của file log và các giá trị riêng biệt của từng cột trong lúc Indexing, và SHALL dùng chúng làm nguồn cho Filter thay cho danh sách cố định. Trong lúc Indexing, danh sách giá trị và số đếm SHALL tăng dần theo các dòng đã index, và picker SHALL cho biết file chưa được đọc hết.

#### Scenario: Giá trị lấy từ file
- **WHEN** file log có `module` nhận các giá trị `API`, `DB`, `SEARCH`
- **THEN** Filter của cột `module` liệt kê đúng ba giá trị đó và không liệt kê module nào khác

#### Scenario: Trường JSON
- **WHEN** file log là JSON có trường `region` với các giá trị `us`, `eu`
- **THEN** `region` xuất hiện là một cột để lọc

#### Scenario: Giá trị trong lúc Indexing
- **WHEN** người dùng mở picker của cột `module` khi Indexing mới được 50%, và giá trị `SEARCH` chỉ xuất hiện ở nửa sau file
- **THEN** picker chưa có `SEARCH` và cho biết file chưa đọc hết; khi Indexing xong, `SEARCH` xuất hiện với số đếm đúng

### Requirement: Add value
Hệ thống SHALL cho phép người dùng nhập thêm một Custom Value bên dưới danh sách checkbox của cột. Custom Value SHALL khớp các dòng có giá trị ở cột đó chứa chuỗi đã nhập, không phân biệt hoa thường theo Unicode và không phải regex. Khi việc so khớp cần đọc text của các dòng, hệ thống SHALL chạy nó như một Scan ở nền, và Focus Pane SHALL nhận dòng khớp dần cùng tiến độ.

#### Scenario: Khớp chứa chuỗi
- **WHEN** người dùng thêm Custom Value `pay` ở cột `module`
- **THEN** các dòng có `module` là `PAYMENT` hoặc `paypal` được hiển thị

#### Scenario: Không regex
- **WHEN** người dùng thêm Custom Value `a.c` ở cột `module`
- **THEN** chỉ các dòng có `module` chứa đúng chuỗi `a.c` được hiển thị

#### Scenario: Custom Value trên cột message của file lớn
- **WHEN** người dùng thêm Custom Value `timeout` ở cột `message` trên file 3.000.000 dòng
- **THEN** Focus Pane hiện dần các dòng khớp cùng tiến độ, và UI không bị chặn
