# parsed-view-fidelity

## Purpose
Parsed View hiện đúng text lưu trong file, mỗi dòng một hàng.

## Requirements

### Requirement: Ô hiện đúng text lưu trong file
Mọi ô của Parsed View lấy từ dòng log SHALL hiện đúng các ký tự của dòng đó như lưu trong file. Hệ thống SHALL NOT viết lại text hiển thị: không chuẩn hóa level, không định dạng lại timestamp, không giải mã escape. Gutter, Source Column và dấu `_` của Continuation Line không thuộc quy định này. Hệ thống SHALL NOT tự điền giá trị không có trong dòng, ví dụ module mặc định hay level đoán từ chữ trong dòng.

#### Scenario: Level viết tắt
- **WHEN** dòng log là `2026-10-18 10:00:01 err payment failed`
- **THEN** ô LEVEL hiện `err`, không phải `ERROR`

#### Scenario: Timestamp nguyên văn
- **WHEN** dòng log bắt đầu bằng `2026-10-18T10:32:02.441+07:00`
- **THEN** ô TIMESTAMP hiện `2026-10-18T10:32:02.441+07:00`

#### Scenario: Chuỗi JSON có escape
- **WHEN** dòng JSON có `"msg":"say \"hi\""`
- **THEN** ô `msg` hiện `say \"hi\"`

#### Scenario: Không tự điền giá trị
- **WHEN** dòng log là `connection reset by peer` và không có định dạng nào đọc được
- **THEN** ô MODULE không hiện `app`, và ô LEVEL không hiện level đoán từ chữ trong dòng

#### Scenario: Raw và Parsed giống nhau
- **WHEN** người dùng gõ `err` vào một dòng scratch rồi chuyển sang Parsed View
- **THEN** ô LEVEL của dòng đó hiện `err`

### Requirement: App vẫn hiểu nghĩa giá trị
Hệ thống SHALL dùng nghĩa đã hiểu của level và timestamp để sắp xếp theo thời gian, lọc theo Time Range và đặt Level Marker, mà không đổi text hiển thị.

#### Scenario: Level Marker cho level viết tắt
- **WHEN** ô LEVEL hiện `E`
- **THEN** Gutter của dòng đó có Level Marker lỗi

### Requirement: Nhận diện Continuation Line
Hệ thống SHALL coi một dòng là Continuation Line khi dòng đó không có timestamp đọc được và nằm sau một dòng có timestamp. Parent Line của nó SHALL là dòng gần nhất phía trên có timestamp riêng. Dòng nằm trước timestamp đầu tiên của file SHALL NOT có Parent Line. Hệ thống SHALL NOT nhận diện stack trace theo mẫu text.

#### Scenario: Stack trace Java
- **WHEN** dòng 10 có timestamp, dòng 11 là `    at PaymentService.charge(...)` và dòng 12 là `Caused by: IllegalStateException`, cả hai không có timestamp
- **THEN** dòng 11 và 12 là Continuation Line với Parent Line là dòng 10

#### Scenario: Trước timestamp đầu tiên
- **WHEN** dòng 1 và 2 của file không có timestamp và dòng 3 là dòng đầu tiên có timestamp
- **THEN** dòng 1 và 2 không có Parent Line

### Requirement: Mỗi dòng lưu là một hàng
Hệ thống SHALL hiện mỗi dòng lưu trong file thành một hàng riêng trong Parsed View, kể cả Continuation Line và dòng trống, sao cho số dòng khớp với Raw View. Hệ thống SHALL NOT gộp dòng, và SHALL NOT có nút mở rộng hay thu gọn stack trace.

#### Scenario: Stack trace
- **WHEN** dòng 10 là dòng lỗi và dòng 11–15 là stack trace
- **THEN** Parsed View có 6 hàng với số dòng 10 đến 15

#### Scenario: Dòng trống
- **WHEN** dòng 20 của file là dòng trống
- **THEN** Parsed View có một hàng số 20 không có nội dung

### Requirement: Ô của Continuation Line
Hệ thống SHALL hiện text của Continuation Line ở cột MESSAGE, và SHALL hiện dấu `_` màu nhạt ở mọi ô khác lấy từ dòng. Dấu `_` SHALL NOT chọn được, SHALL NOT nhận Marker, SHALL NOT được Search khớp, SHALL NOT được chép bằng ⌘C, và menu chuột phải trên ô đó SHALL NOT có "Highlight rows where…".

