## RENAMED Requirements

- FROM: `### Requirement: Thả file vào cửa sổ tạo Imported Log Space`
- TO: `### Requirement: Thả file vào cửa sổ tạo Static Log Space`

- FROM: `### Requirement: Imported và Scratch không trộn lẫn`
- TO: `### Requirement: Loại file theo loại Log Space`

- FROM: `### Requirement: File Imported chỉ đọc`
- TO: `### Requirement: File trên đĩa chỉ đọc`

- FROM: `### Requirement: Thêm file vào Imported Log Space`
- TO: `### Requirement: Thêm file vào Log Space đã có`

## MODIFIED Requirements

### Requirement: Mở dialog tạo Log Space
Hệ thống SHALL mở dialog New Log Space khi người dùng bấm nút **+** ở đầu danh sách Log Space trên sidebar. Dialog SHALL gồm thanh chọn loại, ô tên, phần nội dung của nấc đang chọn, nút Create và nút Cancel.
- Nấc `Static` SHALL gồm vùng kéo thả file, nút "Choose files…" và "Choose folder…", cùng danh sách file đã chọn.
- Nấc `Live` SHALL gồm vùng kéo thả file, nút "Choose files…", "Add remote files…" và "Add remote command…", cùng một danh sách gồm file local, Remote File, Live Glob và Remote Command đã chọn. Gạt sang nấc này SHALL NOT tự mở remote picker.
- Nấc `Scratch` SHALL chỉ có dòng mô tả.

#### Scenario: Mở dialog
- **WHEN** người dùng bấm **+** trên sidebar
- **THEN** dialog New Log Space hiện ra ở nấc `Scratch` với tên gợi ý "Scratch" và nút "Create" bật

#### Scenario: Hủy
- **WHEN** người dùng đã nhập tên và thả file rồi bấm Cancel
- **THEN** dialog đóng và không có Log Space nào được tạo

#### Scenario: Tạo Live Log Space từ Remote File
- **WHEN** người dùng gạt sang `Live`, nhập tên "Checkout", bấm "Add remote files…", chọn `web1:/app/app.log` và `web2:/app/app.log`, rồi bấm Create
- **THEN** Live Log Space "Checkout" được tạo với hai Remote File đó

#### Scenario: Tạo Live Log Space trộn local và remote
- **WHEN** người dùng ở nấc `Live` thả `api.log`, thêm `web1:/app/app.log` qua "Add remote files…", rồi bấm "Create with 1 file and 1 remote file"
- **THEN** một Live Log Space chứa cả `api.log` và `web1:/app/app.log` được tạo, và hai file hiện trên cùng một timeline

#### Scenario: Gạt sang Live không mở picker
- **WHEN** người dùng gạt từ `Scratch` sang `Live`
- **THEN** remote picker không mở, và nấc `Live` hiện danh sách trống cùng các nút "Choose files…", "Add remote files…" và "Add remote command…"

### Requirement: Điều kiện bật nút Create
Hệ thống SHALL chỉ bật nút Create khi hai điều kiện cùng đúng:
- Tên, sau khi bỏ khoảng trắng hai đầu, không rỗng và không trùng tên Log Space khác trong cùng Project.
- Nấc đang hiện có thứ để tạo:
  - nấc `Scratch` luôn có;
  - nấc `Static` cần ít nhất một file thêm được và không còn Archive đang đọc;
  - nấc `Live` cần ít nhất một file local thêm được, Remote File, Live Glob hoặc Remote Command.

Hệ thống SHALL hiện lý do ngay dưới ô tên khi tên bị trùng, và hiện hướng dẫn dưới danh sách khi nấc `Static` hoặc `Live` chưa có gì. Nhãn nút Create SHALL cho biết kết quả, ví dụ "Create", "Create with 3 files", "Create with 2 remote files" hoặc "Create with 1 file and 2 remote files".

#### Scenario: Tên rỗng
- **WHEN** ô tên chỉ chứa khoảng trắng
- **THEN** nút Create bị tắt

#### Scenario: Tên trùng trong Project
- **WHEN** Project đã có Log Space "Payment" và người dùng nhập ` Payment `
- **THEN** nút Create bị tắt và dưới ô tên hiện thông báo tên đã tồn tại

#### Scenario: Tên trùng ở Project khác
- **WHEN** Project "Staging" có Log Space "Payment" và người dùng đang tạo "Payment" trong Project "Prod"
- **THEN** nút Create được bật

#### Scenario: Static chưa có file
- **WHEN** người dùng gạt sang `Static` khi danh sách của nấc này trống
- **THEN** nút Create bị tắt và dưới danh sách hiện "Choose or drop files"

