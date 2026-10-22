> **Đã bị thay thế một phần (ADR-0015).** Extraction Rule, "Make column…" và Column Label đã bị gỡ; Log Format là cách duy nhất để định nghĩa cột, và mục Columns chỉ bật/tắt cột của Focus Pane.  Các phần nhắc đến chúng dưới đây chỉ còn là lịch sử, không còn đúng với app. Xem `docs/adr/0015-log-format-is-the-only-way-to-define-columns.md` và `CONTEXT.md`.

## Why

Parsed View chỉ có bốn cột cố định: TIMESTAMP, LEVEL, MODULE, MESSAGE. Ngoài JSON và ba định dạng có sẵn (standard, logcat, syslog), mọi thứ còn lại của dòng đều dồn vào MESSAGE. Thực tế một dòng log thường chứa nhiều trường đa dạng như `user=alice`, `req=7f3a…`, IP, thời gian xử lý, mã HTTP. Người dùng không tách được, không đặt tên được, nên không lọc, không tô màu và không ghim được những trường đó. ADR-0005 đã ghi rõ hệ quả: "cột nào không được phát hiện thì không lọc được". Change này cho người dùng tự đặt tên và tách cột, chia làm ba giai đoạn, mỗi giai đoạn dùng được ngay khi xong.

## What Changes

**Nền chung (Giai đoạn 0, làm trước cả ba giai đoạn):**
- **Field Column**: một cột của Parsed View ngoài bốn cột cố định, hiện giá trị của một trường có tên đọc được từ dòng. Ô của nó luôn là một đoạn text của dòng như lưu trong file (rule 30). Field Column ghim được, lọc được (nếu là Filterable Column), dùng được trong Highlight Rule, có Auto-fit và kéo đổi rộng như cột cố định.
- **Column Label**: nhấp đúp vào Column Header để đặt tên hiển thị cho bất kỳ cột nào trừ Gutter. Tên này chỉ là nhãn: Filter, Highlight Rule và Search không đổi theo nó. Column Label thuộc Project.
- **Coverage**: khi không phải mọi dòng đều có giá trị ở một Field Column, Column Header hiện tỉ lệ dòng có giá trị (ví dụ `79%`). Di chuột vào sẽ hiện số dòng cụ thể.
- Dòng không có giá trị ở một Field Column thì ô để trống. Không dòng nào bị mất, và text của dòng vẫn nằm nguyên ở MESSAGE.
- **Mục Columns** mới trong context sidebar, cạnh Filter, Highlights và Inspector, là **nơi duy nhất** quản lý cột của view. Mục này liệt kê mọi cột theo nhóm:
  - *Built-in*: TIMESTAMP, LEVEL, MODULE, Source, Stream.
  - *From Log Format*: từ Giai đoạn 2.
  - *Rules*: từ Giai đoạn 1.
  - *Detected*: lúc này chỉ gồm các trường JSON.

  Mỗi cột có công tắc hiện/ẩn và số dòng có giá trị trong view. Cột không có giá trị ở view này hiện `0 lines here` thay vì biến mất không lời giải thích. Kéo trong danh sách để đổi thứ tự Field Column. Mọi cột trừ MESSAGE đều ẩn được, kể cả trường của Log Format. Detected Field mặc định tắt, để log JSON không đột ngột có thêm hàng chục cột.
- Menu chuột phải trên Column Header có "Hide column" và "Manage columns…" (mở mục Columns). Công tắc Source Column trong Quick Settings vẫn giữ, và dùng chung trạng thái với mục Columns.

