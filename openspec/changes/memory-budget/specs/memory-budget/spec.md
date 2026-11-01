## ADDED Requirements

### Requirement: File scratch đọc từ đĩa ở nền
Hệ thống SHALL đọc một file scratch qua Line Index trên chính file của nó trên đĩa, ở nền, có tiến độ, như một file local trong Static Log Space. Hệ thống SHALL NOT đọc cả file scratch vào memory để index.

#### Scenario: Mở Scratch Log Space có file lớn
- **WHEN** người dùng chọn tab một Scratch Log Space có file scratch 200 MB chưa được sửa trong lần chạy này
- **THEN** tab hiện tiến độ Indexing, view đọc và cuộn được trong lúc đó, và memory tăng thêm theo Line Index chứ không theo kích thước file

#### Scenario: File scratch rỗng
- **WHEN** người dùng tạo một file scratch mới, chưa có dòng nào
- **THEN** view hiện file rỗng ngay, không có lỗi

### Requirement: Scratch đã sửa được lưu rồi index lại từ đĩa
Khi một file scratch đã được sửa trong Raw View và Parsed View cần đọc lại nó (chuyển sang Parsed View, chuyển tab), hệ thống SHALL lưu file trước, rồi SHALL index lại từ file trên đĩa ở nền. Hệ thống SHALL NOT index từ một bản copy của text trong memory.

#### Scenario: Sửa rồi chuyển sang Parsed View
- **WHEN** người dùng dán 10.000 dòng vào file scratch trong Raw View rồi lập tức chuyển sang Parsed View
- **THEN** file được lưu, Parsed View hiện tiến độ rồi hiện cả 10.000 dòng vừa dán

#### Scenario: Lưu thất bại
- **WHEN** người dùng sửa file scratch, thư mục dữ liệu của app không ghi được, rồi người dùng chuyển sang Parsed View
- **THEN** app báo không lưu được, text đã sửa vẫn được giữ trong editor, và Parsed View tiếp tục hiện Line Index cũ

### Requirement: Editor của Scratch được bỏ khi tab Dormant
Khi một tab có file scratch thành Dormant Tab, hệ thống SHALL lưu text đã sửa của file, rồi SHALL bỏ khỏi memory editor, text đã sửa và các dòng tách sẵn của file đó. Nếu lưu thất bại, hệ thống SHALL giữ chúng lại. Khi tab là Warm Tab, editor SHALL được giữ.

#### Scenario: Rời tab scratch cho tới khi Dormant
- **WHEN** người dùng sửa một file scratch, rời tab, và tab đó thành Dormant Tab
- **THEN** text đã được lưu trên đĩa, và không còn bản text nào của file trong memory

#### Scenario: Quay lại Warm Tab scratch
- **WHEN** người dùng sửa một file scratch, rời tab, rồi quay lại trong khi tab vẫn là Warm Tab
- **THEN** editor hiện đúng text, đúng vị trí con trỏ như lúc rời

### Requirement: Archive Entry giải nén và index ở nền
Hệ thống SHALL giải nén Archive Entry ra archive cache ở nền, một lượt cho mỗi Archive, có tiến độ và huỷ được, rồi SHALL index entry đã giải nén ở nền như một file local. Việc giải nén và Indexing SHALL NOT chặn UI. Entry đã giải nén từ trước SHALL vào thẳng bước Indexing ở nền.

#### Scenario: Mở lần đầu một Archive lớn
- **WHEN** người dùng chọn tab một Static Log Space có Archive Entry 1 GB nằm trong `logs.tar.gz` chưa từng được giải nén
- **THEN** tab hiện tiến độ giải nén rồi tiến độ Indexing; trong suốt thời gian đó cửa sổ vẫn phản hồi và người dùng chuyển được sang tab khác

#### Scenario: Nhiều entry của cùng một Archive
- **WHEN** một Log Space có ba entry của cùng `logs.tar.gz` và người dùng chọn tab đó
- **THEN** Archive được đọc một lượt duy nhất cho cả ba entry

#### Scenario: Huỷ giữa chừng
- **WHEN** người dùng rời tab trong lúc Archive đang được giải nén
- **THEN** việc giải nén dừng lại, và archive cache không còn file `.part` nào của lần đó

#### Scenario: Entry đã giải nén từ trước
- **WHEN** người dùng mở lại một tab có Archive Entry đã nằm trong archive cache, và Archive không đổi
- **THEN** Archive không được đọc lại; entry được index ở nền từ file đã giải nén

#### Scenario: Archive hỏng
- **WHEN** Archive không đọc được hoặc entry vượt giới hạn giải nén
- **THEN** sau khi việc nền kết thúc, app báo lỗi như hiện tại và file đó được coi là rỗng

### Requirement: Rời tab giữ Line Index trong Warm Tab Budget
Khi người dùng rời một tab, hệ thống SHALL dừng mọi việc đọc của tab đó (source live, theo dõi Live Glob, watch file static), như với Dormant Tab. Hệ thống SHALL giữ Line Index của các file của tab đó, và tab SHALL là Warm Tab, nếu tổng memory ước lượng của mọi Warm Tab không vượt Warm Tab Budget. Khi vượt, hệ thống SHALL chuyển tab rời lâu nhất thành Dormant Tab, lặp lại cho tới khi tổng nằm trong budget. Requirement này thay phần "bỏ khỏi memory Line Index" của requirement "Rời tab thì bỏ dòng, giữ kết nối" trong `dormant-tab`; phần giữ kết nối và lưu scratch giữ nguyên.

