> **Đã bị thay thế một phần (ADR-0015).** Extraction Rule, "Make column…" và Column Label đã bị gỡ; Log Format là cách duy nhất để định nghĩa cột, và mục Columns chỉ bật/tắt cột của Focus Pane. Các yêu cầu về Column Label, sắp xếp lại cột và việc Columns điều khiển Canvas không còn hiệu lực. Các phần nhắc đến chúng dưới đây chỉ còn là lịch sử, không còn đúng với app. Xem `docs/adr/0015-log-format-is-the-only-way-to-define-columns.md` và `CONTEXT.md`.

## ADDED Requirements

### Requirement: Field Column trong Parsed View
Hệ thống SHALL hiện mỗi trường có tên đọc được từ dòng thành một Field Column của Parsed View. Trường ở đây là trường của Extraction Rule đang bật, trường của Log Format, hoặc Detected Field đã bật. Field Column SHALL nằm sau MODULE và Source Column, trước MESSAGE. Hệ thống SHALL NOT vẽ một Field Column khi không dòng nào trong view có giá trị ở cột đó.

#### Scenario: Cột từ Extraction Rule
- **WHEN** Project có Extraction Rule `user` đang bật và dòng 5 chứa `user=alice`
- **THEN** Parsed View có cột `user` nằm trước MESSAGE, và ô `user` của hàng 5 hiện `alice`

#### Scenario: Không dòng nào có giá trị
- **WHEN** Extraction Rule `user` đang bật nhưng không dòng nào của Log Space chứa `user=`
- **THEN** Parsed View của Log Space đó không có cột `user`, và mục Columns hiện `user` kèm `0 lines here`

### Requirement: Ô Field Column là text lưu trong file
Mỗi ô của Field Column SHALL hiện đúng một đoạn liên tục các ký tự của dòng như lưu trong file. Hệ thống SHALL NOT giải escape, bỏ ngoặc kép hay định dạng lại giá trị. Dòng không có giá trị ở Field Column SHALL có ô trống ở cột đó, và text của dòng SHALL vẫn hiện đầy đủ ở MESSAGE như khi không có Field Column.

#### Scenario: Giá trị có ngoặc kép
- **WHEN** dòng chứa `msg="disk full"` và Detected Field `msg` đã bật
- **THEN** ô `msg` hiện `"disk full"`

#### Scenario: Chuỗi JSON có escape
- **WHEN** dòng JSON có `"note":"say \"hi\""` và Detected Field `note` đã bật
- **THEN** ô `note` hiện `say \"hi\"`

#### Scenario: Dòng không khớp
- **WHEN** Extraction Rule `user` đang bật và dòng 8 không chứa `user=`
- **THEN** ô `user` của hàng 8 trống và MESSAGE của hàng 8 không đổi

### Requirement: Ô Field Column của Continuation Line
Hệ thống SHALL hiện dấu `_` màu nhạt ở ô Field Column của Continuation Line, giống các ô khác lấy từ dòng. Filter, Time Range và Highlight Rule SHALL xử lý Continuation Line theo giá trị Field Column của Parent Line.

#### Scenario: Stack trace
- **WHEN** dòng 10 có `user=alice` và dòng 11 là Continuation Line của dòng 10
- **THEN** ô `user` của hàng 11 hiện `_` màu nhạt

#### Scenario: Lọc theo Field Column
- **WHEN** Filter chỉ hiện `user` = `alice` và dòng 10 có `user=alice` kèm stack trace dòng 11–15
- **THEN** dòng 10–15 đều qua Filter

### Requirement: Ghim và đổi rộng Field Column
Hệ thống SHALL cho ghim, bỏ ghim, kéo đổi rộng và Auto-fit mọi Field Column giống TIMESTAMP, LEVEL và MODULE. Cột đã ghim SHALL giữ thứ tự cột của view, không theo thứ tự người dùng bấm ghim.

#### Scenario: Ghim
- **WHEN** người dùng bấm Pin Toggle trên Column Header của cột `user`
- **THEN** cột `user` bám mép trái cạnh các cột đã ghim khi cuộn ngang

#### Scenario: Thứ tự khi ghim
- **WHEN** người dùng ghim cột `user` rồi ghim LEVEL
- **THEN** LEVEL nằm bên trái `user` ở vùng ghim

