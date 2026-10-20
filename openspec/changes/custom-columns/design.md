## Context

Hiện trạng:
- `parse_line` (`src/parser/text.rs`) thử lần lượt JSON, `RE_LOGCAT`, `RE_SYSLOG`, `RE_STANDARD` và `RE_STANDARD_NO_LEVEL`. Kết quả là một `LogEvent` có span cho `timestamp`, `level`, `module`, `message`. Trường JSON nằm trong `LogEvent.fields`, rồi được chép thành text vào `LogEntry.json_fields`.
- `Spans` (`src/model.rs:83`) là `Arc<[(&'static str, Range<usize>)]>` với tên trường đã intern. Ô của Parsed View được cắt từ `raw_line` theo span (`src/ui/cells.rs:64`), đúng với rule 30.
- Canvas chỉ vẽ một tập ô cố định (`TextCell`). Trường JSON lọc được (`Source::Json` trong `src/filter_index.rs`) nhưng **không** hiện thành cột.
- `PinnableColumn` (`src/column.rs`) là enum đóng gồm Timestamp, Level, Module, Source, Stream. `FieldMaxChars` có một trường cho mỗi cột.
- `column_value` (`src/filter_index.rs:97`) là đường đọc giá trị chung cho Filter và Highlight Rule, không cấp phát bộ nhớ.
- Live dùng `LineParser` để đọc từng dòng khi dòng tới.
- Mỗi dòng tốn khoảng 1.320 byte bộ nhớ (đo trong `add-remote-live-log`).

Ràng buộc từ domain:
- Rule 30: ô luôn là text lưu trong file.
- Rule 9 và 14: Search và Filter không có regex.
- Rule 19: Continuation Line kế thừa giá trị của Parent Line.
- Rule 23: cột được gộp theo tên.
- Rule 17: Quick Settings chỉ chứa công tắc hiển thị.
- Rule 21 và spec `log-filter`: menu chuột phải trên ô chỉ có lệnh tô màu; context sidebar có Filter, Highlights và Inspector.
- Rule 12: Search Expression chưa hợp lệ thì giữ Match của bản hợp lệ gần nhất.

## Goals / Non-Goals

**Goals:**
- Người dùng tách và đặt tên được bất kỳ đoạn nào của dòng mà không cần biết regex (Giai đoạn 1).
- Định dạng log lạ có thể mô tả trọn một lần rồi dùng lại cho nhiều file và nhiều Project (Giai đoạn 2).
- Log dạng `key=value` có cột mà người dùng hầu như không phải làm gì (Giai đoạn 3).
- Mọi cột người dùng tạo đều dùng được ngay với Filter, Highlight Rule, ghim và Auto-fit.
- Mỗi giai đoạn tự đứng được: phát hành Giai đoạn 0 và 1 trước mà không cần 2 và 3.

**Non-Goals:**
- Regex trong Search hoặc Filter.
- Cột tính toán, đổi kiểu dữ liệu (số, thời lượng) hay sắp xếp theo cột.
- Kéo Column Header để đổi thứ tự; thứ tự đổi trong mục Columns.
- Extraction Rule chỉ áp cho một số Log Space; rule áp cho cả Project, dòng không khớp thì để trống.
- Đồng bộ thư viện Log Format giữa các máy, hoặc nhập Log Format từ công cụ khác (Logstash grok, lnav).
- Log nhiều dòng mà một bản ghi trải qua nhiều dòng theo template (Log Format chỉ khớp từng dòng; Continuation Line giữ nguyên quy tắc hiện tại).

## Decisions

### 1. Mọi cột người dùng tạo đều là span trên dòng: một khái niệm Field Column
Extraction Rule, Log Format và Detected Field đều chỉ sinh ra **span** `(tên trường, khoảng byte)` trên `raw_line`. Chúng không bao giờ sinh text mới. Trường JSON cũng được chuyển sang dạng span: lấy khoảng byte của giá trị trong dòng, không lấy chuỗi đã giải escape. Nhờ vậy:
- Rule 30 đúng một cách tự nhiên: ô luôn là text của dòng.
- Không tốn bộ nhớ chép chuỗi; `json_fields: Vec<(String, String)>` sẽ được bỏ dần.
- Filter, Highlight Rule, Auto-fit và Coverage chỉ cần đọc một nguồn là `entry.spans`.

