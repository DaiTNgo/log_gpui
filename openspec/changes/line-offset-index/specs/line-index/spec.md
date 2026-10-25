## ADDED Requirements

### Requirement: Đọc file qua Line Index
Hệ thống SHALL đọc mọi file local, Archive Entry, Remote Copy, file scratch và Live Window của Remote File qua một Line Index. Hệ thống SHALL NOT giữ text của mọi dòng một file trong memory. Text của một dòng SHALL được đọc lại từ nguồn khi dòng đó cần hiện, và chỉ các dòng đang hiện cùng một bộ đệm nhỏ quanh chúng SHALL được giữ đã parse.

#### Scenario: Memory theo số dòng
- **WHEN** người dùng mở một file 376MB có 3.000.000 dòng
- **THEN** sau khi Indexing xong, memory app dùng thêm cho file đó không quá khoảng 150MB

#### Scenario: Dòng hiện đúng như file
- **WHEN** người dùng cuộn tới dòng 2.500.000 của file đó
- **THEN** dòng 2.500.000 hiện đúng text, cột và số dòng như trong file

### Requirement: Indexing ở nền có tiến độ
Hệ thống SHALL dựng Line Index ở nền và SHALL NOT chặn UI trong lúc Indexing. Trong lúc Indexing, tab SHALL hiện tiến độ, view SHALL đọc và cuộn được trên các dòng đã index, và các dòng mới index SHALL xuất hiện dần. Người dùng SHALL huỷ được Indexing; huỷ thì giữ các dòng đã index và hiện rằng file chưa được đọc hết.

#### Scenario: Màn hình đầu
- **WHEN** người dùng mở một file 376MB
- **THEN** các dòng đầu hiện trong vòng khoảng 300ms, và tab hiện tiến độ Indexing

#### Scenario: Dùng app trong lúc Indexing
- **WHEN** Indexing đang chạy và người dùng cuộn, chuyển tab hoặc gõ vào Search
- **THEN** app phản hồi ngay và Indexing tiếp tục ở nền

#### Scenario: Huỷ Indexing
- **WHEN** người dùng huỷ Indexing khi mới được 40%
- **THEN** 40% dòng đầu vẫn đọc được, và tab hiện rằng file chưa được đọc hết

### Requirement: File live mở ở cuối trong lúc Indexing
Một Log Space đọc live SHALL mở ở dòng mới nhất (L3) ngay cả khi Indexing chưa tới cuối file. Các dòng cuối SHALL hiện trước, và số dòng của chúng SHALL đúng khi Indexing hoàn tất.

#### Scenario: Mở file live lớn
- **WHEN** người dùng mở một file local 376MB đang được ghi thêm
- **THEN** view hiện các dòng cuối gần như ngay với Follow bật, và Gutter hiện đúng số dòng khi Indexing xong

### Requirement: Dòng thêm vào và rotate của file local
Khi một file local live được ghi thêm, hệ thống SHALL thêm các dòng mới vào Line Index mà không index lại phần đã có. Khi file bị rotate hoặc truncate, hệ thống SHALL bắt đầu một Generation mới với một Break, và SHALL giữ đọc được dòng của Generation cũ khi file cũ vẫn còn trên đĩa (kể cả đã đổi tên hoặc đã xoá mà app còn mở). Dòng của Generation cũ không còn trên đĩa SHALL hiện là không còn đọc được, không hiện text sai.

#### Scenario: Ghi thêm
- **WHEN** một file 3.000.000 dòng đang mở nhận thêm 1.000 dòng
- **THEN** 1.000 dòng mới hiện trong vòng một nhịp poll, mà không có Indexing lại cả file

#### Scenario: Rotate bằng đổi tên
- **WHEN** `app.log` được đổi tên thành `app.log.1` và một `app.log` mới được tạo
- **THEN** một Break "rotated" hiện, dòng cũ vẫn đọc được, và dòng mới bắt đầu từ dòng 1

#### Scenario: Truncate tại chỗ
- **WHEN** `app.log` bị truncate tại chỗ (`copytruncate`)
- **THEN** một Break hiện, và các dòng cũ không còn trong bộ đệm hiện là "no longer on disk" thay vì text của file mới

### Requirement: File bị sửa khi đang mở
Khi nội dung đã index của một file bị đổi (không phải chỉ ghi thêm), hệ thống SHALL nhận ra trước khi hiện dòng sai, SHALL index lại file và SHALL báo cho người dùng.

#### Scenario: File Imported bị ghi đè
- **WHEN** một file Imported đang mở bị một chương trình khác ghi đè nội dung khác
- **THEN** app báo file đã đổi và index lại, không hiện text lệch với số dòng

### Requirement: Archive Entry giải nén ra file tạm
Hệ thống SHALL giải nén Archive Entry một lần vào thư mục dữ liệu của app rồi index như file local. Các entry của cùng một Archive SHALL được giải nén trong một lượt. File tạm SHALL bị xoá khi Log Space chứa nó bị xoá, và các file tạm không Log Space nào dùng SHALL bị dọn khi app khởi động.

#### Scenario: Mở Archive lớn
- **WHEN** người dùng mở một `.tar.gz` có entry 1GB sau giải nén
- **THEN** memory không tăng theo 1GB đó, và entry hiện dần với tiến độ

#### Scenario: Dọn khi khởi động
- **WHEN** app bị tắt ngang trong lúc giải nén
- **THEN** lần khởi động sau, file tạm còn sót bị xoá

### Requirement: Chuyển tab không dựng lại dòng
Chuyển sang một tab có các file đã được index SHALL NOT clone dòng hay dựng lại chỉ mục Filter của chúng.

#### Scenario: Chuyển qua lại hai tab lớn
- **WHEN** hai tab, mỗi tab một file 3.000.000 dòng đã index, và người dùng chuyển qua lại giữa chúng
- **THEN** mỗi lần chuyển hiện view trong vòng khoảng 50ms
