## ADDED Requirements

### Requirement: Tách Focus Pane ra Focus Window
Focus Pane SHALL có nút "Move to window", và command palette SHALL có lệnh tương ứng. Lệnh này SHALL mở một Focus Window, là cửa sổ hệ điều hành riêng, đổi kích thước được và kéo sang màn hình khác được, rồi chuyển Focus Pane sang đó. Trong lúc Focus Window mở, cửa sổ chính SHALL NOT hiện Focus Pane, và view SHALL chiếm toàn bộ chiều cao.

#### Scenario: Tách ra
- **WHEN** Focus Pane đang mở dưới cửa sổ chính và người dùng bấm "Move to window"
- **THEN** một Focus Window mở ra chứa Focus Pane, và cửa sổ chính không còn Focus Pane

#### Scenario: Chỉ một Focus Window
- **WHEN** Focus Window đang mở và người dùng chạy lại lệnh "Move to window"
- **THEN** không có cửa sổ mới nào, và Focus Window hiện có được đưa lên trước

### Requirement: Nội dung Focus Window
Focus Window SHALL chứa ô Search của Focus Tab đang mở, dải Focus Tab, ô Focus Find và danh sách dòng của Focus Tab. Ô Search ở Focus Window và ô Search trên toolbar của cửa sổ chính SHALL cùng hiện và sửa một Search Expression. Focus Window SHALL NOT chứa Filter.

#### Scenario: Sửa Search ở Focus Window
- **WHEN** người dùng gõ `timeout` vào ô Search của Focus Window
- **THEN** ô Search ở cửa sổ chính cũng hiện `timeout`, view tô Match của `timeout`, và Focus Tab liệt kê các dòng đó

#### Scenario: Filter ở cửa sổ chính
- **WHEN** người dùng tích `ERROR` ở sidebar Filter của cửa sổ chính
- **THEN** Focus Tab trong Focus Window chỉ liệt kê dòng `ERROR`

### Requirement: Focus Window theo view của cửa sổ chính
Focus Window SHALL luôn hiện Focus Pane của view đang mở ở cửa sổ chính. Dòng hiện tại SHALL đồng bộ giữa view và Focus Window như khi Focus Pane nằm dưới cửa sổ chính. Khi tab đang mở ở cửa sổ chính không phải một file log, Focus Window SHALL hiện trạng thái trống.

#### Scenario: Đổi tab ở cửa sổ chính
- **WHEN** Focus Window đang hiện Focus Tab của "Payment" và người dùng mở tab File View `api.log` ở cửa sổ chính
- **THEN** Focus Window hiện các Focus Tab của File View `api.log`

#### Scenario: Bấm dòng ở Focus Window
- **WHEN** người dùng bấm dòng 1200 trong Focus Window
- **THEN** view ở cửa sổ chính cuộn tới dòng 1200 và chọn nó

### Requirement: Đưa Focus Pane về cửa sổ chính
Focus Window SHALL có nút "Move back". Bấm "Move back" hoặc đóng Focus Window SHALL đóng Focus Window và hiện lại Focus Pane dưới cửa sổ chính, với chiều cao như trước khi tách. Focus Tab, Search Expression, Focus Find và dòng hiện tại SHALL giữ nguyên.

#### Scenario: Move back
- **WHEN** người dùng bấm "Move back" trên Focus Window đang có tab 1 và tab 3
- **THEN** Focus Window đóng, Focus Pane hiện lại dưới cửa sổ chính với tab 1 và tab 3 như cũ

#### Scenario: Đóng cửa sổ
- **WHEN** người dùng đóng Focus Window bằng nút đóng của hệ điều hành
- **THEN** Focus Pane hiện lại dưới cửa sổ chính

#### Scenario: Đóng cửa sổ chính
- **WHEN** người dùng đóng cửa sổ chính trong lúc Focus Window đang mở
- **THEN** Focus Window cũng đóng

### Requirement: Không nhớ Focus Window
Hệ thống SHALL NOT nhớ việc Focus Pane đang ở Focus Window. Khi khởi động, Focus Pane SHALL luôn ở dưới cửa sổ chính.

#### Scenario: Khởi động lại
- **WHEN** người dùng thoát app lúc Focus Window đang mở rồi mở lại app
- **THEN** không có Focus Window, và Focus Pane nằm dưới cửa sổ chính