`PinnableColumn` thêm `Field(FieldKey)`, với `FieldKey` là `&'static str` đã intern (giống `Spans`), nên vẫn `Copy` và `Ord`. Riêng `Stream` vẫn không ghim được.

*Thay thế đã cân nhắc:*
- **Giữ giá trị dạng text cho cột người dùng tạo**: bị loại vì phá rule 30 và tốn bộ nhớ.
- **Mỗi nguồn một loại cột riêng** (JSON column, rule column, …): bị loại vì UI và Filter sẽ phải phân nhánh ở mọi chỗ, trong khi người dùng không cần biết cột đến từ đâu.

### 2. Regex chỉ dùng để đọc cột, không bao giờ để lọc
Regex xuất hiện ở hai chỗ: Extraction Rule và Log Format (Format Token biên dịch ra regex). Search và Filter vẫn chỉ so bằng nhau hoặc chứa chuỗi. Lý do:
- Người cần regex chủ yếu muốn *tách* giá trị. Khi đã có cột thì Filter Value và Highlight Rule là đủ.
- ADR-0004 và ADR-0005 giữ được nguyên vẹn.

Dùng crate `regex` đã có sẵn. Crate này chạy thời gian tuyến tính, không backtracking, nên regex người dùng gõ không làm treo app. Đặt `size_limit` khi biên dịch, và báo lỗi ngay dưới ô nhập.

*Thay thế đã cân nhắc:* thêm toán tử regex vào Filter. Bị loại vì đó chính là thứ ADR-0005 đã bỏ, và nó không cho người dùng một cột để nhìn.

### 3. Một đường đọc dòng: `LineConfig`
`parse_line(index, line)` đổi thành `parse_line(index, line, &LineConfig)`. Cùng một `Arc<LineConfig>` được dùng cho `parse_events` và `LineParser`:

```
LineConfig {
    format: Option<CompiledLogFormat>,   // Giai đoạn 2
    rules: Vec<CompiledExtractionRule>,  // Giai đoạn 1, theo thứ tự trong mục Columns
    detect_kv: bool,                     // Giai đoạn 3
}
```

Các bước đọc một dòng:
1. **Đọc nền.** Nếu có Log Format và dòng khớp, các trường `timestamp`, `level`, `module`, `message` điền vào cột cố định, các trường khác thành span Field Column. Nếu không có Log Format hoặc dòng không khớp, dùng Auto-detect như hiện nay.
2. **Extraction Rule.** Mỗi rule chạy trên **cả dòng**, vì người dùng có thể bôi đen ở MODULE hoặc ở phần đầu dòng. Capture group đầu tiên khớp được thành span. Tên trường đã có giá trị từ bước 1 thì bỏ qua (rule 38).
3. **Detected Field.** Chỉ quét trong span `message`, theo mẫu logfmt `([A-Za-z_][\w.\-]*)=("(?:[^"\\]|\\.)*"|\S+)`. Span chỉ lấy phần giá trị; với giá trị có ngoặc kép thì span gồm cả ngoặc kép, vẫn là text như lưu trong file. Detected Field luôn được tìm. Cấu hình chỉ quyết định trường nào đã bật thì được hiện và lọc.

Thứ tự ưu tiên ở rule 38 giúp kết quả luôn xác định: cùng một dòng và cùng cấu hình thì luôn ra cùng các ô.

*Thay thế đã cân nhắc:* chạy Extraction Rule lúc vẽ, không lưu span. Bị loại vì Filter, Coverage và Auto-fit đều cần duyệt mọi dòng, nên sẽ phải chạy regex nhiều lần.

