## Context

App hiện đọc log theo dòng.
- Line Index giữ vị trí từng dòng.
- Parsed View hiện mỗi dòng là một row với các ô là text đang lưu (ADR-0007).
- Filter, Search và Focus Pane làm việc trên các row đó.

OpenTelemetry lại mô tả dữ liệu theo **record** và **span**:
- một log record có severity, body (AnyValue), attributes, resource, scope và trace context;
- một span có thời điểm bắt đầu, kết thúc, span cha, status, events và links.

Log thật của người dùng chưa theo OTel. Người dùng sẽ tự viết adapter để gom **nhiều dòng** thành một span hoặc một record. Vì vậy ở đây một record không ứng với một dòng nào, và mô hình "1 dòng = 1 row" không dùng được.

Nghiên cứu nền: `docs/research/opentelemetry-logs-schema-and-viewer-ui.md`.
- §1–2: Log Data Model và cách mã hoá OTLP/JSON.
- §3: chuẩn không có dạng JSON phẳng mỗi dòng một record. Định dạng chuẩn là OTLP/JSON Lines, mỗi dòng một request.
- §5–6: các viewer khác đều có cột Timestamp · Severity · Service · Body · Trace; panel chi tiết chia nhóm Record → Trace → Exception → Resource → Scope; traceId dẫn sang trace.

## Goals / Non-Goals

**Goals:**
- Một schema input (OtelInput) đủ để dựng cả view log lẫn view trace. Adapter của người dùng chỉ phải tạo ra schema này.
- OTel View có hai chế độ: Logs, và Traces (Trace List, waterfall, flame chart).
- Có thể đi qua lại giữa log và trace, và từ bất kỳ phần tử nào về dòng gốc.
- Một bộ nạp OTLP/JSON và fixture để xem và test view trước khi có adapter thật.

**Non-Goals:**
- Adapter cho log không theo OTel. Người dùng tự viết sau.
- Đọc OTLP protobuf hoặc zstd, receiver OTLP, metrics.
- Dựng trace gần đúng từ log khi không có span.
- Critical path, tìm span theo attribute, so sánh hai trace.
- Cập nhật tăng dần cho Live Log Space: adapter chạy lại cả lượt (xem Open Questions).
- Dùng Filter, Search, Focus Pane hay Highlight Rule trong OTel View.

## Decisions

### 1. OTel View là chế độ thứ ba, không phải row ảo của Parsed View

`ViewMode` thêm `Otel` cạnh `Raw` và `Parsed`. OTel View có bảng, chart và bộ lọc riêng.

- **Phương án bị loại: Parsed View với row là record.** Phải đưa mức (line, record) vào Line Index, `view_rows::pack`, `LineMeta` và RowCache. Filter và Search phải hiểu một row không còn là một dòng. Một span thì không biểu diễn được bằng row trong bất kỳ trường hợp nào. Chi phí lớn mà vẫn phải viết một view riêng cho trace.
- **Hệ quả:** Filter, Search, Focus Pane và Highlight Rule không có trong OTel View; thay vào đó là Bộ lọc OTel tối giản. Đây là quy tắc 45 mới.

### 2. OtelInput là cấu trúc trong bộ nhớ, resource và scope được dùng chung

```rust
pub struct OtelInput { pub resources: Vec<Resource>, pub scopes: Vec<Scope>,
                       pub spans: Vec<OtelSpan>, pub logs: Vec<OtelRecord> }
pub struct Resource { pub attributes: Attrs, pub schema_url: Option<String> }
pub struct Scope { pub name: String, pub version: Option<String>, pub attributes: Attrs }
pub struct OtelSpan { pub trace_id: TraceId, pub span_id: SpanId, pub parent_span_id: Option<SpanId>,
    pub trace_state: Option<String>, pub name: String, pub kind: SpanKind,
    pub start_ns: u64, pub end_ns: u64, pub status: SpanStatus, pub attributes: Attrs,
    pub events: Vec<SpanEvent>, pub links: Vec<SpanLink>,
    pub resource: ResourceId, pub scope: ScopeId, pub source: Vec<LineRef> }
pub struct OtelRecord { pub time_ns: Option<u64>, pub observed_ns: Option<u64>,
    pub severity_number: Option<u8>, pub severity_text: Option<String>, pub body: Option<AnyValue>,
    pub attributes: Attrs, pub event_name: Option<String>,
    pub trace_id: Option<TraceId>, pub span_id: Option<SpanId>, pub flags: u32,
    pub resource: ResourceId, pub scope: ScopeId, pub source: Vec<LineRef> }
pub enum AnyValue { Str(String), Bool(bool), Int(i64), Double(f64), Bytes(Vec<u8>),
                    Array(Vec<AnyValue>), KvList(Attrs) }
pub type Attrs = Vec<(String, AnyValue)>;          // giữ thứ tự, cho phép trùng key như OTLP
pub struct LineRef { pub file: SourceFileId, pub generation: u32, pub lines: Range<u64> }
```