**Giai đoạn 1: tạo cột từ phần chữ bôi đen (Extraction Rule)**
- Bôi đen một đoạn trong một ô của Parsed View hoặc trong Focus Pane, chuột phải rồi chọn **"Make column…"**. Dialog New Column mở ra:
  - Ô **Name**, được điền sẵn bằng key đứng ngay trước đoạn được chọn (`user=` cho ra `user`).
  - Vài cách khớp app đoán sẵn, hiện dạng lựa chọn: "sau `user=`", "từ thứ N", hoặc "regex tự viết".
  - **Preview** bằng dòng thật: 3 dòng khớp, mỗi dòng tô sáng đúng đoạn được lấy, và 1 dòng *không* khớp (nếu có) để người dùng thấy ngay cách khớp bị trượt ở đâu. Kèm theo là số dòng khớp trên tổng số dòng và vài giá trị thường gặp.
- Người dùng không cần biết regex. Ai muốn thì sửa regex trực tiếp, nhưng regex phải có đúng một capture group.
- **Extraction Rule** thuộc Project, giống Highlight Rule. Rule hiện trong nhóm *Rules* của mục Columns để sửa, tắt, xoá và đổi thứ tự.

**Giai đoạn 2: định nghĩa cả dòng (Log Format)**
- **Log Format** là một template mô tả cả dòng bằng các **Format Token** có tên, ví dụ `%{TIMESTAMP:time} [%{LEVEL:level}] %{IP:client} %{GREEDY:message}`. Có thể dùng regex với named group thay cho token.
- Trình soạn Log Format xem trước trực tiếp trên dòng thật của file: mỗi trường một màu, dòng không khớp được đánh dấu, có tỉ lệ khớp.
- **Lỗi khi gõ regex hoặc template** (cả dialog New Column lẫn trình soạn Log Format): app chỉ báo lỗi khi người dùng ngừng gõ khoảng 500 ms hoặc rời khỏi ô, không báo khi đang gõ dở. Trong lúc regex hoặc template chưa hợp lệ, Preview giữ kết quả của bản hợp lệ gần nhất (giống rule 12 của Search) và hiện mờ. Lỗi gồm icon và câu nói cách sửa, và biến mất khi người dùng gõ tiếp. Nút lưu bị tắt cho tới khi hợp lệ.
- Có sẵn một số mẫu thông dụng (nginx/Apache combined, Python `logging`, log4j mặc định) để người dùng chép rồi sửa.
- Log Format là thư viện chung cho cả app, giống Server. Mỗi Log Space hoặc Source File chọn một Log Format, hoặc để **Auto-detect**. Một Log Format cũng có thể tự áp vào file có tên khớp một glob (`nginx-*.log`).
- Trường tên `timestamp`, `level`, `module`, `message` của Log Format điền vào cột cố định tương ứng, nên sắp xếp theo thời gian, Level Marker và Continuation Line vẫn đúng. Dòng không khớp Log Format thì được đọc bằng Auto-detect như hiện nay.

**Giai đoạn 3: tự nhận diện trường (Detected Field)**
- App tìm các cặp `key=value` (kể cả `key="có khoảng trắng"`, kiểu logfmt) trong MESSAGE, và thêm chúng vào danh sách Detected Field đã có từ Giai đoạn 0, cạnh các trường JSON. Mỗi trường kèm số dòng có nó và một công tắc. Bật công tắc thì trường đó thành Field Column.
- Detected Field kiểu `key=value` chỉ thành Filterable Column khi đã được bật. Trường JSON vẫn lọc được như hiện nay, kể cả khi chưa hiện thành cột.

**Ngoài phạm vi:** regex trong Search và Filter (rule 9 và 14 giữ nguyên), kéo Column Header để đổi thứ tự cột, cột tính toán (ví dụ chia hai trường), sửa text của ô.

## Domain changes

Change này không sửa CONTEXT.md hay ADR vì có các session khác đang làm song song. Khi áp dụng change, cần cập nhật CONTEXT.md như sau.

