## ADDED Requirements

### Requirement: Nhớ thư mục cuối theo Server
Hệ thống SHALL nhớ, cho từng Server, thư mục cuối cùng người dùng mở trong bộ chọn file, và SHALL mở bộ chọn ở thư mục đó khi người dùng chọn lại Server ấy, ở mọi Project và sau khi khởi động lại ứng dụng. Khi chưa có thư mục nhớ, hoặc thư mục đó không còn tồn tại hay không đọc được, hệ thống SHALL mở thư mục home của Server và SHALL hiện một dòng báo thư mục cũ không mở được.

#### Scenario: Mở lại ở thư mục cũ
- **WHEN** người dùng duyệt tới `/var/log/nginx/` trên "web1", đóng bộ chọn, rồi mở lại bộ chọn ở một Project khác và chọn "web1"
- **THEN** bộ chọn liệt kê `/var/log/nginx/`

#### Scenario: Thư mục cũ đã bị xoá
- **WHEN** thư mục nhớ của "web1" là `/tmp/debug/` và thư mục đó đã bị xoá trên Server
- **THEN** bộ chọn mở thư mục home của "web1" và hiện "/tmp/debug/ is no longer available"

#### Scenario: Mỗi Server một thư mục
- **WHEN** thư mục nhớ của "web1" là `/var/log/app/` và của "db" là `/var/lib/pgsql/log/`
- **THEN** chuyển giữa "web1" và "db" trong bộ chọn mở đúng thư mục của từng Server

### Requirement: Recent Paths
Hệ thống SHALL giữ cho từng Server danh sách Recent Paths gồm tối đa 10 đường dẫn file và Live Glob được thêm gần nhất từ Server đó, mới nhất ở trên, không trùng. Bộ chọn SHALL hiện Recent Paths của Server đang chọn, và bấm một mục SHALL đưa nó vào danh sách chờ thêm như khi gõ lại đúng đường dẫn đó. Người dùng SHALL xoá được từng mục hoặc cả danh sách. Khi một Server bị xoá, Recent Paths và thư mục nhớ của nó SHALL bị xoá theo.

#### Scenario: Thêm lại đường dẫn đã dùng
- **WHEN** hôm qua người dùng đã thêm `web1:/app/logs/app.log` và hôm nay mở bộ chọn với "web1"
- **THEN** `/app/logs/app.log` có trong Recent Paths, và bấm vào nó thì file được đưa vào danh sách chờ thêm

#### Scenario: Giới hạn 10 mục
- **WHEN** Recent Paths của "web1" đã có 10 mục và người dùng thêm một đường dẫn mới
- **THEN** đường dẫn mới đứng đầu và mục cũ nhất bị bỏ

#### Scenario: Đường dẫn dùng lại lên đầu
- **WHEN** `/var/log/syslog` đang là mục thứ 5 và người dùng thêm lại nó
- **THEN** `/var/log/syslog` chuyển lên đầu và chỉ xuất hiện một lần

### Requirement: Lọc danh sách khi gõ
Bộ chọn SHALL có ô lọc cho danh sách thư mục đang mở. Khi gõ, danh sách SHALL chỉ còn các mục có tên chứa chuỗi đã gõ, không phân biệt hoa thường; khi chuỗi có ký tự glob (`*`, `?`, `[`), danh sách SHALL chỉ còn các mục có tên khớp glob đó. Lọc SHALL NOT gửi yêu cầu mới tới Server. Mục `..` SHALL luôn hiện. Ô lọc SHALL được xoá khi mở thư mục khác. Ô lọc chỉ dùng để chọn file và SHALL NOT tạo Live Glob.

#### Scenario: Gõ một phần tên
- **WHEN** `/var/log/` có `syslog`, `auth.log`, `nginx/` và người dùng gõ `LOG` vào ô lọc
- **THEN** danh sách còn `..`, `syslog`, `auth.log`, và `nginx/` bị ẩn

