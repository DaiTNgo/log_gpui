> **Đã bị thay thế một phần (ADR-0015).** Extraction Rule, "Make column…" và Column Label đã bị gỡ; Log Format là cách duy nhất để định nghĩa cột, và mục Columns chỉ bật/tắt cột của Focus Pane. Các nhắc đến Extraction Rule không còn hiệu lực. Các phần nhắc đến chúng dưới đây chỉ còn là lịch sử, không còn đúng với app. Xem `docs/adr/0015-log-format-is-the-only-way-to-define-columns.md` và `CONTEXT.md`.

## ADDED Requirements

### Requirement: Field Column là nguồn cột của Filter
Mọi Field Column đang hiện SHALL là nguồn cột của Filter, theo cùng luật Filterable Column (tối đa 500 giá trị riêng biệt), Hide Mode, Custom Value và số đếm như cột phát hiện được. Filter Value của Field Column SHALL là text nguyên văn của ô, chỉ gộp các giá trị khác nhau về hoa thường.

#### Scenario: Lọc theo Extraction Rule
- **WHEN** Extraction Rule `user` đang bật và cột `user` có 12 giá trị riêng biệt
- **THEN** người dùng thêm được Filter cho cột `user` với 12 checkbox

#### Scenario: Lọc theo trường Log Format
- **WHEN** Log Format có trường `client` với hơn 500 IP khác nhau
- **THEN** cột `client` hiện mờ kèm Distinct Count và không thêm Filter Value được

### Requirement: Filter hiện Column Label
Filter SHALL hiện Column Label của cột nếu có, thay cho tên trường. Điều kiện Filter SHALL được lưu theo tên trường, nên đổi Column Label SHALL NOT làm mất hay đổi điều kiện.

#### Scenario: Đổi Label khi đang lọc
- **WHEN** Filter có `module` = `DB` và người dùng đổi Column Label của MODULE thành `Service`
- **THEN** mục Filter hiện `Service` với `DB` vẫn được tick

### Requirement: Điều kiện trên cột không còn được đọc
Khi một cột có điều kiện Filter không còn được đọc (Extraction Rule bị tắt hoặc xoá, Log Format bị đổi, Detected Field bị tắt), hệ thống SHALL giữ điều kiện, SHALL coi mọi dòng là không có giá trị ở cột đó, và SHALL hiện chú thích "column no longer read" ở mục Filter của cột. Bỏ điều kiện SHALL làm mục Filter của cột đó biến mất.

#### Scenario: Tắt Extraction Rule
- **WHEN** Filter chỉ hiện `user` = `alice` và người dùng tắt Extraction Rule `user`
- **THEN** điều kiện vẫn còn kèm chú thích, và Focus Pane không còn dòng nào qua Filter vì không dòng nào có giá trị `user`

### Requirement: Focus Pane giải thích khi Filter dùng cột không còn được đọc
Khi Focus Pane không còn dòng nào qua Filter, và trong Filter có điều kiện trên một cột không còn được đọc, Focus Pane SHALL hiện empty state nêu tên cột đó (theo Column Label nếu có), lý do (rule bị tắt, rule bị xoá, Log Format bị đổi, Detected Field bị tắt), và hai nút: "Remove condition", và "Turn column back on" khi cột vẫn bật lại được.

#### Scenario: Tắt rule khi đang lọc
- **WHEN** Filter chỉ hiện `user` = `alice` và người dùng tắt Extraction Rule `user`
- **THEN** Focus Pane hiện "No lines: Filter uses user, which is no longer read (rule turned off)" cùng nút "Remove condition" và "Turn column back on"

#### Scenario: Bật lại
- **WHEN** người dùng bấm "Turn column back on" trong empty state đó
- **THEN** Extraction Rule `user` được bật lại và Focus Pane hiện lại các dòng `alice`

## MODIFIED Requirements

### Requirement: Không còn các điểm vào cũ
Hệ thống SHALL NOT có panel "View", danh sách module cố định, "Saved View", panel Marker, hay menu nhanh để lọc theo giá trị của ô. Menu chuột phải trên ô SHALL chỉ có các lệnh tô màu ("Mark with color", "Highlight rows where…") và "Make column…". Ghim cột trên Column Header, đổi độ rộng cột và Auto-fit SHALL vẫn hoạt động. Density và công tắc Source Column SHALL nằm trong Quick Settings trên toolbar, và Quick Settings SHALL chỉ chứa các công tắc hiển thị. Hiện/ẩn và thứ tự cột, Extraction Rule và việc chọn Log Format SHALL nằm ở mục Columns của context sidebar.

#### Scenario: Các mục của context sidebar
- **WHEN** người dùng mở context sidebar
- **THEN** các mục là Filter, Highlights, Columns và Inspector, không có View hay Marker

#### Scenario: Ghim cột vẫn dùng được
- **WHEN** người dùng bấm Pin Toggle trên Column Header `LEVEL`
- **THEN** cột `LEVEL` được ghim như trước

#### Scenario: Density trong Quick Settings
- **WHEN** người dùng mở Quick Settings trên toolbar
- **THEN** có lựa chọn density và công tắc "Show source file", và không có danh sách cột
