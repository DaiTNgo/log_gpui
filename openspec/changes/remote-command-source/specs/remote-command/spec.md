## ADDED Requirements

### Requirement: Thêm Remote Command
Hệ thống SHALL cho thêm Remote Command vào Remote Log Space, khi tạo Log Space hoặc qua "Add remote command…", bằng cách chọn một Server và gõ lệnh hoặc chọn một Command Preset. Trước khi thêm, hệ thống SHALL hiện lệnh đầy đủ, tên Server và user sẽ chạy lệnh, và chỉ thêm khi người dùng bấm Add. Một Remote Log Space SHALL NOT có hai Remote Command cùng Server và cùng lệnh.

#### Scenario: Gõ lệnh
- **WHEN** người dùng chọn Server "web1" (user `deploy`), gõ `journalctl -f -u nginx` và bấm Add
- **THEN** Remote Command `web1$ journalctl -f -u nginx` được thêm và các dòng output của nó bắt đầu hiện

#### Scenario: Xác nhận trước khi chạy
- **WHEN** người dùng gõ lệnh nhưng chưa bấm Add
- **THEN** dialog hiện "Runs on web1 as deploy each time this Log Space opens" và chưa có lệnh nào chạy trên Server

#### Scenario: Thêm trùng
- **WHEN** Log Space đã có `web1$ journalctl -f -u nginx` và người dùng thêm lại đúng lệnh đó trên web1
- **THEN** danh sách thành viên không đổi và hiện thông báo lệnh đã có

### Requirement: Tên Remote Command
Hệ thống SHALL gọi một Remote Command bằng `<server>$ <lệnh>` ở sidebar, Source Column, File Section, File View và Focus Pane.

#### Scenario: Gộp với Remote File
- **WHEN** Log Space có `web1:/app/app.log` và `web1$ docker logs -f --timestamps --tail 1000 api`
- **THEN** dòng của cả hai xếp xen theo timestamp, và Source Column hiện `web1$ docker logs -f --timestamps --tail 1000 api` cho dòng của lệnh

### Requirement: Command Preset
Hệ thống SHALL có Command Preset cho journalctl (unit), Docker (container) và Kubernetes (pod, namespace, container, context). Mỗi preset SHALL sinh một lệnh theo dõi output, có timestamp trên mỗi dòng và một số dòng ban đầu (mặc định 1.000), với mọi giá trị được shell-quote. Lệnh sinh ra SHALL hiện trong một ô sửa được trước khi thêm, và lệnh được lưu là chuỗi trong ô đó. Nếu người dùng sửa tay ô lệnh, Remote Command SHALL được coi là lệnh gõ tay và dialog SHALL báo rằng nó sẽ không được Resume By Time.

#### Scenario: Preset Docker
- **WHEN** người dùng chọn preset Docker và container `api`
- **THEN** ô lệnh hiện `docker logs -f --timestamps --tail 1000 'api'`

#### Scenario: Sửa lệnh của preset
- **WHEN** người dùng chọn preset journalctl với unit `nginx` rồi sửa `-n 1000` thành `-n 50` trong ô lệnh
- **THEN** dialog báo "Edited: will rerun as typed after a lost connection", và Remote Command được thêm chạy đúng lệnh đã sửa

### Requirement: Gợi ý giá trị cho preset
Khi mở form của một Command Preset, hệ thống SHALL chạy một lệnh liệt kê cố định, chỉ đọc, trên Server đã chọn để gợi ý unit, container hoặc pod. Nếu lệnh liệt kê lỗi, hệ thống SHALL hiện lỗi dưới trường và vẫn cho gõ tay giá trị.

#### Scenario: Liệt kê container
- **WHEN** người dùng chọn preset Docker trên "web1" có container `api` và `worker` đang chạy
- **THEN** trường container gợi ý `api` và `worker`

#### Scenario: Không có quyền với Docker
- **WHEN** user của Server không truy cập được Docker socket
- **THEN** trường container hiện lỗi "permission denied" của lệnh liệt kê và người dùng vẫn gõ được tên container

