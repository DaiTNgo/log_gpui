## ADDED Requirements

### Requirement: Load more
Hệ thống SHALL cho người dùng Load more trên từng Remote File, từ nút trên Break đầu phần dòng của file, menu của file, Command Palette và phím tắt `Cmd+Shift+↑` cho file của dòng đang chọn. Remote Command SHALL NOT có Load more. Mỗi lần Load more SHALL đọc từ Server tối đa số dòng đã chọn (mặc định Load More Step) ngay trước dòng cũ nhất file đang giữ, trong cùng Generation, và chèn chúng phía trên mà vị trí người dùng đang đọc SHALL NOT bị nhảy. Số dòng SHALL là số dòng thật trong Generation của dòng trên Server. Load More Step SHALL là một setting chung cho cả app, mặc định 10.000, chỉnh trong Settings. Imported Log Space và Scratch Log Space SHALL NOT có Load more.

#### Scenario: Nạp thêm một phần
- **WHEN** `web1:/app/app.log` giữ dòng 1.900.001–2.000.000 và người dùng bấm "Load more (10,000)"
- **THEN** dòng 1.890.001–1.900.000 được đọc từ Server và hiện phía trên, và dòng người dùng đang đọc vẫn ở nguyên chỗ trên màn hình

#### Scenario: Bấm nhiều lần
- **WHEN** người dùng bấm Load more ba lần với Load More Step 10.000
- **THEN** file giữ thêm 30.000 dòng liền nhau phía trên các dòng đã có

#### Scenario: Vượt qua Starting Time
- **WHEN** file được mở với Starting Time `30m` và việc nạp dần dừng ở dòng cũ nhất trong 30 phút đó
- **THEN** Break đầu file có nút Load more, và bấm nó nạp các dòng cũ hơn Starting Time

#### Scenario: Khi mất kết nối
- **WHEN** Server của file mất kết nối
- **THEN** nút Load more bị tắt, với tooltip "Disconnected"

#### Scenario: Huỷ khi đang đọc
- **WHEN** Load more đang đọc và người dùng bấm Cancel
- **THEN** các dòng đã nhận được giữ lại, và nút Load more hiện lại

### Requirement: Chọn số dòng cho Load more
Nút Load more SHALL nạp Load More Step dòng khi bấm phần chính, và SHALL có một menu cho chọn số dòng của riêng lần đó: 1.000, 10.000, 100.000, hoặc "Custom…" để nhập một số từ 1 tới Full Read Limit. Lựa chọn trong menu SHALL NOT đổi setting Load More Step. Menu của Remote File SHALL có cùng các lựa chọn. Phím tắt SHALL nạp Load More Step dòng.

#### Scenario: Chọn 100.000 dòng
- **WHEN** Load More Step là 10.000 và người dùng chọn "100,000" trong menu của nút Load more
- **THEN** 100.000 dòng cũ hơn được nạp, và lần bấm phần chính sau đó vẫn nạp 10.000 dòng

#### Scenario: Nhập số tuỳ chọn
- **WHEN** người dùng chọn "Custom…" và nhập 2.500
- **THEN** 2.500 dòng cũ hơn được nạp

#### Scenario: Đổi mặc định trong Settings
- **WHEN** người dùng đặt Load More Step là 1.000 trong Settings
- **THEN** phần chính của nút ghi "Load more (1,000)" và nạp 1.000 dòng

### Requirement: Extended Window
Sau một lần Load more, file SHALL có Extended Window: hệ thống SHALL giữ mọi dòng của file từ dòng cũ nhất đã nạp trở về sau, và dòng mới SHALL NOT đẩy dòng cũ ra, cho đến khi file giữ quá Full Read Limit. Khi đó dòng cũ nhất SHALL rời như dòng rời Live Window. Extended Window SHALL là của riêng từng Remote File. Đổi Live Window trong Settings SHALL NOT cắt file đang có Extended Window.

#### Scenario: Dòng mới không đẩy dòng đã nạp ra
- **WHEN** file giữ 100.000 dòng, người dùng Load more 10.000 dòng, rồi server ghi thêm 50.000 dòng
- **THEN** file giữ 160.000 dòng, gồm cả 10.000 dòng đã nạp bằng Load more

#### Scenario: Chạm Full Read Limit
- **WHEN** file có Extended Window đang giữ đúng Full Read Limit dòng và có thêm dòng mới
- **THEN** các dòng cũ nhất rời view như rời Live Window, kèm Break "left the window"

#### Scenario: File khác không bị ảnh hưởng
- **WHEN** `web1:/app/app.log` có Extended Window và `web2:/app/app.log` thì không
- **THEN** `web2:/app/app.log` vẫn chỉ giữ Live Window dòng mới nhất

