## MODIFIED Requirements

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
