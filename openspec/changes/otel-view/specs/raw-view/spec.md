## MODIFIED Requirements

### Requirement: Chuyển giữa Raw View và Parsed View
Hệ thống SHALL có nút chuyển Raw/Parsed trên toolbar cho mọi Log Space và File View. Imported Log Space SHALL mở ở Parsed View. Scratch Log Space SHALL mở ở Raw View khi mọi file đều rỗng; nếu không, SHALL mở ở chế độ dùng lần cuối. Khi OtelInput của một Log Space có ít nhất một span hoặc record, nút chuyển SHALL có thêm lựa chọn thứ ba là OTel. Lựa chọn OTel SHALL không có trong File View. Nếu chế độ dùng lần cuối là OTel nhưng Log Space không còn OtelInput, Log Space SHALL mở ở Parsed View.

#### Scenario: Mở Imported
- **WHEN** người dùng mở Imported Log Space "Payment" lần đầu
- **THEN** Parsed View được hiện

#### Scenario: Mở Scratch rỗng
- **WHEN** người dùng mở Scratch Log Space vừa tạo
- **THEN** Raw View được hiện và con trỏ nằm trong `untitled.log`

#### Scenario: Nhớ chế độ
- **WHEN** người dùng chuyển một Scratch Log Space có nội dung sang Parsed, đóng tab rồi mở lại
- **THEN** Parsed View được hiện

#### Scenario: Có OtelInput
- **WHEN** adapter dựng được OtelInput khác rỗng cho Log Space "Checkout"
- **THEN** nút chuyển của "Checkout" có ba lựa chọn: Raw, Parsed và OTel

#### Scenario: OtelInput mất
- **WHEN** Log Space lần trước đóng ở OTel, nay mở lại nhưng không còn file OTLP nào
- **THEN** Log Space mở ở Parsed View và nút OTel không hiện
