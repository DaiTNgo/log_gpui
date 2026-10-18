## Context

Hiện trạng liên quan:
- Parser tạo `LogEvent`/`LogEntry` với `time_str` (có khi lấy nguyên đoạn trong dòng, có khi là `format!("{:.3}", timestamp)` với log JSON) và level đã chuẩn hóa (`level.as_str()` trong [text.rs](src/parser/text.rs)). Không có vị trí ký tự của từng trường trong dòng gốc.
- Parser đã đọc mỗi dòng thành một entry riêng. Dòng không đọc được timestamp có `has_time = false`, nhưng bị tự điền module `app` và level đoán từ chữ trong cả dòng (`LogLevel::from_str(line_trimmed)`). Dòng trống bị bỏ qua. `LogEntry.sub_lines` và nút mở rộng (`is_expanded`) chỉ dùng cho dữ liệu mẫu ([sample.rs](src/sample.rs)).
- Marker là `MarkerType` (Error, Warning, Bookmark, Important, Note) gắn vào cả dòng, kèm `note`. Error/Warning được suy từ level qua `effective_marker()`. Change `add-log-space` lưu Marker là `StoredMarker { line, marker, note, hash }` trên `LogFile` và tô ký hiệu trong Raw View ([raw.rs](src/app/raw.rs), [log_space.rs](src/workspace/log_space.rs)).
- Canvas chỉ chọn được cả hàng (`selected_row_pos`); không chọn được text.
- Context sidebar có các mục Filter, Marker, Inspector (`ContextSection`, [state.rs](src/workspace/state.rs)).
- Overview ruler đã vẽ vạch cho Marker và Match (`RulerMark`, `compute_ruler_marks`).
- Font là monospace (ADR-0003), nên đổi tọa độ chuột ra vị trí ký tự chỉ cần chia cho độ rộng một ký tự.

Thuật ngữ và rule: [CONTEXT.md](CONTEXT.md) (Marker, Level Marker, Highlight Rule, Highlight Palette; rule 21, 30–35). Quyết định: [ADR-0007](docs/adr/0007-parsed-view-shows-stored-text.md). Change này dựng trên `add-log-space` (Raw View, Log Space, `LogFile.markers`).

## Goals / Non-Goals

**Goals:**
- Mọi ô lấy từ dòng hiện đúng text lưu trong file; mỗi dòng là một hàng.
- Chọn text qua nhiều ô và nhiều hàng trong Parsed View; ⌘C chép text gốc.
- Marker là màu tô trên khoảng ký tự, đồng nhất giữa Raw View và Parsed View.
- Highlight Rule theo cột, có thứ tự ưu tiên, thuộc Project.
- Một bảng màu dùng chung cho Marker và Highlight Rule.

**Non-Goals:**
- Chuyển đổi Marker cũ (theo dòng, có loại và note) sang Marker mới.
- Highlight Rule trong Raw View, hay rule trên nhiều cột cùng lúc.
- Lọc theo Highlight Rule (ví dụ "chỉ hiện hàng được tô").
- Danh sách Marker, nhảy tới Marker kế tiếp bằng phím tắt, xuất ảnh chụp.
- Đổi định dạng hiển thị timestamp theo ý người dùng.

## Decisions

### 1. Mỗi trường mang khoảng ký tự trong dòng gốc

`LogEvent` thêm `spans: HashMap<String, Range<usize>>` (byte offset trong `raw_line`) cho mọi trường lấy từ dòng: `timestamp`, `level`, `module`, `message`, và các trường JSON. Text hiển thị của một ô luôn là `&raw_line[span]`. Giá trị đã hiểu (`timestamp: f64`, `LogLevel`) được giữ riêng, chỉ dùng để sắp xếp, lọc, Time Range và Level Marker.

- Trường JSON kiểu chuỗi: span là phần bên trong cặp ngoặc kép, giữ nguyên escape.
- Trường JSON kiểu số hoặc bool: span là nguyên token.
- `time_str` và việc hiển thị level qua `level.as_str()` bị bỏ.

*Phương án khác:* lưu text hiển thị thành chuỗi riêng. Bị loại vì đó chính là nguồn gây lệch, và không cho Marker ánh xạ giữa hai view.

### 2. Continuation Line: một hàng riêng, dùng giá trị của Parent Line

