## Why

Sau change `line-offset-index`, file local chỉ tốn khoảng 29 byte Line Index mỗi dòng, vì text nằm trên đĩa. Remote File và Remote Command thì vẫn giữ text của cả Live Window trong memory (`LineSource` trong memory là bước tạm, xem decision 2 của `line-offset-index`). Vì vậy:
- Remote vẫn là nguồn tốn memory nhất.
- Remote vẫn đi một đường code riêng (`ParsedFile::entries`, `ParsedFile::lines`, `LiveBuffer`), nên task 12.1 của `line-offset-index` chưa xoá được đường cũ.
- Quay lại một Dormant Tab Remote phải tải lại Live Window từ Server.

## What Changes

- **Cache trên đĩa cho mọi dòng Remote nhận được.** Dòng mà Remote File và Remote Command nhận được ghi vào một file trong `<app data>/remote-cache/`. File đó được index và đọc như file local: Line Index, Scan, LRU.
  - Live Window không còn là giới hạn memory.
  - `LineSource` trong memory của Remote bị bỏ.
  - Task 9.4 của `line-offset-index` ("Line Index trên bytes của Live Window") được thay bằng change này.
- **Dormant Tab Remote mở lại từ cache.** Chọn lại tab thì view hiện ngay từ cache, rồi tail tiếp từ kích thước đã có, sau khi kiểm tra cache còn khớp với file trên Server (xem "Đồng bộ" bên dưới). Không còn phải tải lại Live Window.
- **Đồng bộ theo hash khối, qua SFTP sẵn có, không cài tailer lên Server.** Khi kết nối lại, hoặc khi mở lại Project, app làm như sau:
  - Đọc kích thước của file trên Server.
  - Đọc khối 4 KiB đầu file, và khối 4 KiB ngay trước kích thước cache đã có.
  - So hash hai khối đó với hash của cùng hai khối trong cache. Đây cũng là cách nhận ra rotate và truncate của file local trong `static-live-log-spaces`.
  - Khớp: chỉ tải phần từ kích thước cache trở đi.
  - Không khớp: file đã bị rotate, thay hoặc cắt. Cache cũ thành Generation cũ, có Break, và file mới được đọc từ đầu trong phạm vi Live Window. Cách xử lý giống file local trong `static-live-log-spaces`.
- **Remote Command không có file để đồng bộ.** Output của lần chạy hiện tại ghi vào cache như Remote File. Mở lại thì cache của lần chạy trước hiện ngay, dưới một Break, rồi lệnh chạy lại và output mới nối tiếp.
- **Quy tắc dọn cache:**
  - Cache của một file bị xoá khi file đó rời Log Space, khi Log Space bị xoá, hoặc khi Server bị sửa sao cho file thành file khác.
  - Lúc khởi động, app quét dọn cache không có Log Space nào trỏ tới, như `remote::copy::sweep` và `archive-cache/`.
  - Tổng dung lượng có giới hạn chung cho cả app (mặc định 2 GB, chỉnh trong Settings). Khi vượt, app bỏ cache của file lâu nhất chưa được mở. Không bao giờ bỏ cache của file thuộc tab đang mở.
  - Cache không được mở quá 14 ngày cũng bị bỏ. lnav dọn sau 2 ngày; app chọn lâu hơn vì người dùng hay quay lại các Project cũ.
  - Xoá thư mục `remote-cache/` lúc nào cũng an toàn: app tải lại từ Server.
- **Live Window đổi nghĩa.** Live Window không còn là số dòng giữ trong memory. Nó là số dòng mới nhất mỗi Remote File giữ trong cache:
  - Dòng cũ hơn rời cache. L12 (Leaving The Window) và Load more giữ nguyên hành vi.
  - Clear vẫn chỉ có cho Remote File và vẫn bỏ các dòng đang giữ (cắt cache). File local vẫn không có Clear.
  - **BREAKING** (nhỏ): mặc định của Live Window có thể tăng lên nhiều, vì mỗi dòng giờ tốn đĩa chứ không tốn RAM. Con số cụ thể chốt ở design.
- **Xoá đường cũ.** Khi Remote đã đi qua cache, task 12.1 của `line-offset-index` làm được: xoá `ParsedFile::entries`, `ParsedFile::lines`, `LiveBuffer` cho Remote, và các đường dựng view cũ.

## Capabilities

### New Capabilities
- `remote-cache`:
  - Dòng Remote nhận được ghi vào cache trên đĩa và được đọc qua Line Index.
  - Cache được đồng bộ với file trên Server theo hash khối khi kết nối lại.
  - Dormant Tab Remote mở lại từ cache.
  - Cache được dọn theo các quy tắc ở trên.

### Modified Capabilities
- `live-log-space`: requirement "Thao tác chỉ của Remote File" đổi nghĩa của Live Window (số dòng giữ trong cache, không còn là memory). Clear và Load more tác động lên cache. Requirement mới: mở lại Live Log Space hiện ngay các dòng đã có trong cache của Remote File, rồi tail tiếp.

## Impact

- **Code chịu ảnh hưởng chính:**
  - `src/live/buffer.rs` (`LiveBuffer`)
  - `src/live/remote.rs` và nguồn của Remote Command
  - `src/app/live.rs`, `src/app/live_more.rs`, `src/app/live_trim.rs`
  - `src/space_view.rs` (`ParsedFile`), `src/index/source.rs` (thêm source ghi vào cuối file cache)
  - `src/remote/copy.rs` (dùng lại sweep)
- **Lưu trữ:** thư mục mới `<app data>/remote-cache/<server id>/<hash của path>/`, mỗi Generation một file, kèm `meta.json` ghi kích thước, hash hai khối và thời điểm mở gần nhất.
- **Settings:** thêm giới hạn dung lượng cache của Remote.
- **CONTEXT.md:** sửa Live Window, L10 (Live Is A Window, Not An Archive: dòng nhận được nay được giữ trên đĩa giữa các lần mở) và quy tắc 43 (Text Stays On Disk: bỏ ngoại lệ của Remote). Thêm thuật ngữ Remote Cache.
- **Phụ thuộc:** change này nối tiếp `line-offset-index`. Nó thay task 9.4 của change đó, và làm được task 12.1.
