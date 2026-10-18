## ADDED Requirements

### Requirement: Bố cục Focus Pane
Hệ thống SHALL hiện Focus Pane bên dưới view, rộng hết cửa sổ, nằm dưới cả sidebar trái lẫn sidebar phải. Người dùng SHALL kéo được ranh giới giữa view và Focus Pane để đổi chiều cao. Hệ thống SHALL có nút trên thanh Search và một phím tắt để bật/tắt Focus Pane. Trạng thái mở và chiều cao SHALL được nhớ theo ứng dụng, không theo tab.

#### Scenario: Pane nằm dưới cả hai sidebar
- **WHEN** Focus Pane đang mở và cả hai sidebar đang hiện
- **THEN** Focus Pane trải từ mép trái tới mép phải cửa sổ, và hai sidebar kết thúc ở cạnh trên của Focus Pane

#### Scenario: Bật tắt
- **WHEN** người dùng bấm nút Focus Pane trên thanh Search
- **THEN** Focus Pane ẩn đi và view chiếm lại toàn bộ chiều cao

#### Scenario: Nhớ chiều cao
- **WHEN** người dùng kéo Focus Pane cao 300px, chuyển sang tab Log Space khác rồi khởi động lại ứng dụng
- **THEN** Focus Pane vẫn mở và cao 300px

### Requirement: Nội dung Focus Tab
Trong Parsed View, Focus Tab đang mở SHALL liệt kê đúng các dòng qua Filter của view và qua Search Expression của tab, theo thứ tự của view. Search Expression rỗng SHALL không giới hạn theo Search; Filter rỗng SHALL không giới hạn theo Filter. Khi cả hai rỗng, Focus Tab SHALL không liệt kê dòng nào và SHALL hiện gợi ý gõ Search hoặc chọn Filter.

#### Scenario: Filter và Search cùng áp dụng
- **WHEN** Filter chỉ hiện `level` = `ERROR` và Search Expression là `timeout`
- **THEN** Focus Tab liệt kê các dòng `ERROR` có chứa `timeout`, và view vẫn hiện mọi dòng

#### Scenario: Chỉ có Filter
- **WHEN** Search Expression rỗng và Filter chỉ hiện `module` = `DB`
- **THEN** Focus Tab liệt kê mọi dòng có `module` = `DB`

#### Scenario: Cả hai rỗng
- **WHEN** Search Expression rỗng và không có Filter nào
- **THEN** Focus Tab trống và hiện gợi ý

#### Scenario: Continuation Line
- **WHEN** Search Expression là `NullPointerException` và chỉ một dòng stack trace (Continuation Line) chứa chữ đó
- **THEN** Focus Tab liệt kê đúng dòng stack trace đó, không kèm Parent Line

### Requirement: Focus Pane trong Raw View
Khi view là Raw View, Focus Tab SHALL chỉ dùng Search Expression và SHALL NOT áp dụng Filter.

#### Scenario: Filter bị bỏ qua
- **WHEN** Filter ẩn `DEBUG`, Search Expression là `timeout`, và người dùng chuyển sang Raw View
- **THEN** Focus Tab liệt kê mọi dòng chứa `timeout`, kể cả dòng `DEBUG`

### Requirement: Cách hiện dòng trong Focus Pane
Mỗi dòng trong Focus Pane SHALL hiện raw line đúng như lưu trữ, với số dòng trong file ở Gutter. Khi view có nhiều Source File, mỗi dòng SHALL hiện tên Source File. Mọi lần xuất hiện của Phrase trong Search Expression SHALL được tô. Focus Pane SHALL hiện Marker và Level Marker. Khi view là Parsed View, Focus Pane SHALL tô Highlight Rule như ở view.

#### Scenario: Log Space nhiều file
- **WHEN** Log Space có `api.log` và `worker.log` và Focus Tab liệt kê dòng 42 của `worker.log`
- **THEN** dòng đó hiện số `42` và tên `worker.log`

#### Scenario: File View
- **WHEN** view là File View `api.log`
- **THEN** các dòng trong Focus Pane không hiện tên file

#### Scenario: Highlight Rule chỉ khi Parsed View
- **WHEN** có Highlight Rule `module` = `DB` màu xanh và view là Raw View
- **THEN** các dòng `DB` trong Focus Pane không được tô màu xanh

### Requirement: Đồng bộ dòng hiện tại với view
Dòng hiện tại của Focus Tab và dòng đang chọn ở view SHALL luôn đi theo nhau. Bấm một dòng trong Focus Pane SHALL cuộn view tới dòng đó và chọn nó. Nhảy tới Match ở view SHALL chọn dòng đó trong Focus Pane và cuộn pane tới. Chọn ở view một dòng không có trong Focus Tab SHALL cuộn pane tới dòng gần nhất phía trước có trong tab, nhưng SHALL NOT chọn nó.

#### Scenario: Bấm trong pane
- **WHEN** người dùng bấm dòng 1200 trong Focus Pane
- **THEN** view cuộn tới dòng 1200 và dòng đó được chọn

#### Scenario: Next ở view
- **WHEN** người dùng bấm next trên thanh Search
- **THEN** view nhảy tới Match kế tiếp và Focus Pane chọn đúng dòng đó

#### Scenario: Chọn dòng không có trong tab
- **WHEN** Focus Tab có dòng 100 và 300, và người dùng bấm dòng 250 ở view
- **THEN** Focus Pane cuộn tới dòng 100 và không dòng nào trong pane được chọn

### Requirement: Focus Tab đánh số
Focus Pane SHALL có ít nhất một Focus Tab. Mỗi Focus Tab SHALL có Search Expression riêng. Nút "+" SHALL tạo một Focus Tab trống mang số lớn nhất hiện có cộng một, mở nó và đặt con trỏ vào ô Search. Số của các tab còn lại SHALL NOT đổi khi một tab bị đóng. Nhãn tab SHALL là số kèm phần đầu của Search Expression. Khi chỉ còn một tab, tab đó SHALL NOT có nút đóng.