**Thuật ngữ mới (nhóm Canvas, Search and Filter):**
- **Field Column**: một cột của Parsed View ngoài TIMESTAMP, LEVEL, MODULE và MESSAGE, hiện một trường có tên đọc từ dòng: trường JSON, trường của Extraction Rule, của Log Format, hoặc Detected Field đã bật. Ô của nó là một đoạn text của dòng. _Avoid_: custom column, extra column, dynamic column.
- **Column Label**: tên hiển thị người dùng đặt cho một cột, thuộc Project. Tên trường bên dưới không đổi. _Avoid_: alias, display name, column title.
- **Coverage**: tỉ lệ Parent Line có giá trị ở một cột, tính trên các dòng mà view đang giữ. _Avoid_: fill rate, match rate.
- **Extraction Rule**: một tên và một regex có đúng một capture group, thuộc Project. Rule này đọc một Field Column từ mọi dòng của mọi Log Space trong Project. _Avoid_: parser, pattern, custom field.
- **Log Format**: một template có tên mô tả cả dòng bằng Format Token, chung cho cả app. Log Format được chọn cho Log Space hoặc Source File, hoặc tự áp theo glob tên file. _Avoid_: grok, parser, profile, schema.
- **Format Token**: một mẫu có tên trong Log Format, như `TIMESTAMP` hay `IP`, viết `%{TOKEN:field}`. _Avoid_: placeholder, macro.
- **Auto-detect**: cách đọc dòng khi không có Log Format, tức JSON và các định dạng có sẵn. _Avoid_: default parser.
- **Detected Field**: một trường app tự tìm thấy trong dòng (khoá JSON, hoặc cặp `key=value` trong MESSAGE), hiện trong nhóm *Detected* của mục Columns với số dòng và một công tắc, mặc định tắt. _Avoid_: suggested column, auto column.
- **Columns** (mục của context sidebar): nơi duy nhất để hiện/ẩn và sắp xếp cột của view, quản lý Extraction Rule và chọn Log Format. _Avoid_: column manager, view panel.

**Quy tắc đổi:**
- **2. Pinnability** → "Gutter luôn được ghim; MESSAGE không bao giờ ghim được. TIMESTAMP, LEVEL, MODULE, Source Column và mọi Field Column đều ghim được."
- **3. Order Preservation** → "Cột đã ghim luôn giữ đúng thứ tự cột của view (TIMESTAMP → LEVEL → MODULE → Source → Field Column theo thứ tự trong mục Columns), bất kể thứ tự người dùng bấm ghim."
- **17. Quick Settings Only**: giữ nguyên nội dung (Quick Settings chỉ có Source Column và density). Thêm câu "Hiện/ẩn và thứ tự cột, Extraction Rule và Log Format nằm ở mục Columns; công tắc Source Column ở hai nơi dùng chung một trạng thái."
- **21. No Quick Filter Menu**: thêm "Menu chuột phải trên ô có thể có 'Make column…', mục này tạo một Extraction Rule."
- **23. Columns Are United By Name**: thêm "Field Column từ JSON, Extraction Rule, Log Format và Detected Field cũng được gộp theo tên trường. Column Label không tham gia việc gộp."

**Quy tắc mới:**
- **36. Regex Only Reads Columns**: regex chỉ dùng trong Extraction Rule và Log Format, để quyết định ô nào lấy đoạn text nào. Search và Filter vẫn không có regex (rule 9, 14).
- **37. Labels Never Rename Fields**: Column Label chỉ đổi chữ trên Column Header, trong Filter, Highlights và mục Columns. Tên trường, giá trị của Filter và Highlight Rule không đổi khi đổi Label.
- **38. One Value Per Field Per Line**: khi nhiều nguồn cho cùng một tên trường trên một dòng, giá trị lấy theo thứ tự: cột cố định hoặc JSON, rồi Log Format, rồi Extraction Rule (theo thứ tự trong nhóm *Rules* của mục Columns), rồi Detected Field.
- **40. Invalid Patterns Keep Last Preview**: trong lúc regex của Extraction Rule hoặc template của Log Format chưa hợp lệ, Preview giữ kết quả của bản hợp lệ gần nhất. Lỗi chỉ hiện khi người dùng ngừng gõ hoặc rời khỏi ô.
- **39. Unmatched Lines Lose Nothing**: một dòng không khớp Log Format hay Extraction Rule vẫn là một hàng, giữ text ở MESSAGE, và chỉ để trống các ô không đọc được.

