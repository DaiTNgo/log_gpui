## ADDED Requirements

### Requirement: Memory của một view đang mở theo số dòng
Memory mà app dùng thêm cho một view đang mở, ngoài Row Cache, SHALL tăng theo số dòng và SHALL NOT vượt quá:

- **50 byte mỗi dòng** cho một File View hoặc một Log Space có 1 file;
- **60 byte mỗi dòng** cho một Log Space nhiều file.

Giới hạn này tính cả Line Index, metadata của dòng, các cột Filter và mọi thứ view giữ cho từng dòng, khi chưa có Filter, Search hay Highlight Rule nào.

#### Scenario: Log Space 1 file 3.000.000 dòng
- **WHEN** người dùng thả một file 3.000.000 dòng vào cửa sổ, tạo một Static Log Space, và Indexing xong
- **THEN** malloc dùng thêm không quá 150MB so với trước khi mở

#### Scenario: File View của file đó
- **WHEN** người dùng mở File View của cùng file 3.000.000 dòng
- **THEN** malloc dùng thêm không quá 150MB so với trước khi mở

#### Scenario: Log Space 3 file
- **WHEN** người dùng mở một Static Log Space gồm 3 file, mỗi file 1.000.000 dòng có timestamp xen kẽ nhau
- **THEN** malloc dùng thêm không quá 180MB so với trước khi mở

### Requirement: Không giữ gì cho từng dòng khi không có Filter hay Highlight Rule
Khi không có Filter nào, view SHALL NOT giữ list các dòng qua Filter: mọi dòng đều qua. Khi không có Highlight Rule nào tô màu, view SHALL NOT giữ màu cho từng dòng. View SHALL NOT giữ list vị trí hiển thị của các dòng: vị trí trong view chính là số dòng của view.

#### Scenario: Thêm rồi bỏ Filter
- **WHEN** người dùng thêm một Filter Level = ERROR trên một Log Space 3.000.000 dòng, rồi bỏ Filter đó
- **THEN** sau khi bỏ, view không còn giữ memory nào cho kết quả Filter

#### Scenario: Tắt Highlight Rule cuối cùng
- **WHEN** người dùng tắt Highlight Rule cuối cùng đang bật
- **THEN** view không còn giữ màu cho từng dòng, và không dòng nào được tô màu

### Requirement: View dùng chung index của file
Một File View và một Log Space có 1 file SHALL dùng chung các cột Filter và timestamp của Indexed, SHALL NOT chép hay dựng lại chúng. Một Log Space nhiều file SHALL tra giá trị cột của một dòng qua cột của file chứa dòng đó, SHALL NOT dựng một bản cột riêng cho các dòng đã merge. Thứ tự các dòng của một Log Space nhiều file SHALL là phần duy nhất view giữ cho từng dòng.

#### Scenario: Dòng live trên Log Space 1 file
- **WHEN** một Local Live File trong Log Space 1 file nhận 100.000 dòng mới
- **THEN** mỗi dòng mới được parse cho các cột Filter đúng một lần, và memory view giữ thêm chỉ tăng theo phần Indexed của các dòng đó

#### Scenario: Giá trị cột của Log Space nhiều file
- **WHEN** Log Space gồm `a.log` (Level INFO, WARN) và `b.log` (Level ERROR), và người dùng lọc Level = ERROR
- **THEN** chỉ các dòng của `b.log` qua Filter, và danh sách giá trị của cột Level hiện INFO, WARN, ERROR với số đếm như trước change

### Requirement: Cột không có giá trị không tốn memory theo dòng
Một cột Filter mà không dòng nào có giá trị (ví dụ Stream, RequestId hay TraceId của một file log thường) SHALL NOT giữ gì cho từng dòng. Một cột SHALL chỉ bắt đầu giữ id cho từng dòng từ khi có dòng đầu tiên mang giá trị. Cột đó vẫn hiện và lọc được như trước.

#### Scenario: File không có RequestId
- **WHEN** người dùng mở một file 3.000.000 dòng mà không dòng nào có RequestId
- **THEN** cột RequestId không giữ memory nào cho từng dòng

#### Scenario: Giá trị đầu tiên đến muộn
- **WHEN** một file live chỉ có dòng mang RequestId từ dòng 2.000.000, và người dùng lọc theo RequestId đó
- **THEN** đúng các dòng mang RequestId đó qua Filter, như trước change

### Requirement: Kết quả không đổi
Mọi thứ người dùng thấy SHALL giống hệt trước change, trên cùng dữ liệu: các dòng và thứ tự của chúng, Filter Values và số đếm, Coverage, Search Matches, màu của Highlight Rule, Time Histogram, Go To Time, Marker, và các dòng live mới.

#### Scenario: So sánh trên cùng Log Space
- **WHEN** cùng một Log Space 3 file được mở trước và sau change, với cùng Filter, Search và Highlight Rule
- **THEN** danh sách dòng, Filter Values, số Matches, màu từng dòng và các cột của Time Histogram giống hệt nhau

### Requirement: Đo được memory theo từng phần
Repository SHALL có một probe giữ lại được, `examples/view_memory_probe.rs`. Probe SHALL báo memory malloc dùng thêm cho một File View, một Log Space 1 file và một Log Space nhiều file, tách theo từng phần mà view giữ. Probe là cách kiểm các giới hạn của requirement "Memory của một view đang mở theo số dòng".

#### Scenario: Chạy probe
- **WHEN** người phát triển chạy `cargo run --release --example view_memory_probe`
- **THEN** probe in ra memory theo từng phần và theo byte mỗi dòng cho cả ba loại view, và báo vượt giới hạn nếu có
