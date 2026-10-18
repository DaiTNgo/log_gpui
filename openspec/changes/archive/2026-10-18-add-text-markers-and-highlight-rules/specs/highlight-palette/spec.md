## ADDED Requirements

### Requirement: Bảng màu cố định nhiều sắc độ
Hệ thống SHALL có Highlight Palette gồm 10 màu, mỗi màu 3 sắc độ (nhạt, vừa, đậm), dùng chung cho Marker và Highlight Rule. Mỗi màu SHALL giữ chữ đọc rõ ở cả theme sáng lẫn theme tối.

#### Scenario: Mở palette
- **WHEN** người dùng mở Highlight Palette từ thanh màu nổi
- **THEN** palette hiện 30 ô màu xếp theo 10 màu và 3 sắc độ

#### Scenario: Đổi theme
- **WHEN** người dùng đổi từ theme sáng sang theme tối
- **THEN** các đoạn và hàng đã tô vẫn đọc rõ chữ

### Requirement: Color picker tự do
Hệ thống SHALL có color picker để chọn màu bất kỳ cho Marker và Highlight Rule. Màu chữ trên nền đã tô SHALL được chọn tự động (sáng hoặc tối) theo độ tương phản.

#### Scenario: Màu tự chọn tối
- **WHEN** người dùng tô một đoạn bằng màu `#1A1A40`
- **THEN** chữ của đoạn đó hiện màu sáng

### Requirement: Màu gần đây
Hệ thống SHALL nhớ tối đa 8 màu dùng gần nhất, kể cả màu tự chọn, và SHALL hiện chúng ở hàng "Recent" của palette và trên thanh màu nổi.

#### Scenario: Dùng lại màu
- **WHEN** người dùng vừa tô một đoạn bằng màu tự chọn rồi chọn một đoạn khác
- **THEN** thanh màu nổi có màu đó ở đầu hàng Recent
