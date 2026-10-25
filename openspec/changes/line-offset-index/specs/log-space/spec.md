## MODIFIED Requirements

### Requirement: Tab theo view
Hệ thống SHALL mở mỗi Log Space và mỗi File View trong một tab riêng. Mở một Log Space hoặc File View đã có tab SHALL kích hoạt tab đó thay vì mở tab mới. Một tab SHALL có thể nằm trên tab strip như một Dormant Tab mà file của nó chưa được đọc; file chỉ được đọc khi tab được chọn.

#### Scenario: Mở lại
- **WHEN** tab "Payment" đang mở và người dùng bấm "Payment" trên sidebar
- **THEN** tab "Payment" được kích hoạt và số tab không đổi

#### Scenario: Log Space và File View là hai tab
- **WHEN** tab "Payment" đang mở và người dùng bấm `api.log` của "Payment"
- **THEN** một tab mới cho File View `api.log` được mở

#### Scenario: Tab chưa được chọn
- **WHEN** app vừa mở với tab "Payment" và tab "Billing", trong đó "Payment" đang chọn
- **THEN** tab "Billing" nằm trên tab strip như Dormant Tab và file của nó chưa được đọc
