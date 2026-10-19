## ADDED Requirements

### Requirement: Break của Remote Command
Ngoài các lý do sẵn có, hệ thống SHALL vẽ Break cho Remote Command khi lệnh thoát (nêu mã thoát), khi lệnh được Restart, và khi lệnh chạy lại sau mất kết nối. Mỗi Break trước một Run chạy mới SHALL bắt đầu một Generation mới; Break trước một Run được Resume By Time sau mất kết nối SHALL NOT bắt đầu Generation mới. Break của Remote Command tuân theo mọi quy tắc của Break: không phải row, không là Match, không chịu Filter, không được copy.

#### Scenario: Thoát rồi Restart
- **WHEN** `web1$ docker logs -f api` thoát mã 0 và người dùng bấm Restart
- **THEN** sau dòng cuối của Run cũ có Break "exited (0)", tiếp theo là Break "restarted", rồi dòng 1 của Run mới

### Requirement: Trạng thái live của Remote Command
Ngoài các trạng thái sẵn có, trạng thái kết nối trên status bar SHALL có "Exited" cho Remote Command đã thoát, xếp xấu hơn "Waiting for file" và tốt hơn "Reconnecting". Follow và Pause SHALL áp dụng cho Remote Command như cho Remote File.

#### Scenario: Lệnh thoát trong Log Space hai thành viên
- **WHEN** Log Space có một Remote File đang live và một Remote Command đã thoát
- **THEN** status bar hiện "Exited (1 of 2)" và Follow, Pause vẫn hoạt động cho cả Log Space
