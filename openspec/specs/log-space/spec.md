# log-space Specification

## Purpose
TBD - created by archiving change add-log-space. Update Purpose after archive.
## Requirements
### Requirement: Project chứa nhiều Log Space
Hệ thống SHALL cho mỗi Project chứa nhiều Log Space. Mỗi Log Space SHALL có một tên và một danh sách file. Hai Project SHALL NOT dùng chung Log Space.

#### Scenario: Nhiều Log Space trong một Project
- **WHEN** người dùng tạo Log Space "Payment" và "Auth" trong Project "Prod"
- **THEN** sidebar của Project "Prod" hiện cả hai Log Space

#### Scenario: Project khác không thấy
- **WHEN** người dùng chuyển sang Project "Staging"
- **THEN** sidebar không hiện "Payment" hay "Auth"

### Requirement: Mở dialog tạo Log Space
Hệ thống SHALL mở dialog New Log Space khi người dùng bấm nút **+** ở đầu danh sách Log Space trên sidebar. Dialog SHALL gồm thanh chọn loại, ô tên, phần nội dung của nấc đang chọn, nút Create và nút Cancel. Nấc `Local files` SHALL gồm vùng kéo thả file kèm nút "Choose files…" và "Choose folder…" cùng danh sách file đã chọn. Nấc `Remote` SHALL gồm danh sách Remote File, Live Glob và Remote Command đã chọn cùng nút "Add remote files…" và "Add remote command…"; gạt sang nấc này SHALL NOT tự mở remote picker. Nấc `Scratch` SHALL chỉ có dòng mô tả.

#### Scenario: Mở dialog
- **WHEN** người dùng bấm **+** trên sidebar
- **THEN** dialog New Log Space hiện ra ở nấc `Scratch` với tên gợi ý "Scratch" và nút "Create" bật

#### Scenario: Hủy
- **WHEN** người dùng đã nhập tên và thả file rồi bấm Cancel
- **THEN** dialog đóng và không có Log Space nào được tạo

#### Scenario: Tạo Remote Log Space
- **WHEN** người dùng gạt sang `Remote`, nhập tên "Checkout", bấm "Add remote files…", chọn `web1:/app/app.log` và `web2:/app/app.log`, rồi bấm Create
- **THEN** Remote Log Space "Checkout" được tạo với hai Remote File đó

#### Scenario: Gạt sang Remote không mở picker
- **WHEN** người dùng gạt từ `Scratch` sang `Remote`
- **THEN** remote picker không mở, và nấc `Remote` hiện danh sách trống cùng hai nút "Add remote files…" và "Add remote command…"

### Requirement: Điều kiện bật nút Create
Hệ thống SHALL chỉ bật nút Create khi tên, sau khi bỏ khoảng trắng hai đầu, không rỗng và không trùng tên Log Space khác trong cùng Project, và nấc đang hiện có thứ để tạo: nấc `Scratch` luôn có; nấc `Local files` cần ít nhất một file thêm được và không còn Archive đang đọc; nấc `Remote` cần ít nhất một Remote File, Live Glob hoặc Remote Command. Hệ thống SHALL hiện lý do ngay dưới ô tên khi tên bị trùng, và hiện hướng dẫn dưới danh sách khi nấc `Local files` hoặc `Remote` chưa có gì. Nhãn nút Create SHALL cho biết kết quả, ví dụ "Create", "Create with 3 files" hoặc "Create with 2 remote files".

#### Scenario: Tên rỗng
- **WHEN** ô tên chỉ chứa khoảng trắng
- **THEN** nút Create bị tắt

#### Scenario: Tên trùng trong Project
- **WHEN** Project đã có Log Space "Payment" và người dùng nhập ` Payment `
- **THEN** nút Create bị tắt và dưới ô tên hiện thông báo tên đã tồn tại

#### Scenario: Tên trùng ở Project khác
- **WHEN** Project "Staging" có Log Space "Payment" và người dùng đang tạo "Payment" trong Project "Prod"
- **THEN** nút Create được bật

#### Scenario: Local files chưa có file
- **WHEN** người dùng gạt sang `Local files` khi danh sách của nấc này trống
- **THEN** nút Create bị tắt và dưới danh sách hiện "Choose or drop files"

#### Scenario: Remote chưa có gì
- **WHEN** người dùng gạt sang `Remote` khi danh sách của nấc này trống
- **THEN** nút Create bị tắt và dưới danh sách hiện "Add remote files or a command"

