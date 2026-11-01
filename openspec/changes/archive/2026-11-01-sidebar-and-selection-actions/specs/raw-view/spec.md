## ADDED Requirements

### Requirement: Menu chuột phải trong Raw View
Chuột phải lên vùng chọn trong Raw View (cả danh sách chỉ đọc lẫn editor scratch) SHALL mở Selection Menu, không có nhóm lọc vì Raw View không áp dụng Filter. Chọn text trong Raw View SHALL NOT tự mở thanh màu.

#### Scenario: Tô trong Raw View
- **WHEN** người dùng chọn `timeout` trong Raw View của `api.log`, chuột phải và bấm màu đỏ ở hàng "Mark here"
- **THEN** `timeout` ở dòng đó có Marker đỏ

#### Scenario: Editor scratch
- **WHEN** người dùng chọn text trong editor của `untitled.log` rồi chuột phải
- **THEN** Selection Menu mở với Copy và các mục tô màu
