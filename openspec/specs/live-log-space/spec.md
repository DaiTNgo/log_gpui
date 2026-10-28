# live-log-space Specification

## Purpose
Live Log Space theo dõi file local và Remote File đang được ghi trên một timeline, với Live/Pause, Follow và Generation.
## Requirements
### Requirement: Live Log Space trộn file local và Remote File
Một Live Log Space SHALL theo dõi mọi file của nó khi có dòng mới, dù file nằm trên máy hay trên server, và SHALL hiện chúng trên cùng một timeline. Live/Pause và Follow SHALL áp cho mọi file của space: Pause SHALL dừng nhận dòng mới của cả file local lẫn Remote File, và Live SHALL nhận mọi dòng được ghi trong lúc Pause. Mỗi file SHALL có trạng thái riêng, nên mất kết nối tới một server SHALL NOT dừng file local.

#### Scenario: Trộn trên một timeline
- **WHEN** Live Log Space có `api.log` (local) và `web1:/app/app.log`, và cả hai file có dòng mới
- **THEN** dòng mới của cả hai file xuất hiện trên một timeline, theo thời gian

#### Scenario: Pause dừng cả hai
- **WHEN** người dùng bấm Pause, cả hai file ghi thêm dòng, rồi người dùng bấm Live
- **THEN** trong lúc Pause view không đổi, và khi bấm Live mọi dòng mới của cả hai file xuất hiện

#### Scenario: Server mất kết nối
- **WHEN** kết nối tới `web1` bị mất trong lúc `api.log` vẫn được ghi thêm
- **THEN** dòng mới của `api.log` vẫn xuất hiện, và `web1:/app/app.log` có Break "disconnected"

### Requirement: Thao tác chỉ của Remote File
Load more, Open in full, Live Window, Back to Live Window, số dòng đang giữ ("N lines held") và Clear SHALL chỉ có khi Live Log Space có ít nhất một Remote File, và SHALL chỉ tác động lên Remote File. Một file local SHALL NOT có các thao tác đó, kể cả trong File View của nó. Clear SHALL bỏ các dòng đang giữ của Remote File và SHALL NOT bỏ dòng nào của file local.

#### Scenario: Live space chỉ có file local
- **WHEN** người dùng mở Live Log Space chỉ chứa `api.log` và `worker.log`
- **THEN** status bar chỉ có Live/Pause, Follow và trạng thái file, không có Load more, Live Window, "N lines held" hay Clear

#### Scenario: Clear trong space trộn
- **WHEN** Live Log Space có `api.log` và `web1:/app/app.log`, và người dùng chọn Clear
- **THEN** các dòng của `web1:/app/app.log` bị bỏ khỏi view, còn mọi dòng của `api.log` vẫn còn

#### Scenario: File View của file local
- **WHEN** người dùng mở File View của `api.log` trong một Live Log Space có cả Remote File
- **THEN** status bar của File View không có Load more, Live Window hay Clear

### Requirement: Generation của file local
Khi một file local của Live Log Space bị rotate, bị thay, bị cắt ngắn hoặc bị ghi đè, hệ thống SHALL mở một Generation mới và SHALL vẽ một Break trước dòng đầu tiên của Generation mới. Break SHALL ghi lý do:
- "rotated": file ở đường dẫn là inode mới và file cũ vẫn còn dưới tên khác;
- "replaced": file ở đường dẫn là inode mới và file cũ đã bị xoá;
- "truncated": cùng inode nhưng file ngắn hơn phần đã đọc;
- "rewritten": cùng inode, phần đầu hoặc phần cuối đã đọc khác đi.

Dòng của Generation cũ SHALL còn đọc được sau Break "rotated" hoặc "replaced". Sau Break "truncated" hoặc "rewritten", dòng cũ SHALL bị bỏ, vì text của chúng không còn trên đĩa.

#### Scenario: Rotate kiểu logrotate
- **WHEN** `app.log` có 100 dòng được đổi tên thành `app.log.1`, và một `app.log` mới được tạo rồi ghi 3 dòng
- **THEN** view có 100 dòng cũ, một Break "rotated", rồi 3 dòng mới

#### Scenario: Thay bằng mv
- **WHEN** người dùng chạy `mv other.log app.log`
- **THEN** view giữ các dòng cũ của `app.log`, có một Break "replaced", rồi các dòng của `other.log`

#### Scenario: Ghi đè cùng inode
- **WHEN** người dùng chạy `cp bigger.log app.log` với `bigger.log` dài hơn và khác nội dung
- **THEN** các dòng cũ bị bỏ, view có Break "rewritten", rồi các dòng của `bigger.log`, và không dòng nào hiện nội dung đọc lệch

#### Scenario: copytruncate
- **WHEN** `app.log` bị cắt về 0 rồi được ghi tiếp
- **THEN** view có Break "truncated" rồi các dòng mới, và các dòng cũ không còn

### Requirement: Đóng Generation cũ khi rời tab
Khi tab của một Live Log Space thành Dormant, hệ thống SHALL đóng handle của các Generation cũ của file local, và chỉ giữ Generation hiện tại. Khi tab được mở lại, dòng của các Generation đã đóng SHALL NOT quay lại.

#### Scenario: Rời tab sau khi rotate
- **WHEN** `app.log` đã có một Break "replaced", người dùng chuyển sang tab khác, rồi quay lại
- **THEN** view chỉ có các dòng của `app.log` hiện tại, và file cũ đã xoá không còn bị app giữ mở

