## MODIFIED Requirements

### Requirement: Chọn file trên Server
Hệ thống SHALL cho chọn Remote File bằng ba cách: duyệt thư mục qua SFTP, gõ đường dẫn tuyệt đối, hoặc gõ glob như `/var/log/app/*.log`. Glob SHALL chỉ nằm ở tên file, không ở thư mục. Glob gõ vào ô đường dẫn SHALL được thêm thành một Live Glob gắn với Remote Log Space: mọi file khớp lúc thêm và mọi file khớp sinh ra sau đó SHALL được thêm làm Remote File. Live Glob SHALL được thêm cả khi chưa có file nào khớp, miễn là thư mục của nó tồn tại.

#### Scenario: Duyệt thư mục
- **WHEN** người dùng chọn Server "web1" và mở `/var/log/app/`
- **THEN** hệ thống liệt kê file và thư mục con kèm kích thước và thời gian sửa, và người dùng tích được nhiều file

#### Scenario: Glob
- **WHEN** người dùng gõ `/var/log/app/*.log` và thư mục có `a.log`, `b.log`
- **THEN** Live Glob `web1:/var/log/app/*.log` được thêm cùng hai Remote File `web1:/var/log/app/a.log` và `web1:/var/log/app/b.log`, và khi `c.log` được tạo sau đó thì `web1:/var/log/app/c.log` cũng được thêm

#### Scenario: Glob chưa khớp file nào
- **WHEN** người dùng gõ `/var/log/app/worker-*.log` khi `/var/log/app/` có nhưng chưa có file nào khớp
- **THEN** Live Glob được thêm, chưa có Remote File nào, và file khớp đầu tiên xuất hiện sau đó được thêm và đọc

#### Scenario: Glob ở thư mục
- **WHEN** người dùng gõ `/var/log/*/app.log`
- **THEN** hệ thống từ chối và báo glob chỉ được nằm ở tên file

## ADDED Requirements

### Requirement: Live Glob thêm file mới
Trong lúc Remote Log Space đang mở, không bị Pause, và Live Glob không bị dừng, hệ thống SHALL kiểm tra thư mục của mỗi Live Glob theo nhịp quét và SHALL thêm mỗi file mới khớp làm Remote File trong vòng hai nhịp quét kể từ khi file xuất hiện. File được thêm như vậy SHALL được mở như mọi Remote File, có Live Window riêng, và tên `<server>:<path>`. Sidebar SHALL hiện Live Glob như một nhóm chứa các Remote File nó đã thêm. Hệ thống SHALL NOT thêm thư mục, và SHALL NOT thêm file nén hay Archive mà không phải Rotated File của một Remote File, nhận biết bằng vài byte đầu.

#### Scenario: Log theo ngày
- **WHEN** Live Glob `web1:/app/logs/app-*.log` đang có `app-2026-10-19.log` và lúc 00:00 Server tạo `app-2026-10-20.log`
- **THEN** với nhịp quét mặc định, trong vòng 10 giây `web1:/app/logs/app-2026-10-20.log` xuất hiện trong nhóm của Live Glob và các dòng của nó xen vào view theo timestamp

#### Scenario: File nén không phải Rotated File
- **WHEN** Live Glob `web1:/var/log/app/*` khớp `app.log` và `dump.tar.gz`
- **THEN** chỉ `web1:/var/log/app/app.log` được thêm

### Requirement: Live Glob không đọc lại file đã đọc
Một Live Glob SHALL NOT thêm một Rotated File làm Remote File riêng, kể cả khi tên của nó khớp glob, vì Rotated File là Generation cũ hơn của Remote File mà nó thuộc về; khi Live Glob khớp cả một file và các Rotated File của nó, chỉ file đó SHALL được thêm. Ngoài ra, một Live Glob SHALL NOT đọc cùng một file trên Server hai lần dưới hai đường dẫn: khi một file mới khớp là file Live Glob đó đã đọc dưới đường dẫn khác (nhận biết qua inode), hệ thống SHALL NOT thêm nó. Khi đường dẫn của một Remote File thuộc Live Glob giờ chứa một file mà Live Glob đó đã đọc dưới đường dẫn khác, Remote File đó SHALL chuyển sang Gone thay vì đọc file đó như một Generation mới. Khi Server không cho biết inode, hệ thống SHALL xử lý file như file mới.

#### Scenario: Logrotate với glob rộng
- **WHEN** Live Glob `web1:/var/log/app/app.log*` đang đọc `app.log`, và logrotate đổi tên `app.log` thành `app.log.1` rồi tạo `app.log` mới
- **THEN** `web1:/var/log/app/app.log` vẽ Break "rotated" và đọc file mới như Generation mới, còn `app.log.1` không được thêm làm Remote File riêng vì nó là Rotated File của `app.log`

#### Scenario: Thêm glob khi đã có file xoay vòng
- **WHEN** người dùng thêm Live Glob `web1:/var/log/app/app.log*` và thư mục có `app.log`, `app.log.1`, `app.log.2.gz`
- **THEN** chỉ `web1:/var/log/app/app.log` được thêm; `app.log.1` và `app.log.2.gz` là Rotated File của nó