#### Scenario: Rời tab file lớn trong budget
- **WHEN** Warm Tab Budget là 512 MB và người dùng rời tab có file 3.000.000 dòng (Line Index ~90 MB) để sang tab khác
- **THEN** tab vừa rời là Warm Tab, và không còn source nào đọc file đó

#### Scenario: Vượt budget
- **WHEN** Warm Tab Budget là 200 MB, đã có Warm Tab A (~90 MB) rời trước, và người dùng rời tab B (~150 MB)
- **THEN** tab A thành Dormant Tab và memory của nó được trả lại; tab B là Warm Tab

#### Scenario: Một tab lớn hơn cả budget
- **WHEN** Warm Tab Budget là 100 MB và người dùng rời tab có Line Index ~300 MB
- **THEN** tab đó thành Dormant Tab ngay, như khi chưa có Warm Tab

#### Scenario: Budget bằng 0
- **WHEN** Warm Tab Budget là 0 và người dùng rời một tab
- **THEN** tab thành Dormant Tab ngay

#### Scenario: Mở app
- **WHEN** người dùng mở app với 10 tab
- **THEN** chỉ tab đang chọn được đọc; không tab nào là Warm Tab cho tới khi người dùng rời một tab đã đọc

### Requirement: Chọn lại Warm Tab không index lại
Khi người dùng chọn lại một Warm Tab, hệ thống SHALL dựng lại view từ Line Index đã giữ, không index lại file. Trước đó hệ thống SHALL so file trên đĩa với phần đã index: file không đổi thì dùng nguyên; file chỉ được ghi thêm thì đọc tiếp phần mới (Live Log Space) hoặc như cách Static Log Space xử lý file đổi; file bị thay, bị cắt hoặc không còn thì xử lý như file đang mở gặp trường hợp đó. Filter, Focus Tab, Search Expression, vị trí đang xem và Marker SHALL được giữ.

#### Scenario: Quay lại Warm Tab
- **WHEN** người dùng rời tab có file 3.000.000 dòng, đang xem dòng 1.200.000 với Filter `level = ERROR`, rồi quay lại khi tab vẫn là Warm Tab
- **THEN** view hiện lại không có tiến độ Indexing, với Filter đó, ở dòng 1.200.000

#### Scenario: File live được ghi thêm khi tab Warm
- **WHEN** một Live Log Space là Warm Tab, file của nó được ghi thêm 5.000 dòng, rồi người dùng chọn lại tab
- **THEN** chỉ 5.000 dòng mới được đọc, và chúng hiện ở cuối view

#### Scenario: File bị rotate khi tab Warm
- **WHEN** một Live Log Space là Warm Tab, file của nó bị rotate, rồi người dùng chọn lại tab
- **THEN** app nhận ra rotate như với file đang mở: có Break và file mới được đọc từ đầu

#### Scenario: File static bị sửa khi tab Warm
- **WHEN** một Static Log Space là Warm Tab, file của nó bị viết lại, rồi người dùng chọn lại tab
- **THEN** file được index lại với tiến độ

### Requirement: Remote File không thành Warm Tab
Trong change này, file của Remote File và Remote Command SHALL NOT được giữ trong Warm Tab. Khi rời tab, Line Index của chúng SHALL được bỏ như Dormant Tab, và khi chọn lại SHALL được mở lại từ Remote Cache. Các file khác của cùng tab vẫn được giữ Warm.

#### Scenario: Live Log Space có cả file local và Remote File
- **WHEN** người dùng rời tab một Live Log Space có `app.log` local và `web1:/var/log/app.log`
- **THEN** Line Index của `app.log` được giữ trong Warm Tab, còn Remote File được bỏ và mở lại từ Remote Cache khi chọn lại

### Requirement: Đặt Warm Tab Budget
Settings SHALL có Warm Tab Budget tính bằng MB, mặc định 512, với 0 nghĩa là tắt Warm Tab. Giá trị SHALL được lưu cùng các Settings khác. Giảm budget SHALL áp dụng ngay: các Warm Tab vượt budget mới SHALL thành Dormant Tab, rời lâu nhất trước.

#### Scenario: Giảm budget
- **WHEN** có hai Warm Tab tổng ~300 MB và người dùng đặt Warm Tab Budget về 100 MB
- **THEN** các Warm Tab thành Dormant Tab, rời lâu nhất trước, cho tới khi tổng không vượt 100 MB

#### Scenario: Session cũ không có giá trị
- **WHEN** người dùng mở app với session lưu trước change này
- **THEN** Warm Tab Budget là 512 MB

### Requirement: Một Row Cache chung có giới hạn byte
Hệ thống SHALL giữ các dòng đã parse trong một cache chung cho cả app, có giới hạn theo byte ước lượng, thay vì một giới hạn số dòng cho mỗi file. Khi vượt giới hạn, hệ thống SHALL bỏ dòng dùng lâu nhất trên toàn app. Khi Line Index của một file bị bỏ (Dormant Tab, gỡ file, index lại), hệ thống SHALL bỏ mọi dòng của file đó khỏi cache.

#### Scenario: Log Space nhiều file
- **WHEN** người dùng mở một Log Space gộp 50 file và cuộn qua toàn bộ
- **THEN** memory dành cho dòng đã parse không vượt giới hạn của Row Cache, như khi mở một file

#### Scenario: Màn hình đang xem luôn có dòng
- **WHEN** Row Cache đầy vì một Scan và người dùng cuộn tới một vị trí mới
- **THEN** các dòng ở vị trí mới được đọc và hiện; dòng bị bỏ là dòng dùng lâu nhất

#### Scenario: Rời tab tới Dormant
- **WHEN** một tab thành Dormant Tab
- **THEN** Row Cache không còn dòng nào của các file chỉ tab đó dùng