#### Scenario: Thêm tab
- **WHEN** đang có tab 1 và 3 và người dùng bấm "+"
- **THEN** tab 4 được tạo, được mở, và ô Search trống

#### Scenario: Đóng tab giữa
- **WHEN** có tab 1, 2, 3 và người dùng đóng tab 2
- **THEN** còn tab 1 và 3 với số không đổi

#### Scenario: Tab cuối
- **WHEN** chỉ còn tab 1
- **THEN** tab 1 không có nút đóng

### Requirement: Ô Search sửa tab đang mở
Ô Search trên toolbar SHALL luôn hiện và sửa Search Expression của Focus Tab đang mở. View SHALL chỉ tô Match của tab đang mở, và bộ đếm Match, next/prev, overview ruler SHALL theo tab đó. Mọi Focus Tab của một view SHALL dùng chung Filter của view đó.

#### Scenario: Đổi tab đổi ô Search
- **WHEN** tab 1 có `timeout`, tab 2 có `refused`, và người dùng mở tab 2
- **THEN** ô Search hiện `refused` và view chỉ tô Match của `refused`

#### Scenario: Filter dùng chung
- **WHEN** người dùng tích `ERROR` ở Filter khi đang mở tab 1, rồi mở tab 2
- **THEN** tab 2 cũng chỉ liệt kê dòng `ERROR`

### Requirement: Mở tab nhảy view tới dòng hiện tại của tab
Mỗi Focus Tab SHALL nhớ dòng hiện tại của nó; khi tab có kết quả Focus Find đang chọn thì đó là dòng hiện tại. Mở một Focus Tab SHALL cuộn view tới dòng hiện tại của tab đó và chọn nó, nếu dòng đó vẫn còn trong tab.

#### Scenario: Quay lại tab
- **WHEN** ở tab 1 người dùng dùng Focus Find tới dòng 500, chuyển sang tab 2, rồi mở lại tab 1
- **THEN** view cuộn tới dòng 500 và dòng đó được chọn

### Requirement: Focus Find
Mỗi Focus Tab SHALL có ô Focus Find riêng trong Focus Pane. Focus Find SHALL dùng cùng cú pháp và luật khớp với Search Expression, và SHALL chỉ tìm trong các dòng tab đang liệt kê. Focus Find SHALL NOT thu hẹp tab. Kết quả Focus Find SHALL chỉ được tô trong Focus Pane. Focus Find SHALL có next/prev quay vòng và hiện vị trí trên tổng số kết quả. Nhảy tới một kết quả SHALL đổi dòng hiện tại của tab, nên view cuộn theo.

#### Scenario: Không thu hẹp
- **WHEN** Focus Tab liệt kê 40 dòng và người dùng gõ `user=42` vào Focus Find, có 3 dòng khớp
- **THEN** tab vẫn liệt kê 40 dòng, 3 dòng khớp được tô trong pane, và bộ đếm hiện `1/3`

#### Scenario: Không tô ở view
- **WHEN** Focus Find là `user=42`
- **THEN** view không tô `user=42`, chỉ tô Match của Search Expression

#### Scenario: Cú pháp chung
- **WHEN** người dùng gõ `user=42 || user=43` vào Focus Find
- **THEN** các dòng chứa `user=42` hoặc `user=43` là kết quả

### Requirement: Nhớ Focus Tab theo view
Hệ thống SHALL nhớ các Focus Tab của mỗi view (Log Space hoặc File View), gồm số, Search Expression, Focus Find, dòng hiện tại và tab đang mở, và SHALL khôi phục chúng khi mở lại view hoặc khởi động lại ứng dụng. View chưa có Focus Tab nào được nhớ SHALL mở với một tab 1 trống.

#### Scenario: Khôi phục
- **WHEN** Log Space "Payment" có tab 1 `timeout` và tab 3 `refused` đang mở, người dùng đóng tab "Payment" rồi mở lại
- **THEN** có tab 1 `timeout` và tab 3 `refused`, tab 3 đang mở và ô Search hiện `refused`

#### Scenario: Trạng thái cũ
- **WHEN** người dùng mở một Log Space được lưu trước khi có Focus Pane
- **THEN** Focus Pane có một tab 1 trống

### Requirement: Focus Pane cập nhật theo nội dung
Hệ thống SHALL tính lại Focus Tab khi có dòng mới do tail, khi file scratch được sửa, khi Filter đổi, và khi Search Expression hợp lệ đổi. Dòng hiện tại của tab SHALL được giữ nếu dòng đó vẫn còn trong tab.

#### Scenario: Tail
- **WHEN** Search Expression là `timeout` và file đang tail nhận thêm một dòng chứa `timeout`
- **THEN** dòng đó được thêm vào cuối Focus Tab mà dòng hiện tại không đổi

#### Scenario: Sửa scratch
- **WHEN** Search Expression là `timeout` và người dùng sửa một dòng trong file scratch ở Raw View để nó chứa `timeout`
- **THEN** dòng đó xuất hiện trong Focus Tab

### Requirement: Chép từ Focus Pane
Chép vùng chọn trong Focus Pane SHALL chỉ cho text lưu trữ của các dòng pane đang liệt kê, theo thứ tự trong pane.

#### Scenario: Chép nhiều dòng
- **WHEN** Focus Tab liệt kê dòng 10, 50, 90 và người dùng chọn cả ba rồi bấm ⌘C
- **THEN** clipboard có đúng ba dòng 10, 50, 90, mỗi dòng một hàng
