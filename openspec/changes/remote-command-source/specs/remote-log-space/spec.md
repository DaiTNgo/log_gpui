## ADDED Requirements

### Requirement: Thành viên của Remote Log Space
Remote Log Space SHALL chỉ chứa Remote File và Remote Command, có thể từ nhiều Server, và có thể có cả hai loại cùng lúc. Imported Log Space và Scratch Log Space SHALL NOT nhận Remote Command.

#### Scenario: File và lệnh trong một Log Space
- **WHEN** người dùng tạo Remote Log Space "Checkout" với `web1:/app/app.log` và `web1$ journalctl -f -u nginx`
- **THEN** Log Space mở ra với dòng của cả hai xếp xen theo timestamp

#### Scenario: Imported Log Space
- **WHEN** người dùng mở menu thêm trên một Imported Log Space
- **THEN** không có mục "Add remote command…"

### Requirement: Download all bỏ qua Remote Command
"Download all" trên Remote Log Space SHALL chỉ tải các Remote File và SHALL NOT tải output của Remote Command. Khi Log Space có Remote Command, hệ thống SHALL nói trước khi tải rằng các Remote Command bị bỏ qua. Remote Command SHALL NOT có mục Download.

#### Scenario: Download all có lệnh
- **WHEN** "Checkout" có hai Remote File và một Remote Command và người dùng chọn "Download all"
- **THEN** hệ thống báo "1 remote command will be skipped", tải hai file, và đề nghị mở chúng thành Imported Log Space
