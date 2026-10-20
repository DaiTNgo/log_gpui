## ADDED Requirements

### Requirement: Make column từ phần chữ bôi đen
Menu chuột phải trên một ô của Parsed View, và trên dòng trong Focus Pane, SHALL có mục "Make column…" khi đang có vùng chọn nằm trọn trong một dòng. Mục này SHALL mở dialog New Column cho đoạn được chọn. Mục này SHALL NOT có khi vùng chọn trải qua nhiều dòng, hoặc khi ô là `_` của Continuation Line.

#### Scenario: Mở dialog
- **WHEN** người dùng bôi đen `alice` trong `user=alice order=8812`, chuột phải rồi chọn "Make column…"
- **THEN** dialog New Column mở ra cho đoạn `alice`

#### Scenario: Vùng chọn nhiều dòng
- **WHEN** vùng chọn trải từ hàng 3 tới hàng 5
- **THEN** menu chuột phải không có "Make column…"

### Requirement: Đoán cách khớp
Dialog New Column SHALL đưa ra các cách khớp được đoán từ đoạn đã chọn, mỗi cách hiện bằng mô tả dễ đọc (ví dụ "after `user=`", "bracket group 3", "word 2 of message") và không hiện regex. Hệ thống SHALL chỉ đưa ra cách khớp mà trên chính dòng đó lấy ra đúng đoạn đã chọn. Cách khớp có Coverage cao nhất trên mẫu dòng của view SHALL được chọn sẵn. Cách khớp neo theo key đứng ngay trước đoạn chọn (`key=`, `key:`, `"key":`) SHALL đứng trước cách khớp theo vị trí khi Coverage bằng nhau.

#### Scenario: Có key
- **WHEN** đoạn chọn là `8812` trong `order=8812 took 34ms`
- **THEN** dialog có cách khớp "after `order=`" và nó được chọn sẵn

#### Scenario: Trong ngoặc
- **WHEN** đoạn chọn là `worker-3` trong `2026-10-20 10:00:01 [INFO] [api] [worker-3] started`
- **THEN** dialog có cách khớp "bracket group 3"

#### Scenario: Chỉ khớp được theo vị trí
- **WHEN** đoạn chọn là `GET` trong message `GET /health 200`
- **THEN** dialog có cách khớp "word 1 of message"

### Requirement: Gợi ý Name
Ô Name SHALL được điền sẵn bằng key đứng ngay trước đoạn chọn nếu có. Nếu không có key, ô Name SHALL được điền bằng tên đoán theo hình dạng giá trị (`ip`, `uuid`, `duration`), hoặc để trống. Hệ thống SHALL NOT cho tạo khi Name trống, hoặc khi Name trùng `timestamp`, `level`, `module`, `message`, `source` hay `stream`. Lỗi của Name SHALL chỉ hiện khi người dùng ngừng gõ 500 ms hoặc rời khỏi ô. Khi Name trùng một Field Column đã có, dialog SHALL báo rằng giá trị sẽ gộp vào cột đó.

#### Scenario: Có key
- **WHEN** đoạn chọn là `alice` trong `user=alice`
- **THEN** ô Name hiện `user`

#### Scenario: Hình dạng IP
- **WHEN** đoạn chọn là `10.0.4.17` và không có key đứng trước
- **THEN** ô Name hiện `ip`

#### Scenario: Trùng cột cố định
- **WHEN** người dùng gõ Name là `level`
- **THEN** nút Add column bị tắt và dialog báo tên dành cho cột có sẵn

#### Scenario: Trùng Field Column
- **WHEN** đã có cột `user` và người dùng gõ Name là `user`
- **THEN** dialog báo "Values will join column user" và vẫn cho tạo

### Requirement: Preview bằng dòng thật trong dialog New Column
Dialog New Column SHALL hiện Preview cho cách khớp đang chọn, gồm:
- Tối đa 3 dòng thật khớp, mỗi dòng có đoạn được lấy tô sáng. Dòng chứa đoạn người dùng đã bôi đen SHALL luôn đứng đầu.
- Tối đa 1 dòng thật không khớp, đánh dấu "not matched", nếu mẫu dòng của view có dòng như vậy.
- Số dòng khớp trên tổng số Parent Line của view, và tối đa 5 giá trị khác nhau thường gặp nhất.

Dòng dài SHALL được cắt quanh đoạn tô sáng, có `…` ở đầu bị cắt. Bấm vào một dòng trong Preview SHALL đưa view tới dòng đó mà không đóng dialog. Preview SHALL cập nhật khi người dùng đổi cách khớp hoặc sửa regex.

#### Scenario: Dòng khớp
- **WHEN** cách khớp "after `user=`" khớp 1.204 trên 1.530 dòng với các giá trị `alice`, `bob`, `svc-billing`
- **THEN** Preview hiện 3 dòng có giá trị `user` được tô sáng, rồi "1,204 of 1,530 lines (78%)" và ba giá trị đó

#### Scenario: Dòng không khớp
- **WHEN** cách khớp "word 3 of message" và mẫu dòng có dòng `GET /health` chỉ có hai từ
- **THEN** Preview có thêm một dòng `GET /health` đánh dấu "not matched"

