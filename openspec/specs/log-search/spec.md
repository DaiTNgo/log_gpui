# log-search Specification

## Purpose

Search trên dòng log bằng Search Expression, tô sáng Match và đi qua các Match.
## Requirements
### Requirement: Search khớp trên toàn bộ dòng log
Hệ thống SHALL khớp Search Expression trên toàn bộ `raw_line` của từng dòng log, không theo trường hay theo cột nào.

#### Scenario: Từ khóa nằm ở bất kỳ vị trí nào trong dòng
- **WHEN** người dùng gõ `error` và một dòng có `ERROR` ở cột level, một dòng khác có `error` trong message
- **THEN** cả hai dòng đều là Match

#### Scenario: Search không đổi tập dòng
- **WHEN** người dùng gõ một Search Expression
- **THEN** danh sách dòng đang hiển thị không đổi, chỉ có Match được đánh dấu và có thể điều hướng

### Requirement: Search chỉ tìm trên dòng Filter đang hiển thị
Trong Parsed View, hệ thống SHALL chỉ tìm và đếm Match trên các dòng Filter đang hiển thị, và SHALL NOT đếm dòng khớp nhưng đang bị Filter ẩn. Trong Raw View, Filter không áp dụng, nên hệ thống SHALL tìm và đếm Match trên mọi dòng của mọi File Section.

#### Scenario: Dòng bị Filter ẩn
- **WHEN** đang ở Parsed View, Filter ẩn mọi dòng `DEBUG` và người dùng gõ `timeout`, trong đó 5 dòng `DEBUG` có `timeout` và 12 dòng khác có `timeout`
- **THEN** tổng số Match là 12

#### Scenario: Raw View đếm mọi dòng
- **WHEN** cùng Filter và Search Expression như trên nhưng người dùng chuyển sang Raw View
- **THEN** tổng số Match là 17

### Requirement: Search không phân biệt hoa thường
Hệ thống SHALL khớp Search Term không phân biệt hoa thường và SHALL NOT cung cấp tùy chọn match case.

#### Scenario: Khác hoa thường vẫn khớp
- **WHEN** người dùng gõ `timeout` và dòng log chứa `TimeOut`
- **THEN** dòng đó là Match

### Requirement: Search so khớp đúng Unicode và không bỏ dấu
Hệ thống SHALL không phân biệt hoa thường theo Unicode, SHALL NOT bỏ dấu khi so khớp, và SHALL coi dạng tổ hợp (NFC) và dạng rời (NFD) của cùng một ký tự là khớp nhau.

#### Scenario: Hoa thường tiếng Việt
- **WHEN** người dùng gõ `đã` và dòng log chứa `ĐÃ`
- **THEN** dòng đó là Match

#### Scenario: Không bỏ dấu
- **WHEN** người dùng gõ `loi` và dòng log chỉ chứa `lỗi`
- **THEN** dòng đó không phải Match

#### Scenario: Dấu phải đúng
- **WHEN** người dùng gõ `lỗi` và dòng log chỉ chứa `lối`
- **THEN** dòng đó không phải Match

#### Scenario: NFC và NFD
- **WHEN** người dùng gõ `lỗi` ở dạng NFC và dòng log chứa `lỗi` ở dạng NFD
- **THEN** dòng đó là Match và highlight nằm đúng trên chuỗi gốc của dòng

### Requirement: Phrase là các chữ liền nhau
Hệ thống SHALL coi các chữ đứng liền nhau không có toán tử là một Phrase và khớp các chữ đó theo đúng thứ tự. Giữa hai chữ liền nhau của Phrase, bất kỳ chuỗi khoảng trắng nào trong dòng log SHALL thỏa một dấu cách.

#### Scenario: Cụm từ khớp theo thứ tự
- **WHEN** người dùng gõ `connection refused`
- **THEN** dòng chứa `connection refused` là Match và dòng chỉ chứa `refused connection` thì không

#### Scenario: Nhiều khoảng trắng hoặc tab trong dòng
- **WHEN** người dùng gõ `connection refused` và dòng log chứa `connection\t\trefused`
- **THEN** dòng đó là Match

#### Scenario: Chữ and và or là chữ thường
- **WHEN** người dùng gõ `failed and retried`
- **THEN** đó là một Phrase duy nhất và không bị tách thành hai vế