#### Scenario: Lọc bằng glob rồi tích
- **WHEN** người dùng gõ `app-*.log` vào ô lọc, danh sách còn `app-1.log` và `app-2.log`, và người dùng tích cả hai
- **THEN** hai file đó được đưa vào danh sách chờ thêm như hai Remote File thường, không có Live Glob nào được tạo

### Requirement: Sắp xếp danh sách
Bộ chọn SHALL cho sắp xếp danh sách theo tên, thời gian sửa hoặc kích thước, tăng hoặc giảm, bằng cách bấm tiêu đề cột; bấm lại cùng cột SHALL đảo chiều. Thư mục SHALL luôn đứng trước file, và thư mục SHALL được sắp theo tên khi sắp theo kích thước. Mặc định là theo tên tăng dần. Cách sắp xếp đã chọn SHALL được nhớ cho cả ứng dụng.

#### Scenario: File mới sửa lên đầu
- **WHEN** người dùng bấm tiêu đề "Modified" hai lần
- **THEN** các file được sắp từ mới sửa nhất tới cũ nhất, sau các thư mục

#### Scenario: Nhớ cách sắp xếp
- **WHEN** người dùng đã sắp theo kích thước giảm dần rồi đóng và mở lại bộ chọn
- **THEN** danh sách vẫn sắp theo kích thước giảm dần

### Requirement: Lối tắt thư mục
Bộ chọn SHALL có lối tắt mở ngay thư mục home của Server và `/var/log`. Khi `/var/log` không tồn tại hoặc không đọc được trên Server, hệ thống SHALL báo lỗi trong bộ chọn và giữ nguyên thư mục đang mở.

#### Scenario: Tới /var/log
- **WHEN** bộ chọn đang ở `/home/deploy/` trên "web1" và người dùng bấm lối tắt `/var/log`
- **THEN** bộ chọn liệt kê `/var/log/` của "web1"

### Requirement: Mở thư mục từ ô đường dẫn
Khi người dùng gõ vào ô đường dẫn một đường dẫn tuyệt đối tới thư mục rồi xác nhận, bộ chọn SHALL mở thư mục đó trong danh sách thay vì báo lỗi, và SHALL NOT thêm gì vào danh sách chờ.

#### Scenario: Gõ thư mục
- **WHEN** người dùng gõ `/opt/app/logs` và nhấn Enter, và đó là thư mục
- **THEN** bộ chọn liệt kê `/opt/app/logs/` và ô đường dẫn được xoá

### Requirement: Cùng đường dẫn trên nhiều Server
Khi thêm một đường dẫn hoặc Live Glob từ ô đường dẫn hay từ Recent Paths, bộ chọn SHALL cho tích thêm các Server khác ngoài Server đang chọn, và SHALL đưa đúng đường dẫn đó vào danh sách chờ trên từng Server đã tích. Mỗi Server SHALL được kiểm tra riêng theo các luật thêm Remote File (kết nối được, thư mục cha tồn tại); Server bị từ chối SHALL được báo riêng và SHALL NOT chặn các Server khác. Lựa chọn Server được tích SHALL được giữ cho lần thêm tiếp theo trong cùng phiên của bộ chọn.

#### Scenario: Một đường dẫn, ba Server
- **WHEN** người dùng gõ `/app/logs/app.log`, tích "web1", "web2", "web3" và bấm Add
- **THEN** danh sách chờ có `web1:/app/logs/app.log`, `web2:/app/logs/app.log` và `web3:/app/logs/app.log`

#### Scenario: Một Server thiếu thư mục
- **WHEN** người dùng thêm `/app/logs/app.log` trên "web1" và "web2", và "web2" không có `/app/logs/`
- **THEN** `web1:/app/logs/app.log` vào danh sách chờ, và bộ chọn báo "web2: The folder /app/logs does not exist"

#### Scenario: Glob trên nhiều Server
- **WHEN** người dùng gõ `/var/log/app/*.log` và tích "web1", "web2"
- **THEN** danh sách chờ có hai Live Glob `web1:/var/log/app/*.log` và `web2:/var/log/app/*.log`