### Requirement: Remote Command không nâng quyền
Hệ thống SHALL chạy Remote Command bằng user của Server qua SSH, không cấp terminal, và đóng input của lệnh ngay khi bắt đầu. Hệ thống SHALL từ chối lệnh mà một đoạn lệnh (tách bởi `|`, `;`, `&&`, `||`) bắt đầu bằng `sudo`, `su`, `doas`, `pkexec` hoặc `run0`, kể cả sau gán biến hay `env` và kể cả `sudo -n`. Hệ thống SHALL NOT thêm, bớt hay đổi chữ nào trong lệnh của người dùng, trừ việc thay cờ số dòng ban đầu bằng cờ since khi Resume By Time một Command Preset chưa sửa.

#### Scenario: Lệnh có sudo
- **WHEN** người dùng gõ `sudo journalctl -f`
- **THEN** nút Add bị khoá và dialog báo "Commands run as deploy; elevation is not allowed"

#### Scenario: sudo không hỏi mật khẩu
- **WHEN** người dùng gõ `sudo -n journalctl -f` cho Server có sudoers `NOPASSWD`
- **THEN** nút Add vẫn bị khoá với cùng thông báo

#### Scenario: Lệnh chờ nhập
- **WHEN** Remote Command là một lệnh đọc từ input
- **THEN** lệnh nhận hết input ngay, không có lời nhắc nào hiện ra, và output của nó vẫn được đọc

### Requirement: Output thành dòng
Hệ thống SHALL đọc cả stdout và stderr của Remote Command thành dòng, theo thứ tự đến, không để dòng của luồng này cắt ngang dòng của luồng kia. Dòng chưa có ký tự xuống dòng SHALL chỉ hiện khi có ký tự xuống dòng hoặc khi Run kết thúc.

#### Scenario: Lỗi ra stderr
- **WHEN** container ghi `panic: nil map` ra stderr và `GET /health 200` ra stdout
- **THEN** cả hai dòng hiện trong view, mỗi dòng nguyên vẹn

#### Scenario: Dòng cuối không xuống dòng
- **WHEN** lệnh in `done` không có ký tự xuống dòng rồi thoát
- **THEN** dòng `done` hiện khi lệnh thoát

### Requirement: Run và số dòng
Mỗi lần chạy một Remote Command SHALL là một Run. Run chạy mới SHALL bắt đầu một Generation, với số dòng là thứ tự của dòng trong output, bắt đầu từ 1. Run được Resume By Time sau mất kết nối SHALL tiếp tục Generation và số dòng của Run trước. Remote Command SHALL NOT nạp dòng cũ hơn những gì lệnh in ra.

#### Scenario: Số dòng của Run đầu
- **WHEN** `web1$ journalctl -f -n 1000 -u nginx` chạy và in 1.000 dòng rồi thêm 5 dòng
- **THEN** Gutter hiện số 1–1.005 cho các dòng đó

#### Scenario: Run mới
- **WHEN** người dùng Restart Remote Command
- **THEN** một Break xuất hiện và dòng đầu tiên của Run mới có số dòng 1

### Requirement: Lệnh thoát
Khi lệnh của Remote Command thoát, hệ thống SHALL giữ các dòng đã nhận, vẽ Break nêu mã thoát, hiện trạng thái "Exited (code N)" và nút Restart. Hệ thống SHALL NOT tự chạy lại lệnh trừ khi Restart on exit đang bật. Các thành viên khác của Log Space SHALL tiếp tục live.

#### Scenario: Container dừng
- **WHEN** `docker logs -f api` thoát với mã 0 vì container dừng
- **THEN** các dòng vẫn hiện, có Break "web1$ docker logs -f api exited (0)", và trạng thái nguồn đó là "Exited (0)"

#### Scenario: Lệnh không tồn tại
- **WHEN** Server không có `kubectl` và lệnh thoát với mã 127
- **THEN** dòng lỗi của shell hiện trong view và trạng thái nguồn đó là "Exited (127)"

### Requirement: Restart
Hệ thống SHALL cho người dùng Restart một Remote Command. Restart SHALL kết thúc Run đang chạy trên Server nếu có, vẽ Break "restarted", và chạy lệnh thành Run mới.

