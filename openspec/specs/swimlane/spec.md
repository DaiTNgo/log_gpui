# swimlane Specification

## Purpose
Swimlane: dải riêng ngay dưới Time Histogram, cùng trục thời gian, chia các dòng của view thành làn theo giá trị của một cột (Lane By, mặc định là Log Name Column), để thấy nguồn nào lỗi trước, nguồn nào im khi nguồn khác bùng lên. Bật/tắt và thu gọn riêng với histogram.
## Requirements
### Requirement: Làn dưới Time Histogram
Hệ thống SHALL vẽ các làn trong một dải riêng (dải Swimlane) ngay dưới Time Histogram, trên cùng trục thời gian. Dải Swimlane SHALL bật/tắt riêng bằng nút "Lanes" (nhớ cho mọi view như nút Histogram), SHALL hiện được cả khi Time Histogram tắt, và SHALL có nút thu gọn riêng; thu gọn chỉ còn header. Zoom, khung vùng đang xem và dải Time Range của Time Histogram SHALL trải qua cả các làn. Mỗi làn SHALL là một giá trị của cột Lane By, có tên ở đầu làn. Trong mỗi cột thời gian, làn SHALL có một vạch khi có ít nhất một Parent Line của giá trị đó; vạch tô màu theo level cao nhất trong cột đó: Error/Fatal đỏ, Warn cam, còn lại xám. Khi dải Swimlane tắt, hệ thống SHALL NOT đếm làn.

#### Scenario: Tắt làn, giữ histogram
- **WHEN** người dùng tắt nút "Lanes"
- **THEN** dải Swimlane biến mất, Time Histogram vẫn hiện và lấy lại phần lề của cột tên làn

#### Scenario: Tắt histogram, giữ làn
- **WHEN** người dùng tắt nút "Histogram" trong khi nút "Lanes" đang bật
- **THEN** dải Swimlane vẫn hiện các làn, kèm thời điểm đầu và cuối của trục ở header

#### Scenario: Hai service
- **WHEN** Lane By là `service`, `db` có dòng Error lúc `10:30` và `api` có dòng Error lúc `10:31`
- **THEN** làn `db` có vạch đỏ ở cột `10:30`, và làn `api` có vạch đỏ ở cột `10:31`

#### Scenario: Zoom chung
- **WHEN** người dùng ⌘ + cuộn để zoom vào `10:20`–`10:50` trên Time Histogram
- **THEN** các làn cũng chỉ hiện `10:20`–`10:50`

### Requirement: Lane By
Dải Swimlane SHALL có menu "Lane by" ở đầu, liệt kê "Không chia" và mọi Filterable Column của view, kể cả Source Column và Log Name Column. Chọn "Không chia" SHALL ẩn mọi làn. Cột vượt giới hạn giá trị riêng biệt của Filter SHALL hiện trong menu ở dạng không chọn được, kèm lý do. Lane By SHALL được nhớ theo view, như Filter.

#### Scenario: Chọn cột
- **WHEN** người dùng chọn Lane by `thread`
- **THEN** mỗi làn là một giá trị của `thread`

#### Scenario: Cột quá nhiều giá trị
- **WHEN** cột `request_id` có hơn 1000 giá trị riêng biệt
- **THEN** `request_id` trong menu không chọn được và ghi rõ cột có quá nhiều giá trị

#### Scenario: Khôi phục
- **WHEN** người dùng chọn Lane by `service`, đóng tab rồi mở lại
- **THEN** Lane by vẫn là `service`

### Requirement: Lane By mặc định
Khi view chưa từng chọn Lane By: nếu Log Space có từ hai giá trị Log Name Column, Lane By SHALL là Log Name Column. Ngược lại Lane By SHALL là "Không chia". Khi đó, nếu view có cột MODULE hoặc Field Column tên `service`, `module`, `component`, `thread`, `host` hay `pod`, menu SHALL đưa cột đầu tiên trong số đó lên đầu danh sách kèm nhãn gợi ý, và SHALL NOT tự chọn cột đó.