#### Scenario: Live chưa có gì
- **WHEN** người dùng gạt sang `Live` khi danh sách của nấc này trống
- **THEN** nút Create bị tắt và dưới danh sách hiện "Add files to follow, remote files or a command"

#### Scenario: Tạo Scratch
- **WHEN** người dùng ở nấc `Scratch`, nhập tên "Scratch 1" và bấm "Create"
- **THEN** Scratch Log Space "Scratch 1" được tạo với một file rỗng `untitled.log`

#### Scenario: Tạo Static
- **WHEN** người dùng ở nấc `Static`, nhập tên "Payment", thả `api.log` và `worker.log`, rồi bấm "Create with 2 files"
- **THEN** Static Log Space "Payment" được tạo với hai file đó

### Requirement: Nhận file vào dialog
Hệ thống SHALL nhận file vào nấc `Static` và nấc `Live` qua kéo thả và qua nút "Choose files…"; nấc `Static` SHALL nhận thêm qua "Choose folder…".
- Thả file vào dialog khi đang ở nấc `Live` SHALL thêm file vào danh sách của nấc `Live`.
- Thả file khi đang ở nấc khác SHALL tự gạt sang `Static` rồi thêm file vào danh sách của nấc này.
- Thả file SHALL NOT đụng tới danh sách của nấc còn lại.

Thư mục SHALL được mở rộng một cấp và chỉ lấy file text. File binary SHALL hiện trong danh sách kèm cảnh báo và SHALL NOT được thêm khi tạo. Archive trong nấc `Live` SHALL hiện kèm cảnh báo "Archives can only be added to a Static Log Space" và SHALL NOT được thêm khi tạo. Người dùng SHALL bỏ được từng mục khỏi danh sách trước khi bấm Create.

#### Scenario: Thả thư mục
- **WHEN** người dùng thả thư mục `logs/` chứa `a.log`, `b.log` và thư mục con `old/`
- **THEN** danh sách có `a.log` và `b.log`, không có file nào trong `old/`

#### Scenario: File binary
- **WHEN** người dùng thả `core.dump` có byte NUL
- **THEN** `core.dump` hiện trong danh sách với cảnh báo "Not a text file" và không được đếm vào nhãn Create

#### Scenario: Bỏ một file
- **WHEN** danh sách có `a.log` và `b.log` và người dùng bấm bỏ `b.log`
- **THEN** chỉ còn `a.log` và nhãn nút là "Create with 1 file"

#### Scenario: Thả file khi đang ở nấc Live
- **WHEN** dialog đang ở nấc `Live` có `web1:/app/app.log` và người dùng thả `api.log` vào dialog
- **THEN** dialog vẫn ở nấc `Live`, danh sách có cả `web1:/app/app.log` và `api.log`, và danh sách của nấc `Static` không đổi

#### Scenario: Thả file khi đang ở nấc Scratch
- **WHEN** dialog đang ở nấc `Scratch` và người dùng thả `api.log`
- **THEN** dialog gạt sang `Static` với `api.log` trong danh sách

#### Scenario: Archive ở nấc Live
- **WHEN** dialog đang ở nấc `Live` và người dùng thả `logs.tar.gz`
- **THEN** `logs.tar.gz` hiện trong danh sách với cảnh báo "Archives can only be added to a Static Log Space" và không được đếm vào nhãn Create

### Requirement: Chọn loại Log Space bằng thanh chọn
Dialog New Log Space SHALL có thanh chọn ba nấc `Static`, `Live`, `Scratch` ngay dưới tiêu đề, mỗi lúc đúng một nấc được chọn, kèm một dòng mô tả nấc đang chọn bên dưới. Dialog SHALL luôn mở ở nấc `Scratch` và SHALL NOT nhớ nấc của lần mở trước.

Mỗi nấc SHALL giữ danh sách riêng của nó. Gạt sang nấc khác SHALL chỉ ẩn danh sách đang hiện; SHALL NOT xóa nó và SHALL NOT hiện số đếm của nấc bị ẩn.

Nút Create SHALL chỉ tạo Log Space từ nấc đang hiện: `Static` tạo Static, `Live` tạo Live, `Scratch` tạo Scratch. Danh sách của các nấc khác SHALL bị bỏ.

⌘1, ⌘2, ⌘3 SHALL chọn lần lượt ba nấc khi dialog mở. Thanh chọn SHALL nhận focus bằng Tab và đổi nấc bằng ← và →.

#### Scenario: Mở ở Scratch
- **WHEN** người dùng mở dialog New Log Space, kể cả ngay sau khi vừa tạo một Live Log Space
- **THEN** nấc `Scratch` được chọn và dòng mô tả nói Log Space sẽ chứa text người dùng gõ hoặc dán