#### Scenario: Auto-fit
- **WHEN** người dùng nhấp đúp vào Column Resize Handle bên phải cột `user` và giá trị dài nhất là `svc-billing`
- **THEN** cột `user` rộng vừa `svc-billing` và tiêu đề

### Requirement: Coverage trên Column Header
Khi một Field Column có giá trị ở ít hơn mọi Parent Line mà view đang giữ, Column Header của nó SHALL hiện Coverage dưới dạng phần trăm làm tròn xuống. Di chuột vào Coverage SHALL hiện số dòng có giá trị trên tổng số dòng. Coverage SHALL NOT tính Continuation Line và SHALL NOT đổi theo Filter.

#### Scenario: Một phần dòng có giá trị
- **WHEN** 1.204 trên 1.530 Parent Line có giá trị ở cột `user`
- **THEN** Column Header `user` hiện `78%`, và di chuột vào hiện "1,204 of 1,530 lines"

#### Scenario: Gần đủ
- **WHEN** 1.529 trên 1.530 Parent Line có giá trị ở cột `user`
- **THEN** Column Header `user` hiện `99%`, không phải `100%`

#### Scenario: Đủ cả
- **WHEN** mọi Parent Line có giá trị ở cột `user`
- **THEN** Column Header `user` không hiện Coverage

### Requirement: Column Label
Hệ thống SHALL cho người dùng nhấp đúp vào chữ trên Column Header của bất kỳ cột nào trừ Gutter để sửa Column Label ngay tại chỗ. Enter lưu, Esc huỷ, để trống thì trả về tên trường. Menu chuột phải trên Column Header và menu `⋯` trong mục Columns SHALL có "Rename" để mở cùng ô nhập đó. Column Label SHALL thuộc Project và SHALL được hiện ở Column Header, Filter, mục Highlights và mục Columns. Hệ thống SHALL NOT cho lưu Column Label trùng tên hoặc Label của một cột khác trong view. Lỗi SHALL chỉ hiện khi người dùng bấm Enter hoặc rời khỏi ô, gồm icon và câu nói cách sửa, và SHALL ẩn đi khi người dùng gõ tiếp.

#### Scenario: Đặt tên
- **WHEN** người dùng nhấp đúp vào chữ MODULE, gõ `Service` rồi Enter
- **THEN** Column Header hiện `Service`, và Filter liệt kê cột đó là `Service`

#### Scenario: Trả về tên gốc
- **WHEN** người dùng xoá hết Column Label `Service` rồi Enter
- **THEN** Column Header hiện lại `MODULE`

#### Scenario: Trùng tên
- **WHEN** view có cột `user` và người dùng đặt Column Label của cột `uid` là `user`
- **THEN** Label không được lưu, ô nhập hiện icon lỗi và báo tên đã có

#### Scenario: Lỗi ẩn khi gõ lại
- **WHEN** ô nhập đang báo tên đã có và người dùng gõ thêm một ký tự
- **THEN** lỗi ẩn đi cho tới lần bấm Enter hoặc rời khỏi ô tiếp theo

### Requirement: Column Label không đổi tên trường
Đổi Column Label SHALL NOT đổi tên trường bên dưới. Filter, Highlight Rule và việc gộp cột theo tên SHALL tiếp tục dùng tên trường.

#### Scenario: Highlight Rule giữ nguyên
- **WHEN** Highlight Rule `module` = `DB` đang bật và người dùng đổi Column Label của MODULE thành `Service`
- **THEN** các hàng `DB` vẫn được tô, và mục Highlights hiện rule là `Service = DB`

### Requirement: Một giá trị cho mỗi trường trên một dòng
Khi nhiều nguồn cho giá trị của cùng một tên trường trên một dòng, hệ thống SHALL lấy giá trị theo thứ tự: cột cố định hoặc trường JSON, rồi Log Format, rồi Extraction Rule theo thứ tự trong mục Columns, rồi Detected Field `key=value`.

#### Scenario: Extraction Rule trùng trường JSON
- **WHEN** dòng JSON có trường `user` là `alice` và Extraction Rule `user` khớp `bob` ở cùng dòng
- **THEN** ô `user` hiện `alice`

#### Scenario: Hai Extraction Rule cùng tên
- **WHEN** Extraction Rule `user` thứ nhất khớp `u-1` và Extraction Rule `user` thứ hai khớp `u-2` ở cùng dòng
- **THEN** ô `user` hiện `u-1`

