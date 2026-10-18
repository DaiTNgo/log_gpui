## MODIFIED Requirements

### Requirement: Search chỉ tìm trên dòng Filter đang hiển thị
Trong Parsed View, hệ thống SHALL chỉ tìm và đếm Match trên các dòng Filter đang hiển thị, và SHALL NOT đếm dòng khớp nhưng đang bị Filter ẩn. Trong Raw View, Filter không áp dụng, nên hệ thống SHALL tìm và đếm Match trên mọi dòng của mọi File Section.

#### Scenario: Dòng bị Filter ẩn
- **WHEN** đang ở Parsed View, Filter ẩn mọi dòng `DEBUG` và người dùng gõ `timeout`, trong đó 5 dòng `DEBUG` có `timeout` và 12 dòng khác có `timeout`
- **THEN** tổng số Match là 12

#### Scenario: Raw View đếm mọi dòng
- **WHEN** cùng Filter và Search Expression như trên nhưng người dùng chuyển sang Raw View
- **THEN** tổng số Match là 17
