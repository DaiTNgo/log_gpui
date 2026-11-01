# text-marker

## Purpose
Marker tô màu đoạn text đã chọn trong một dòng.
## Requirements
### Requirement: Marker tô màu đoạn text đã chọn
Hệ thống SHALL cho người dùng tô một màu lên đoạn text đã chọn trong một dòng, gọi là Marker. Vùng chọn qua nhiều dòng SHALL tạo một Marker trên mỗi dòng cho phần được chọn của dòng đó. Một dòng SHALL có được nhiều Marker.

#### Scenario: Tô một đoạn
- **WHEN** người dùng chọn `payment_id=123` trong dòng 42 và chọn màu cam
- **THEN** chỉ đoạn `payment_id=123` của dòng 42 có nền cam

#### Scenario: Tô qua nhiều dòng
- **WHEN** người dùng chọn từ giữa dòng 3 tới giữa dòng 5 và chọn màu xanh
- **THEN** dòng 3, 4, 5 mỗi dòng có một Marker xanh phủ đúng phần được chọn

### Requirement: Thanh màu nổi khi chọn text
Chọn text SHALL NOT tự mở thanh màu, ở mọi view (Parsed View, Raw View chỉ đọc, editor scratch), dù chọn bằng chuột hay bàn phím. Thanh màu nổi SHALL mở cạnh vùng chọn khi người dùng:
- chọn "Color…" trong Selection Menu, hoặc
- dùng lệnh "Mark: Color Selection…".

Thanh màu gồm lựa chọn "This line only" / "Everywhere", các màu dùng gần đây, nút mở Highlight Palette và nút mở color picker. Selection Menu SHALL cho tô Marker ("Mark here") mà không cần mở thanh màu.

#### Scenario: Chọn để copy
- **WHEN** người dùng kéo chọn text trong Parsed View rồi nhả chuột
- **THEN** text được chọn và không có thanh màu nào hiện

#### Scenario: Mở thanh từ menu rồi tô
- **WHEN** người dùng chọn text, chuột phải, chọn "Color…" rồi bấm màu đầu tiên trên thanh
- **THEN** đoạn đó được tô màu đó và thanh nổi đóng lại

#### Scenario: Bỏ chọn
- **WHEN** thanh màu đang mở và người dùng bấm ra ngoài vùng chọn
- **THEN** thanh nổi đóng và không có Marker nào được tạo

### Requirement: Đổi màu và gỡ Marker
Bấm (không kéo) lên một Marker SHALL mở thanh màu để đổi màu hoặc gỡ Marker đó. Selection Menu mở trên một Marker SHALL có "Remove marker". "Remove marker" SHALL gỡ cả Marker chứa vị trí đó. Tô chồng lên một Marker cũ SHALL cắt Marker cũ; phần chồng nhận màu mới.

#### Scenario: Gỡ Marker
- **WHEN** người dùng chuột phải lên một Marker và chọn "Remove marker"
- **THEN** Marker đó biến mất, các Marker khác trên dòng vẫn còn

#### Scenario: Đổi màu bằng cách bấm
- **WHEN** người dùng bấm lên một Marker đỏ và chọn màu xanh trên thanh màu
- **THEN** Marker đó thành màu xanh

#### Scenario: Tô chồng
- **WHEN** `abcdef` có Marker đỏ và người dùng tô xanh lên `cd`
- **THEN** `ab` và `ef` đỏ, `cd` xanh

### Requirement: Marker giống nhau ở Raw View và Parsed View
Hệ thống SHALL hiện Marker ở cả Raw View và Parsed View tại đúng các ký tự đã tô. Phần Marker nằm ngoài mọi ô (khoảng trắng hay dấu phân cách giữa các trường) SHALL chỉ thấy trong Raw View.

#### Scenario: Tô trong Raw, xem trong Parsed
- **WHEN** người dùng tô `timeout` trong Raw View rồi chuyển sang Parsed View
- **THEN** chữ `timeout` trong ô MESSAGE có cùng màu

### Requirement: Marker nằm trên Highlight Rule
Hệ thống SHALL vẽ màu Marker trên màu Highlight Rule, màu hàng đang chọn và màu Match của Search.

#### Scenario: Hàng được tô có Marker
- **WHEN** một hàng được Highlight Rule tô cam và có Marker xanh trên một đoạn
- **THEN** đoạn đó hiện xanh, phần còn lại của hàng hiện cam

### Requirement: Không có danh sách Marker
Hệ thống SHALL NOT có panel hay danh sách Marker, và Marker SHALL NOT có ghi chú. Overview ruler SHALL hiện một vạch màu cho mỗi dòng có Marker; bấm vạch SHALL cuộn tới dòng đó.

#### Scenario: Context sidebar
- **WHEN** người dùng mở context sidebar
- **THEN** không có mục Marker

#### Scenario: Nhảy bằng ruler
- **WHEN** người dùng bấm vạch màu trên overview ruler
- **THEN** canvas cuộn tới dòng có Marker đó

### Requirement: Level Marker
Hệ thống SHALL tự đặt Level Marker ở Gutter của dòng có level lỗi hoặc cảnh báo, dựa trên nghĩa của level. Người dùng SHALL NOT đặt hay gỡ Level Marker, và Level Marker SHALL NOT tính là Marker.

#### Scenario: Dòng lỗi
- **WHEN** dòng có level `FATAL`
- **THEN** Gutter có Level Marker lỗi, và menu chuột phải không có "Remove marker" cho nó

### Requirement: Marker được nhớ
Hệ thống SHALL tự lưu Marker theo dòng của từng file và khôi phục khi mở lại file hoặc khởi động lại ứng dụng. Sửa nội dung một dòng scratch SHALL làm mất mọi Marker của dòng đó.

#### Scenario: Khởi động lại
- **WHEN** người dùng tô ba đoạn trong `api.log`, thoát rồi mở lại ứng dụng
- **THEN** ba Marker vẫn ở đúng chỗ

#### Scenario: Sửa dòng scratch
- **WHEN** dòng 10 của `untitled.log` có Marker và người dùng sửa dòng đó
- **THEN** sau khi lưu, dòng 10 không còn Marker nào

### Requirement: Lệnh mở thanh màu cho vùng chọn
Hệ thống SHALL có lệnh "Mark: Color Selection…" trong Command Palette và Keymap, phím mặc định `cmd-shift-h`. Khi view đang hiện có vùng chọn khác rỗng (Parsed View, Raw View chỉ đọc hoặc editor scratch), lệnh SHALL mở thanh màu cho vùng chọn đó, cạnh vùng chọn, với lựa chọn "This line only" / "Everywhere" như khi thanh tự mở. Khi không có vùng chọn, lệnh SHALL NOT làm gì.

#### Scenario: Tô trong editor scratch bằng lệnh
- **WHEN** người dùng chọn `timeout` ở dòng 3 của `untitled.log` đang sửa được rồi bấm `cmd-shift-h` và chọn màu đỏ
- **THEN** `timeout` ở dòng 3 có Marker đỏ và thanh màu đóng lại

#### Scenario: Mở thanh cho vùng chọn trong Raw View chỉ đọc
- **WHEN** người dùng kéo chọn text trong Raw View của `api.log`, thanh màu đã đóng mà vùng chọn vẫn còn, rồi bấm `cmd-shift-h`
- **THEN** thanh màu mở lại cho đúng vùng chọn đó

#### Scenario: Không có vùng chọn
- **WHEN** người dùng bấm `cmd-shift-h` khi không có text nào được chọn
- **THEN** không có thanh màu nào hiện

