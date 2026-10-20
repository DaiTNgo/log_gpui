## ADDED Requirements

### Requirement: Log Format bằng Format Token
Hệ thống SHALL cho tạo Log Format gồm một tên và một template. Template SHALL hiểu các cú pháp sau:
- `%{TOKEN}`: khớp mà không lấy giá trị.
- `%{TOKEN:field}`: lấy giá trị vào trường `field`.
- Regex thô, trong đó named group `(?<field>…)` cũng thành trường.
- Một chuỗi khoảng trắng trong template khớp một hay nhiều khoảng trắng.
- Ký tự khác khớp đúng chính nó.

Template SHALL khớp cả dòng, sau khi bỏ khoảng trắng ở đầu dòng. Format Token SHALL gồm ít nhất `TIMESTAMP`, `LEVEL`, `WORD`, `NOTSPACE`, `INT`, `NUMBER`, `IP`, `UUID`, `HEX`, `QUOTED`, `DATA` và `GREEDY`.

#### Scenario: Template đơn giản
- **WHEN** Log Format có template `%{TIMESTAMP:timestamp} [%{LEVEL:level}] %{IP:client} %{GREEDY:message}` và dòng là `2026-10-20 10:00:01 [INFO] 10.0.4.17 GET /health`
- **THEN** trường `client` là `10.0.4.17` và MESSAGE là `GET /health`

#### Scenario: Named group
- **WHEN** template chứa `took (?<took>\d+ms)` và dòng chứa `took 34ms`
- **THEN** trường `took` là `34ms`

#### Scenario: Token không có
- **WHEN** template chứa `%{FOO:x}`
- **THEN** trình soạn báo `FOO` không phải Format Token và Log Format không lưu được

### Requirement: Trường cố định của Log Format
Trường tên `timestamp`, `level`, `module` và `message` của Log Format SHALL điền vào cột TIMESTAMP, LEVEL, MODULE và MESSAGE. Thời gian của trường `timestamp` SHALL được dùng để xếp theo thời gian, cho Time Range và để nhận diện Continuation Line, giống timestamp do Auto-detect đọc. Các trường khác SHALL thành Field Column theo thứ tự trong template.

#### Scenario: Sắp xếp theo thời gian
- **WHEN** hai file của một Log Space dùng một Log Format có `%{TIMESTAMP:timestamp}`
- **THEN** các hàng của hai file được xếp xen theo thời gian của trường đó

#### Scenario: Level Marker
- **WHEN** trường `level` của dòng 7 là `E`
- **THEN** Gutter của dòng 7 có Level Marker lỗi

### Requirement: Dòng không khớp Log Format
Một dòng không khớp Log Format đang dùng SHALL được đọc bằng Auto-detect, và SHALL vẫn là một hàng với đầy đủ text ở MESSAGE. Ô Field Column của Log Format ở dòng đó SHALL trống.

#### Scenario: Stack trace
- **WHEN** dòng 11 là `    at PaymentService.charge(...)` và không khớp Log Format
- **THEN** dòng 11 vẫn là Continuation Line của dòng 10 như khi không có Log Format

### Requirement: Trình soạn Log Format có xem trước
Trình soạn Log Format SHALL xem trước template trên tối đa 20 dòng thật của view đang mở, tô mỗi trường một màu khác nhau, và đánh dấu dòng không khớp. Trình soạn SHALL hiện tỉ lệ dòng khớp trên mẫu dòng của view, và SHALL cho nhảy tới dòng không khớp đầu tiên. Xem trước SHALL cập nhật khi người dùng gõ, mỗi khi template hợp lệ. Trong lúc template chưa hợp lệ, xem trước SHALL giữ kết quả của template hợp lệ gần nhất, hiện mờ kèm nhãn "Showing last valid pattern", và nút Save SHALL bị tắt. Lỗi SHALL chỉ hiện khi người dùng ngừng gõ 500 ms hoặc rời khỏi ô, gồm icon, câu nói cách sửa, và gạch chân đúng phần sai của template. Lỗi SHALL ẩn đi khi người dùng gõ tiếp.

#### Scenario: Gõ template
- **WHEN** người dùng gõ thêm `%{IP:client}` vào template
- **THEN** các dòng xem trước tô phần IP bằng một màu mới, và tỉ lệ khớp cập nhật

#### Scenario: Đang gõ dở token
- **WHEN** template hợp lệ và người dùng gõ thêm `%{IP` rồi gõ tiếp ngay
- **THEN** chưa có lỗi nào hiện, nút Save bị tắt, và xem trước giữ kết quả của template trước đó ở dạng mờ

#### Scenario: Ngừng gõ khi template sai
- **WHEN** template chứa `%{IP` chưa đóng và người dùng ngừng gõ 500 ms
- **THEN** phần `%{IP` được gạch chân, và trình soạn hiện icon lỗi cùng câu báo token chưa đóng `}`