- `TraceId([u8;16])` và `SpanId([u8;8])` hiện ra dạng hex chữ thường. Một id toàn số 0 được coi là không có id (theo spec).
- `ResourceId` và `ScopeId` là chỉ số vào `resources` và `scopes`.
  - Mỗi span hay record không mang một bản resource riêng: hàng nghìn record của cùng một service chỉ dùng chung một Resource.
  - Hai resource có cùng attributes và schema_url được gộp làm một khi các OtelInput được gộp lại.
- `source` bắt buộc khác rỗng. Một phần tử không có source thì bị loại và được đếm vào cảnh báo của adapter.
- **Phương án bị loại: dùng chính struct của OTLP (`resource_logs → scope_logs → log_records`).** Lồng như vậy khó lọc và khó gộp. Hơn nữa adapter của người dùng không có lý do gì để dựng request OTLP.
- **Phương án bị loại: ghi OtelInput ra file chuẩn hoá trên đĩa.** Adapter phải ghi file vào thư mục dữ liệu của app, và phải dọn file đó khi file gốc đổi. Người dùng đã chọn để adapter tự lo chuyện bộ nhớ.

### 3. Ô hiện giá trị, không phải text đang lưu; mọi phần tử dẫn về dòng gốc

OTel View hiện giá trị của OtelInput: Timestamp đã định dạng, body chuyển thành chuỗi, attribute chuyển thành chuỗi. Đây là ngoại lệ có chủ ý của ADR-0007, ghi thành quy tắc 46 và ADR-0016.
- Marker không đặt được trong OTel View.
- Nút **Xem dòng gốc** thay cho chuyện "ô là text": nó chuyển Log Space sang Parsed View, chọn dòng đầu của `source[0]`, và theo rule 41 không thu hẹp gì.
- Nếu `source` có nhiều khoảng dòng, nút này có menu liệt kê từng khoảng (`file:dòng–dòng`).

### 4. Adapter: trait, chạy nền, gộp theo Log Space

```rust
pub trait OtelAdapter: Send + Sync {
    fn name(&self) -> &str;
    fn build(&self, files: &[AdapterFile], progress: &Progress, cancel: &CancelToken) -> OtelInput;
}
```

- `AdapterFile` cho adapter đọc file qua cùng đường đọc mà app đang dùng: đọc dòng theo Line Index, có Generation, chạy được với Archive Entry và Remote Copy. Nhờ đó `LineRef` luôn khớp với số dòng hiện ở Parsed View.
- Danh sách adapter được đăng ký tĩnh trong `src/otel/adapter.rs`. Mỗi adapter nhận **mọi file** của Log Space, vì một span có thể nằm ở một file và log của nó ở file khác. Adapter nào không nhận ra định dạng thì trả OtelInput rỗng.
- App chạy mọi adapter trên executor nền (ADR-0014): khi mở Log Space, khi một file được thêm hoặc gỡ, khi file đổi, và khi Indexing xong.
  - Kết quả được gộp: nối `spans` và `logs`, dồn và gộp `resources`/`scopes`.
  - Lượt chạy trước bị huỷ nếu một lượt mới bắt đầu.
  - Tiến độ hiện ở status bar như một Scan.
- **Phương án bị loại: chương trình ngoài, đọc OTLP/JSON từ stdout.** Phải spawn process, lo vấn đề sandbox trên Windows, và phải tự ánh xạ `LineRef`. Nếu người dùng cần cách này thì có thể viết một adapter gọi chương trình ngoài.
- **Phương án bị loại: chọn adapter theo Log Format.** ADR-0015 giữ Log Format cho việc định nghĩa cột. Adapter làm một việc khác: gom nhiều dòng.

### 5. Bộ nạp OTLP/JSON là adapter đầu tiên

Bộ nạp đọc từng dòng.
- Dòng mở đầu bằng `{"resourceLogs"` hoặc `{"resourceSpans"` (sau khi bỏ khoảng trắng) được parse bằng các struct `serde` riêng, theo §2 của bản nghiên cứu:
  - key camelCase;
  - int64 nhận cả chuỗi lẫn số;
  - trace/span id dạng hex;
  - enum là số;
  - `bytesValue` dạng base64;
  - key không biết thì bỏ qua.
