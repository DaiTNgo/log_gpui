## ADDED Requirements

### Requirement: Mở Edit
Hệ thống SHALL có mục "Edit…" trong menu chuột phải ở sidebar của mỗi Remote File, Live Glob và Remote Command. Mục này mở dialog đã dùng khi thêm mục đó: file picker cho Remote File và Live Glob, dialog Add remote command cho Remote Command. Dialog SHALL điền sẵn giá trị hiện tại, chỉ chứa đúng mục đang sửa, và có nút Save thay cho nút thêm. Remote Copy và file của Imported Log Space hay Scratch Log Space SHALL NOT có "Edit…".

#### Scenario: Sửa Remote File
- **WHEN** người dùng chuột phải `web1:/app/app.log` rồi chọn "Edit…"
- **THEN** file picker mở ra với Server "web1", ô đường dẫn `/app/app.log` và Starting Time hiện tại, chỉ có một mục được chọn, và nút xác nhận là Save

#### Scenario: Remote Copy
- **WHEN** người dùng chuột phải Remote Copy `web1:/app/app.log` trong một Imported Log Space
- **THEN** menu có "Read again" và không có "Edit…"

### Requirement: Save chỉ khi có thay đổi hợp lệ
Nút Save SHALL bị tắt khi giá trị trong dialog giống giá trị hiện tại. Khi người dùng bấm Save, hệ thống SHALL kiểm tra giá trị mới như lúc thêm: kết nối được Server, thư mục cha tồn tại, Starting Time đọc được, và không trùng với Remote File, Live Glob hay Remote Command khác của cùng Log Space. Nếu giá trị không hợp lệ, hệ thống SHALL không đổi gì, giữ dialog mở và báo lỗi. Hủy dialog SHALL không đổi gì.

#### Scenario: Thư mục không tồn tại
- **WHEN** người dùng sửa `web1:/app/app.log` thành `/no/such/dir/app.log` rồi bấm Save
- **THEN** dialog báo thư mục không tồn tại, và `web1:/app/app.log` vẫn đọc live như trước

#### Scenario: Trùng file khác
- **WHEN** Log Space có `web1:/app/a.log` và `web1:/app/b.log`, và người dùng sửa `a.log` thành `/app/b.log`
- **THEN** dialog báo file đã có trong Log Space và không có gì thay đổi

### Requirement: Đổi Starting Time của Remote File
Khi chỉ Starting Time của một Remote File thay đổi, hệ thống SHALL coi đó vẫn là cùng một file, bỏ các dòng đã nạp và nạp lại file theo Starting Time mới như khi thêm. Marker của file SHALL được giữ cho những dòng còn được nạp. File SHALL tiếp tục nhận dòng mới sau khi nạp xong.

#### Scenario: Lùi Starting Time
- **WHEN** `web1:/app/app.log` đang có Starting Time `1h` với một Marker trên dòng 40.000, và người dùng đổi thành `3h`
- **THEN** file được nạp lại từ 3 giờ trước, Marker trên dòng 40.000 vẫn còn, và dòng mới tiếp tục đến

#### Scenario: Xoá Starting Time
- **WHEN** người dùng xoá trống ô Starting Time rồi bấm Save
- **THEN** file được nạp lại theo các dòng cuối như mặc định

### Requirement: Đổi path hoặc Server của Remote File
Khi path hoặc Server của một Remote File thay đổi, hệ thống SHALL dừng đọc file cũ, bỏ dòng và Marker của file cũ, rồi đọc file mới như khi vừa thêm, kể cả trạng thái "Waiting for file" nếu file chưa có. File mới SHALL giữ chỗ của file cũ trong sidebar và trong thứ tự file của Log Space. File View và Pinned Tab của file cũ SHALL chuyển sang file mới và đổi tên theo. File trên cả hai Server SHALL không bị thay đổi.

#### Scenario: Sửa path gõ nhầm
- **WHEN** Log Space "Checkout" có `web1:/app/ap.log` (đang "Waiting for file") ở vị trí thứ hai, và người dùng sửa thành `/app/app.log`
- **THEN** sidebar hiện `web1:/app/app.log` ở vị trí thứ hai, dòng của `app.log` bắt đầu hiện, và File View đang mở của file đó đổi tên thành `web1:/app/app.log`

#### Scenario: Chuyển sang Server khác
- **WHEN** người dùng đổi Server của `web1:/app/app.log` thành "web2"
- **THEN** dòng và Marker của `web1:/app/app.log` biến mất, `web2:/app/app.log` được đọc từ các dòng cuối, và kết nối tới web1 được đóng nếu Log Space không còn mục nào trên web1

### Requirement: Sửa Live Glob
Hệ thống SHALL cho sửa pattern, thư mục và Server của một Live Glob. Trước khi Save, dialog SHALL liệt kê file trên Server khớp với giá trị mới và hiện tóm tắt so với hiện tại dạng `+N files · −M files · K kept`. Khi Save, hệ thống SHALL:
- giữ nguyên file vẫn khớp, kể cả dòng, Marker và việc đọc đang chạy;
- gỡ file không còn khớp, như "Remove from Log Space" (không coi là Gone);
- thêm file mới khớp theo giới hạn số file của Live Glob.

Danh sách file người dùng đã gỡ khỏi glob SHALL chỉ giữ những đường dẫn còn khớp giá trị mới. File đã được thêm bằng path SHALL không bị glob thêm lần nữa.