### 4. Đoán cách khớp từ phần chữ bôi đen (Giai đoạn 1)
Đầu vào: dòng, khoảng byte được chọn `[s, e)`, và mẫu dòng của view (tối đa 1.000 dòng, lấy rải đều). Các ứng viên sinh theo thứ tự:
1. **Key đứng trước.** Nếu ngay trước `s` có `key=`, `key:`, `key: ` hoặc `"key":` thì neo vào key đó. Lớp ký tự của giá trị được đoán từ text được chọn: `\d+`, `[0-9a-fA-F-]+`, `[\w.\-]+`, `"[^"]*"`, hoặc nếu không thì `\S+`. Có tính thêm ký tự ngay sau `e` làm mốc kết thúc.
2. **Giữa hai dấu ngoặc.** Nếu đoạn chọn nằm trọn trong `[…]` hoặc `(…)` thì thử: "nhóm ngoặc thứ N của dòng".
3. **Từ thứ N.** Thử "từ thứ N tính từ đầu MESSAGE", rồi "từ thứ N tính từ đầu dòng". Từ được tách theo khoảng trắng.
4. **Regex tự viết.** Mở sẵn với regex của ứng viên đang chọn để người dùng sửa.

Một ứng viên chỉ được giữ nếu, trên chính dòng đó, capture group của nó ra **đúng** `[s, e)`. Các ứng viên còn lại được xếp theo Coverage trên mẫu, ứng viên cao nhất được chọn sẵn. Mỗi ứng viên hiện bằng một mô tả dễ đọc ("sau `user=`", "nhóm ngoặc thứ 3", "từ thứ 2 của message") chứ không phải regex. Regex chỉ hiện khi người dùng mở "Edit regex".

Name được gợi ý theo thứ tự: key ở bước 1, rồi tên đoán theo hình dạng giá trị (`ip`, `uuid`, `duration`), rồi để trống. Name được kiểm tra trùng: trùng cột cố định thì bị chặn; trùng Field Column đã có thì hiện "Values will join column *X*" (rule 23 và 38).

### 5. Log Format: template biên dịch ra regex (Giai đoạn 2)
Cú pháp:
- `%{TOKEN}` để khớp mà không lấy giá trị, `%{TOKEN:field}` để lấy giá trị vào trường `field`.
- Có thể chen regex thô; named group `(?<field>…)` cũng thành trường.
- Một chuỗi khoảng trắng trong template khớp `\s+`. Ký tự khác được escape.
- Toàn template được neo `^…$`, chạy trên dòng đã bỏ khoảng trắng đầu dòng (giống hiện nay, có cộng lại `lead` vào span).

Format Token có sẵn:
- `TIMESTAMP`: hợp của mọi dạng thời gian mà `read_timestamp` hiểu.
- `LEVEL`, `WORD`, `NOTSPACE`, `INT`, `NUMBER`, `IP`, `UUID`, `HEX`.
- `QUOTED`: chuỗi trong ngoặc kép, có escape.
- `DATA`: `.*?`; `GREEDY`: `.*`.

Trường `timestamp` được đọc nghĩa bằng `read_timestamp`, nên sắp xếp và Time Range vẫn chạy (rule 19 và 22). Trường `level` được hiểu như cột LEVEL hiện nay.

Thư viện Log Format nằm ở cấu hình app, cạnh Server, vì cùng một định dạng (nginx, app nội bộ) xuất hiện ở nhiều Project. Việc **chọn** Log Format lưu trong Project, theo thứ tự ưu tiên:
1. Chọn tay cho một Source File.
2. Chọn tay cho cả Log Space.
3. Glob tên file của Log Format (glob khớp đầu tiên theo thứ tự trong thư viện).
4. Auto-detect.

*Thay thế đã cân nhắc:*
- **Chỉ cho regex với named group**: bị loại vì quá khó với đa số người dùng; token giải quyết 90% trường hợp.
- **Dùng nguyên cú pháp grok của Logstash**: bị loại vì kéo theo một thư viện mẫu lớn; chỉ mượn dạng `%{TOKEN:field}` cho quen mắt.
- **Log Format thuộc Project**: bị loại vì phải soạn lại ở mỗi Project.