- Mọi phần tử trong một dòng nhận cùng `LineRef` là dòng đó. Dòng parse lỗi được bỏ qua và đếm vào cảnh báo.
- Một file không có dòng OTLP nào thì cho OtelInput rỗng, nên nút OTel không hiện vì file đó.
- Không cần dependency mới: `serde_json` đã có. Base64 được giải mã bằng một hàm nhỏ trong module.

### 6. Severity, thời gian, sắp xếp

- **Tên ngắn** suy từ `severity_number`: 1–4 TRACE, 5–8 DEBUG, 9–12 INFO, 13–16 WARN, 17–20 ERROR, 21–24 FATAL, có hậu tố 2–4 trong mỗi nhóm.
  - Nếu không có số, app đọc `severity_text` bằng bộ nhận level đang có của Parsed View.
- **Màu:** từ 17 trở lên dùng màu lỗi (FATAL dùng chung màu với ERROR); 13–16 màu cảnh báo; dưới 9 màu mờ; còn lại màu chữ thường.
- **Cột Severity** hiện đúng `severity_text`; trống thì để trống. OTel Detail hiện `TÊN (text) · số`.
- **Thời gian** hiện theo UTC dạng `YYYY-MM-DDTHH:MM:SS.mmmZ`. Cột Timestamp hiện `HH:MM:SS.mmm` và có tooltip ngày đầy đủ.
  - Record không có `time_ns` thì dùng `observed_ns`.
  - Record không có cả hai thì xếp cuối và hiện `—`.
- **Thứ tự bảng Logs:** theo thời gian. Hai record cùng thời gian giữ thứ tự của `source` (file trong Log Space, rồi dòng).

### 7. Dựng trace

- Span được nhóm theo `trace_id`. Trong mỗi trace, cây được dựng theo `parent_span_id`; các span con xếp theo `start_ns`.
- **Span mồ côi** (span cha không có trong OtelInput) được coi là gốc và có nhãn "thiếu span cha". Một trace có thể có nhiều gốc.
- Mỗi dòng của Trace List:
  - root service · span: gốc sớm nhất;
  - bắt đầu: `start_ns` nhỏ nhất;
  - thời lượng: `end_ns` lớn nhất trừ `start_ns` nhỏ nhất;
  - số span, và số span có `status = Error`.
- Danh sách xếp theo thời điểm bắt đầu, mới nhất ở trên.
- Log thuộc span: các record có cùng `trace_id` và `span_id`. Chỉ mục traceId → spans và (traceId, spanId) → records được dựng một lần sau khi gộp.

### 8. Vẽ Trace Chart bằng phần tử paint riêng, chỉ vẽ phần nhìn thấy

- **Waterfall:**
  - cột trái là cây span (thụt lề theo độ sâu, chevron để thu gọn);
  - cột phải là thanh span, đặt theo tỉ lệ thời gian trong cửa sổ phóng to `[a, b]`;
  - các row được ảo hoá theo cách `uniform_list` của GPUI đang dùng cho canvas.
- **Flame chart:**
  - mỗi độ sâu là một hàng, mỗi span là một khối;
  - chữ trong khối được cắt cho vừa;
  - khối hẹp dưới 2px không vẽ chữ;
  - các khối liền nhau hẹp dưới 1px được gộp thành một dải.
- **Minimap:** vẽ toàn bộ trace thu nhỏ. Kéo chuột trên minimap để đặt `[a, b]`; nút "Bỏ phóng to" trả về toàn trace.
- **Màu và chấm:**
  - màu theo `service.name`, lấy từ Highlight Palette theo hash ổn định của tên;
  - span có `status = Error` có viền màu lỗi;
  - log của span là chấm tròn tại `time_ns`, màu theo severity. Bấm chấm thì chuyển sang chế độ Logs và chọn record đó.
- Hai dạng chart dùng chung một lớp tính layout trên cây đã thu gọn. Dạng chart và trạng thái thu gọn được nhớ cho từng trace đang mở.

### 9. Bộ lọc OTel

- **Severity tối thiểu:** so theo số. Record không có số và không đọc được text chỉ qua khi đặt "Mọi severity".
- **Service:** so với `service.name` của resource. Danh sách chọn lấy từ các resource có trong OtelInput.
- **Chữ trong body:** chứa chuỗi đó, không phân biệt hoa thường, trên body đã chuyển thành chuỗi.
- **TraceId:** khớp đúng; dán vào 7 ký tự đầu trở lên cũng nhận theo tiền tố.

