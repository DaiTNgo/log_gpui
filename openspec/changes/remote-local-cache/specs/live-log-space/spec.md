## MODIFIED Requirements

### Requirement: Thao tác chỉ của Remote File
Load more, Open in full, Live Window, Back to Live Window, số dòng đang giữ ("N lines held") và Clear SHALL chỉ có khi Live Log Space có ít nhất một Remote File, và SHALL chỉ tác động lên Remote File. Một file local SHALL NOT có các thao tác đó, kể cả trong File View của nó. Live Window SHALL là số dòng mới nhất mỗi Remote File giữ trong cache trên đĩa, mặc định 1.000.000 dòng. Clear SHALL bỏ các dòng đang giữ của Remote File, kể cả trong cache, và SHALL NOT bỏ dòng nào của file local.

#### Scenario: Live space chỉ có file local
- **WHEN** người dùng mở Live Log Space chỉ chứa `api.log` và `worker.log`
- **THEN** status bar chỉ có Live/Pause, Follow và trạng thái file, không có Load more, Live Window, "N lines held" hay Clear

#### Scenario: Clear trong space trộn
- **WHEN** Live Log Space có `api.log` và `web1:/app/app.log`, và người dùng chọn Clear
- **THEN** các dòng của `web1:/app/app.log` bị bỏ khỏi view và khỏi cache, còn mọi dòng của `api.log` vẫn còn

#### Scenario: File View của file local
- **WHEN** người dùng mở File View của `api.log` trong một Live Log Space có cả Remote File
- **THEN** status bar của File View không có Load more, Live Window hay Clear

#### Scenario: Cắt Live Window trong cache
- **WHEN** `web1:/app/app.log` có Live Window 1.000.000 dòng và nhận thêm dòng mới vượt quá đó
- **THEN** các dòng cũ nhất rời view và rời cache, với Break ở đầu view báo dòng cũ đã rời, như L12