#### Scenario: Mọi dòng đều khớp
- **WHEN** cách khớp đang chọn khớp mọi Parent Line của view
- **THEN** Preview không có dòng "not matched"

#### Scenario: Nhảy tới dòng
- **WHEN** người dùng bấm vào dòng thứ hai trong Preview
- **THEN** view cuộn tới dòng đó và dialog New Column vẫn mở

### Requirement: Regex tự viết
Dialog New Column SHALL có "Edit regex", mở regex của cách khớp đang chọn để người dùng sửa. Regex SHALL có đúng một capture group. Regex không hợp lệ, không có capture group, hoặc có nhiều hơn một capture group SHALL làm nút Add column bị tắt ngay. Trong lúc regex chưa hợp lệ, Preview SHALL giữ kết quả của regex hợp lệ gần nhất, hiện mờ kèm nhãn "Showing last valid pattern". Lỗi SHALL chỉ hiện dưới ô nhập khi người dùng ngừng gõ 500 ms hoặc rời khỏi ô, gồm icon và câu nói cách sửa, và SHALL ẩn đi khi người dùng gõ tiếp.

#### Scenario: Sửa regex
- **WHEN** người dùng sửa regex thành `user=(\w+)`
- **THEN** Preview cập nhật theo regex mới

#### Scenario: Đang gõ dở
- **WHEN** regex đang là `user=(\w+)` và người dùng gõ tiếp thành `user=(\w+` rồi gõ tiếp ngay
- **THEN** ô nhập chưa báo lỗi, nút Add column bị tắt, và Preview vẫn hiện kết quả của `user=(\w+)` ở dạng mờ

#### Scenario: Ngừng gõ khi regex sai
- **WHEN** người dùng gõ `(\w+)=(\w+)` rồi ngừng gõ 500 ms
- **THEN** ô regex hiện icon lỗi và câu báo cần đúng một capture group, và nút Add column bị tắt

#### Scenario: Gõ tiếp sau lỗi
- **WHEN** ô regex đang báo lỗi và người dùng gõ thêm một ký tự
- **THEN** lỗi ẩn đi cho tới lần ngừng gõ tiếp theo

### Requirement: Extraction Rule thuộc Project
Bấm Add column SHALL tạo một Extraction Rule gồm Name và regex. Extraction Rule SHALL thuộc Project, SHALL áp cho mọi Log Space và File View của Project, và SHALL được nhớ khi mở lại Project. Extraction Rule SHALL chạy trên cả dòng.

#### Scenario: Áp cho Log Space khác
- **WHEN** người dùng tạo Extraction Rule `user` trong Log Space A rồi mở Log Space B có dòng chứa `user=bob`
- **THEN** Parsed View của Log Space B có cột `user` hiện `bob`

#### Scenario: Mở lại Project
- **WHEN** người dùng đóng rồi mở lại Project có Extraction Rule `user`
- **THEN** cột `user` vẫn hiện

### Requirement: Nhóm Rules trong mục Columns
Nhóm *Rules* của mục Columns SHALL liệt kê các Extraction Rule của Project, mỗi rule có Name, mô tả cách khớp, số dòng có giá trị trong view, công tắc bật/tắt và menu `⋯` gồm Edit, Rename, Delete. Thứ tự trong nhóm *Rules* SHALL là thứ tự ưu tiên giữa các rule cùng tên. Vị trí vẽ của cột SHALL theo thứ tự kéo trong mục Columns. Tắt một rule SHALL ngừng đọc và ẩn cột của nó mà không xoá rule. Edit SHALL mở lại dialog New Column với Name và regex của rule.

#### Scenario: Tắt rule
- **WHEN** người dùng tắt Extraction Rule `user` trong mục Columns
- **THEN** cột `user` biến mất khỏi Parsed View và rule vẫn còn trong danh sách

#### Scenario: Đổi thứ tự
- **WHEN** mục Columns có `user` rồi `order`, và người dùng kéo `order` lên trước
- **THEN** cột `order` nằm bên trái cột `user`

#### Scenario: Xoá rule
- **WHEN** người dùng xoá Extraction Rule `user` mà Filter của view đang có điều kiện `user` = `alice`
- **THEN** cột `user` biến mất, và mục Filter của `user` vẫn còn điều kiện cùng dòng chú thích "column no longer read"

### Requirement: Đổi rule không chặn view
Khi Extraction Rule được thêm, sửa, bật hay tắt, hệ thống SHALL đọc lại giá trị ở nền, và view SHALL vẫn cuộn, tìm và chọn được trong lúc đó. Column Header của cột đang được đọc lại SHALL hiện tiến độ. Trong Remote Log Space, dòng mới tới SHALL dùng rule mới ngay.

#### Scenario: File lớn
- **WHEN** người dùng thêm Extraction Rule trên Log Space 1.000.000 dòng
- **THEN** view vẫn cuộn được, và Column Header của cột mới hiện tiến độ cho tới khi đọc xong
