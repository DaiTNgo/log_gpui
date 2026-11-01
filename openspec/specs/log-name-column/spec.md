# log-name-column Specification

## Purpose
Log Name Column: cột của Log Space ghi Log Name của Source File (tên file bỏ hậu tố xoay vòng và nén, theo quy tắc Rotated File), để các khúc của một log (`app.log`, `app.log.1`, `message.0`, `message.1`) là một giá trị khi lọc, ghim và chia làn.
## Requirements
### Requirement: Log Name của Source File
Hệ thống SHALL suy ra Log Name của mỗi Source File từ tên của nó. Log Name là tên file sau khi bỏ hậu tố nén (`.gz`, `.bz2`, `.xz`, `.zst`, `.Z`) rồi bỏ hậu tố xoay vòng (`.<N>`, `-<YYYYMMDD>`, `-<YYYYMMDDhhmmss>`, `-<YYYY-MM-DD>`), theo đúng quy tắc nhận diện Rotated File. Tên không có hậu tố xoay vòng SHALL giữ nguyên, kể cả phần đuôi nén. Phần đứng trước tên file (thư mục, `<server>:`, `<archive> › `) SHALL được giữ nguyên.

#### Scenario: File bị cắt khúc
- **WHEN** Log Space có `message`, `message.0` và `message.1`
- **THEN** cả ba có Log Name `message`

#### Scenario: Bỏ cả hậu tố nén
- **WHEN** Log Space có `db.log.1.gz`, `db.log.2` và `db.log-20261029`
- **THEN** cả ba có Log Name `db.log`

#### Scenario: Khác Server là khác Log Name
- **WHEN** Log Space có `web1:/var/log/app.log.1` và `web2:/var/log/app.log`
- **THEN** Log Name là `web1:/var/log/app.log` và `web2:/var/log/app.log`

#### Scenario: Tên không phải xoay vòng
- **WHEN** Log Space có `api.log` và `trace.gz`
- **THEN** Log Name là `api.log` và `trace.gz`

### Requirement: Log Name Column
Hệ thống SHALL thêm Log Name Column vào Parsed View của Log Space, ghi Log Name của Source File của mỗi dòng. Log Name Column SHALL là Filterable Column và SHALL ghim được, với vị trí ngay sau Source Column trong thứ tự cột. Log Name Column SHALL mặc định ẩn và SHALL có công tắc hiện/ẩn trong mục Columns, nhớ theo từng Log Space. Filter đặt trên Log Name Column SHALL giữ hiệu lực khi cột bị ẩn.

#### Scenario: Lọc theo Log Name
- **WHEN** Log Space có `api.log`, `api.log.1` và `db.log`, và người dùng chỉ tích `api.log` ở Log Name Column
- **THEN** Focus Pane liệt kê dòng của cả `api.log` lẫn `api.log.1`, và không có dòng của `db.log`

#### Scenario: Ẩn mà vẫn lọc
- **WHEN** người dùng tắt Log Name Column trong mục Columns trong khi Filter trên Log Name Column đang bật
- **THEN** cột biến mất khỏi Parsed View và Filter vẫn giữ hiệu lực

### Requirement: Log Name Column theo file thêm hoặc bớt
Khi file được thêm vào hoặc bớt khỏi Log Space, hoặc khi tên của Remote File đổi, Log Name Column SHALL được tính lại cùng lúc với Source Column.

#### Scenario: Thêm khúc mới
- **WHEN** Log Space có `message.0` và người dùng thêm `message.1`
- **THEN** Log Name Column vẫn chỉ có một giá trị `message`, với số đếm gồm dòng của cả hai file

