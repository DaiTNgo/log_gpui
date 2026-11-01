## MODIFIED Requirements

### Requirement: Mục Highlights
Context sidebar SHALL có mục Highlights liệt kê các rule theo thứ tự ưu tiên. Mỗi rule SHALL hiện ô màu, mô tả một dòng (cột và giá trị), số hàng khớp trong view đang mở và công tắc bật/tắt. Kéo cả hàng SHALL đổi thứ tự. Xoá rule SHALL làm được từ menu chuột phải của hàng, hoặc nút "⋯" hiện khi trỏ vào hàng.

Nút "+" trên header của mục SHALL mở form New rule: chọn cột, khối chọn giá trị dùng chung với Filter, và 8 màu kèm nút "…" mở Highlight Palette. Bấm "+" khi form đang mở SHALL giữ nguyên nội dung form.

#### Scenario: Thêm rule
- **WHEN** người dùng bấm "+", chọn cột `module`, tick `DB`, chọn màu cam rồi bấm Add
- **THEN** rule mới nằm cuối danh sách và các hàng `DB` được tô cam

#### Scenario: Đổi màu
- **WHEN** người dùng bấm ô màu của một rule và chọn màu khác
- **THEN** các hàng khớp đổi sang màu mới

#### Scenario: Bấm "+" lần nữa
- **WHEN** form New rule đang mở với `DB` đã tick và người dùng bấm "+" lần nữa
- **THEN** form vẫn có `DB` đã tick

### Requirement: Tạo nhanh từ ô
Selection Menu SHALL có "Highlight rows where <cột> = <giá trị>…" khi vùng chọn phủ trọn giá trị một ô, hoặc khi chuột phải lên một ô mà không có vùng chọn. Mục này mở form New rule đã điền cột, giá trị và màu kế tiếp trong palette. Trên MESSAGE, giá trị là text đã chọn và được dùng làm Custom Value. Mục này SHALL NOT có trên TIMESTAMP.

#### Scenario: Tạo nhanh
- **WHEN** người dùng chuột phải lên ô `module` có giá trị `DB`, chọn "Highlight rows where module = DB…" rồi bấm Add
- **THEN** rule `module` = `DB` xuất hiện trong mục Highlights và các hàng `DB` được tô

#### Scenario: Không có trên TIMESTAMP
- **WHEN** người dùng chuột phải lên một ô TIMESTAMP
- **THEN** menu không có "Highlight rows where …"