#### Scenario: Mô tả hai nấc file
- **WHEN** người dùng gạt sang `Static`, rồi sang `Live`
- **THEN** dòng mô tả của `Static` nói file được đọc như đang có trên đĩa, không theo dõi dòng mới; dòng mô tả của `Live` nói file local và file trên server được theo dõi khi có dòng mới

#### Scenario: Gạt nấc giữ danh sách
- **WHEN** nấc `Static` có `a.log` và `b.log`, người dùng gạt sang `Live`, rồi gạt lại `Static`
- **THEN** ở nấc `Live` không thấy `a.log` hay `b.log` và không có số đếm nào của nấc `Static`, và khi gạt lại thì `a.log` và `b.log` vẫn còn

#### Scenario: Create chỉ dùng nấc đang hiện
- **WHEN** nấc `Static` có `a.log`, nấc `Live` có `web1:/app/app.log`, nấc đang hiện là `Live`, và người dùng bấm Create
- **THEN** một Live Log Space chỉ chứa `web1:/app/app.log` được tạo, và `a.log` không được thêm vào đâu

#### Scenario: Chọn nấc bằng phím
- **WHEN** dialog đang mở ở nấc `Scratch` và người dùng nhấn ⌘2
- **THEN** nấc `Live` được chọn và các tab của Working Set không đổi

### Requirement: Tên gợi ý theo nấc
Khi người dùng chưa tự gõ vào ô tên, dialog SHALL gợi ý tên theo nấc đang hiện: theo mục đầu tiên trong danh sách của nấc `Static` hoặc `Live`, và "Scratch" ở nấc `Scratch`. Tên gợi ý SHALL thêm số (" 2", " 3", …) khi trùng tên Log Space khác trong Project. Khi người dùng đã tự gõ tên, gạt nấc hay thêm file SHALL NOT đổi tên.

#### Scenario: Scratch gợi ý tên
- **WHEN** Project đã có Log Space "Scratch" và người dùng mở dialog
- **THEN** ô tên là "Scratch 2"

#### Scenario: Tên đi theo nấc
- **WHEN** người dùng chưa gõ tên, thả `api.log` vào nấc `Static`, rồi gạt sang `Scratch`
- **THEN** ô tên là "api" khi ở `Static` và trở lại "Scratch" khi ở `Scratch`

#### Scenario: Tên đã gõ được giữ
- **WHEN** người dùng gõ "Checkout" rồi gạt từ `Scratch` sang `Static` và thả `api.log`
- **THEN** ô tên vẫn là "Checkout"

### Requirement: Thả file vào cửa sổ tạo Static Log Space
Khi không có dialog nào đang mở, thả file hoặc thư mục vào cửa sổ SHALL tạo ngay một Static Log Space chứa mọi file đã thả, theo cùng quy tắc nhận file như dialog, mà SHALL NOT mở dialog New Log Space.
- Tên SHALL là tên gợi ý theo file đầu tiên, thêm số khi trùng.
- Archive SHALL được đọc xong rồi mới tạo Log Space. Trong lúc đọc, status bar SHALL hiện "Reading archive…".
- Nếu không có file nào, hoặc có ít nhất một file không thêm được (không phải text, quá lớn, không đọc được, Archive không có file text), hệ thống SHALL NOT tạo Log Space. Thay vào đó, hệ thống SHALL mở dialog New Log Space ở nấc `Static` với danh sách đã thả và lỗi của từng file.
- Khi đang có dialog khác mở, thả file SHALL bị bỏ qua.

#### Scenario: Thả một file
- **WHEN** không có dialog nào mở và người dùng thả `api.log` vào cửa sổ
- **THEN** Static Log Space "api" chứa `api.log` được tạo, được mở, và không có dialog nào hiện ra

#### Scenario: Thả nhiều file
- **WHEN** Project đã có Log Space "api" và người dùng thả `api.log` cùng `worker.log`
- **THEN** một Static Log Space "api 2" chứa cả hai file được tạo

#### Scenario: Thả Archive
- **WHEN** người dùng thả `logs.tar.gz` chứa `a.log` và `b.log`
- **THEN** status bar hiện "Reading archive…" trong lúc đọc, rồi một Static Log Space chứa hai Archive Entry đó được tạo

#### Scenario: Có file không dùng được
- **WHEN** người dùng thả `api.log` cùng `core.dump` có byte NUL
- **THEN** không Log Space nào được tạo, và dialog New Log Space mở ở nấc `Static` với `api.log` và `core.dump` kèm cảnh báo "Not a text file"