#### Scenario: Nhiều service mỗi file một service
- **WHEN** Log Space mới có `api.log`, `api.log.1` và `db.log`
- **THEN** Lane By là Log Name Column với hai làn `api.log` và `db.log`

#### Scenario: File bị cắt khúc
- **WHEN** Log Space mới có `message.0` và `message.1`, và Log Format có field `service`
- **THEN** không có làn nào, và menu gợi ý `service`

#### Scenario: File View
- **WHEN** người dùng mở File View `app.log`
- **THEN** Lane By là "Không chia"

### Requirement: Giới hạn số làn
Hệ thống SHALL hiện tối đa 8 làn, là những giá trị có nhiều Parent Line nhất trong toàn view, xếp theo số dòng giảm dần. Mọi giá trị còn lại SHALL gom vào một làn "Other · n" ở cuối.

#### Scenario: Nhiều thread
- **WHEN** Lane By là `thread` và cột có 300 giá trị
- **THEN** có 8 làn của 8 thread nhiều dòng nhất và một làn "Other · 292"

### Requirement: Dòng không có giá trị
Continuation Line SHALL nằm cùng làn với Parent Line của nó và SHALL NOT tự tạo vạch. Parent Line không có giá trị ở cột Lane By SHALL vào làn "(trống)". Làn này đứng ngay trước "Khác" và chỉ hiện khi có dòng như vậy.

#### Scenario: Dòng không parse được
- **WHEN** Lane By là `service` và có 12 Parent Line không có `service`
- **THEN** có làn "(trống)" chứa 12 dòng đó

#### Scenario: Stack trace
- **WHEN** một dòng `service=db` có 40 Continuation Line
- **THEN** cả 41 dòng thuộc làn `db`, và làn "(trống)" không xuất hiện vì chúng

### Requirement: Tương tác với làn
Bấm tên một làn SHALL thêm giá trị của làn đó vào Filter của cột Lane By, như tích giá trị đó trong sidebar Filter. Bấm lại tên một làn đang được tích SHALL bỏ tích giá trị đó, và tên làn đang được tích SHALL được tô nổi. Bấm tên làn "Khác" hoặc "(trống)" SHALL NOT đổi Filter. Rê chuột lên một làn SHALL hiện ở header của dải tên làn, khoảng thời gian, số Parent Line và level cao nhất trong cột đó. Làn của Source Column hoặc Log Name Column SHALL có tên theo màu của file; vạch chỉ có dòng không phải Warn/Error SHALL là một vạch mảnh, nhạt. Bấm một vạch SHALL đặt dòng hiện tại của view thành dòng đầu tiên của giá trị đó trong cột thời gian đó, và SHALL NOT thu hẹp gì.

#### Scenario: Lọc theo làn
- **WHEN** người dùng bấm tên làn `order-service`
- **THEN** Filter của `service` có `order-service` được tích, và Focus Pane chỉ liệt kê dòng của `order-service`

#### Scenario: Nhảy từ vạch
- **WHEN** người dùng bấm vạch của làn `db` ở cột `10:30`
- **THEN** danh sách cuộn tới và chọn dòng đầu tiên của `db` lúc `10:30`, còn Filter không đổi

### Requirement: Làn theo Live và Filter
Các làn SHALL được cập nhật cùng lúc với Time Histogram khi có dòng mới, khi trục đổi và khi Indexing tiến thêm. Thứ tự và tập 8 làn SHALL chỉ được chọn lại khi tính lại toàn bộ, không phải ở mỗi lần cộng dồn. Các làn SHALL đếm mọi dòng của view, không theo Filter.

#### Scenario: Giá trị mới khi tail
- **WHEN** Lane By là `service` với 3 làn, và một dòng `service=auth` mới tới
- **THEN** vạch của dòng đó hiện ngay trong làn "Khác" hoặc trong làn `auth` nếu đã còn chỗ, và thứ tự các làn đang có không nhảy

#### Scenario: Filter không làm mất làn
- **WHEN** Lane By là `service` và Filter chỉ cho qua `service` = `db`
- **THEN** các làn khác vẫn hiện đủ vạch

