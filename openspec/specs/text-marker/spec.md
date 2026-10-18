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
Hệ thống SHALL hiện một thanh màu nhỏ nổi cạnh vùng chọn khi người dùng chọn text. Thanh màu gồm các màu dùng gần đây, nút mở Highlight Palette và nút mở color picker. Menu chuột phải trên vùng chọn SHALL có "Mark with color".

#### Scenario: Chọn rồi tô
- **WHEN** người dùng chọn text và bấm màu đầu tiên trên thanh nổi
- **THEN** đoạn đó được tô màu đó và thanh nổi đóng lại

#### Scenario: Bỏ chọn
- **WHEN** người dùng bấm ra ngoài vùng chọn
- **THEN** thanh nổi đóng và không có Marker nào được tạo

### Requirement: Đổi màu và gỡ Marker
Menu chuột phải trên text đã tô SHALL có "Change color" và "Remove marker". "Remove marker" SHALL gỡ cả Marker chứa vị trí đó. Tô chồng lên một Marker cũ SHALL cắt Marker cũ, phần chồng nhận màu mới.

#### Scenario: Gỡ Marker
- **WHEN** người dùng chuột phải lên một Marker và chọn "Remove marker"
- **THEN** Marker đó biến mất, các Marker khác trên dòng vẫn còn

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
