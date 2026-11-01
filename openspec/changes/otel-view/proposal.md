## Why

Các service đang chuyển sang OpenTelemetry, nhưng app chỉ đọc log theo dòng nên không có cách xem một log theo đúng mô hình OTel (severity, resource, scope, attributes, trace context). App cũng không xem được trace: không có cây span, không có waterfall hay flame chart. Log OTel thật của người dùng chưa có dạng cố định. Vì vậy change này định nghĩa trước **schema input** và **view**. Người dùng sẽ tự viết adapter sau: adapter gom nhiều dòng log thành schema đó để ra được view.

Nghiên cứu nền: `docs/research/opentelemetry-logs-schema-and-viewer-ui.md` (Log Data Model, OTLP/JSON, cách Aspire, Grafana, SigNoz, otel-tui và Jaeger hiển thị log và trace).

## What Changes

- **OtelInput**: schema input trong bộ nhớ, đi theo OTel Log Data Model và Trace Data Model.
  - Gồm `resources`, `scopes`, `spans: Vec<OtelSpan>` và `logs: Vec<OtelRecord>`.
  - Mỗi span và mỗi record **bắt buộc** mang `source`, là danh sách các dòng gốc (Source File, Generation, khoảng dòng) mà adapter đã gom để tạo ra nó.
  - Body và giá trị attribute là `AnyValue`: chuỗi, bool, int, double, bytes, mảng, kvlist.
- **OtelAdapter**: trait trong code, `build(files, progress, cancel) -> OtelInput`.
  - Chạy nền, có tiến độ và có thể huỷ. Chạy khi mở Log Space và khi file của nó thay đổi.
  - Mọi adapter đăng ký chạy trên mọi file của Log Space, rồi kết quả được gộp lại thành một OtelInput.
  - Bản cài đặt đầu tiên là **bộ nạp OTLP/JSON** chuẩn (`resourceLogs` / `resourceSpans`, mỗi dòng một request). Bộ nạp này dùng cho fixture và cho file OTLP do Collector ghi ra.
- **OTel View**: chế độ thứ ba cạnh Raw và Parsed, tính cho cả Log Space. Nút OTel chỉ hiện khi OtelInput của Log Space có ít nhất một span hoặc record. View có:
  - **Bộ lọc OTel** riêng: severity tối thiểu, service, chữ trong body và traceId. Filter, Search, Focus Pane và Highlight Rule của app không áp dụng trong OTel View.
  - **Chế độ Logs**:
    - Bảng có 5 cột cố định: Timestamp (UTC, đến mili giây) · Severity · Service · Body · Trace (7 ký tự đầu).
    - Có nút thêm cột từ bất kỳ key attribute nào, kể cả `resource.<key>` và `scope.name` / `scope.version`.
    - Severity hiện đúng `severityText`, còn màu lấy theo `severityNumber`.
  - **Chế độ Traces**:
    - Mở đầu bằng **Trace List** (root service · span, thời điểm bắt đầu, thời lượng, số span, số span lỗi).
    - Chọn một trace để mở **Trace Chart** dạng **waterfall** hoặc **flame chart**, có nút chuyển giữa hai dạng.
    - Thao tác trên chart: chọn span, thu gọn nhánh, phóng to theo thời gian bằng minimap.
    - Màu theo service, span lỗi có viền đỏ. Log của span hiện thành chấm trên thanh span; bấm chấm thì nhảy sang chế độ Logs.
  - **OTel Detail**:
    - Là panel bên phải, dùng chung cho record và span.
    - Với record: Record · Trace · Exception · Resource · Scope.
    - Với span: Attributes · Events · Links · Logs · Trace · Resource · Scope.
    - Có nút **Xem dòng gốc** để nhảy về các dòng `source` trong Parsed View.
  - Trạng thái của OTel View được nhớ theo Log Space như Filter (rule 20): chế độ, bộ lọc, trace đang mở, dạng chart, khoảng phóng to và các cột đã thêm.
- **Fixture**: script sinh file OTLP/JSON mẫu gồm nhiều service, span lồng nhau, span lỗi, exception, đủ các mức severity, body kiểu kvlist, và một log không thuộc trace nào. Fixture dùng cho test và để xem thử view.
- Không có thay đổi nào là **BREAKING**: Raw View và Parsed View giữ nguyên với mọi Log Space.