### 6. Extraction Rule thuộc Project
Extraction Rule giống Highlight Rule (rule 33): thuộc Project, áp cho mọi Log Space, tắt được mà không xoá. Một rule không khớp ở file khác thì chỉ để trống ô, và Coverage cho thấy điều đó. Field Column không có giá trị ở dòng nào trong view thì không được vẽ.

*Thay thế đã cân nhắc:*
- **Theo Log Space**: bị loại vì người dùng mở cùng loại log ở nhiều Log Space.
- **Chung cho cả app**: bị loại vì một rule theo vị trí ("từ thứ 2") rất dễ khớp nhầm ở Project khác.

### 7. Đọc lại khi cấu hình đổi
- **Thêm, sửa, tắt Extraction Rule hoặc bật Detected Field**: dòng nền không đổi, chỉ tính lại phần span của Field Column rồi dựng lại `FilterIndex`. Việc này chạy ở nền theo khối (`src/par.rs`). Trong lúc chạy, cột hiện thanh tiến độ mảnh trên Column Header, còn view vẫn dùng được.
- **Đổi Log Format**: timestamp có thể đổi, nên phải đọc lại cả file và dựng lại timeline. Marker được giữ vì Marker gắn với số dòng và Generation, không gắn với ô.
- **Live**: dòng mới dùng cấu hình mới ngay. Các dòng đang trong Live Window được đọc lại như trên.
- Filter và Highlight Rule trên một trường mà sau khi đổi cấu hình không còn tồn tại thì vẫn được giữ, nhưng không khớp dòng nào (rule 18). Mục Filter của trường đó hiện dòng chú thích "column no longer read".

### 8. Column Label và thứ tự cột
- `HashMap<FieldKey, String>` lưu trong Project. Nhấp đúp vào chữ trên Column Header thì chuyển thành ô nhập ngay tại chỗ. Enter lưu, Esc huỷ, để trống thì trả về tên trường.
- Label phải khác nhau và khác tên của cột khác trong view, nếu không thì bị chặn kèm lỗi ngay tại ô.
- Thứ tự cột: cột cố định → Source → Field Column theo thứ tự người dùng kéo trong mục Columns. Field Column mới được thêm vào cuối: trường Log Format theo thứ tự trong template, Extraction Rule và Detected Field theo thứ tự tạo hoặc bật.
- Thứ tự hiển thị tách khỏi thứ tự ưu tiên của rule 38. Kéo một cột chỉ đổi chỗ vẽ; thứ tự ưu tiên giữa các Extraction Rule cùng tên theo thứ tự trong nhóm *Rules*.

### 9. Coverage và kích thước cột
- `FilterIndex` khi dựng thì đếm thêm số Parent Line có giá trị ở mỗi cột. Phần này rẻ vì đã duyệt mọi dòng sẵn.
- Coverage hiện trên Column Header khi dưới 100%, làm tròn xuống (để 99,6% không hiện thành 100%).
- `FieldMaxChars` thêm `HashMap<FieldKey, usize>` cho Auto-fit (ADR-0003).

### 10. Chia giai đoạn
- **Giai đoạn 0**: Field Column (vẽ, ghim, Auto-fit, Coverage), chuyển JSON sang span, Column Label, mục Columns với nhóm *Built-in* và *Detected* (lúc này chỉ gồm trường JSON), menu của Column Header. Người dùng thấy ngay: trường JSON bật được thành cột, và cột cố định ẩn được.
- **Giai đoạn 1**: Extraction Rule, "Make column…", Preview bằng dòng thật, nhóm *Rules*.
- **Giai đoạn 2**: Log Format, trình soạn, cách chọn, glob, nhóm *From Log Format*.
- **Giai đoạn 3**: nhận diện `key=value` và thêm các trường đó vào danh sách Detected Field đã có.