#### Scenario: Tạo Scratch
- **WHEN** người dùng ở nấc `Scratch`, nhập tên "Scratch 1" và bấm "Create"
- **THEN** Scratch Log Space "Scratch 1" được tạo với một file rỗng `untitled.log`

#### Scenario: Tạo Imported
- **WHEN** người dùng ở nấc `Local files`, nhập tên "Payment", thả `api.log` và `worker.log`, rồi bấm "Create with 2 files"
- **THEN** Imported Log Space "Payment" được tạo với hai file đó

### Requirement: Nhận file vào dialog
Hệ thống SHALL nhận file vào nấc `Local files` qua kéo thả và qua nút "Choose files…" và "Choose folder…". Thả file vào dialog khi đang ở nấc khác SHALL tự gạt sang `Local files` rồi thêm file vào danh sách của nấc này, và SHALL NOT đụng tới danh sách của nấc `Remote`. Thư mục SHALL được mở rộng một cấp và chỉ lấy file text. File binary SHALL hiện trong danh sách kèm cảnh báo và SHALL NOT được thêm khi tạo. Người dùng SHALL bỏ được từng file khỏi danh sách trước khi bấm Create.

#### Scenario: Thả thư mục
- **WHEN** người dùng thả thư mục `logs/` chứa `a.log`, `b.log` và thư mục con `old/`
- **THEN** danh sách có `a.log` và `b.log`, không có file nào trong `old/`

#### Scenario: File binary
- **WHEN** người dùng thả `core.dump` có byte NUL
- **THEN** `core.dump` hiện trong danh sách với cảnh báo "Not a text file" và không được đếm vào nhãn Create

#### Scenario: Bỏ một file
- **WHEN** danh sách có `a.log` và `b.log` và người dùng bấm bỏ `b.log`
- **THEN** chỉ còn `a.log` và nhãn nút là "Create with 1 file"

#### Scenario: Thả file khi đang ở nấc Remote
- **WHEN** dialog đang ở nấc `Remote` có `web1:/app/app.log` và người dùng thả `api.log` vào dialog
- **THEN** dialog gạt sang `Local files` với `api.log` trong danh sách, và gạt lại `Remote` vẫn thấy `web1:/app/app.log`

### Requirement: Chọn loại Log Space bằng thanh chọn
Dialog New Log Space SHALL có thanh chọn ba nấc `Local files`, `Remote`, `Scratch` ngay dưới tiêu đề, mỗi lúc đúng một nấc được chọn, kèm một dòng mô tả nấc đang chọn bên dưới. Dialog SHALL luôn mở ở nấc `Scratch` và SHALL NOT nhớ nấc của lần mở trước. Mỗi nấc SHALL giữ danh sách riêng của nó: gạt sang nấc khác SHALL chỉ ẩn danh sách đang hiện, SHALL NOT xóa nó và SHALL NOT hiện số đếm của nấc bị ẩn. Nút Create SHALL chỉ tạo Log Space từ nấc đang hiện: `Local files` tạo Imported, `Remote` tạo Remote, `Scratch` tạo Scratch; danh sách của các nấc khác SHALL bị bỏ. ⌘1, ⌘2, ⌘3 SHALL chọn lần lượt ba nấc khi dialog mở; thanh chọn SHALL nhận focus bằng Tab và đổi nấc bằng ← và →.

#### Scenario: Mở ở Scratch
- **WHEN** người dùng mở dialog New Log Space, kể cả ngay sau khi vừa tạo một Remote Log Space
- **THEN** nấc `Scratch` được chọn và dòng mô tả nói Log Space sẽ chứa text người dùng gõ hoặc dán

#### Scenario: Gạt nấc giữ danh sách
- **WHEN** nấc `Local files` có `a.log` và `b.log`, người dùng gạt sang `Remote`, rồi gạt lại `Local files`
- **THEN** ở nấc `Remote` không thấy `a.log` hay `b.log` và không có số đếm nào của nấc `Local files`, và khi gạt lại thì `a.log` và `b.log` vẫn còn

#### Scenario: Create chỉ dùng nấc đang hiện
- **WHEN** nấc `Local files` có `a.log`, nấc `Remote` có `web1:/app/app.log`, nấc đang hiện là `Remote`, và người dùng bấm Create
- **THEN** một Remote Log Space chỉ chứa `web1:/app/app.log` được tạo, và `a.log` không được thêm vào đâu

