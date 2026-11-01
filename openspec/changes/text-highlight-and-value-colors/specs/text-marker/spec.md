## MODIFIED Requirements

### Requirement: Thanh màu nổi khi chọn text
Hệ thống SHALL hiện một thanh màu nhỏ nổi cạnh vùng chọn khi người dùng chọn text. Thanh màu gồm một nút chọn phạm vi hai phần, "Chỉ dòng này" (tạo Marker) và "Mọi chỗ “<text>”" (tạo Text Highlight), nằm trên các màu dùng gần đây, nút mở Highlight Palette và nút mở color picker. Phạm vi SHALL là "Chỉ dòng này" mỗi lần thanh mở. Alt+bấm một màu SHALL tạo Text Highlight dù nút chọn đang ở phạm vi nào. Menu chuột phải trên vùng chọn SHALL có "Mark with color".

#### Scenario: Chọn rồi tô
- **WHEN** người dùng chọn text và bấm màu đầu tiên trên thanh nổi mà không đổi phạm vi
- **THEN** đoạn đó được tô Marker màu đó và thanh nổi đóng lại

#### Scenario: Chọn phạm vi mọi chỗ
- **WHEN** người dùng chọn `conn-7f3a`, chọn phạm vi "Mọi chỗ có “conn-7f3a”" rồi bấm một màu
- **THEN** một Text Highlight được tạo, không có Marker nào được tạo, và thanh nổi đóng lại

#### Scenario: Alt+bấm
- **WHEN** nút chọn đang ở "Chỉ dòng này" và người dùng Alt+bấm một màu
- **THEN** một Text Highlight được tạo, không có Marker nào được tạo

#### Scenario: Mở lại thanh
- **WHEN** người dùng vừa tạo Text Highlight bằng phạm vi "Mọi chỗ" rồi chọn một đoạn text khác
- **THEN** thanh mới mở với phạm vi "Chỉ dòng này"

#### Scenario: Bỏ chọn
- **WHEN** người dùng bấm ra ngoài vùng chọn
- **THEN** thanh nổi đóng và không có Marker hay Text Highlight nào được tạo

### Requirement: Đổi màu và gỡ Marker
Menu chuột phải trên text đã tô SHALL có "Change color" và "Remove marker" khi ở đó có Marker, và "Change highlight color" và "Remove highlight" khi ở đó có Text Highlight; khi có cả hai thì menu có cả bốn mục. "Remove marker" SHALL gỡ cả Marker chứa vị trí đó và SHALL NOT gỡ Text Highlight. Tô chồng lên một Marker cũ SHALL cắt Marker cũ, phần chồng nhận màu mới.

#### Scenario: Gỡ Marker
- **WHEN** người dùng chuột phải lên một Marker và chọn "Remove marker"
- **THEN** Marker đó biến mất, các Marker khác trên dòng vẫn còn

#### Scenario: Gỡ Marker nằm trên Text Highlight
- **WHEN** `conn-7f3a` ở dòng 40 có Marker vàng nằm trên Text Highlight tím, và người dùng chọn "Remove marker"
- **THEN** `conn-7f3a` ở dòng 40 hiện lại nền tím của Text Highlight

#### Scenario: Tô chồng
- **WHEN** `abcdef` có Marker đỏ và người dùng tô xanh lên `cd`
- **THEN** `ab` và `ef` đỏ, `cd` xanh

### Requirement: Marker nằm trên Highlight Rule
Hệ thống SHALL vẽ màu Marker trên màu Text Highlight, màu Highlight Rule, màu hàng đang chọn và màu Match của Search. Màu chữ trên Marker SHALL được chọn theo độ tương phản, thay cho Value Color.

#### Scenario: Hàng được tô có Marker
- **WHEN** một hàng được Highlight Rule tô cam và có Marker xanh trên một đoạn
- **THEN** đoạn đó hiện xanh, phần còn lại của hàng hiện cam

#### Scenario: Marker trên Text Highlight
- **WHEN** `retry` có Text Highlight tím và người dùng đặt Marker vàng lên `retry` ở dòng 7
- **THEN** `retry` ở dòng 7 hiện vàng, `retry` ở các dòng khác vẫn tím