### Requirement: Kết hợp điều kiện bằng && || và ngoặc
Hệ thống SHALL hỗ trợ `&&`, `||` và `()` để kết hợp các Search Term, trong đó `&&` có độ ưu tiên cao hơn `||`. Hệ thống SHALL NOT hỗ trợ phủ định.

#### Scenario: AND yêu cầu cả hai
- **WHEN** người dùng gõ `timeout && db`
- **THEN** chỉ dòng chứa cả `timeout` và `db` là Match

#### Scenario: OR chấp nhận một trong hai
- **WHEN** người dùng gõ `timeout || refused`
- **THEN** dòng chứa `timeout` hoặc `refused` là Match

#### Scenario: Độ ưu tiên
- **WHEN** người dùng gõ `a || b && c`
- **THEN** biểu thức được hiểu là `a || (b && c)`

#### Scenario: Nhóm bằng ngoặc
- **WHEN** người dùng gõ `(timeout || refused) && db`
- **THEN** dòng phải chứa `db` và ít nhất một trong `timeout`, `refused`

#### Scenario: Phrase liền kề nhóm ngoặc không có toán tử
- **WHEN** người dùng gõ `(db || cache) timeout`
- **THEN** hệ thống báo lỗi cú pháp

### Requirement: Search không có regex
Hệ thống SHALL coi mọi ký tự trong Search Term là ký tự thường, không diễn giải regex.

#### Scenario: Ký tự regex là chữ thường
- **WHEN** người dùng gõ `5.0` và dòng log chứa `5x0` nhưng không chứa `5.0`
- **THEN** dòng đó không phải Match

### Requirement: Dấu ngoặc kép làm lối thoát
Hệ thống SHALL cho phép đặt Phrase trong dấu `"..."` để khớp các ký tự `&`, `|`, `(`, `)` như chữ thường.

#### Scenario: Khớp ký tự trùng cú pháp
- **WHEN** người dùng gõ `"foo(bar)"`
- **THEN** dòng chứa đúng chuỗi `foo(bar)` là Match và không có lỗi cú pháp

### Requirement: Biểu thức không hợp lệ giữ Match gần nhất
Khi Search Expression đang gõ dở hoặc sai cú pháp, hệ thống SHALL giữ danh sách Match của biểu thức hợp lệ gần nhất và SHALL hiển thị chỉ báo lỗi bên cạnh ô Search.

#### Scenario: Gõ dở không làm danh sách về 0
- **WHEN** biểu thức hợp lệ `timeout` đang có 12 Match và người dùng gõ tiếp thành `timeout ||`
- **THEN** danh sách vẫn là 12 Match và chỉ báo lỗi được hiển thị

#### Scenario: Biểu thức trở lại hợp lệ
- **WHEN** người dùng gõ tiếp thành `timeout || refused`
- **THEN** danh sách Match được tính lại theo biểu thức mới và chỉ báo lỗi biến mất

#### Scenario: Biểu thức rỗng
- **WHEN** người dùng xóa hết nội dung ô Search
- **THEN** không có Match và không có chỉ báo lỗi

### Requirement: Highlight mọi lần xuất hiện của Phrase
Hệ thống SHALL highlight mọi lần xuất hiện của mỗi Phrase trong biểu thức trên dòng Match, với offset byte đúng trên chuỗi gốc.

#### Scenario: Highlight nhiều vị trí
- **WHEN** biểu thức là `error` và dòng chứa `Error: another error`
- **THEN** cả hai vị trí được highlight đúng phạm vi ký tự trong dòng gốc

#### Scenario: Ký tự đa byte
- **WHEN** dòng chứa chữ Việt có dấu trước từ khóa
- **THEN** phạm vi highlight bắt đầu và kết thúc đúng ranh giới ký tự

### Requirement: Điều hướng giữa các Match
Hệ thống SHALL cho phép nhảy tới Match kế tiếp hoặc trước đó, quay vòng ở hai đầu, và hiển thị vị trí hiện tại trên tổng số Match.

#### Scenario: Quay vòng
- **WHEN** đang ở Match cuối và người dùng nhảy tới Match kế tiếp
- **THEN** vị trí hiện tại là Match đầu tiên

