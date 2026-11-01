## ADDED Requirements

### Requirement: Time Range từ Time Histogram
Time Range SHALL đặt, đổi và xoá được từ Time Histogram (xem `time-histogram`), và kết quả SHALL giống hệt nhập Từ và Đến. Hai ô Từ và Đến SHALL hiện giá trị do Time Histogram đặt: chỉ giờ (`HH:MM:SS`, thêm phần lẻ giây khi cột nhỏ hơn 1 giây) khi khoảng nằm trong một ngày, và ngày giờ đầy đủ khi khoảng trải qua nhiều ngày. Sửa hai ô SHALL cập nhật dải tô nền trên Time Histogram.

#### Scenario: Kéo chọn điền hai ô
- **WHEN** người dùng kéo chọn `10:30:00`–`10:35:00` của cùng một ngày trên Time Histogram
- **THEN** ô Từ hiện `10:30:00` và ô Đến hiện `10:35:00`

#### Scenario: Khoảng qua nhiều ngày
- **WHEN** người dùng kéo chọn từ `2026-10-28 23:50:00` tới `2026-10-29 00:10:00`
- **THEN** hai ô hiện ngày giờ đầy đủ

#### Scenario: Gõ vào ô thì dải đổi theo
- **WHEN** Time Range là `10:30:00`–`10:35:00` và người dùng sửa ô Đến thành `10:40:00`
- **THEN** dải tô nền trên Time Histogram trải tới `10:40:00`