**Nhận diện:** dòng không có timestamp đọc được và nằm sau một dòng có timestamp là Continuation Line. Parent Line là dòng gần nhất phía trên có timestamp riêng. Việc này làm một lượt khi parse, sau khi biết `has_time` của mọi dòng. Không dùng mẫu text (`at `, `Caused by`, `Traceback`) vì mỗi ngôn ngữ một kiểu và dễ sai.

**Dữ liệu:** `LogEntry` thêm `parent: Option<usize>` (số dòng của Parent Line). Continuation Line chỉ có span `message` phủ cả dòng và không có giá trị riêng nào khác. Parser bỏ việc tự điền module `app` và đoán level từ cả dòng: dòng không đọc được định dạng không có level hay module. Dòng trống cũng là một entry; nếu nằm sau một dòng có timestamp thì nó là Continuation Line.

**Hiển thị:** mỗi Continuation Line là một hàng. Ô MESSAGE hiện text của dòng; các ô khác lấy từ dòng hiện `_` màu nhạt. `_` chỉ là cách vẽ, không phải text: không nằm trong span nào, không chọn được, không nhận Marker, không được Search khớp, không được chép.

**Giá trị khi lọc và tô:** Filter, Time Range và Highlight Rule đọc giá trị của Continuation Line qua Parent Line (`value_of(row, column)` đi theo `parent`). Continuation Line được hiện khi và chỉ khi Parent Line được hiện, nên không bao giờ có stack trace lơ lửng mà mất dòng lỗi. Search và Marker chỉ dùng text của chính dòng.

**Đếm:** chỉ mục Filter (Filter Value và số dòng) chỉ đếm Parent Line và dòng đứng riêng. Level Marker chỉ đặt trên Parent Line.

*Phương án khác:*
- Gộp Continuation Line vào hàng của Parent Line, có nút mở rộng: bị loại vì số hàng không khớp Raw View, không chọn text và đặt Marker trên từng dòng stack trace được, và hàng cao thấp khác nhau làm virtualization phức tạp hơn.
- Hiện giá trị của Parent Line trong ô: bị loại vì người dùng dễ tưởng dòng tiếp nối tự ghi giá trị đó (rule 30).
- Để trống ô và không kế thừa: bị loại vì Filter `level = ERROR` sẽ ẩn mọi dòng stack trace (rule 18).

### 3. Vùng chọn tính theo vị trí ký tự trong dòng gốc

```
TextSelection { anchor: (row_pos, byte), head: (row_pos, byte) }
```

- Bấm chuột vào một ô: tìm ô dưới con trỏ, đổi `x` ra chỉ số ký tự trong text của ô, rồi cộng `span.start` để ra byte trong `raw_line`.
- Hàng đầu được chọn từ anchor tới cuối dòng, các hàng giữa chọn cả dòng, hàng cuối từ đầu dòng tới head.
- Mỗi ô tô phần giao giữa span của ô với vùng chọn của hàng đó.
- ⌘C ghép `raw_line[đoạn chọn]` của từng hàng đang hiện, nối bằng `\n`.
- Kéo ra mép thì canvas tự cuộn theo hai trục.
- Bấm mà không kéo thì vẫn chọn hàng như hiện nay (`selected_row_pos`).

Raw View dùng cùng mô hình. Một vùng chọn ở hai view có cùng nghĩa, nên lệnh "Mark with color" chỉ có một cách làm.

### 4. Marker là khoảng ký tự kèm màu

```
StoredMarker { line, range: Range<usize>, color: HighlightColor, line_hash }
```

- Mỗi dòng có thể có nhiều Marker, không chồng lên nhau. Tô chồng thì cắt Marker cũ ra, phần chồng nhận màu mới.
- "Remove marker" trên một vị trí bỏ cả Marker chứa vị trí đó.
- `line_hash` giữ để bỏ Marker khi dòng scratch bị sửa (rule 26).
- Parsed View vẽ phần giao giữa Marker và span của từng ô. Phần nằm giữa hai ô (khoảng trắng, dấu phân cách không thuộc ô nào) chỉ thấy trong Raw View.
- Overview ruler: một vạch cho mỗi dòng có Marker, lấy màu của Marker đầu tiên trên dòng.
- Bỏ `MarkerType::{Bookmark, Important, Note}`, `note`, `cycle`, `toggle_bookmark`, panel Marker. Level Marker là hàm thuần `level_marker(LogLevel) -> Option<LevelMarker>`, chỉ dùng để vẽ Gutter.