Mỗi giai đoạn chỉ thêm vào `LineConfig` một trường. Các giai đoạn sau không đổi API của giai đoạn trước.

### 11. Mục Columns là nơi duy nhất quản lý cột
Mục Columns được thêm vào context sidebar, cạnh Filter, Highlights và Inspector. Mục này liệt kê mọi cột của view theo bốn nhóm: *Built-in*, *From Log Format*, *Rules* và *Detected*. Mỗi dòng gồm:
- Column Label (hoặc tên trường).
- Số Parent Line có giá trị trong view; `0 lines here` khi không có.
- Công tắc hiện/ẩn.
- Tay nắm để kéo đổi thứ tự (chỉ Field Column).
- Menu `⋯`: Rename, và Edit/Delete cho Extraction Rule.

Nhóm *From Log Format* có ô chọn Log Format của Log Space ngay ở đầu nhóm.

Mọi cột trừ MESSAGE và Gutter đều ẩn được. Ẩn không làm cột thôi được đọc: Filter và Highlight Rule trên cột đã ẩn vẫn chạy. Chỉ riêng Detected Field `key=value` là phải bật mới được đọc (quyết định 3, Giai đoạn 3). Trạng thái ẩn/hiện lưu theo Project, riêng Source Column vẫn lưu theo Log Space như hiện nay.

Có hai đường vào mục này:
- Menu chuột phải trên Column Header: "Hide column", "Rename", "Manage columns…".
- Công tắc Source Column trong Quick Settings vẫn giữ, và dùng chung trạng thái với dòng Source trong mục Columns.

Quick Settings không thêm gì, nên rule 17 vẫn đúng.

*Thay thế đã cân nhắc:*
- **Detected Field trong Quick Settings, Extraction Rule trong sidebar** (bản trước của design này): bị loại. Người dùng phải nhớ cột nào nằm ở đâu, và trường của Log Format không có chỗ nào để ẩn.
- **Popover "Columns" trên toolbar**: bị loại. Danh sách có thể dài tới hàng chục trường, cần ô tìm và kéo thả, nên không vừa một popover. Context sidebar thì đã có sẵn chỗ cho các mục kiểu này.
- **Kéo Column Header để đổi thứ tự**: vẫn để ngoài phạm vi. Header đã nhận kéo để đổi rộng và chọn text, thêm kéo đổi chỗ dễ gây thao tác nhầm.

### 12. Preview bằng dòng thật
Preview của dialog New Column hiện:
- Tối đa 3 dòng khớp, mỗi dòng có đoạn được lấy tô sáng bằng màu nhấn. Dòng chứa đoạn người dùng vừa bôi đen luôn đứng đầu.
- Tối đa 1 dòng **không khớp**, đánh dấu "not matched". Dòng này được chọn trong những dòng có cùng kiểu với dòng gốc (cùng Source File, cùng có hoặc cùng không có key), để cho thấy chỗ cách khớp bị trượt.
- Dưới các dòng đó là "1,204 of 1,530 lines (78%)" và tối đa 5 giá trị thường gặp.

Dòng trong Preview được cắt quanh đoạn tô sáng, có `…` ở hai đầu khi dòng quá dài, dùng font monospace của canvas. Bấm vào một dòng trong Preview thì view nhảy tới dòng đó và dialog vẫn mở.

Các dòng mẫu lấy từ mẫu 1.000 dòng đã dùng để xếp hạng ứng viên (quyết định 4), nên không phải duyệt thêm.