## Domain changes

Khi áp dụng change này, cập nhật CONTEXT.md như sau.

**Thuật ngữ mới (nhóm Views):**
- **OTel View**: chế độ thứ ba của một Log Space, cạnh Raw View và Parsed View. Nó hiện OtelInput của Log Space ở hai chế độ, Logs và Traces, với Bộ lọc OTel và OTel Detail riêng. _Avoid_: Telemetry tab, trace tab.
- **OtelInput**: các span và log record theo mô hình OpenTelemetry mà adapter dựng từ các file của một Log Space. Mỗi phần tử trỏ về các dòng gốc của nó. _Avoid_: OTel data, telemetry dataset.
- **Adapter**: đoạn code đọc các file của một Log Space và dựng OtelInput, có thể gom nhiều dòng thành một span hoặc một record. _Avoid_: Parser (Parser đọc từng dòng cho Parsed View), converter.
- **Trace List**: danh sách các trace trong OtelInput ở chế độ Traces. _Avoid_: Trace search.
- **Trace Chart**: một trace vẽ theo thời gian, dạng waterfall (cây span cạnh các thanh) hoặc flame chart (các khối xếp theo độ sâu). _Avoid_: Gantt, timeline (Timeline là Merged Timeline).
- **OTel Detail**: panel bên phải của OTel View, hiện một record hoặc một span theo nhóm. _Avoid_: Inspector (Inspector hiện dòng thô).

**Quy tắc mới:**
- **45. OTel View Has Its Own Filter**: Filter, Search Expression, Focus Pane và Highlight Rule không áp dụng trong OTel View. Chỉ Bộ lọc OTel thu hẹp các record và trace mà view hiện.
- **46. OTel Shows Values, Raw Shows Lines**: OTel View hiện giá trị của OtelInput, không phải text đang lưu (ngoại lệ có chủ ý của ADR-0007). Marker không đặt được trong OTel View. Mọi span và record đều dẫn về dòng gốc của nó.
- **47. OtelInput Lives In Memory**: OtelInput nằm trong bộ nhớ (ngoại lệ của rule 43). View không giới hạn kích thước; giới hạn là việc của Adapter.

**ADR:** thêm ADR 0016 "OTel View Is A Third View Over Adapter-Built Records" (xem design.md, quyết định 1–3).

## Capabilities

### New Capabilities
- `otel-input`: schema OtelInput (Resource, Scope, OtelSpan, OtelRecord, AnyValue, LineRef), trait OtelAdapter, cách adapter chạy và gộp kết quả theo Log Space, bộ nạp OTLP/JSON, fixture.
- `otel-view`: chế độ OTel, Bộ lọc OTel, chế độ Logs, Trace List, Trace Chart (waterfall, flame, minimap, thu gọn, chấm log), OTel Detail, Xem dòng gốc, trạng thái được nhớ.

### Modified Capabilities
- `raw-view`: requirement "Chuyển giữa Raw View và Parsed View" thêm chế độ OTel thứ ba, chỉ hiện khi Log Space có OtelInput.

## Impact

- **Code mới:**
  - `src/otel/`: `model.rs` (schema), `adapter.rs` (trait, chạy nền, gộp), `otlp_json.rs` (bộ nạp), `query.rs` (lọc, Trace List, cây span).
  - `src/ui/otel/`: `logs_table.rs`, `trace_list.rs`, `trace_chart.rs`, `minimap.rs`, `detail.rs`, `filter_bar.rs`.
- **Code sửa:**
  - `src/workspace/working_set.rs`: thêm `ViewMode::Otel`.
  - `src/app/raw.rs`: nút chuyển chế độ.
  - `src/workspace/session.rs`: lưu trạng thái OTel View.
  - `src/app/`: chạy adapter khi mở Log Space và khi file đổi; nhảy về dòng gốc.
- **Dependency:** không thêm; `serde_json` đã có. OTLP/JSON được đọc bằng `serde` derive trên struct riêng, không dùng crate `opentelemetry-proto`.
- **Fixture và script:** `tests/fixtures/otel/` và `scripts/gen_otel_fixture.*`.
- **Bộ nhớ:** OtelInput của các tab Dormant được giải phóng như các dòng của tab đó (rule 44), và được dựng lại khi tab được chọn lại.