#### Scenario: Chọn nấc bằng phím
- **WHEN** dialog đang mở ở nấc `Scratch` và người dùng nhấn ⌘2
- **THEN** nấc `Remote` được chọn và các tab của Working Set không đổi

#### Scenario: Không còn câu nhắc trộn
- **WHEN** nấc `Local files` có file và người dùng gạt sang `Remote`
- **THEN** nút "Add remote files…" và "Add remote command…" bật, và không có câu nhắc nào về việc không trộn file local với remote

### Requirement: Tên gợi ý theo nấc
Khi người dùng chưa tự gõ vào ô tên, dialog SHALL gợi ý tên theo nấc đang hiện: theo mục đầu tiên trong danh sách của nấc `Local files` hoặc `Remote`, và "Scratch" ở nấc `Scratch`. Tên gợi ý SHALL thêm số (" 2", " 3", …) khi trùng tên Log Space khác trong Project. Khi người dùng đã tự gõ tên, gạt nấc hay thêm file SHALL NOT đổi tên.

#### Scenario: Scratch gợi ý tên
- **WHEN** Project đã có Log Space "Scratch" và người dùng mở dialog
- **THEN** ô tên là "Scratch 2"

#### Scenario: Tên đi theo nấc
- **WHEN** người dùng chưa gõ tên, thả `api.log` vào nấc `Local files`, rồi gạt sang `Scratch`
- **THEN** ô tên là "api" khi ở `Local files` và trở lại "Scratch" khi ở `Scratch`

#### Scenario: Tên đã gõ được giữ
- **WHEN** người dùng gõ "Checkout" rồi gạt từ `Scratch` sang `Local files` và thả `api.log`
- **THEN** ô tên vẫn là "Checkout"

### Requirement: Thả file vào cửa sổ tạo Imported Log Space
Khi không có dialog nào đang mở, thả file hoặc thư mục vào cửa sổ SHALL tạo ngay một Imported Log Space chứa mọi file đã thả, theo cùng quy tắc nhận file như dialog, mà SHALL NOT mở dialog New Log Space. Tên SHALL là tên gợi ý theo file đầu tiên, thêm số khi trùng. Archive SHALL được đọc xong rồi mới tạo Log Space, và trong lúc đọc status bar SHALL hiện "Reading archive…". Nếu có ít nhất một file không thêm được (không phải text, quá lớn, không đọc được, Archive không có file text) hoặc không có file nào, hệ thống SHALL NOT tạo Log Space mà SHALL mở dialog New Log Space ở nấc `Local files` với danh sách đã thả và lỗi của từng file. Khi đang có dialog khác mở, thả file SHALL bị bỏ qua.

#### Scenario: Thả một file
- **WHEN** không có dialog nào mở và người dùng thả `api.log` vào cửa sổ
- **THEN** Imported Log Space "api" chứa `api.log` được tạo, được mở, và không có dialog nào hiện ra

#### Scenario: Thả nhiều file
- **WHEN** Project đã có Log Space "api" và người dùng thả `api.log` cùng `worker.log`
- **THEN** một Imported Log Space "api 2" chứa cả hai file được tạo

#### Scenario: Thả Archive
- **WHEN** người dùng thả `logs.tar.gz` chứa `a.log` và `b.log`
- **THEN** status bar hiện "Reading archive…" trong lúc đọc, rồi một Imported Log Space chứa hai Archive Entry đó được tạo

#### Scenario: Có file không dùng được
- **WHEN** người dùng thả `api.log` cùng `core.dump` có byte NUL
- **THEN** không Log Space nào được tạo, và dialog New Log Space mở ở nấc `Local files` với `api.log` và `core.dump` kèm cảnh báo "Not a text file"

### Requirement: Imported và Scratch không trộn lẫn
Hệ thống SHALL chỉ thêm file có sẵn trên đĩa vào Imported Log Space, và SHALL chỉ thêm file scratch rỗng vào Scratch Log Space.

#### Scenario: Menu của Imported
- **WHEN** người dùng chuột phải lên một Imported Log Space
- **THEN** menu có "Add files…" và không có "New scratch file"

#### Scenario: Menu của Scratch
- **WHEN** người dùng chuột phải lên một Scratch Log Space
- **THEN** menu có "New scratch file" và không có "Add files…"

### Requirement: File Imported chỉ đọc
Hệ thống SHALL NOT sửa, đổi tên hay xóa file Imported trên đĩa. Gỡ file khỏi Log Space SHALL chỉ bỏ nó khỏi danh sách.

