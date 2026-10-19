## Why

Remote Log Space chỉ cho đọc **Live Window** (mặc định 100.000 dòng cuối) của file hiện tại. Muốn đọc cả file, đọc `app.log.1` hay `app.log.2.gz` sau logrotate, hay đọc "từ 1 giờ trước", người dùng phải Download về một thư mục, mở thành Imported Log Space, và tự giải nén file đã xoay vòng. Các trình đọc log từ xa khác làm được những việc này trong một bước, nên đây là chỗ app thua rõ nhất khi điều tra sự cố trên server.

## What Changes

- **Rotated File**: app tìm các file đã xoay vòng nằm cạnh một Remote File trên Server (`app.log.1`, `app.log.2.gz`, `app.log-20261018.gz`…) và coi chúng là các **Generation cũ hơn** của chính Remote File đó, không phải file riêng. Mỗi Rotated File là một Generation, xếp theo thời gian sửa, ngăn cách bằng Break "rotated". File nén gzip, zstd, xz hoặc bzip2 được giải nén ngay khi đọc; kiểu nén được nhận biết bằng byte đầu, không theo đuôi tên.
- **Open in full**: một lệnh trên Remote File (và trên Remote Log Space) đọc trọn file, kèm các Rotated File người dùng chọn, vào một **Remote Copy** trong Imported Log Space, không qua bước chọn thư mục Download rồi Import. Dòng hiện dần trong lúc đọc, có tiến độ và nút huỷ. Remote Copy là bản tĩnh: không theo file trên Server, không có Live Window; nội dung lưu trong thư mục dữ liệu của app nên mở lại Project không cần kết nối. "Read again" đọc lại từ Server.
- **Starting Time**: khi mở Remote File (live) hoặc Open in full, người dùng chọn đọc từ một thời điểm thay vì từ N dòng cuối: thời điểm tuyệt đối theo đồng hồ của file, hoặc khoảng như `1h`, `30m`, `2d` tính lùi từ **dòng mới nhất của file** (không dùng đồng hồ máy người dùng). Open in full còn có thêm mốc "To" tuỳ chọn.
  - Live: nạp ngược dừng ở dòng cũ hơn Starting Time hoặc khi đầy Live Window, tuỳ cái nào đến trước, kể cả khi phải đi tiếp vào Rotated File.
  - Open in full: tìm vị trí byte của Starting Time bằng tìm nhị phân trên file, đọc timestamp bằng parser sẵn có, rồi đọc xuôi. Rotated File nằm hẳn ngoài khoảng thời gian thì bị bỏ qua.
- **Full Read Limit**: giới hạn chung cho cả app về số dòng một lần Open in full được nạp (mặc định 2.000.000 dòng, chỉnh trong Settings). Trước khi đọc, app ước lượng số dòng. Nếu vượt giới hạn, app hỏi người dùng: thu hẹp Starting Time/To, đọc phần mới nhất vừa giới hạn, hoặc huỷ. App không bao giờ cắt dòng mà không báo.
- Remote File live: khi file hiện tại có ít dòng hơn Live Window (ví dụ ngay sau logrotate), app tự nạp tiếp từ cuối Rotated File mới nhất nếu file đó không nén (`app.log.1` hay tên theo ngày tương đương), kèm Break giữa hai Generation. Rotated File cũ hơn hoặc nén chỉ được nạp khi người dùng bấm "Load earlier generation" ở Break đầu file, vẫn trong Live Window. Khi Live Window đã đầy, nút này đề nghị "Open in full" thay thế.
- Download / Download all: thêm tuỳ chọn tải kèm Rotated File (giữ nguyên dạng nén). Mặc định vẫn chỉ tải file hiện tại.

## Domain changes

Change này không sửa CONTEXT.md hay ADR vì các session khác đang làm song song. Khi áp dụng change, cần cập nhật CONTEXT.md như sau.

