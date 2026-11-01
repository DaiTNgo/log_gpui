## MODIFIED Requirements

### Requirement: Thanh màu nổi khi chọn text
Hệ thống SHALL hiện một thanh màu nhỏ nổi cạnh vùng chọn khi người dùng chọn text bằng cách kéo chuột trong Parsed View hoặc trong Raw View chỉ đọc. Trong Raw View đang sửa được (editor của một file scratch), chọn text SHALL NOT tự mở thanh màu, dù chọn bằng chuột hay bàn phím. Thanh màu gồm các màu dùng gần đây, nút mở Highlight Palette và nút mở color picker. Menu chuột phải trên vùng chọn SHALL có "Mark with color".

#### Scenario: Chọn rồi tô
- **WHEN** người dùng chọn text và bấm màu đầu tiên trên thanh nổi
- **THEN** đoạn đó được tô màu đó và thanh nổi đóng lại

#### Scenario: Bỏ chọn
- **WHEN** người dùng bấm ra ngoài vùng chọn
- **THEN** thanh nổi đóng và không có Marker nào được tạo

#### Scenario: Chọn trong editor scratch
- **WHEN** người dùng kéo chuột, dùng Shift+mũi tên hoặc Cmd+A để chọn text trong Raw View của `untitled.log` đang sửa được
- **THEN** text được chọn và không có thanh màu nào hiện

## ADDED Requirements

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