### 5. Highlight Rule dùng lại bộ so khớp của Filter

```
HighlightRule { id, column, values: Vec<String>, custom: Vec<String>, color, enabled }
Project.highlight_rules: Vec<HighlightRule>   // thứ tự = ưu tiên
```

So khớp dùng đúng hàm của Filter: Filter Value bằng nhau (gấp hoa thường theo Unicode), Custom Value chứa chuỗi. Cột thiếu giá trị thì không khớp (rule 18).

Kết quả tô màu tính cho từng hàng đang hiện khi render: duyệt các rule đang bật, rule đầu tiên khớp thắng. Cache theo `(phiên bản rule, phiên bản dữ liệu)` để không tính lại mỗi frame.

Vẽ: nền hàng bằng sắc độ nhạt của màu rule, ô thỏa điều kiện bằng sắc độ vừa. Thứ tự lớp từ dưới lên: nền → Highlight Rule → hàng đang chọn → Match của Search → Marker → vùng chọn.

### 6. Highlight Palette là token màu theo theme

10 màu × 3 sắc độ, khai báo thành token có giá trị riêng cho nền sáng và nền tối, để màu nền vẫn đọc rõ chữ. Màu từ color picker được lưu dạng hex, kèm tối đa 8 màu gần đây trong session. Khi tô, màu chữ được chọn tự động (sáng hoặc tối) theo độ tương phản với màu nền.

### 7. Mục Highlights thay cho mục Marker

`ContextSection::Marker` đổi thành `Highlights`. Mỗi rule là một dòng: ô màu (bấm để mở palette), `cột: giá trị, …`, số hàng khớp, công tắc, nút xóa, tay kéo để đổi thứ tự. Nút "+ Add rule" mở form chọn cột và giá trị, dùng lại UI chọn giá trị của Filter.

Menu chuột phải trên ô có hai nhóm:
- Nếu đang có vùng chọn: "Mark with color".
- "Highlight rows where <cột> = <giá trị>": tạo rule mới với màu kế tiếp trong palette. Không có trên TIMESTAMP; với MESSAGE thì dùng text của vùng chọn làm Custom Value.

## Risks / Trade-offs

- [Cột TIMESTAMP rộng hơn vì hiện cả ngày và múi giờ] → Dynamic Column Width (ADR-0003) tự co giãn; người dùng có thể kéo hẹp và cuộn ngang.
- [Dòng trống thành hàng và bỏ `sub_lines` làm đổi số hàng, ảnh hưởng chỉ số hàng hiển thị của Search, Filter và canvas] → Làm bước parser trước, chạy lại toàn bộ test Search và Filter trước khi đụng tới UI.
- [Filter Value của LEVEL tách `E` và `ERROR`] → Đã chốt theo rule 30. Level Marker vẫn dựa vào nghĩa, nên lướt vẫn thấy dòng lỗi.
- [Marker cũ bị mất khi nâng cấp] → Đã chấp nhận, cùng lần bump version session.
- [Hit-test ký tự sai với ký tự rộng như CJK hay emoji, vì monospace không bảo đảm mọi glyph cùng độ rộng] → Dùng kết quả shaping của gpui cho dòng đó thay vì chia đều. Nếu không lấy được thì chấp nhận lệch với glyph rộng.
- [Log có định dạng không đọc được timestamp (định dạng parser chưa biết) khiến cả file thành Continuation Line của một dòng đầu] → Chỉ dòng sau một dòng có timestamp mới là Continuation Line; file hoàn toàn không có timestamp thì mọi dòng đứng riêng. Định dạng lạ cần được thêm vào parser.
- [Bỏ module `app` và level đoán làm đổi Filter Value của file cũ] → Đây là chủ ý theo rule 30; Filter Value chỉ còn giá trị thật trong file.

## Migration Plan

Bump version session. Session cũ vẫn mở được, nhưng Marker theo dòng và `note` bị bỏ. Không có rollback riêng ngoài việc quay về bản trước.

## Open Questions

- Có cần phím tắt để tô vùng chọn bằng màu gần nhất không (ví dụ ⌘⇧H)?