### 13. Lỗi regex và template: báo khi ngừng gõ, giữ Preview hợp lệ gần nhất
Ô regex của New Column và ô template của trình soạn Log Format theo cùng một luật, giống rule 12 của Search:
- Mỗi lần gõ, app biên dịch lại. Nếu hợp lệ, Preview cập nhật ngay.
- Nếu chưa hợp lệ, Preview giữ kết quả của bản hợp lệ gần nhất, hiện mờ kèm nhãn "Showing last valid pattern". Ô nhập chưa đổi sang trạng thái lỗi.
- Lỗi chỉ hiện khi người dùng ngừng gõ 500 ms hoặc rời khỏi ô. Lỗi gồm viền lỗi, icon cảnh báo và một câu nói cách sửa ("Needs exactly one capture group — wrap the value in ( )"). Với template, phần sai được gạch chân đúng chỗ.
- Gõ tiếp thì lỗi ẩn đi, chờ lần ngừng gõ sau.
- Nút Add column / Save bị tắt khi bản hiện tại chưa hợp lệ, kể cả lúc lỗi chưa hiện.

Column Label và Name kiểm tra khi bấm Enter hoặc rời khỏi ô, và lỗi ẩn đi khi người dùng gõ lại.

*Thay thế đã cân nhắc:* báo lỗi ngay mỗi lần gõ (bản trước của design này). Bị loại vì gõ dở `%{IP` hay `(\d+` nào cũng là lỗi, nên Preview nhấp nháy và biến mất đúng lúc người dùng cần nhìn nó.

## Risks / Trade-offs

- **[Bộ nhớ của span tăng theo số trường]**: mỗi span hiện tốn khoảng 32 byte; 10 trường × 1.000.000 dòng ≈ 320 MB. → Đo trước khi làm (task 0.1). Nếu vượt ngưỡng, nén span thành `(u16 id, u32 start, u32 len)`, khoảng 12 byte. Detected Field chưa bật thì không lưu span, chỉ lưu số đếm.
- **[Regex chậm trên file lớn]**: N rule × M dòng. → Dùng `RegexSet` để biết nhanh rule nào có thể khớp một dòng; chạy song song theo khối; chỉ đọc lại phần span.
- **[Đoán sai cách khớp]**: ví dụ "từ thứ 3" khớp nhầm ở dòng có định dạng khác. → Preview hiện dòng thật với đoạn được lấy tô sáng, kèm một dòng không khớp. Ứng viên theo key được ưu tiên vì ổn định hơn ứng viên theo vị trí.
- **[Quá nhiều Field Column làm canvas rối]**: → Detected Field mặc định tắt. Field Column không có giá trị trong view thì không được vẽ. Extraction Rule tắt được.
- **[Log Format khớp một phần của file]**: → Dòng không khớp vẫn đọc bằng Auto-detect (rule 39). Trình soạn hiện tỉ lệ khớp và cho nhảy tới dòng không khớp đầu tiên.
- **[Bỏ `json_fields` ảnh hưởng tới Filter và persist hiện có]**: → Giữ tên trường JSON y như cũ, nên Filter đã lưu vẫn khớp. Viết test hồi quy cho Filter trên trường JSON trước khi chuyển.
- **[Label trùng tên trường khác gây nhầm]**: → Label bắt buộc khác nhau trong view (quyết định 8).

## Đo bộ nhớ của span (task 0.1)

Đo bằng `tests/span_memory_test.rs` (allocator đếm byte, bản release, 1.000.000 dòng):

| Cách lưu | Byte mỗi dòng |
|---|---|
| `Spans`, 4 trường cố định | 160 |
| `Spans`, 4 cố định + 10 trường | 480 |
| `json_fields` dạng text, 10 trường | 744 |
| Nén `(u16, u32, u32)`, 4 / 14 trường | 80 / 200 |

10 trường thêm tốn 320 MB dạng `(&'static str, Range<usize>)`, 120 MB nếu nén. Dòng JSON hiện đã trả cả span lẫn `json_fields`; bỏ `json_fields` tiết kiệm khoảng 744 MB cho mỗi 1.000.000 dòng JSON có 10 trường, nhiều hơn phần nén tiết kiệm được.

**Quyết định:** giữ `(&'static str, Range<usize>)`. Nén cần một bảng tên toàn cục đánh số `u16` dùng chung giữa các luồng đọc song song, trong khi lợi ích chỉ có ở log không phải JSON với nhiều Extraction Rule. Detected Field chưa bật thì không lưu span (Giai đoạn 3). Xem lại khi đo task 4.1.