**Thuật ngữ mới (nhóm Spaces / Live):**
- **Rotated File**: một file trên Server chứa một Generation cũ hơn của Remote File, nằm cùng thư mục và được nhận ra theo tên (`<name>.<N>`, `<name>-<YYYYMMDD>`, `<name>-<YYYY-MM-DD>`, có thể kèm đuôi nén). Rotated File không phải Remote File riêng: nó hiện như các Generation phía trên file hiện tại. _Avoid_: backup file, old log, archive log.
- **Remote Copy**: nội dung của một Remote File và các Rotated File đã chọn, đọc trọn một lần vào thư mục dữ liệu của app, rồi thêm vào Imported Log Space với tên `<server>:<path>`. Remote Copy là bản tĩnh và chỉ đọc, không theo file trên Server. _Avoid_: download, snapshot, cached file.
- **Starting Time**: thời điểm bắt đầu đọc một Remote File, thay cho các dòng cuối. Đó là một thời điểm theo đồng hồ của file, hoặc một khoảng tính lùi từ dòng mới nhất của file. _Avoid_: since, offset, time filter (Time Range là Filter, Starting Time quyết định dòng nào được đọc).
- **Full Read Limit**: số dòng tối đa một lần Open in full được nạp, một giá trị chung cho cả app. _Avoid_: max lines, cap.

**Thuật ngữ đổi nghĩa:**
- **Generation**: thêm câu "Các Generation cũ hơn của một Remote File cũng có thể đọc từ Rotated File của nó trên Server; Generation ngay trước file hiện tại là Rotated File mới nhất."
- **Imported Log Space**: "file được thả hoặc chọn khi tạo, Archive Entry, hoặc Remote Copy".
- **Log Space**: không đổi. Không thêm loại Log Space thứ tư; Remote Copy nằm trong Imported Log Space.

**Quy tắc đổi:**
- **L4 Remote Opens At The End** → "Một Remote File mở ra với các dòng cuối hiện ngay, hoặc từ Starting Time nếu người dùng chọn, rồi nạp các dòng cũ hơn ở nền cho đến khi đầy Live Window, chạm Starting Time, hoặc hết file. Khi còn chỗ, app tự nạp tiếp vào Rotated File mới nhất nếu nó không nén (L24); Rotated File khác chỉ được nạp khi người dùng chọn. Mọi lần nạp vẫn trong Live Window. Không nạp dòng vượt quá Live Window. Số dòng luôn là số dòng thật của dòng trong Generation của nó trên Server."
- **L10 Live Is A Window, Not An Archive** → giữ nguyên câu đầu; câu cuối đổi thành "Để đọc trọn một Remote File, người dùng dùng Open in full để có một Remote Copy, hoặc Download để giữ file trong thư mục của mình."
- **L14 Download For Full Reading** đổi tên thành **Download To Keep**: "Một Remote File, hoặc mọi file của một Remote Log Space, có thể được tải về thư mục chọn rồi mở thành Imported Log Space mới. Mặc định chỉ tải file hiện tại; người dùng có thể chọn tải kèm Rotated File, giữ nguyên dạng nén."
- **L3 Live Starts Fresh**: thêm "Imported Log Space được tạo bởi Open in full mở với Follow tắt và view ở dòng đầu tiên được đọc."
- **24 Space Kinds Never Mix**: "Imported Log Space chỉ nhận file và Archive Entry qua import, và Remote Copy qua Open in full."
- **25 Imported And Remote Files Are Read-Only**: thêm "Remote Copy là bản tĩnh: nó không theo Remote File khi file đổi, cho đến khi người dùng chọn Read again. Gỡ Remote Copy khỏi Log Space sẽ xoá nội dung của nó khỏi thư mục dữ liệu của app."

