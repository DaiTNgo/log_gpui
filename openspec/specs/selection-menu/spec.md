# selection-menu Specification

## Purpose
Selection Menu: menu chuột phải duy nhất trên text ở Parsed View, Raw View và editor scratch, để copy, tô Marker hoặc Text Highlight, lọc theo giá trị ô, tạo Highlight Rule và gỡ màu có sẵn. Chọn text không tự mở gì.
## Requirements
### Requirement: Selection Menu trên vùng chọn
Chuột phải lên vùng chọn khác rỗng trong Parsed View, Raw View chỉ đọc hoặc editor scratch SHALL mở Selection Menu. Chuột phải lên một ô của Parsed View khi không có vùng chọn SHALL mở Selection Menu như thể giá trị ô đó đang được chọn trọn. Selection Menu SHALL chỉ chứa các mục dùng được cho vùng chọn đó, theo thứ tự: Copy; nhóm màu; nhóm lọc; nhóm gỡ màu; Log Format…

#### Scenario: Mở menu trên vùng chọn
- **WHEN** người dùng chọn `POST /checkout` trong ô MESSAGE rồi chuột phải lên chỗ đã chọn
- **THEN** Selection Menu mở với Copy, Mark here, Highlight everywhere, Color…, Show only và Hide

#### Scenario: Chuột phải lên ô không có vùng chọn
- **WHEN** không có vùng chọn và người dùng chuột phải lên ô `source` có giá trị `web`
- **THEN** Selection Menu mở với các mục cho `web`, gồm "Highlight rows where source = web…"

### Requirement: Copy trong Selection Menu
Selection Menu SHALL có "Copy" ở đầu, kèm phím tắt ⌘C. Copy SHALL chép vùng chọn như ⌘C.

#### Scenario: Copy
- **WHEN** người dùng chọn Copy trong Selection Menu
- **THEN** clipboard chứa đúng text đã chọn

### Requirement: Tô màu từ Selection Menu
Selection Menu SHALL có:
- Hàng "Mark here" với 6 màu. Bấm một màu SHALL tô Marker lên vùng chọn rồi đóng menu.
- Hàng "Highlight everywhere" với 6 màu. Bấm một màu SHALL tạo Text Highlight cho text đã chọn rồi đóng menu. Hàng này SHALL chỉ có khi vùng chọn nằm trong một dòng, và SHALL NOT hiện số chỗ khớp.
- Mục "Color…", kèm phím tắt của lệnh "Mark: Color Selection…". Mục này SHALL mở thanh màu đầy đủ cho vùng chọn.

6 màu SHALL là các màu dùng gần đây, bù bằng màu đầu của Highlight Palette khi chưa đủ. Màu vừa dùng SHALL vào danh sách màu dùng gần đây.

#### Scenario: Mark here
- **WHEN** người dùng chọn `timeout` ở dòng 42, chuột phải và bấm màu đỏ ở hàng "Mark here"
- **THEN** `timeout` ở dòng 42 có Marker đỏ, các chỗ `timeout` khác không đổi, và menu đóng

#### Scenario: Highlight everywhere
- **WHEN** người dùng chọn `timeout`, chuột phải và bấm màu tím ở hàng "Highlight everywhere"
- **THEN** mọi chỗ có `timeout` trong Project có nền tím

#### Scenario: Vùng chọn nhiều dòng
- **WHEN** vùng chọn đi từ dòng 3 tới dòng 5 và người dùng chuột phải
- **THEN** menu có "Mark here" nhưng không có "Highlight everywhere"

### Requirement: Lọc từ Selection Menu
Trong Parsed View, khi vùng chọn nằm trong một ô, Selection Menu SHALL có "Show only" và "Hide". Nhãn của hai mục SHALL ghi đúng điều kiện sẽ tạo:
- Chọn trọn giá trị của một ô thuộc cột lọc được SHALL tạo Filter Value trên cột đó.
- Chọn một phần ô, hoặc ô thuộc cột vượt ngưỡng giá trị riêng biệt, SHALL tạo Custom Value bằng text đã chọn trên cột đó.
- Chọn trong MESSAGE SHALL tạo Custom Value trên thẻ Message.

Nếu cột đã có điều kiện cùng chế độ (hiện hoặc Hide Mode), giá trị mới SHALL được thêm vào điều kiện đó. Nếu khác chế độ, điều kiện của cột SHALL được thay bằng giá trị mới với chế độ mới. Hai mục SHALL NOT có trong Raw View, và SHALL NOT có khi vùng chọn trải qua nhiều ô hoặc nhiều dòng.

#### Scenario: Show only trọn ô
- **WHEN** người dùng chuột phải lên ô `level` có giá trị `error` và chọn "Show only level = error"
- **THEN** Filter có `level` = `error` và chỉ các dòng `error` được hiển thị

#### Scenario: Hide một phần message
- **WHEN** người dùng chọn `POST /checkout` trong MESSAGE và chọn `Hide Message ∋ "POST /checkout"`
- **THEN** thẻ Message ở Hide Mode có Custom Value `POST /checkout`, và các dòng chứa nó bị ẩn

#### Scenario: Thêm vào điều kiện cùng chế độ
- **WHEN** Filter đang hiện `level` = `error` và người dùng chọn "Show only level = warn" trên một ô `warn`
- **THEN** Filter hiện `level` là `error` hoặc `warn`

#### Scenario: Không lọc trong Raw View
- **WHEN** người dùng chuột phải lên vùng chọn trong Raw View
- **THEN** menu không có Show only hay Hide

### Requirement: Highlight Rule từ Selection Menu
Khi vùng chọn phủ trọn giá trị một ô của cột dùng được cho Highlight Rule, Selection Menu SHALL có "Highlight rows where <cột> = <giá trị>…". Mục này SHALL mở form New rule trong mục Highlights, đã điền cột, giá trị và màu kế tiếp.

#### Scenario: Tạo rule từ ô
- **WHEN** người dùng chuột phải lên ô `module` = `DB` và chọn "Highlight rows where module = DB…"
- **THEN** form New rule mở với cột `module`, `DB` đã tick, và khi bấm Add thì các hàng `DB` được tô

### Requirement: Gỡ màu từ Selection Menu
Khi con trỏ lúc chuột phải nằm trên một Marker, Selection Menu SHALL có "Remove marker". Khi con trỏ nằm trên một Text Highlight, Selection Menu SHALL có "Remove highlight".

#### Scenario: Gỡ Marker
- **WHEN** người dùng chuột phải lên một đoạn có Marker và chọn "Remove marker"
- **THEN** Marker đó biến mất