#### Scenario: Đổi tên không theo kiểu Rotated File
- **WHEN** Live Glob `web1:/data/*.log` đang đọc `current.log`, và một job đổi tên `current.log` thành `old-batch.log` rồi tạo `current.log` mới
- **THEN** `old-batch.log` không được thêm vì Live Glob đã đọc file đó dưới tên `current.log`

### Requirement: File Gone
Khi một Remote File được Live Glob thêm không còn tồn tại trên Server, hệ thống SHALL giữ các dòng đã đọc, vẽ Break "deleted", và chuyển file sang trạng thái Gone. Trạng thái Gone SHALL được liệt kê khi hover hoặc bấm vào trạng thái trên status bar nhưng SHALL NOT được tính là trạng thái xấu nhất. Nếu một file lại xuất hiện ở đúng đường dẫn đó trong lúc Log Space đang mở, hệ thống SHALL đọc nó như một Generation mới của Remote File đó. "Download all" SHALL bỏ qua file Gone. Remote File được thêm bằng đường dẫn, không qua Live Glob, SHALL giữ hành vi chờ file như trước.

#### Scenario: Xoá log cũ
- **WHEN** Live Glob đang có `app-2026-10-12.log` và job dọn dẹp xoá file đó trên Server
- **THEN** các dòng của file vẫn hiện, có Break "web1:/app/logs/app-2026-10-12.log deleted", và status bar vẫn hiện "Live" nếu các file khác đang live

#### Scenario: File có lại
- **WHEN** `web1:/var/log/app/a.log` đang Gone và một file mới được tạo ở `/var/log/app/a.log`
- **THEN** file đó được đọc như một Generation mới của `web1:/var/log/app/a.log`, bắt đầu từ dòng 1

### Requirement: Gỡ file và gỡ Live Glob
Gỡ một Remote File do Live Glob thêm SHALL đóng việc đọc file đó như mọi Remote File, và Live Glob SHALL NOT tự thêm lại đường dẫn đó. Khi người dùng thêm lại đúng đường dẫn đó trên cùng Server qua bộ chọn, hệ thống SHALL bỏ đường dẫn khỏi danh sách loại trừ của Live Glob và SHALL thêm file trở lại nhóm của Live Glob. Gỡ một Live Glob SHALL gỡ mọi Remote File nó đã thêm và SHALL NOT gỡ Remote File được thêm bằng đường dẫn. Không thao tác nào SHALL thay đổi file trên Server.

#### Scenario: Gỡ một file khỏi glob
- **WHEN** người dùng gỡ `web1:/var/log/app/debug.log` khỏi nhóm của Live Glob `web1:/var/log/app/*.log`
- **THEN** các dòng của `debug.log` biến mất, và khi `debug.log` được ghi tiếp hoặc tạo lại thì nó không được thêm lại

#### Scenario: Thêm lại file đã gỡ
- **WHEN** `web1:/var/log/app/debug.log` đã bị gỡ khỏi Live Glob `web1:/var/log/app/*.log` và người dùng gõ `/var/log/app/debug.log` trên "web1" trong bộ chọn rồi thêm
- **THEN** `debug.log` trở lại nhóm của Live Glob và được mở như một Remote File mới

#### Scenario: Gỡ cả Live Glob
- **WHEN** Log Space có Live Glob `web1:/var/log/app/*.log` với `a.log`, `b.log`, và `web1:/var/log/app/a.log` cũng đã được thêm bằng đường dẫn trước đó
- **THEN** gỡ Live Glob bỏ `b.log` khỏi Log Space, còn `a.log` vẫn ở lại như Remote File thêm bằng đường dẫn

### Requirement: Không trùng giữa Live Glob và file
Một Remote Log Space SHALL NOT có hai Live Glob cùng Server và cùng mẫu, và SHALL NOT có hai Remote File cùng Server và cùng đường dẫn dù chúng đến từ Live Glob hay từ đường dẫn. Khi Live Glob khớp một file đã có trong Log Space, file đó SHALL giữ nguồn gốc ban đầu của nó.

#### Scenario: Thêm trùng Live Glob
- **WHEN** Log Space đã có Live Glob `web1:/var/log/app/*.log` và người dùng thêm lại đúng mẫu đó
- **THEN** danh sách không đổi và hiện thông báo Live Glob đã có

### Requirement: Giới hạn số file và nhịp quét của Live Glob
Settings SHALL có hai giá trị chung cho cả ứng dụng: số Remote File không Gone tối đa một Live Glob đọc cùng lúc (mặc định 50) và nhịp quét thư mục của Live Glob (mặc định 5 giây). Một Live Glob SHALL NOT đọc nhiều hơn giới hạn đó. Khi có nhiều file khớp hơn, hệ thống SHALL chọn các file sửa gần nhất, và SHALL báo trên status bar số file khớp nhưng không được đọc. Khi một file của Live Glob chuyển sang Gone hoặc bị gỡ, hoặc khi giới hạn được tăng, file khớp sửa gần nhất đang chờ SHALL được thêm vào chỗ trống. Khi giới hạn được giảm xuống dưới số file đang đọc, các file đang đọc SHALL được giữ và Live Glob SHALL NOT thêm file mới cho đến khi xuống dưới giới hạn. Đổi nhịp quét SHALL có hiệu lực ngay cho mọi Live Glob đang mở.