#### Scenario: Gỡ file
- **WHEN** người dùng chọn "Remove from Log Space" trên `api.log`
- **THEN** `api.log` biến mất khỏi Log Space, file trên đĩa vẫn còn nguyên, và các Marker của nó bị bỏ

#### Scenario: Gỡ file cuối cùng
- **WHEN** người dùng gỡ file duy nhất của một Imported Log Space
- **THEN** Log Space vẫn còn, và canvas hiện trạng thái rỗng kèm nút "Add files"

### Requirement: Thêm file vào Imported Log Space
Hệ thống SHALL cho thêm file vào Imported Log Space qua "Add files…", nhận file theo cùng quy tắc như dialog. File đã có trong Log Space SHALL bị bỏ qua kèm thông báo nhẹ.

#### Scenario: Thêm file trùng
- **WHEN** Log Space đã có `/var/log/api.log` và người dùng thêm lại file đó
- **THEN** danh sách file không đổi và hiện thông báo file đã có

### Requirement: File scratch
Hệ thống SHALL lưu file scratch trong thư mục dữ liệu của ứng dụng. File đầu tiên SHALL tên `untitled.log`; "New scratch file" SHALL tạo `untitled-2.log`, `untitled-3.log`… theo số nhỏ nhất chưa dùng. Người dùng SHALL đổi tên và xóa được file scratch. Xóa file SHALL cần xác nhận, và SHALL NOT xóa được file scratch cuối cùng của Log Space.

#### Scenario: Thêm file scratch
- **WHEN** Scratch Log Space có `untitled.log` và người dùng chọn "New scratch file"
- **THEN** `untitled-2.log` rỗng được thêm vào

#### Scenario: Không xóa file cuối
- **WHEN** Scratch Log Space chỉ còn một file
- **THEN** mục Delete của file đó bị tắt

### Requirement: Xóa Log Space
Hệ thống SHALL xóa Log Space sau khi người dùng xác nhận. Xóa Scratch Log Space SHALL xóa các file scratch của nó. Xóa Imported Log Space SHALL NOT đụng tới file trên đĩa.

#### Scenario: Xóa Imported
- **WHEN** người dùng xác nhận xóa Imported Log Space "Payment"
- **THEN** "Payment" và các tab của nó biến mất, file log trên đĩa vẫn còn

### Requirement: Thứ tự file trong Log Space
Hệ thống SHALL sắp file trong mỗi Log Space theo thứ tự thêm vào (mặc định) hoặc theo tên, chọn qua "Sort files by" trong menu chuột phải. Thứ tự này SHALL được dùng cho sidebar, File Section, và khi hai dòng trùng timestamp.

#### Scenario: Đổi sang theo tên
- **WHEN** Log Space thêm `worker.log` trước rồi `api.log`, và người dùng chọn "Sort files by → Name"
- **THEN** sidebar và Raw View đều liệt kê `api.log` trước `worker.log`

### Requirement: Sidebar Log Space
Hệ thống SHALL hiện mỗi Log Space trên sidebar thành một dòng có chevron để mở danh sách file, cùng biểu tượng phân biệt Imported và Scratch. Bấm tên Log Space SHALL mở Log Space; bấm tên file SHALL mở File View của file đó.

#### Scenario: Mở danh sách file
- **WHEN** người dùng bấm chevron của "Payment"
- **THEN** các file của "Payment" hiện bên dưới theo thứ tự file của Log Space

### Requirement: Tab theo view
Hệ thống SHALL mở mỗi Log Space và mỗi File View trong một tab riêng. Mở một Log Space hoặc File View đã có tab SHALL kích hoạt tab đó thay vì mở tab mới.

#### Scenario: Mở lại
- **WHEN** tab "Payment" đang mở và người dùng bấm "Payment" trên sidebar
- **THEN** tab "Payment" được kích hoạt và số tab không đổi

#### Scenario: Log Space và File View là hai tab
- **WHEN** tab "Payment" đang mở và người dùng bấm `api.log` của "Payment"
- **THEN** một tab mới cho File View `api.log` được mở

### Requirement: Đổi tên Log Space
Hệ thống SHALL cho đổi tên Log Space qua menu chuột phải, theo cùng điều kiện tên của dialog.

#### Scenario: Đổi sang tên trùng
- **WHEN** người dùng đổi "Auth" thành "Payment" trong khi "Payment" đã có
- **THEN** tên không đổi và hiện thông báo tên đã tồn tại