#### Scenario: Restart khi đang chạy
- **WHEN** `web1$ journalctl -f -u nginx` đang chạy và người dùng bấm Restart
- **THEN** tiến trình cũ trên web1 được yêu cầu dừng, Break "restarted" xuất hiện, và Run mới bắt đầu từ dòng 1

### Requirement: Restart on exit
Mỗi Remote Command SHALL có tuỳ chọn Restart on exit, tắt mặc định. Khi bật, hệ thống SHALL chạy lại lệnh sau khi nó thoát, bằng Resume By Time nếu là Command Preset chưa sửa, chờ 1s, 2s, 4s… tối đa 30s giữa các lần, và quay về 1s khi một Run chạy quá 60s. Nếu 5 Run liên tiếp thoát với mã khác 0 trong vòng 10 giây mỗi Run, hệ thống SHALL ngừng chạy lại và chờ người dùng bấm Restart.

#### Scenario: Pod khởi động lại
- **WHEN** Restart on exit bật và `kubectl logs -f api-7d9` thoát vì container khởi động lại
- **THEN** sau 1 giây lệnh chạy lại từ giờ của dòng cuối đã nhận, có Break "restarted", các dòng đã có không hiện lại, và dòng của container mới tiếp tục hiện

#### Scenario: Lệnh lỗi liên tục
- **WHEN** Restart on exit bật và lệnh thoát mã 1 ngay sau mỗi lần chạy, 5 lần liên tiếp
- **THEN** hệ thống ngừng chạy lại và hiện "Exited (1)" với nút Restart

### Requirement: Nối lại Remote Command
Khi mất kết nối tới Server, hệ thống SHALL nối lại như với Remote File, rồi chạy lại lệnh dù Restart on exit đang tắt. Command Preset chưa sửa SHALL được Resume By Time. Lệnh gõ tay, preset đã sửa, hoặc lệnh chưa nhận dòng nào có timestamp SHALL được chạy lại nguyên văn thành Run mới, với Break nói rằng lệnh đã chạy lại và dòng trong lúc mất kết nối có thể thiếu hoặc lặp.

#### Scenario: Preset qua mạng chập chờn
- **WHEN** mạng tới web1 mất 20 giây trong lúc preset `web1$ journalctl --no-pager -o short-iso-precise -f -n 1000 -u 'nginx'` đang chạy ở dòng 1.200, và nginx ghi 300 dòng trong lúc đó
- **THEN** có Break "disconnected 10:02 – reconnected 10:03, resumed from 10:02:03", 300 dòng hiện với số dòng 1.201–1.500, và không dòng nào hiện hai lần

#### Scenario: Lệnh gõ tay qua mạng chập chờn
- **WHEN** mạng tới web1 mất 20 giây rồi có lại trong lúc lệnh gõ tay `web1$ tail -F /tmp/x.log` đang chạy
- **THEN** có Break "web1$ tail -F /tmp/x.log disconnected 10:02 – reconnected 10:03, command restarted, lines may be missing or repeated" và Run mới bắt đầu từ dòng 1

### Requirement: Resume By Time
Khi chạy lại một Command Preset chưa sửa bằng Resume By Time, hệ thống SHALL thay cờ số dòng ban đầu bằng cờ since của công cụ (`--since` cho journalctl và Docker, `--since-time` cho Kubernetes) với giờ là timestamp của dòng có timestamp cuối cùng đã nhận, đúng như Server đã ghi kèm offset của nó. Hệ thống SHALL NOT dùng đồng hồ máy local hay của Server khác để tính giờ này. Hệ thống SHALL bỏ các dòng của Run mới cũ hơn mốc đó và các dòng ở mốc đó đã nhận rồi, so theo văn bản và theo số lần xuất hiện, rồi giữ mọi dòng từ dòng đầu tiên mới hơn mốc hoặc chưa nhận. Connection Details SHALL hiện lệnh của Run hiện tại.

