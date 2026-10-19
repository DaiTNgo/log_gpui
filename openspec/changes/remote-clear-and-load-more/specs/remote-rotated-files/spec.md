## MODIFIED Requirements

### Requirement: Load earlier generation khi Live
Khi một Remote File live đã nạp tới dòng đầu của Generation cũ nhất đang giữ và còn Rotated File cũ hơn chưa nạp, hệ thống SHALL hiện ở Break đầu phần dòng của file đó một nút "Load earlier generation" kèm tên và kích thước của Rotated File kế tiếp. Hệ thống SHALL NOT tự đi sang Generation cũ hơn bằng Load more. Khi Live Window của file còn chỗ, bấm nút SHALL nạp các dòng cuối của Rotated File đó cho đến khi đầy Live Window, chạm Starting Time hoặc hết file. Khi Live Window đã đầy hoặc file có Extended Window, bấm nút SHALL nạp Load More Step dòng cuối của Rotated File đó và đưa file vào Extended Window. Khi file đã giữ đủ Full Read Limit, nút SHALL được thay bằng "Open in full…".

#### Scenario: Nạp Generation nén
- **WHEN** `app.log` và `app.log.1` đã được nạp hết, Live Window còn chỗ, và người dùng bấm "Load earlier generation (app.log.2.gz, 38 MB)"
- **THEN** status bar hiện tiến độ giải nén, các dòng cuối của `app.log.2.gz` vừa chỗ còn trống được nạp phía trên với Break "rotated", và vị trí người dùng đang đọc không bị nhảy

#### Scenario: Window đã đầy
- **WHEN** file đã giữ đủ 100.000 dòng tới dòng 1 của `app.log`, Load More Step là 10.000, và người dùng bấm "Load earlier generation (app.log.1, 12 MB)"
- **THEN** 10.000 dòng cuối của `app.log.1` được nạp phía trên với Break "rotated", và file có Extended Window

#### Scenario: Chạm Full Read Limit
- **WHEN** file đã giữ đủ Full Read Limit dòng và còn Rotated File chưa nạp
- **THEN** Break đầu file hiện "Open in full…" thay vì "Load earlier generation"