### Requirement: Field Column gộp theo tên trong Log Space
Field Column của các file trong một Log Space SHALL được gộp theo tên trường. Hệ thống SHALL NOT gộp theo Column Label.

#### Scenario: Hai file cùng trường
- **WHEN** `a.log` có Detected Field `user` và `b.log` có trường `user` từ Log Format, cả hai đều đang hiện
- **THEN** Parsed View của Log Space có đúng một cột `user` chứa giá trị của cả hai file

### Requirement: Mục Columns
Context sidebar SHALL có mục Columns, là nơi duy nhất để hiện/ẩn và sắp xếp cột của view. Mục này SHALL liệt kê mọi cột của view trừ Gutter, theo bốn nhóm theo thứ tự: *Built-in* (TIMESTAMP, LEVEL, MODULE, Source, Stream, MESSAGE), *From Log Format*, *Rules* và *Detected*. Nhóm không có cột nào SHALL NOT được hiện. Mỗi cột SHALL có Column Label hoặc tên trường, số Parent Line có giá trị trong view, và công tắc hiện/ẩn (MESSAGE không có công tắc). Cột không có giá trị nào trong view SHALL hiện `0 lines here` thay cho số dòng. Trạng thái ẩn/hiện SHALL thuộc Project, riêng Source Column vẫn theo Log Space.

#### Scenario: Liệt kê theo nhóm
- **WHEN** view có trường JSON `region`, Extraction Rule `user` và Log Format có trường `client`
- **THEN** mục Columns có `client` trong nhóm *From Log Format*, `user` trong nhóm *Rules* và `region` trong nhóm *Detected*

#### Scenario: Rule không khớp ở view này
- **WHEN** Extraction Rule `user` đang bật nhưng không dòng nào của view có giá trị
- **THEN** dòng `user` trong mục Columns hiện `0 lines here`

### Requirement: Ẩn cột
Người dùng SHALL ẩn được mọi cột trừ MESSAGE và Gutter, kể cả TIMESTAMP, LEVEL, MODULE và trường của Log Format, bằng công tắc trong mục Columns hoặc "Hide column" trên menu chuột phải của Column Header. Ẩn một cột SHALL NOT làm cột đó thôi được đọc: Filter, Time Range, Highlight Rule và Level Marker trên cột đã ẩn SHALL vẫn chạy. Công tắc Source Column trong Quick Settings và trong mục Columns SHALL dùng chung một trạng thái.

#### Scenario: Ẩn trường Log Format
- **WHEN** Log Format có trường `referrer` và người dùng tắt `referrer` trong mục Columns
- **THEN** Parsed View không còn cột `referrer`

#### Scenario: Ẩn mà vẫn lọc
- **WHEN** Filter có `level` = `ERROR` và người dùng ẩn LEVEL
- **THEN** cột LEVEL biến mất và Filter `level` = `ERROR` vẫn chạy

#### Scenario: Source Column hai nơi
- **WHEN** người dùng tắt Source trong mục Columns
- **THEN** công tắc "Show source file" trong Quick Settings cũng tắt

### Requirement: Thứ tự Field Column
Người dùng SHALL đổi được thứ tự Field Column bằng cách kéo trong mục Columns, giữa mọi nhóm Field Column. Parsed View SHALL vẽ Field Column theo thứ tự này, sau các cột *Built-in* và trước MESSAGE. Field Column mới SHALL được thêm vào cuối. Kéo SHALL NOT đổi thứ tự ưu tiên giữa các Extraction Rule cùng tên. Cột *Built-in* SHALL giữ thứ tự cố định.

#### Scenario: Kéo đổi chỗ
- **WHEN** thứ tự là `client`, `user`, `region` và người dùng kéo `region` lên đầu
- **THEN** Parsed View vẽ `region`, `client`, `user` theo thứ tự đó

### Requirement: Menu chuột phải trên Column Header
Menu chuột phải trên Column Header SHALL có "Hide column", "Rename" và "Manage columns…". "Manage columns…" SHALL mở mục Columns trong context sidebar và làm nổi dòng của cột đó. Menu trên MESSAGE SHALL NOT có "Hide column".

#### Scenario: Manage columns
- **WHEN** người dùng chuột phải lên Column Header `user` và chọn "Manage columns…"
- **THEN** context sidebar mở mục Columns với dòng `user` được làm nổi