Ở chế độ Traces, service và traceId lọc Trace List: một trace qua khi có ít nhất một span khớp. Severity và chữ trong body chỉ lọc bảng Logs và các chấm log trên chart.

Bấm traceId ở bảng Logs:
- nếu có span của trace đó, mở trace trong chế độ Traces;
- nếu không, đặt bộ lọc TraceId.

Nút Copy luôn có.

### 10. OTel Detail

Panel bên phải, rộng thay đổi được, dùng chung cho record và span. Mỗi nhóm có số đếm và có thể gập lại. Resource và Scope gập sẵn.

- **Record:** Record (severity, time, observed time, event name, body, attributes) · Trace (trace id, span id, flags, đều có copy và link) · Exception · Resource · Scope.
  - Exception chỉ có khi có attribute `exception.*`. Các attribute này được chuyển khỏi nhóm Record sang đây; stacktrace hiện monospace, nhiều dòng.
- **Span:** tên, kind, status, thời lượng, độ lệch so với đầu trace · Attributes · Events (có Exception nếu event tên `exception`) · Links (bấm để mở span đích nếu có) · Logs (bấm để nhảy) · Trace · Resource · Scope.
- **Body và giá trị lồng:** chuỗi hiện nguyên văn và giữ xuống dòng. Kvlist và mảng hiện dạng cây, mở sẵn khi có tối đa 10 khoá (giống Jaeger, theo §5.5). Bytes hiện hex.
- Cuối panel luôn có **Xem dòng gốc** (quyết định 3).

### 11. Trạng thái được nhớ

Lưu vào session theo Log Space, như Filter (rule 20):
- view mode `Otel`, chế độ Logs/Traces;
- bộ lọc OTel;
- các cột đã thêm;
- trace đang mở, dạng chart, cửa sổ phóng to, các nhánh đã thu gọn;
- phần tử đang chọn.

Nếu sau khi adapter chạy lại mà trace hoặc phần tử đã nhớ không còn, view quay về Trace List hoặc bỏ chọn, không báo lỗi.

### 12. Fixture

`scripts/gen_otel_fixture.py`, không cần thư viện ngoài, sinh vào `tests/fixtures/otel/`:
- `spans.otlp.jsonl`: 2 trace, gồm 1 trace lỗi có 7 span qua 3 service và 1 span mồ côi.
- `logs.otlp.jsonl`: khoảng 30 record, gồm:
  - đủ 6 nhóm severity, có record thiếu `severityText`;
  - body kiểu kvlist;
  - exception;
  - một record không có trace;
  - một dòng chứa batch nhiều record.

File được commit, nên test không phải chạy script.

## Risks / Trade-offs

- **[OtelInput lớn làm tốn bộ nhớ]** → View không đặt giới hạn, theo quyết định của người dùng. Bảng Logs và chart chỉ vẽ phần nhìn thấy. OtelInput của tab Dormant được giải phóng (rule 44).
- **[Adapter chạy lại cả lượt mỗi khi file đổi, chậm với Live Log Space]** → Gộp các lần đổi file trong 1 giây thành một lượt và huỷ lượt cũ. Việc chạy tăng dần để ở Open Questions.
- **[Trace có hàng chục nghìn span khiến layout chậm]** → Layout chỉ tính lại khi thu gọn, phóng to hoặc lọc. Flame chart gộp các khối hẹp dưới 1px.
- **[Người dùng quen dùng Filter, Search và Marker có thể thấy thiếu trong OTel View]** → Nút Xem dòng gốc đưa về Parsed View, nơi có đủ các công cụ đó. Quy tắc 45 và 46 ghi rõ ranh giới này.
- **[Hai adapter cùng nhận một file sinh ra phần tử trùng]** → Không tự loại trùng. Status bar cảnh báo khi có cặp (trace_id, span_id) xuất hiện hai lần, và OTel Detail ghi tên adapter đã tạo ra phần tử.

## Migration Plan

Không cần migrate. Session cũ không có trạng thái OTel thì mở như trước. Nếu session lưu `ViewMode::Otel` nhưng Log Space không còn OtelInput, view quay về Parsed View.

## Open Questions

- Live Log Space: có nên cho adapter một API tăng dần (chỉ nhận các dòng mới) không? Để tới khi người dùng viết adapter thật.
- Có cần chọn hay tắt từng adapter cho một Log Space không? Hiện tại mọi adapter đều chạy.
