## ADDED Requirements

### Requirement: Nhận biết Rotated File
Hệ thống SHALL nhận một file là Rotated File của Remote File `<dir>/<name>` khi file đó nằm trong `<dir>` và có tên khớp `<name>.<N>`, `<name>-<YYYYMMDD>`, `<name>-<YYYYMMDDHHMMSS>`, `<name>-<YYYY-MM-DD>` hoặc `<name>.<YYYY-MM-DD>`, có thể kèm đuôi `.gz`, `.bz2`, `.xz` hoặc `.zst`. Các Rotated File SHALL được xếp theo thời gian sửa, mới nhất trước; Rotated File thứ k trong thứ tự đó SHALL là Generation cách file hiện tại k bước.

#### Scenario: Logrotate đánh số
- **WHEN** `/app/` trên `web1` có `app.log`, `app.log.1`, `app.log.2.gz`, `app.log.3.gz` và `app.log.bak`
- **THEN** Rotated File của `web1:/app/app.log` là `app.log.1`, `app.log.2.gz`, `app.log.3.gz` theo thứ tự đó, và `app.log.bak` không có trong danh sách

#### Scenario: Logrotate dateext
- **WHEN** `/app/` có `app.log-20261017.gz` sửa lúc 2026-10-17 và `app.log-20261018` sửa lúc 2026-10-18
- **THEN** Generation ngay trước file hiện tại là `app.log-20261018`, rồi tới `app.log-20261017.gz`

### Requirement: Rotated File là Generation cũ hơn
Hệ thống SHALL hiện dòng của Rotated File như các Generation phía trên file hiện tại trong cùng Source File, với Break "rotated" giữa hai Generation liền nhau. Hệ thống SHALL NOT hiện Rotated File như một Source File riêng ở sidebar hay Source Column. Số dòng SHALL bắt đầu từ 1 trong mỗi Generation, khớp với file trên Server.

#### Scenario: Hai Generation
- **WHEN** người dùng đọc `web1:/app/app.log` kèm `app.log.1`
- **THEN** Source Column của mọi dòng là `web1:/app/app.log`, dòng của `app.log.1` nằm phía trên, có Break "web1:/app/app.log rotated" giữa hai phần, và dòng đầu của mỗi phần có số dòng 1

#### Scenario: Marker trên Generation cũ
- **WHEN** người dùng đặt Marker trên dòng 120 của Generation đọc từ `app.log.1`
- **THEN** Marker được lưu với Generation cách file hiện tại 1 bước và dòng 120

### Requirement: Khi nào Rotated File được đọc
Khi Live đã nạp hết file hiện tại mà Live Window của file còn chỗ, hệ thống SHALL tự nạp tiếp từ cuối Rotated File mới nhất nếu file đó không nén, kèm Break "rotated" giữa hai Generation, cho đến khi đầy Live Window, chạm Starting Time hoặc hết file. Hệ thống SHALL NOT tự đọc Rotated File nào khác, ở cả Live và Open in full: Rotated File cũ hơn, và Rotated File mới nhất khi nó nén, chỉ được đọc khi người dùng chọn. Danh sách Rotated File SHALL được đọc lại mỗi lần cần đến, không theo dõi liên tục.

#### Scenario: Ngay sau logrotate
- **WHEN** `app.log` mới có 300 dòng, Live Window là 100.000, và thư mục có `app.log.1` (không nén) và `app.log.2.gz`
- **THEN** 99.700 dòng cuối của `app.log.1` tự được nạp phía trên với Break "rotated" giữa hai Generation, và hệ thống không đọc byte nào của `app.log.2.gz`

#### Scenario: Rotated File mới nhất bị nén
- **WHEN** `app.log` có 300 dòng và Rotated File mới nhất là `app.log-20261018.gz`
- **THEN** chỉ 300 dòng của `app.log` được nạp, và Break đầu file hiện "Load earlier generation"

#### Scenario: File hiện tại đã đủ Live Window
- **WHEN** `app.log` có 2.000.000 dòng và thư mục có `app.log.1`
- **THEN** hệ thống không đọc byte nào của `app.log.1`

### Requirement: Rotated File nén
Hệ thống SHALL đọc Rotated File nén gzip, zstd, xz và bzip2, nhận biết kiểu nén bằng các byte đầu của file chứ không theo đuôi tên, và SHALL giải nén nó trong lúc đọc mà không ghi bản nén ra đĩa. Rotated File có byte đầu của một kiểu nén khác SHALL hiện trong danh sách với nhãn "compression not supported" và SHALL NOT chọn được.

#### Scenario: Đọc file gzip
- **WHEN** người dùng chọn `app.log.2.gz`
- **THEN** các dòng đã giải nén hiện như một Generation, với số dòng đếm từ 1 trong lúc giải nén

#### Scenario: Đọc file zstd, xz, bzip2
- **WHEN** thư mục có `app.log.3.zst`, `app.log.4.xz` và `app.log.5.bz2`, và người dùng chọn cả ba
- **THEN** cả ba được giải nén và hiện như ba Generation cũ hơn, theo thứ tự thời gian sửa

#### Scenario: Kiểu nén không hỗ trợ
- **WHEN** thư mục có `app.log.6.lz4` mà byte đầu là của lz4
- **THEN** `app.log.6.lz4` hiện trong danh sách Rotated File với nhãn "compression not supported" và ô chọn bị tắt

#### Scenario: Đuôi tên sai
- **WHEN** `app.log.1` thực chất là file gzip
- **THEN** hệ thống giải nén nó như gzip, và không tự nạp nó khi Live vì nó nén

### Requirement: Load earlier generation khi Live
Khi một Remote File live đã nạp tới dòng đầu của Generation cũ nhất đang giữ và còn Rotated File cũ hơn chưa nạp, hệ thống SHALL hiện ở Break đầu phần dòng của file đó một nút "Load earlier generation" kèm tên và kích thước của Rotated File kế tiếp. Bấm nút SHALL nạp các dòng cuối của Rotated File đó cho đến khi đầy Live Window, chạm Starting Time hoặc hết file. Khi Live Window của file đã đầy, nút SHALL được thay bằng "Open in full…".

#### Scenario: Nạp Generation nén
- **WHEN** `app.log` và `app.log.1` đã được nạp hết, Live Window còn chỗ, và người dùng bấm "Load earlier generation (app.log.2.gz, 38 MB)"
- **THEN** status bar hiện tiến độ giải nén, các dòng cuối của `app.log.2.gz` vừa chỗ còn trống được nạp phía trên với Break "rotated", và vị trí người dùng đang đọc không bị nhảy

#### Scenario: Window đã đầy
- **WHEN** file đã giữ đủ 100.000 dòng và còn Rotated File chưa nạp
- **THEN** Break đầu file hiện "Open in full…" thay vì "Load earlier generation"