### Requirement: Back to Live Window
Hệ thống SHALL cho người dùng chọn "Back to Live Window" trên một file có Extended Window, từ Break đầu file và menu của file. Việc này SHALL cắt ngay file xuống Live Window dòng mới nhất, giải phóng bộ nhớ của các dòng bị bỏ, và kết thúc Extended Window. Clear SHALL cũng kết thúc Extended Window.

#### Scenario: Trở lại Live Window
- **WHEN** file có Extended Window giữ 300.000 dòng, Live Window là 100.000, và người dùng chọn "Back to Live Window"
- **THEN** file chỉ còn 100.000 dòng mới nhất, Break "left the window" ở đầu, và dòng mới lại đẩy dòng cũ nhất ra như trước

### Requirement: Load more sau Clear
Khi file đã Clear, Load more SHALL đọc các dòng ngay trước Clear Point, tức là đọc lại từ Server chính các dòng đã Clear, mới nhất trước, rồi đi tiếp về trước. Các dòng này SHALL NOT có lại Marker cũ. Break "cleared" SHALL vẫn đứng ở chỗ đã Clear.

#### Scenario: Đưa lại dòng đã Clear
- **WHEN** người dùng Clear file lúc nó giữ dòng 1.900.001–2.000.000, server ghi thêm 20 dòng, rồi người dùng bấm Load more với Load More Step 10.000
- **THEN** dòng 1.990.001–2.000.000 hiện phía trên Break "cleared", và 20 dòng mới vẫn ở dưới Break

### Requirement: Load more dừng ở đầu Generation
Load more SHALL dừng ở dòng 1 của Generation đang đọc. Khi file đã giữ tới dòng 1 của Generation cũ nhất mà còn Rotated File cũ hơn, Break đầu file SHALL hiện "Load earlier generation" thay cho Load more, kể cả khi Rotated File đó không nén. Sau khi nạp một Generation cũ hơn, Load more SHALL đọc tiếp trong Generation đó. Khi dòng cũ nhất đang giữ thuộc một Generation cũ mà phần trước nó không còn trên Server, Load more SHALL dừng với Break "earlier lines of this generation are no longer on the Server".

#### Scenario: Chạm đầu file hiện tại
- **WHEN** Load more đọc tới dòng 1 của `app.log`, và thư mục có `app.log.1` không nén
- **THEN** hệ thống không đọc byte nào của `app.log.1`, và Break đầu file hiện "Load earlier generation (app.log.1, 12 MB)"

#### Scenario: Generation đã xoay vòng trong lúc mở
- **WHEN** dòng cũ nhất đang giữ thuộc Generation trước một lần xoay vòng, và Generation đó giờ là `app.log.1` trên Server
- **THEN** Load more đọc các dòng trước đó từ `app.log.1`

#### Scenario: Generation cũ đã bị nén
- **WHEN** dòng cũ nhất đang giữ thuộc Generation giờ là `app.log.2.gz`
- **THEN** nút ghi "Load more (re-reads 38 MB)", và bấm nó giải nén file từ đầu để lấy Load More Step dòng ngay trước dòng cũ nhất đang giữ

### Requirement: Giới hạn bằng Full Read Limit
Một Remote File SHALL NOT giữ quá Full Read Limit dòng. Một lần Load more SHALL chỉ đọc phần còn lại tới Full Read Limit. Khi file đã giữ đủ Full Read Limit, nút Load more và Load earlier generation SHALL được thay bằng "Open in full…".

#### Scenario: Gần Full Read Limit
- **WHEN** Full Read Limit là 2.000.000, file giữ 1.995.000 dòng, và người dùng bấm Load more với Load More Step 10.000
- **THEN** chỉ 5.000 dòng được nạp, và Break đầu file hiện "Open in full…"

### Requirement: Hiện số dòng đang giữ
Tooltip của Remote File ở sidebar và header File Section SHALL hiện số dòng file đang giữ, bộ nhớ ước lượng, và việc file có Extended Window hay không.

#### Scenario: Tooltip
- **WHEN** người dùng rê chuột lên `web1:/app/app.log` đang giữ 120.000 dòng sau Load more
- **THEN** tooltip ghi "120,000 lines held (≈160 MB) · Extended"

### Requirement: Extended Window không được nhớ
Hệ thống SHALL NOT nhớ Extended Window hay các dòng đã Load more khi đóng hoặc mở lại Log Space hay Project.

#### Scenario: Mở lại sau Load more
- **WHEN** người dùng Load more trên một file, đóng Project rồi mở lại
- **THEN** file mở với Live Window như lần đầu
