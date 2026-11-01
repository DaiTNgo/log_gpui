## ADDED Requirements

### Requirement: Header phạm vi
Context sidebar SHALL có một dòng header ghi tên view đang áp dụng (tên file của File View, hoặc file đầu và số file còn lại của Log Space) và loại view (Parsed hay Raw).

#### Scenario: Log Space hai file
- **WHEN** view đang mở là Log Space có `untitled.log` và `untitled-2.log` ở Parsed View
- **THEN** header ghi `untitled.log + 1 file · Parsed`

### Requirement: Inspector ghim ở đáy
Inspector SHALL nằm ở đáy context sidebar, ngoài vùng cuộn của các section khác, và SHALL luôn hiện khi chưa bị gập, dù các section phía trên dài tới đâu. Một thanh chia giữa vùng cuộn và Inspector SHALL kéo được để đổi chiều cao Inspector. Chiều cao này SHALL được nhớ. Inspector và vùng cuộn SHALL mỗi bên cao tối thiểu 120px.

#### Scenario: Mở form dài
- **WHEN** Inspector đang hiện một dòng và người dùng mở form New rule cùng một thẻ Filter có 30 giá trị
- **THEN** Inspector vẫn ở nguyên chỗ và đủ chiều cao, chỉ vùng phía trên cuộn

#### Scenario: Đổi chiều cao
- **WHEN** người dùng kéo thanh chia lên 100px rồi khởi động lại ứng dụng
- **THEN** Inspector có chiều cao mới

### Requirement: Đổi độ rộng sidebar
Cạnh trái context sidebar SHALL kéo được để đổi độ rộng, trong khoảng 280–560px, mặc định 320px. Độ rộng SHALL được nhớ.

#### Scenario: Kéo rộng
- **WHEN** người dùng kéo cạnh trái sang trái 120px
- **THEN** sidebar rộng 440px và hai ô Time Range hiện đủ giá trị

### Requirement: Section dạng tóm tắt
Header của mỗi section SHALL có tên section, phần tóm tắt bên phải và nút hành động chính:
- Filter: số điều kiện đang bật, hoặc "paused"; nút "+".
- Highlights: số rule; nút "+".
- Columns: "<số cột đang hiện> of <tổng> shown"; nút tìm field.

Section SHALL NOT có thêm một dòng riêng "+ Add filter" hay "+ Add rule". Columns SHALL gập mặc định khi chưa có trạng thái lưu.

#### Scenario: Thêm filter từ header
- **WHEN** người dùng bấm "+" trên header Filter
- **THEN** danh sách cột để lọc mở ra trong section Filter

#### Scenario: Columns gập lần đầu
- **WHEN** người dùng mở context sidebar lần đầu trên một view
- **THEN** section Columns gập và header ghi số cột đang hiện

### Requirement: Inspector chế độ Fields và Raw
Inspector SHALL có hai chế độ:
- **Fields**: mỗi cột có giá trị của dòng đang chọn là một hàng `tên → giá trị`.
- **Raw**: text lưu trữ của dòng đang chọn, kèm Continuation Line, đúng từng ký tự, chọn và copy được.

Chế độ đã chọn SHALL được nhớ. Khi dòng không có cột nào parse được (Raw View không có Log Format, hoặc dòng không khớp format), Inspector SHALL chỉ có Raw. Header của Inspector SHALL có số dòng, nút Copy (chép text lưu trữ của dòng) và nút ↑↓ để chọn dòng trước hoặc sau trong view đang hiện.

#### Scenario: Fields của dòng JSON
- **WHEN** người dùng chọn một dòng có các field `event`, `method`, `status` ở chế độ Fields
- **THEN** Inspector hiện `event`, `method` và `status`, mỗi field một hàng kèm giá trị

#### Scenario: Raw giữ nguyên text
- **WHEN** người dùng chuyển Inspector sang Raw trên một dòng có 4 dòng stack trace
- **THEN** Inspector hiện dòng đó và 4 dòng stack trace, đúng như trong file

#### Scenario: Sang dòng kế tiếp
- **WHEN** Inspector đang hiện dòng 42 và người dùng bấm ↓
- **THEN** dòng hiển thị kế tiếp được chọn trong view và Inspector hiện dòng đó

### Requirement: Hành động trên field của Inspector
Ở chế độ Fields, khi trỏ chuột vào một hàng field của cột lọc được, Inspector SHALL hiện ba nút:
- Show only và Hide, tạo điều kiện như Selection Menu khi chọn trọn giá trị ô.
- Highlight rows, mở form New rule đã điền cột và giá trị.

Các nút SHALL chỉ có trong Parsed View.

#### Scenario: Ẩn từ Inspector
- **WHEN** người dùng trỏ vào field `source` = `backend` và bấm Hide
- **THEN** Filter có `source` ở Hide Mode với `backend`, và các dòng `backend` bị ẩn