#### Scenario: Thu hẹp pattern
- **WHEN** Live Glob `web1:/var/log/app/*.log` đang có `app-1.log`, `app-2.log` và `worker.log`, và người dùng đổi pattern thành `app-*.log`
- **THEN** dialog hiện `+0 files · −1 file · 2 kept`, và sau khi Save thì `worker.log` cùng dòng của nó biến mất còn `app-1.log` và `app-2.log` vẫn đọc live không bị nạp lại

#### Scenario: Đổi thư mục
- **WHEN** người dùng đổi thư mục của Live Glob từ `/var/log/app` sang `/var/log/app2`
- **THEN** mọi file của `/var/log/app` bị gỡ, và các file `*.log` trong `/var/log/app2` được thêm và đọc từ các dòng cuối

#### Scenario: File mới khớp sau khi sửa
- **WHEN** Live Glob đã được sửa thành `app-*.log` và sau đó `app-3.log` được tạo trên Server
- **THEN** `app-3.log` được thêm như với mọi Live Glob

### Requirement: Sửa Stopped Live Glob
Khi người dùng sửa một Stopped Live Glob, Live Glob SHALL vẫn là Stopped. File không còn khớp SHALL bị gỡ, nhưng file mới khớp SHALL NOT được thêm cho tới khi người dùng chọn Resume watching. Dialog SHALL ghi rõ điều này cạnh phần tóm tắt.

#### Scenario: Sửa khi đang Stopped
- **WHEN** Live Glob `web1:/var/log/app/*.log` đang Stopped, và người dùng đổi pattern thành `api-*.log` trong khi thư mục có `api-1.log`
- **THEN** dialog hiện "Stopped: matching files are added when you resume", sau khi Save các file không khớp bị gỡ, và `api-1.log` chỉ được thêm khi người dùng chọn Resume watching

### Requirement: Sửa Remote Command
Hệ thống SHALL cho sửa lệnh, giá trị của Command Preset và Server của một Remote Command. Khi Save, hệ thống SHALL kết thúc Run đang chạy trên Server nếu có, giữ các dòng và Marker đã có, vẽ Break "command changed" ghi `<server>$ <lệnh>` cũ và mới, rồi chạy lệnh mới thành Run mới bắt đầu từ dòng 1, như Restart. Dòng cũ SHALL vẫn theo Live Window như mọi dòng khác. Source Column SHALL ghi tên mới của Remote Command cho mọi dòng của nó, kể cả dòng của các Run trước; lệnh cũ chỉ còn được ghi trên Break. Lệnh mới SHALL theo đúng quy tắc của Command Preset như lúc thêm: sửa tay ô lệnh thì Remote Command thành lệnh gõ tay và dialog báo nó sẽ không được Resume By Time. Restart on exit SHALL giữ giá trị trước khi sửa. Remote Command SHALL giữ chỗ trong sidebar, và File View của nó SHALL đổi tên theo `<server>$ <lệnh>` mới.

#### Scenario: Đổi giá trị preset
- **WHEN** `web1$ docker logs -f --timestamps --tail 1000 'api'` đang chạy, và người dùng đổi container thành `worker` rồi bấm Save
- **THEN** tiến trình cũ trên web1 được yêu cầu dừng, dòng cũ vẫn còn, Break "command changed: web1$ docker logs … 'api' → web1$ docker logs … 'worker'" xuất hiện sau dòng cuối của Run cũ, và `web1$ docker logs -f --timestamps --tail 1000 'worker'` chạy từ dòng 1 và vẫn được Resume By Time

#### Scenario: Marker trên dòng cũ
- **WHEN** một dòng của Run cũ có Marker và người dùng sửa lệnh rồi bấm Save
- **THEN** Marker vẫn còn trên dòng đó sau khi Run mới bắt đầu

#### Scenario: Đổi Server của lệnh
- **WHEN** người dùng đổi Server của `web1$ journalctl -f -u nginx` thành "web2"
- **THEN** dòng của web1 vẫn còn, Break "command changed: web1$ journalctl -f -u nginx → web2$ journalctl -f -u nginx" xuất hiện, và output của web2 bắt đầu hiện sau Break

#### Scenario: Sửa tay lệnh
- **WHEN** người dùng sửa ô lệnh của preset journalctl từ `-n 1000` thành `-n 50`
- **THEN** dialog báo "Edited: will rerun as typed after a lost connection" trước khi Save

### Requirement: Sửa khi Log Space đang Pause hoặc mất kết nối
Sửa một mục khi Log Space đang Pause SHALL nạp dòng ban đầu của mục mới như khi thêm mục vào Log Space đang Pause, và Pause SHALL vẫn giữ. Khi Server mới cần đăng nhập mà chưa có mật khẩu, hệ thống SHALL hỏi qua Sign-in Prompt trước khi kiểm tra giá trị mới.

#### Scenario: Server cần mật khẩu
- **WHEN** người dùng đổi Server của một Remote File sang "db", và "db" đăng nhập bằng mật khẩu không được ghi nhớ
- **THEN** hệ thống hỏi mật khẩu, và chỉ thay file sau khi đăng nhập thành công và file hợp lệ

### Requirement: Lưu sau khi sửa
Giá trị đã sửa SHALL được lưu vào Project, nên mở lại Project sẽ kết nối theo giá trị mới. Path và Live Glob mới SHALL được thêm vào Recent Paths của Server tương ứng.

#### Scenario: Mở lại sau khi sửa
- **WHEN** người dùng sửa `web1:/app/ap.log` thành `/app/app.log`, thoát app rồi mở lại
- **THEN** Log Space kết nối và đọc `web1:/app/app.log`, và Recent Paths của web1 có `/app/app.log`
