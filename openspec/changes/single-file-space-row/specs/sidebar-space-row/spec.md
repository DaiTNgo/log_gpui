## ADDED Requirements

### Requirement: Chevron chỉ cho nhóm nhiều file
Chevron của Log Space trong sidebar SHALL chỉ hiện khi Space có từ 2 file trở lên hoặc có Live Glob. Điều này SHALL áp dụng cho Imported, Scratch và Remote Log Space, và được tính lại mỗi khi sidebar vẽ.

#### Scenario: Upload một file log
- **WHEN** người dùng tạo Log Space bằng cách chọn đúng một file log
- **THEN** Space hiện một dòng trong sidebar, không có chevron và không có file row con

#### Scenario: Remote một file
- **WHEN** Remote Log Space chỉ có `web1:/app/app.log`
- **THEN** dòng Space không có chevron

#### Scenario: Thêm file thứ hai
- **WHEN** người dùng thêm một file vào Space đang có 1 file
- **THEN** dòng Space có chevron, và hai file row hiện theo trạng thái expand của Space

#### Scenario: Bớt về còn một file
- **WHEN** Space có 2 file và người dùng gỡ một file
- **THEN** chevron và file row con biến mất, và `expanded_spaces` của Space không bị đổi

### Requirement: Space 1 file là một dòng
Space chỉ có 1 file và không có Live Glob SHALL vẽ thành một dòng gồm tên Space, không file row con. Click vào dòng SHALL mở Space canvas như hiện tại. Space SHALL vẫn là Log Space đầy đủ về tên, loại, thứ tự file và tab.

#### Scenario: Mở Space 1 file
- **WHEN** người dùng click dòng Space 1 file
- **THEN** Space canvas mở, và tab hiện tên Space

### Requirement: Space có Live Glob luôn là nhóm
Log Space có Live Glob SHALL có chevron và header glob, kể cả khi glob chỉ khớp 1 file hoặc chưa khớp file nào.

#### Scenario: Glob khớp một file
- **WHEN** Space chỉ có một Live Glob đang khớp đúng 1 file
- **THEN** dòng Space có chevron, header glob hiện pattern và trạng thái, và file row hiện dưới glob khi Space mở rộng

### Requirement: Status bar của Space 1 file
Khi Space 1 file đang mở, status bar SHALL hiện thông tin của file duy nhất và các nút Load more, Clear, Live/Pause và Back to Live Window. Mỗi nút SHALL chỉ hiện khi áp dụng cho loại file đó. Remote Command SHALL NOT có Load more.

#### Scenario: Remote File
- **WHEN** Space 1 Remote File đang mở
- **THEN** status bar hiện Load more, Clear và Live/Pause, và Back to Live Window khi đang xem ngoài Live Window

#### Scenario: Remote Command
- **WHEN** Space 1 Remote Command đang mở
- **THEN** status bar hiện Clear và Live/Pause, không có Load more

### Requirement: Menu chuột phải của Space 1 file
Menu chuột phải của dòng Space 1 file SHALL gộp phẳng các action còn lại của file duy nhất (như Download, copy path, Show in Finder) cùng các mục của Space, và có mục xóa. Menu của Space nhiều file SHALL giữ nguyên.

#### Scenario: Menu Remote File
- **WHEN** người dùng chuột phải dòng Space 1 Remote File
- **THEN** menu có các action download và copy path của file, các mục của Space và mục xóa, không có submenu "File"

### Requirement: Xóa file duy nhất xóa Space
Mục xóa trong menu của Space 1 file SHALL xóa cả Space: đóng tab của Space và dọn Remote Copy của file. Hệ thống SHALL hỏi xác nhận trước khi xóa, như mục Delete của Space hiện có.

#### Scenario: Xóa Space Imported
- **WHEN** người dùng chọn xóa trên Space Imported 1 file đang mở và xác nhận
- **THEN** Space biến mất khỏi sidebar và tab của nó đóng

#### Scenario: Xóa Space Scratch
- **WHEN** người dùng chọn xóa trên Scratch Log Space 1 file
- **THEN** app hỏi xác nhận, và chỉ xóa Space cùng file trong app data folder khi người dùng đồng ý