#### Scenario: Hiển thị
- **WHEN** dòng 11 là Continuation Line của dòng 10 có level `ERROR` và module `api`
- **THEN** ô TIMESTAMP, LEVEL, MODULE của hàng 11 hiện `_` màu nhạt, và ô MESSAGE hiện text của dòng 11

#### Scenario: Chép hàng tiếp nối
- **WHEN** người dùng chọn trọn hàng 11 rồi bấm ⌘C
- **THEN** clipboard chỉ chứa text gốc của dòng 11, không có `_`

### Requirement: Continuation Line dùng giá trị của Parent Line
Filter, Time Range và Highlight Rule SHALL xử lý Continuation Line theo các giá trị của Parent Line. Continuation Line SHALL được hiện khi và chỉ khi Parent Line của nó được hiện. Search và Marker SHALL tính trên text của chính Continuation Line.

#### Scenario: Lọc theo level
- **WHEN** Filter chỉ hiện `level` = `ERROR` và dòng 10 là `ERROR` kèm stack trace dòng 11–15
- **THEN** dòng 10–15 đều được hiện

#### Scenario: Highlight Rule
- **WHEN** rule đỏ `level` = `ERROR` đang bật và dòng 10 là `ERROR` kèm stack trace dòng 11–15
- **THEN** hàng 10–15 đều được tô đỏ

#### Scenario: Search trong stack trace
- **WHEN** người dùng tìm `NullPointerException` chỉ có ở dòng 12
- **THEN** chỉ dòng 12 là Match

### Requirement: Số đếm và Level Marker chỉ tính Parent Line
Số dòng của Filter Value SHALL NOT đếm Continuation Line. Level Marker SHALL chỉ đặt trên Parent Line, không đặt trên Continuation Line.

#### Scenario: Đếm lỗi
- **WHEN** file có một dòng `ERROR` kèm 20 dòng stack trace
- **THEN** Filter Value `ERROR` hiện số đếm 1

#### Scenario: Level Marker
- **WHEN** dòng 10 là `ERROR` kèm stack trace dòng 11–15
- **THEN** chỉ Gutter của dòng 10 có Level Marker lỗi

### Requirement: Chọn text qua nhiều ô và nhiều hàng
Hệ thống SHALL cho kéo chuột để chọn text trong Parsed View, qua nhiều ô và nhiều hàng. Hàng đầu được chọn từ điểm bắt đầu tới cuối dòng, các hàng giữa được chọn cả dòng, hàng cuối từ đầu dòng tới điểm thả. Kéo ra mép SHALL làm canvas tự cuộn. Bấm mà không kéo SHALL chọn hàng như hiện nay.

#### Scenario: Kéo qua ba hàng
- **WHEN** người dùng kéo từ giữa ô MESSAGE của hàng 3 tới giữa ô LEVEL của hàng 5
- **THEN** phần cuối hàng 3, cả hàng 4, và phần đầu hàng 5 tới giữa ô LEVEL được tô vùng chọn

#### Scenario: Tự cuộn
- **WHEN** người dùng kéo chọn xuống quá mép dưới canvas
- **THEN** canvas cuộn xuống và vùng chọn mở rộng theo

### Requirement: Chép vùng chọn ra text gốc
Hệ thống SHALL chép vào clipboard đúng text lưu trong file của vùng chọn khi người dùng bấm ⌘C, mỗi hàng một dòng. Hàng bị Filter ẩn SHALL NOT được chép.

#### Scenario: Chép nhiều hàng
- **WHEN** người dùng chọn trọn hàng 4 và hàng 5 rồi bấm ⌘C
- **THEN** clipboard chứa đúng hai dòng 4 và 5 như trong file

#### Scenario: Hàng bị ẩn
- **WHEN** vùng chọn đi từ hàng 3 tới hàng 7 và hàng 5 bị Filter ẩn
- **THEN** clipboard không chứa dòng 5

### Requirement: Filter Value của LEVEL hiện nguyên văn
Hệ thống SHALL liệt kê Filter Value của cột LEVEL theo text nguyên văn, chỉ gộp các giá trị khác nhau về hoa thường.

#### Scenario: Hai cách viết
- **WHEN** file có các dòng level `E`, `ERROR` và `error`
- **THEN** Filter cột LEVEL có hai checkbox: `E` và `ERROR` (gộp với `error`)