**ADR:** khi áp dụng change, nên thêm ADR 0012 "Users Define Columns; Regex Stays Out Of Search And Filter" (xem design.md, quyết định 1 và 2). ADR này ghi rằng ADR-0005 được mở rộng: cột lọc được là cột phát hiện được *hoặc* do người dùng định nghĩa.

## Capabilities

### New Capabilities
- `field-column`: Field Column trong Parsed View: hiện, ghim, đổi rộng, Coverage, Column Label, ô của Continuation Line, giữ nguyên text lưu trong file; mục Columns (hiện/ẩn, thứ tự, số dòng, `0 lines here`) và menu của Column Header.
- `extraction-rule`: Giai đoạn 1: "Make column…" từ phần chữ bôi đen, các cách khớp được đoán, Preview bằng dòng thật, lỗi regex hiện khi ngừng gõ, nhóm *Rules* của mục Columns, regex có một capture group, Extraction Rule thuộc Project.
- `log-format`: Giai đoạn 2: Log Format, Format Token, trình soạn có xem trước (giữ bản hợp lệ gần nhất khi template lỗi), mẫu có sẵn, chọn theo Log Space hoặc Source File, tự áp theo glob, fallback sang Auto-detect.
- `detected-field`: nhóm *Detected* của mục Columns, bật thành Field Column. Giai đoạn 0 gồm trường JSON; Giai đoạn 3 thêm nhận diện `key=value`.

### Modified Capabilities
- `log-filter`: Field Column là nguồn cột của Filter; Filter hiện Column Label; Focus Pane giải thích khi trống vì điều kiện trên cột không còn được đọc; context sidebar thêm mục Columns; menu chuột phải trên ô có thêm "Make column…".

## Impact

- **Parser** (`src/parser/text.rs`, `src/parser/event.rs`): nhận một cấu hình đọc dòng gồm Log Format đang chọn, Extraction Rule và Detected Field đã bật. `parse_events` và `LineParser` (live) đều dùng chung cấu hình này. Trường mới chỉ được lưu dạng span trong `Spans` (`src/model.rs`), không chép text.
- **Model cột** (`src/column.rs`): `PinnableColumn` thêm `Field(name)`; `FieldMaxChars` thêm bảng theo tên trường; `ColumnLayoutState` lưu thứ tự cột và Column Label.
- **Canvas và ô** (`src/ui/canvas.rs`, `src/ui/cells.rs`): vẽ một số lượng Field Column không cố định, cùng Coverage trên Column Header.
- **Filter và Highlight** (`src/filter_index.rs`, `src/highlight.rs`): `column_value` đọc Field Column qua span; `Source::Json` mở rộng thành `Source::Field`.
- **UI mới**: mục Columns trong context sidebar, menu chuột phải trên Column Header, dialog New Column, trình soạn Log Format, sửa Column Label ngay trên Column Header, mục "Make column…" trong `src/ui/cell_menu.rs`, empty state của Focus Pane khi Filter dùng cột không còn được đọc.
- **Lưu trữ**: Extraction Rule, Column Label, Detected Field đã bật và Log Format đã chọn lưu trong Project. Thư viện Log Format lưu ở cấu hình app, cạnh Server.
- **Hiệu năng**: đổi rule hay Log Format phải đọc lại các span của file đã nạp. Việc này chạy ở nền, song song theo khối (`src/par.rs`). Crate `regex` chạy thời gian tuyến tính nên regex người dùng viết không thể làm treo app.
- Không thêm dependency mới.