## Ghi chú khi làm Giai đoạn 0

- Nhóm *Detected* liệt kê các trường đang bật trước, theo thứ tự vẽ (có tay nắm để kéo), rồi mới tới các trường đang tắt, xếp theo số dòng giảm dần. Lý do: thứ tự kéo và thứ tự theo số dòng không thể cùng là thứ tự của một danh sách.
- Đổi tên một cột không có Column Header trong view (đang ẩn, hoặc chưa có giá trị) thì ô nhập mở ngay trên dòng của cột đó trong mục Columns.
- Column Header của Field Column luôn chừa chỗ cho Coverage (`99%`) và Pin Toggle khi Auto-fit.
- Cấu hình hiện/ẩn và thứ tự cột nằm ở `Project.columns` (`src/columns.rs`); trường JSON có giá trị `null` không có span, nên không có giá trị (giữ đúng hành vi cũ).

## Ghi chú khi làm Giai đoạn 1

- Span của Extraction Rule lưu riêng ở `LogEntry.extra`, không trộn vào `spans`. Đọc lại khi rule đổi chỉ thay `extra`; trường có sẵn trong `spans` (cột cố định, JSON) luôn thắng (rule 38).
- Rule được áp lên các dòng của view khi dựng view và lên từng dòng live khi tới; cache file không đổi. Đổi rule thì đọc lại ở nền (`cx.background_spawn` + `par::map_rows`), rồi chỉ dựng lại các cột Field của `FilterIndex` (`rebuild_fields`).
- Tiến độ trên Column Header là một vạch mảnh không có phần trăm, hiện trong lúc đọc lại.
- Rule có `scope`: `Line` (mặc định) hoặc `Message`, cho ứng viên "từ thứ N của message".
- Kéo trong nhóm *Rules* đổi thứ tự ưu tiên, không đổi chỗ vẽ cột. Cột của rule được vẽ theo thứ tự bật (thêm vào cuối).
- Rename trong menu `⋯` của rule đổi Column Label (rule 37); đổi tên trường thì dùng Edit.
- Rule bị xoá được nhớ trong phiên làm việc, nên "Turn column back on" khôi phục được rule vừa xoá.
- Menu dòng của Focus Pane chỉ có "Make column…"; mục này bị tắt (không ẩn) khi chưa có vùng chọn trong một dòng, để menu không trống.
- Preview đếm trên mọi Parent Line khi view có tối đa 20.000 dòng, nếu nhiều hơn thì đếm trên mẫu 20.000 dòng và ghi "in a sample".

## Ghi chú khi làm Giai đoạn 2

- Template chỉ coi named group `(?<field>…)` hoặc `(?P<field>…)` là regex thô; mọi ký tự khác (kể cả `[`, `(`, `.`) khớp đúng chính nó. Template neo `^…\s*$` trên dòng đã bỏ khoảng trắng đầu.
- `read_timestamp` hiểu thêm phần lẻ sau dấu phẩy (Python `logging`) và giờ của nginx/Apache (`20/Oct/2026:10:00:01 +0000`); `TIMESTAMP` là hợp các dạng đó.
- Mẫu có sẵn: nginx/Apache combined (kèm glob `access*.log`), Python `logging` mặc định, Python `logging` có `asctime`, log4j TTCC (`%r [%t] %p %c %x - %m%n`). Log Format không có trường `message` thì MESSAGE là cả dòng.
- Mỗi `ParsedFile` nhớ Log Format đã dùng để đọc (`format_id`). Khi dựng view, file đọc bằng format khác với format cần dùng thì được đọc lại ngay từ các dòng đã lưu, nên không đọc lại đĩa. Khi người dùng đổi lựa chọn, các file của Log Space đang mở được đọc lại ở nền rồi dựng lại view; số dòng, Generation và Marker được giữ. File live (kể cả Remote) đọc dòng mới bằng format đó qua `LineParser` của `LiveBuffer`.
- Lựa chọn của Log Space có ba trạng thái: chưa chọn (dùng glob), Auto-detect (không dùng glob), một Log Format. Lựa chọn của file: như Log Space, Auto-detect, hoặc một Log Format.
- Trường của Log Format mặc định hiện; tắt một trường thì tên trường được ghi vào `ColumnSettings.hidden`.
- Thư viện nằm ở `log_formats.json` cạnh file session (thư mục cấu hình app), quản lý ở trang Settings › Log Formats. Mở trình soạn từ Settings thì Settings đóng lại trước.
- Menu file trong sidebar có mục "Log Format" dạng danh sách phẳng (không phải submenu), vì callback của context menu không có `Context<PopupMenu>`.

