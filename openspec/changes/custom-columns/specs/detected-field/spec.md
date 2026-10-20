## ADDED Requirements

### Requirement: Nhóm Detected trong mục Columns
Nhóm *Detected* của mục Columns SHALL liệt kê Detected Field của view đang mở. Mỗi mục SHALL có tên trường (hoặc Column Label nếu có), số Parent Line có trường đó, và một công tắc. Danh sách SHALL xếp theo số dòng giảm dần, và SHALL hiện tối đa 50 trường kèm một ô tìm khi có nhiều hơn. Mọi Detected Field SHALL mặc định tắt. Bật công tắc SHALL hiện trường thành Field Column, tắt SHALL ẩn cột đó. Trạng thái công tắc SHALL thuộc Project.

#### Scenario: Bật trường JSON
- **WHEN** log JSON có trường `region` ở 900 dòng và người dùng bật `region` trong nhóm *Detected*
- **THEN** Parsed View có cột `region`

#### Scenario: Mặc định tắt
- **WHEN** người dùng mở log JSON có 30 trường lần đầu
- **THEN** Parsed View không có Field Column nào từ các trường đó

#### Scenario: Nhiều trường
- **WHEN** view có 80 Detected Field
- **THEN** nhóm *Detected* hiện 50 trường có nhiều dòng nhất và một ô tìm cho các trường còn lại

### Requirement: Trường JSON là Detected Field
Mỗi khoá cấp một của dòng JSON SHALL là một Detected Field. Ô của nó SHALL là đoạn text của giá trị trong dòng như lưu trong file. Trường JSON SHALL vẫn là Filterable Column theo luật hiện có, kể cả khi công tắc của nó đang tắt.

#### Scenario: Lọc khi chưa bật
- **WHEN** log JSON có trường `region` và công tắc `region` đang tắt
- **THEN** người dùng vẫn thêm Filter cho cột `region` được

### Requirement: Nhận diện key=value
Hệ thống SHALL tìm các cặp `key=value` trong MESSAGE của mỗi dòng. Key là chữ cái hoặc `_`, theo sau là chữ, số, `_`, `.` hoặc `-`. Value là chuỗi trong ngoặc kép (có thể chứa `\"`) hoặc chuỗi ký tự liền không có khoảng trắng. Mỗi key tìm được SHALL là một Detected Field. Ô của nó SHALL là đoạn value như lưu trong file, gồm cả ngoặc kép nếu có. Khi một key xuất hiện nhiều lần trong một dòng, lần đầu tiên SHALL được dùng.

#### Scenario: Logfmt
- **WHEN** MESSAGE là `charge ok user=alice amount=12.50 note="first order"`
- **THEN** `user`, `amount` và `note` là Detected Field với giá trị `alice`, `12.50` và `"first order"`

#### Scenario: Không phải key=value
- **WHEN** MESSAGE là `a == b`
- **THEN** không có Detected Field nào được tìm thấy từ dòng đó

#### Scenario: Key lặp lại
- **WHEN** MESSAGE là `retry id=1 id=2`
- **THEN** Detected Field `id` của dòng đó là `1`

### Requirement: Detected Field key=value chỉ lọc được khi đã bật
Detected Field tìm từ `key=value` SHALL chỉ là Filterable Column và chỉ dùng được cho Highlight Rule khi công tắc của nó đang bật. Khi người dùng tắt một trường đang có điều kiện Filter, điều kiện SHALL được giữ, và mục Filter của trường đó SHALL hiện chú thích rằng cột đang tắt.

#### Scenario: Chưa bật
- **WHEN** Detected Field `amount` đang tắt
- **THEN** danh sách chọn cột để thêm Filter không có `amount`

#### Scenario: Tắt khi đang lọc
- **WHEN** Filter có `user` = `alice` và người dùng tắt Detected Field `user`
- **THEN** điều kiện `user` = `alice` vẫn còn, và mục Filter của `user` có chú thích cột đang tắt

### Requirement: Detected Field trong Live
Trong Remote Log Space, key mới xuất hiện ở dòng mới tới SHALL được thêm vào danh sách Detected Field với công tắc tắt. Số dòng của mỗi Detected Field SHALL tăng khi dòng tới và giảm khi dòng rời Live Window. Trường không còn dòng nào trong view SHALL biến mất khỏi danh sách, trừ khi công tắc của nó đang bật.

#### Scenario: Key mới
- **WHEN** một dòng mới tới có `tenant=acme` và chưa dòng nào trước đó có `tenant`
- **THEN** `tenant` xuất hiện trong danh sách Detected Field với công tắc tắt