#### Scenario: Trùng ở ranh giới giây
- **WHEN** dòng cuối đã nhận có timestamp `2026-10-19T10:02:03.400000+07:00`, journalctl được chạy lại với `--since` theo giây và in lại hai dòng của giây 10:02:03 đã có cùng một dòng mới của giây đó
- **THEN** hai dòng đã có bị bỏ và chỉ dòng mới được thêm

#### Scenario: Hai dòng giống hệt nhau
- **WHEN** đã nhận một dòng `GET /health 200` ở giây 10:02:03 và Run mới in hai dòng `GET /health 200` của giây đó
- **THEN** một dòng bị bỏ và một dòng được thêm

#### Scenario: Lệnh resume hiện trong Connection Details
- **WHEN** preset Docker `docker logs -f --timestamps --tail 1000 'api'` được Resume By Time từ `2026-10-19T03:02:03.123456789Z`
- **THEN** Connection Details hiện lệnh `docker logs -f --timestamps --since 2026-10-19T03:02:03.123456789Z 'api'` cho Run hiện tại

#### Scenario: Công cụ không nhận cờ since
- **WHEN** Run resume thoát ngay vì `kubectl` trên Server không nhận `--since-time`
- **THEN** hệ thống chạy lại lệnh nguyên văn một lần với Break "lines may be missing or repeated", ghi "resume not supported" vào lỗi cuối của nguồn, và không thử resume lại cho Remote Command đó đến khi mở lại

### Requirement: Pause với Remote Command
Khi Log Space Pause, lệnh của Remote Command SHALL tiếp tục chạy và hệ thống SHALL giữ các dòng mới ở nền, tối đa bằng Live Window. Khi tiếp tục, các dòng đã giữ SHALL được nạp. Nếu số dòng đến trong lúc Pause vượt Live Window, các dòng cũ nhất SHALL bị bỏ như khi rời Live Window.

#### Scenario: Pause ngắn
- **WHEN** người dùng Pause, lệnh in 300 dòng, rồi người dùng tiếp tục
- **THEN** trong lúc Pause view không đổi, và sau khi tiếp tục cả 300 dòng hiện với số dòng liên tục

#### Scenario: Pause lâu
- **WHEN** Live Window là 100.000 và lệnh in 150.000 dòng trong lúc Pause
- **THEN** sau khi tiếp tục, 100.000 dòng mới nhất được nạp và Break nói dòng cũ hơn đã rời window

### Requirement: Live Window cho Remote Command
Hệ thống SHALL áp Live Window cho từng Remote Command riêng, tính trên mọi dòng của nó qua các Run, giống Remote File.

#### Scenario: Lệnh ồn không đẩy file yên
- **WHEN** `web1$ kubectl logs -f api` ghi 1.000 dòng/giây và `web1:/app/cron.log` ghi 1 dòng/phút
- **THEN** dòng của `cron.log` không rời view vì lệnh ồn

### Requirement: Gỡ Remote Command
Gỡ một Remote Command khỏi Log Space, đóng Log Space hoặc thoát ứng dụng SHALL yêu cầu tiến trình của Run đang chạy dừng trên Server rồi đóng kênh. Gỡ SHALL bỏ các dòng và Marker của Remote Command khỏi Log Space.

#### Scenario: Gỡ lệnh
- **WHEN** người dùng gỡ `web1$ journalctl -f -u nginx`
- **THEN** các dòng của lệnh biến mất khỏi view và tiến trình journalctl đó trên web1 được yêu cầu dừng

### Requirement: Mở lại Project với Remote Command
Khi mở lại Project, hệ thống SHALL chạy lại mọi Remote Command của Remote Log Space được mở, như một Run mới, với lệnh và tuỳ chọn Restart on exit đã lưu. Marker trên dòng của Remote Command SHALL NOT được khôi phục.

#### Scenario: Khởi động lại app
- **WHEN** người dùng thoát app khi "Checkout" có `web1$ journalctl -f -n 1000 -u nginx` rồi mở lại
- **THEN** lệnh chạy lại trên web1, hiện 1.000 dòng cuối hiện tại, và các Marker cũ trên dòng của lệnh không còn