#### Scenario: Thư mục quá nhiều file
- **WHEN** người dùng thêm Live Glob `web1:/var/log/app/*.log` và thư mục có 80 file khớp
- **THEN** 50 file sửa gần nhất được thêm và status bar báo "web1:/var/log/app/*.log: 30 matching files not read (limit 50)"

#### Scenario: Tăng giới hạn
- **WHEN** Live Glob đang đọc 50 file với 30 file chờ và người dùng đổi giới hạn trong Settings thành 60
- **THEN** 10 file chờ sửa gần nhất được thêm và status bar báo còn 20 file không được đọc

#### Scenario: Giảm giới hạn
- **WHEN** Live Glob đang đọc 50 file và người dùng đổi giới hạn thành 20
- **THEN** cả 50 file vẫn được đọc, và file khớp mới chỉ được thêm khi số file không Gone xuống dưới 20

### Requirement: Stop watching
Hệ thống SHALL cho dừng một Live Glob bằng "Stop watching" và cho chạy lại bằng "Resume watching" trên nhóm của nó ở sidebar. Khi Live Glob bị dừng, các file nó đã thêm SHALL tiếp tục được đọc live và vẫn có thể chuyển sang Gone, nhưng Live Glob SHALL NOT thêm file khớp mới và SHALL NOT báo file chờ trên status bar. Sidebar SHALL hiện nhãn "stopped" trên nhóm của Live Glob bị dừng. Khi chạy lại, hệ thống SHALL kiểm tra thư mục ngay và SHALL đọc các file khớp sinh ra trong lúc dừng từ đầu file, trong giới hạn Live Window. Trạng thái dừng SHALL được lưu trong Remote Log Space; khi mở lại Project, Live Glob bị dừng SHALL vẫn dừng, và chỉ các file nó đã có mà vẫn còn trên Server SHALL được mở lại. Dừng Live Glob SHALL NOT ảnh hưởng tới Pause hay các Live Glob khác.

#### Scenario: Dừng để giữ nguyên danh sách
- **WHEN** Live Glob `web1:/var/log/app/*.log` có `a.log`, `b.log`, người dùng chọn "Stop watching", rồi `c.log` được tạo và `a.log` được ghi thêm
- **THEN** các dòng mới của `a.log` vẫn hiện, `c.log` không được thêm, và nhóm Live Glob có nhãn "stopped"

#### Scenario: Chạy lại
- **WHEN** Live Glob đang dừng, `c.log` được tạo trong lúc dừng với 300 dòng, và người dùng chọn "Resume watching"
- **THEN** `web1:/var/log/app/c.log` được thêm ngay với cả 300 dòng

#### Scenario: Mở lại Project khi đang dừng
- **WHEN** người dùng đóng Project khi Live Glob đang dừng với `a.log`, `b.log`, và lúc mở lại thư mục có `b.log`, `c.log`
- **THEN** Live Glob vẫn dừng, chỉ `b.log` được mở lại, và `c.log` không được thêm

### Requirement: Live Glob khi Pause
Khi Remote Log Space bị Pause, Live Glob SHALL NOT thêm file mới. Khi tiếp tục, hệ thống SHALL kiểm tra ngay mọi Live Glob và SHALL đọc các file sinh ra trong lúc Pause từ đầu file, trong giới hạn Live Window, để không bỏ dòng nào.

#### Scenario: File mới trong lúc Pause
- **WHEN** Log Space bị Pause lúc 10:00, Server tạo `c.log` khớp Live Glob lúc 10:02 và ghi 200 dòng, và người dùng tiếp tục lúc 10:05
- **THEN** sau khi tiếp tục `web1:/var/log/app/c.log` được thêm với cả 200 dòng

### Requirement: Live Glob khi mở lại Project
Hệ thống SHALL lưu Live Glob, cùng các đường dẫn người dùng đã gỡ khỏi nó, trong Remote Log Space. Khi mở lại Project hoặc khởi động lại ứng dụng, mỗi Live Glob không bị dừng SHALL được khớp lại: các file đang khớp SHALL được thêm và mở theo Live Window, và file đã Gone hay không còn khớp SHALL NOT được thêm lại. Khi thư mục của Live Glob không còn tồn tại, Live Glob SHALL ở trạng thái chờ và bắt đầu thêm file khi thư mục xuất hiện.

#### Scenario: Mở lại sau một ngày
- **WHEN** người dùng đóng "Checkout" khi Live Glob có `app-2026-10-19.log`, và mở lại ngày hôm sau khi thư mục có `app-2026-10-19.log` và `app-2026-10-20.log`
- **THEN** cả hai file được thêm và hiện các dòng cuối hiện tại của chúng