### Requirement: Loại file theo loại Log Space
Hệ thống SHALL quy định loại file theo loại Log Space:
- Static Log Space SHALL chỉ chứa file có sẵn trên đĩa, Archive Entry và Remote Copy.
- Live Log Space SHALL chỉ chứa file local có sẵn trên đĩa, Remote File, Live Glob và Remote Command; nó SHALL NOT chứa Archive Entry hay Remote Copy.
- Scratch Log Space SHALL chỉ chứa file scratch.

#### Scenario: Menu của Static
- **WHEN** người dùng chuột phải lên một Static Log Space
- **THEN** menu có "Add files…" và không có "New scratch file" hay "Add remote files…"

#### Scenario: Menu của Live
- **WHEN** người dùng chuột phải lên một Live Log Space
- **THEN** menu có "Add files…", "Add remote files…" và "Add remote command…", và không có "New scratch file"

#### Scenario: Menu của Scratch
- **WHEN** người dùng chuột phải lên một Scratch Log Space
- **THEN** menu có "New scratch file" và không có "Add files…"

#### Scenario: Open in full tạo Remote Copy trong Static
- **WHEN** người dùng chọn "Open in full" trên Remote File `web1:/app/app.log` của một Live Log Space
- **THEN** Remote Copy `web1:/app/app.log` được thêm vào một Static Log Space, không vào Live Log Space đó

### Requirement: File trên đĩa chỉ đọc
Hệ thống SHALL NOT sửa, đổi tên hay xóa file local của Static Log Space hay Live Log Space trên đĩa. Gỡ file khỏi Log Space SHALL chỉ bỏ nó khỏi danh sách.

#### Scenario: Gỡ file
- **WHEN** người dùng chọn "Remove from Log Space" trên `api.log`
- **THEN** `api.log` biến mất khỏi Log Space, file trên đĩa vẫn còn nguyên, và các Marker của nó bị bỏ

#### Scenario: Gỡ file cuối cùng
- **WHEN** người dùng gỡ file duy nhất của một Static Log Space
- **THEN** Log Space vẫn còn, và canvas hiện trạng thái rỗng kèm nút "Add files"

### Requirement: Thêm file vào Log Space đã có
Hệ thống SHALL cho thêm file local vào Static Log Space và Live Log Space qua "Add files…", nhận file theo cùng quy tắc như nấc tương ứng của dialog. File đã có trong Log Space SHALL bị bỏ qua kèm thông báo nhẹ.

#### Scenario: Thêm file trùng
- **WHEN** Log Space đã có `/var/log/api.log` và người dùng thêm lại file đó
- **THEN** danh sách file không đổi và hiện thông báo file đã có

#### Scenario: Thêm file vào Live
- **WHEN** người dùng chọn "Add files…" trên một Live Log Space và chọn `worker.log`
- **THEN** `worker.log` được thêm vào và được theo dõi khi có dòng mới

### Requirement: Xóa Log Space
Hệ thống SHALL xóa Log Space sau khi người dùng xác nhận. Xóa Scratch Log Space SHALL xóa các file scratch của nó. Xóa Static Log Space hay Live Log Space SHALL NOT đụng tới file trên đĩa.

#### Scenario: Xóa Static
- **WHEN** người dùng xác nhận xóa Static Log Space "Payment"
- **THEN** "Payment" và các tab của nó biến mất, file log trên đĩa vẫn còn

### Requirement: Sidebar Log Space
Hệ thống SHALL hiện mỗi Log Space trên sidebar thành một dòng có chevron để mở danh sách file, cùng biểu tượng phân biệt Static, Live và Scratch. Bấm tên Log Space SHALL mở Log Space; bấm tên file SHALL mở File View của file đó.

#### Scenario: Mở danh sách file
- **WHEN** người dùng bấm chevron của "Payment"
- **THEN** các file của "Payment" hiện bên dưới theo thứ tự file của Log Space

## ADDED Requirements

### Requirement: Session cũ mở được với loại mới
Hệ thống SHALL mở session đã lưu với loại cũ: Imported Log Space SHALL mở thành Static Log Space, Remote Log Space SHALL mở thành Live Log Space, với nguyên file, Filter, Marker và tab. Lần lưu sau SHALL ghi tên loại mới. Khi session có Imported Log Space chứa file local, hệ thống SHALL hiện một lần notice "Imported Log Spaces are now Static. Create a Live Log Space to follow files."

#### Scenario: Mở session có Imported
- **WHEN** session đã lưu có Imported Log Space "Payment" chứa `api.log` với một Marker
- **THEN** "Payment" mở thành Static Log Space, vẫn có `api.log` và Marker đó, và notice hiện một lần

#### Scenario: Mở session có Remote
- **WHEN** session đã lưu có Remote Log Space "Checkout" với `web1:/app/app.log`
- **THEN** "Checkout" mở thành Live Log Space và `web1:/app/app.log` được đọc live như trước
