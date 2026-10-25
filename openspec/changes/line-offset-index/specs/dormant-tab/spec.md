## ADDED Requirements

### Requirement: Chỉ đọc tab đang chọn khi mở app
Khi mở app hoặc chuyển Project, hệ thống SHALL chỉ đọc file của tab đang chọn. Các tab khác SHALL là Dormant Tab: file của chúng SHALL NOT được đọc, source live của chúng SHALL NOT chạy, và Live Glob của chúng SHALL NOT được theo dõi, cho tới khi người dùng chọn tab đó.

#### Scenario: Mở app với nhiều tab
- **WHEN** session có 10 tab và tab thứ 3 đang chọn
- **THEN** chỉ file của tab thứ 3 được đọc; 9 tab còn lại hiện trên tab strip mà không có file nào được đọc

#### Scenario: Tab Remote có Live Glob
- **WHEN** session có một tab Remote Log Space với Live Glob, và tab đó không được chọn
- **THEN** app không kết nối tới Server của tab đó cho tới khi người dùng chọn tab

### Requirement: File View chỉ đọc file của nó
Mở một File View SHALL chỉ đọc file của File View đó, không đọc các file khác của Log Space.

#### Scenario: File View trong Log Space nhiều file
- **WHEN** Log Space "Payment" có 7 file và người dùng mở File View `api.log`
- **THEN** chỉ `api.log` được đọc

### Requirement: Rời tab thì bỏ dòng, giữ kết nối
Khi người dùng rời một tab, hệ thống SHALL bỏ khỏi memory Line Index và dòng đã parse của các file không còn tab đang chọn nào dùng, và SHALL dừng nhận dòng mới cho chúng. Hệ thống SHALL giữ kết nối SSH của Server. Hệ thống SHALL lưu file scratch đang sửa trước khi bỏ.

#### Scenario: Rời tab file lớn
- **WHEN** người dùng rời tab có file 3.000.000 dòng để sang tab khác
- **THEN** memory dành cho file đó được trả lại

#### Scenario: Rời tab Remote
- **WHEN** người dùng rời tab Remote Log Space trên Server `web1` rồi quay lại sau một phút
- **THEN** app đọc lại Live Window qua kết nối có sẵn, không đăng nhập lại và không hỏi lại Sign-in Prompt

#### Scenario: Scratch đang sửa
- **WHEN** người dùng vừa gõ vào một file scratch rồi lập tức chuyển tab
- **THEN** nội dung vừa gõ được lưu trước khi dòng của file bị bỏ khỏi memory

### Requirement: Quay lại tab giữ trạng thái
Khi người dùng chọn lại một Dormant Tab, hệ thống SHALL đọc lại file của nó với tiến độ, và SHALL giữ nguyên Filter, Focus Tab, Search Expression, vị trí đang xem và Marker của tab.

#### Scenario: Quay lại sau khi rời
- **WHEN** người dùng có Filter `level = ERROR` và đang xem dòng 1.200.000 ở một tab, rời tab rồi quay lại
- **THEN** tab đọc lại với tiến độ, rồi hiện lại Filter đó và quay về dòng 1.200.000

### Requirement: Tab hiện trạng thái đọc
Tab strip SHALL phân biệt Dormant Tab với tab đang Indexing (có tiến độ) và tab đã đọc xong.

#### Scenario: Tab đang Indexing
- **WHEN** người dùng chọn một Dormant Tab có file lớn
- **THEN** tab hiện tiến độ Indexing cho tới khi đọc xong
