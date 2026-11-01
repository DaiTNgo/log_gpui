## Why

Time Histogram cho biết chuyện xảy ra lúc nào, nhưng không cho biết ở đâu: service nào lỗi trước, service nào im khi service khác bùng lên. Log Space trộn mọi file vào một timeline, và nhiều file lại gom nhiều service vào một chỗ. Vì vậy trên list rất khó thấy quan hệ trước–sau giữa các nguồn. Ngoài ra, file bị cắt khúc (`message.0`, `message.1`) hiện được coi là các nguồn khác nhau dù chỉ là một luồng log.

## What Changes

**Log Name Column**
- Cột suy ra mới trong Parsed View của Log Space, đứng cạnh Source Column. Cột này ghi Log Name của Source File: tên file sau khi bỏ hậu tố xoay vòng (`.<N>`, `-<YYYYMMDD>`, `-<YYYY-MM-DD>`) và hậu tố nén (`.gz`, `.bz2`, `.xz`, `.zst`, `.Z`), theo đúng quy tắc nhận diện Rotated File. Phần trước tên file (`<server>:`, `<archive> › `, thư mục) được giữ nguyên.
- Log Name Column là Filterable Column và ghim được. Mặc định cột này ẩn, có công tắc trong mục Columns.

**Swimlane**
- Phần làn nằm ngay dưới Time Histogram, trong cùng dải và dùng cùng `TimeAxis` (cùng zoom, cùng khung vùng đang xem, cùng dải Time Range). Mỗi làn là một giá trị của cột đã chọn; mỗi vạch là các Parent Line của giá trị đó trong một cột thời gian, tô theo level.
- Menu "Lane by" ở đầu dải chọn bất kỳ Filterable Column nào của view, kể cả Source Column và Log Name Column. "Không chia" thì ẩn các làn.
- Mặc định: Log Space có từ hai Log Name thì chia theo Log Name Column. Chỉ có một Log Name thì không chia; khi view có cột MODULE hoặc có Field Column tên `service`, `module`, `component`, `thread`, `host`, `pod`, menu gợi ý cột đó nhưng không tự bật.
- Hiện tối đa 8 làn có nhiều dòng nhất; các giá trị còn lại gom vào làn "Khác (n giá trị)". Cột vượt giới hạn giá trị riêng biệt của Filter thì không chọn được, và menu ghi rõ lý do.

**Ngưỡng giá trị riêng biệt**
- Giới hạn giá trị riêng biệt của Filterable Column (`MAX_DISTINCT`) tăng từ 500 lên 1000. Ngưỡng này dùng chung cho Filter và Lane By, để cột như `thread` hay `pod` có vài trăm giá trị vẫn lọc và chia làn được.
- Continuation Line đi theo Parent Line của nó. Dòng không có giá trị ở cột đã chọn vào làn "(trống)", và làn này chỉ hiện khi có dòng như vậy.
- Bấm tên làn thì thêm giá trị đó vào Filter của cột. Bấm một vạch thì nhảy tới dòng đầu tiên của làn đó trong cột thời gian đó.
- Cột đã chọn cho "Lane by" được nhớ theo view, như Filter.

**Ngoài phạm vi**
- App không tự đoán module nằm lẫn trong message. Người dùng phải khai báo field đó trong Log Format.
- Không gom file vào cùng luồng bằng tay, và không đoán luồng theo thời gian.

## Capabilities

### New Capabilities
- `log-name-column`: Log Name Column: Log Name suy từ Source File theo quy tắc Rotated File, lọc và ghim được, công tắc hiện/ẩn.
- `swimlane`: các làn dưới Time Histogram: Lane by, mặc định, giới hạn số làn, dòng không có giá trị, tương tác, ghi nhớ theo view.

### Modified Capabilities
- `log-filter`: ngưỡng của Filterable Column tăng từ 500 lên 1000 giá trị riêng biệt (Cột lọc được, số giá trị riêng biệt, Khi tail).

<!-- `time-histogram` chưa có trong openspec/specs (nằm trong change add-time-histogram), nên phần nối với nó được viết trong spec `swimlane`. -->

## Impact

- **Phụ thuộc** change `add-time-histogram` (`TimeAxis`, dải, tính trên background). Change đó cần được áp dụng trước.
- Code:
  - `src/remote/rotated.rs`: tách phần nhận diện theo tên ra module dùng chung, không phụ thuộc SFTP.
  - `src/filter_index/merge.rs`: thêm `log_name_column` cạnh `source_column`.
  - `src/time_histogram.rs`: thêm đếm theo làn.
  - `src/ui/time_histogram.rs`: vẽ làn và menu Lane by.
  - `src/app/filter.rs`: thêm Filter khi bấm tên làn.
  - mục Columns: công tắc Log Name Column.
  - `src/workspace/session.rs` / `state.rs`: lưu Lane by theo view.
- Dữ liệu lưu: thêm `lane_by` vào trạng thái theo view. Trạng thái cũ không có giá trị này thì dùng mặc định.
- `src/filter_index.rs`: `MAX_DISTINCT` 500 → 1000. Các nơi khác (`chunk.rs`, `merge.rs`, `filter_panel.rs`, `highlights_panel.rs`, `tests/log_filter_test.rs`) đã đọc hằng số này nên không phải sửa.
- Bộ nhớ: cột có từ 501 tới 1000 giá trị giờ giữ `row_ids` (2 byte mỗi dòng), cộng thêm danh sách giá trị.
- Docs: thêm Log Name Column, Swimlane và Lane By vào `CONTEXT.md`. Đổi 500 thành 1000 trong `CONTEXT.md` (Filterable Column, quy tắc 15), ADR 0005 và ADR 0016.