#### Scenario: Dòng không khớp
- **WHEN** 3 trên 20 dòng xem trước không khớp template
- **THEN** 3 dòng đó được đánh dấu không khớp và tỉ lệ khớp hiện `85%`

### Requirement: Mẫu Log Format có sẵn
Trình soạn SHALL có các mẫu dựng sẵn ít nhất cho nginx/Apache combined, Python `logging` mặc định và log4j `PatternLayout` mặc định. Chọn một mẫu SHALL chép template vào trình soạn để người dùng sửa, không gắn mẫu vào Log Format.

#### Scenario: Dùng mẫu nginx
- **WHEN** người dùng chọn mẫu nginx combined trên file access log của nginx
- **THEN** template được điền và tỉ lệ khớp hiện `100%`

### Requirement: Thư viện Log Format chung cho cả app
Log Format SHALL được lưu ở cấu hình app và dùng được ở mọi Project. Người dùng SHALL tạo, sửa, nhân bản và xoá Log Format từ thư viện. Xoá Log Format đang được chọn ở một Log Space hay Source File SHALL được hỏi lại trước, kèm danh sách nơi đang dùng. Sau khi xoá, những nơi đó SHALL trở về Auto-detect.

#### Scenario: Dùng ở Project khác
- **WHEN** người dùng tạo Log Format `billing` trong Project A rồi mở Project B
- **THEN** `billing` có trong danh sách chọn Log Format của Project B

#### Scenario: Xoá Log Format đang dùng
- **WHEN** người dùng xoá Log Format `billing` đang được chọn cho Log Space `prod`
- **THEN** app hỏi lại và nêu `prod`; nếu đồng ý thì `prod` trở về Auto-detect

### Requirement: Chọn Log Format
Người dùng SHALL chọn được Log Format cho một Log Space, và cho riêng một Source File trong Log Space, từ ô chọn ở đầu nhóm *From Log Format* của mục Columns hoặc từ menu của file trong sidebar. Lựa chọn SHALL thuộc Project. Hệ thống SHALL dùng Log Format cho mỗi file theo thứ tự: chọn cho Source File, rồi chọn cho Log Space, rồi glob tên file của Log Format, rồi Auto-detect.

#### Scenario: Chọn cho file
- **WHEN** Log Space dùng Log Format `app` và người dùng chọn Log Format `nginx` cho riêng `access.log`
- **THEN** `access.log` được đọc bằng `nginx`, các file khác bằng `app`

#### Scenario: Về Auto-detect
- **WHEN** người dùng chọn Auto-detect cho Log Space
- **THEN** các file không có lựa chọn riêng được đọc như trước khi có Log Format

### Requirement: Tự áp Log Format theo glob tên file
Một Log Format SHALL có thể có một danh sách glob tên file. Một file không được chọn Log Format riêng, và thuộc Log Space không được chọn Log Format, SHALL dùng Log Format đầu tiên trong thư viện có glob khớp tên file. Glob SHALL chỉ so với tên file, không so đường dẫn.

#### Scenario: Glob khớp
- **WHEN** Log Format `nginx` có glob `access*.log` và người dùng thêm `access-2026-10-20.log` vào Log Space chưa chọn Log Format
- **THEN** file đó được đọc bằng `nginx`

#### Scenario: Chọn tay thắng glob
- **WHEN** Log Space đã chọn Log Format `app` và file `access.log` khớp glob của `nginx`
- **THEN** `access.log` được đọc bằng `app`

### Requirement: Đổi Log Format đọc lại file
Đổi Log Format của một file SHALL đọc lại file ở nền và dựng lại thứ tự theo thời gian của view. Marker SHALL được giữ. Filter và Highlight Rule trên trường không còn được đọc SHALL được giữ nhưng không khớp dòng nào.

#### Scenario: Giữ Marker
- **WHEN** dòng 42 có Marker và người dùng đổi Log Format của file đó
- **THEN** dòng 42 vẫn có Marker sau khi đọc lại

### Requirement: Nhóm From Log Format trong mục Columns
Nhóm *From Log Format* của mục Columns SHALL có ở đầu nhóm ô chọn Log Format của Log Space đang mở, và SHALL liệt kê các trường của Log Format đang dùng theo thứ tự trong template, trừ `timestamp`, `level`, `module` và `message`. Mỗi trường SHALL có công tắc hiện/ẩn như mọi Field Column.

#### Scenario: Chọn từ mục Columns
- **WHEN** người dùng chọn Log Format `nginx` ở đầu nhóm *From Log Format*
- **THEN** Log Space được đọc lại bằng `nginx` và nhóm liệt kê các trường của `nginx`