**Quy tắc mới:**
- **L24 Rotated Files Are Earlier Generations**: Rotated File của một Remote File được hiện như các Generation phía trên file hiện tại, không bao giờ là Source File riêng. Thứ tự là theo thời gian sửa, mới nhất ở gần file hiện tại. Khi Live còn chỗ trong Live Window sau khi đã nạp hết file hiện tại, app tự nạp tiếp từ cuối Rotated File mới nhất nếu nó không nén. Rotated File cũ hơn hoặc nén chỉ được đọc khi người dùng chọn.
- **L25 Starting Time Uses The File's Clock**: Starting Time được so với timestamp như ghi trong file. Một khoảng như `1h` được tính lùi từ dòng mới nhất của file, không phải từ đồng hồ máy người dùng hay đồng hồ của Server (L8).
- **L26 Full Read Never Cuts Silently**: Open in full không nạp quá Full Read Limit. Khi ước lượng thấy vượt giới hạn, app hỏi trước khi đọc. Nếu khi đọc thật vẫn chạm giới hạn, app dừng lại và vẽ Break "stopped at Full Read Limit".
- L27 để trống, dành cho change này.

Số quy tắc L24–L27 thuộc change này; L16–L19, L20–L23 và L28–L31 thuộc các change `ssh-connect-ergonomics`, `remote-file-picker` và `remote-command-source`.

**ADR:** nên thêm ADR 0011 "Full Reading Goes Through A Remote Copy, Not A Fourth Space Kind" (xem design.md, quyết định 1) khi áp dụng change.

## Capabilities

### New Capabilities
- `remote-rotated-files`: tìm Rotated File cạnh Remote File, hiện chúng như Generation cũ hơn, giải nén gzip/zstd/xz/bzip2, tự nạp Rotated File mới nhất không nén khi Live còn chỗ, "Load earlier generation" cho các Rotated File khác.
- `remote-full-read`: Open in full tạo Remote Copy trong Imported Log Space; tiến độ, huỷ, Read again; lưu trong thư mục dữ liệu app; Full Read Limit và ước lượng trước khi đọc.
- `remote-starting-time`: chọn Starting Time (tuyệt đối hoặc tương đối với dòng mới nhất) khi mở live và khi Open in full; tìm vị trí theo timestamp; số dòng thật khi bắt đầu giữa file.

### Modified Capabilities
- `live-log`: "Mở Remote File và nạp dần" thêm Starting Time, tự nạp Rotated File mới nhất không nén, và nạp Rotated File khác khi người dùng chọn (vẫn trong Live Window); "Trạng thái live khi mở" thêm ngoại lệ cho Log Space tạo bởi Open in full.
- `remote-log-space`: "Download Remote File" thêm tuỳ chọn tải kèm Rotated File.
- `log-space`: "Imported và Scratch không trộn lẫn" thêm Remote Copy là cách thứ ba để Imported Log Space nhận file; "File Imported chỉ đọc" thêm việc gỡ Remote Copy thì xoá nội dung đã lưu.

Lưu ý: `live-log` và `remote-log-space` hiện mới có trong change `add-remote-live-log`, chưa được archive vào `openspec/specs/`. Change này cần được archive sau `add-remote-live-log`.

## Impact

- Code: `src/live/remote.rs` (Starting Time, nạp tiếp vào Rotated File), `src/live/backward.rs` (dừng theo timestamp), module mới `src/remote/rotated.rs` (tìm và xếp Rotated File), `src/remote/full_read.rs` (Open in full, tìm nhị phân theo timestamp, ước lượng, ghi Remote Copy), `src/remote/download.rs` (tải kèm Rotated File), `src/workspace/` (loại file Remote Copy trong Imported Log Space, persist), `src/app/` (dialog Open in full, Break có nút, status bar tiến độ, Settings Full Read Limit), `src/parser/text.rs` (dùng lại `read_timestamp`, không đổi).
- Dependency mới, đều thuần Rust: `ruzstd` (zstd), `lzma-rs` (xz), `bzip2` với backend `libbz2-rs-sys` (bzip2). `flate2` (đã có cho Archive) dùng cho gzip. Xem design.md, quyết định 3.
- Dữ liệu lưu: thư mục `remote-copies/` trong thư mục dữ liệu app, mỗi Remote Copy một thư mục gồm một file text cho mỗi Generation và một `meta.json`. Session lưu thêm loại file Remote Copy; session cũ không cần migrate.
- Phối hợp với change `remote-file-picker`: picker có thể nhóm Rotated File dưới Remote File và có lối vào "Open in full", nhưng quy tắc nhận biết Rotated File là của change này.