## Đo hiệu năng (task 4.1)

Đo bằng `tests/custom_columns_perf_test.rs` (bản release, 1.000.000 dòng dạng standard có `user=` và `took=`):

| Bước | Luồng | Thời gian |
|---|---|---|
| Đọc file bằng Auto-detect (tham chiếu) | nền | 2.095 ms |
| Thêm Extraction Rule: chép dòng ra | UI | 14 ms |
| Thêm Extraction Rule: đọc span | nền | 156 ms |
| Thêm Extraction Rule: gắn span + dựng lại cột Field của `FilterIndex` | UI | 53 ms |
| Đổi Log Format: chép dòng ra | UI | 22 ms |
| Đổi Log Format: đọc lại | nền | 1.399 ms |
| Đổi Log Format: gắn kết quả vào entry | UI | 151 ms |
| Đổi Log Format: dựng lại `FilterIndex` | UI | 322 ms |

Kết luận:
- Thêm, sửa, tắt Extraction Rule hay bật Detected Field chỉ chiếm luồng UI khoảng 70 ms; phần đọc chạy ở nền, view vẫn dùng được.
- Đổi Log Format đọc lại ở nền (khoảng 1,4 s), nhưng luồng UI vẫn bị chiếm khoảng 0,5 s khi gắn kết quả và dựng lại view (thứ tự thời gian và `FilterIndex`). Mức này tương đương mở lại Log Space nên chưa tối ưu thêm. Nếu cần, có thể dựng `FilterIndex` và timeline ở nền rồi chỉ hoán đổi kết quả trên luồng UI.

## Ghi chú khi làm Giai đoạn 3

- `key=value` được tìm bằng bộ quét viết tay (`extract::scan_kv`): key phải đứng đầu text hoặc sau khoảng trắng hay `,;([{`; giá trị không được rỗng hay bắt đầu bằng `=`.
- `FilterIndex` đếm số Parent Line có mỗi key (kể cả khi chưa bật), theo cùng đường append/truncate, nên số đếm tăng giảm theo Live Window.
- Field Column đang bật mà không phải tên rule thì được đọc như Detected Field `key=value` (sau Extraction Rule, rule 38). Bật hoặc tắt sẽ đọc lại ở nền khi cấu hình đọc dòng thật sự đổi.

## Migration Plan

- Project cũ không có Extraction Rule, Column Label hay Log Format, nên mọi thứ chạy như hiện nay (Auto-detect, không Field Column ngoài JSON).
- Trường JSON chuyển từ text sang span trong Giai đoạn 0. Filter đã lưu dùng tên trường nên vẫn khớp.
- Thêm khoá mới vào file Project với `#[serde(default)]`, nên bản cũ của app bỏ qua chúng mà không lỗi.

## Open Questions

- Có cho Extraction Rule giới hạn theo glob tên file như Log Format (để giảm khớp nhầm) không, hay đợi phản hồi sau Giai đoạn 1?
- Trường JSON đi qua danh sách Detected Field và mặc định tắt, để canvas không đột ngột thêm hàng chục cột. Có nên tự bật sẵn vài trường JSON phổ biến (`user`, `request_id`) không?
- Có cần lệnh "Export Log Format" ra file để chia sẻ trong team không?
