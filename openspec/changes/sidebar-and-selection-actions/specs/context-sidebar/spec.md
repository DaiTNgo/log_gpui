## ADDED Requirements

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
Header của mỗi section SHALL là một dải có nền riêng, tách rõ khỏi nội dung, ghi tên section. Khi section gập, header SHALL ghi phần tóm tắt:
- Filter: số điều kiện đang bật, hoặc "paused".
- Highlights: số rule.
- Columns: "<số cột đang hiện> of <tổng> shown".

Khi section mở, header SHALL bỏ phần tóm tắt và hiện nút của section: "+" ở Filter và Highlights, "Pause"/"Resume" ở Filter khi Filter khác rỗng.

Trong section, mỗi mục (thẻ Filter, rule, cột) SHALL là một hàng cao một dòng, không viền, cùng một kiểu: tên, thông tin phụ màu mờ ở bên phải, rồi công tắc hoặc nút. Nhãn nhóm ("Time range", "Built-in") SHALL là chữ nhỏ viết hoa màu mờ. Điều kiện chứa text SHALL viết là `contains "…"`.

Section SHALL NOT có thêm một dòng riêng "+ Add filter" hay "+ Add rule". Context sidebar SHALL NOT có dòng ghi tên file hay loại view, vì tab đã ghi. Columns SHALL gập mặc định khi chưa có trạng thái lưu.

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

Chế độ đã chọn SHALL được nhớ. Khi dòng không có cột nào parse được (Raw View không có Log Format, hoặc dòng không khớp format), Inspector SHALL chỉ có Raw. Header của Inspector SHALL ghi số dòng; khi Inspector mở, header SHALL có Fields/Raw và nút Copy. Đi sang dòng khác dùng phím ↑↓ của view, Inspector SHALL NOT có nút riêng cho việc đó.

#### Scenario: Fields của dòng JSON
- **WHEN** người dùng chọn một dòng có các field `event`, `method`, `status` ở chế độ Fields
- **THEN** Inspector hiện `event`, `method` và `status`, mỗi field một hàng kèm giá trị

#### Scenario: Raw giữ nguyên text
- **WHEN** người dùng chuyển Inspector sang Raw trên một dòng có 4 dòng stack trace
- **THEN** Inspector hiện dòng đó và 4 dòng stack trace, đúng như trong file

### Requirement: Hành động trên field của Inspector
Ở chế độ Fields, khi trỏ chuột vào một hàng field của cột lọc được, Inspector SHALL hiện ba nút:
- Show only và Hide, tạo điều kiện như Selection Menu khi chọn trọn giá trị ô.
- Highlight rows, mở form New rule đã điền cột và giá trị.

Các nút SHALL chỉ có trong Parsed View.

#### Scenario: Ẩn từ Inspector
- **WHEN** người dùng trỏ vào field `source` = `backend` và bấm Hide
- **THEN** Filter có `source` ở Hide Mode với `backend`, và các dòng `backend` bị ẩn
