## ADDED Requirements

### Requirement: OtelInput theo mô hình OpenTelemetry
Hệ thống SHALL định nghĩa OtelInput gồm danh sách Resource, Scope, OtelSpan và OtelRecord theo OTel Log Data Model và Trace Data Model.
- OtelRecord SHALL có các trường: time, observed time, severity number, severity text, body, attributes, event name, trace id, span id, flags, resource, scope.
- OtelSpan SHALL có các trường: trace id, span id, parent span id, trace state, name, kind, start, end, status (code, message), attributes, events, links, resource, scope.
- Body và giá trị attribute SHALL là AnyValue: chuỗi, bool, int, double, bytes, mảng hoặc kvlist.
- Trace id và span id toàn số 0 SHALL được coi là không có.

#### Scenario: Record tối thiểu
- **WHEN** adapter tạo một OtelRecord chỉ có body "started", resource và source
- **THEN** record đó hợp lệ và hiện trong OTel View với Timestamp `—` và cột Severity trống

#### Scenario: Span id toàn số 0
- **WHEN** một record có span id `0000000000000000`
- **THEN** record được coi là không thuộc span nào

### Requirement: Mọi span và record dẫn về dòng gốc
Mỗi OtelSpan và OtelRecord SHALL mang `source` gồm ít nhất một LineRef (Source File, Generation, khoảng dòng). Phần tử có `source` rỗng SHALL bị loại và được tính vào số cảnh báo của adapter.

#### Scenario: Record gom từ nhiều dòng
- **WHEN** adapter gom dòng 405–409 của `payment.log` thành một record
- **THEN** source của record là `payment.log` dòng 405–409

#### Scenario: Thiếu source
- **WHEN** adapter trả một span không có source
- **THEN** span đó không hiện trong OTel View và status bar báo 1 cảnh báo của adapter

### Requirement: Resource và Scope dùng chung
Span và record SHALL trỏ tới Resource và Scope bằng chỉ số. Khi các OtelInput được gộp, hai Resource có cùng attributes và schema URL SHALL được gộp làm một, và hai Scope có cùng name, version và attributes SHALL được gộp làm một.

#### Scenario: Hai adapter cùng service
- **WHEN** hai adapter cùng trả Resource `service.name=payment` với attributes giống hệt nhau
- **THEN** OtelInput sau khi gộp có một Resource `payment` duy nhất

### Requirement: Adapter dựng OtelInput từ mọi file của Log Space
Hệ thống SHALL có trait OtelAdapter nhận mọi file của một Log Space và trả về một OtelInput. Adapter SHALL đọc file qua đường đọc của app, để số dòng trong LineRef khớp với Parsed View, kể cả với Archive Entry, Remote Copy và các Generation. Adapter không nhận ra định dạng SHALL trả OtelInput rỗng.

#### Scenario: Span và log ở hai file
- **WHEN** Log Space có `spans.otlp.jsonl` và `logs.otlp.jsonl`
- **THEN** OtelInput của Log Space có span từ file đầu và record từ file sau, và chấm log hiện trên đúng span

#### Scenario: File không theo OTel
- **WHEN** Log Space chỉ có `app.log` dạng text thường
- **THEN** OtelInput rỗng và nút OTel không hiện

### Requirement: Adapter chạy nền và có thể huỷ
Hệ thống SHALL chạy mọi adapter đã đăng ký ở nền, có tiến độ ở status bar, và có thể huỷ.
- Adapter SHALL chạy khi mở Log Space, khi Indexing xong, khi thêm hoặc gỡ file, và khi file thay đổi.
- Các lần thay đổi file cách nhau dưới 1 giây SHALL được gộp thành một lượt.
- Lượt mới SHALL huỷ lượt đang chạy.
- View SHALL vẫn đọc và cuộn được trong lúc adapter chạy.

#### Scenario: File thay đổi khi adapter đang chạy
- **WHEN** adapter đang chạy và một file của Log Space được ghi thêm
- **THEN** lượt đang chạy bị huỷ, một lượt mới bắt đầu, và OTel View giữ kết quả cũ cho tới khi lượt mới xong

#### Scenario: Huỷ
- **WHEN** người dùng bấm huỷ trên tiến độ của adapter
- **THEN** adapter dừng, và OTel View giữ OtelInput của lượt xong gần nhất

### Requirement: Bộ nạp OTLP/JSON
Hệ thống SHALL có sẵn một adapter đọc OTLP/JSON Lines: mỗi dòng là một request `{"resourceLogs":[…]}` hoặc `{"resourceSpans":[…]}`. Bộ nạp SHALL:
- nhận key camelCase;
- nhận int64 ở dạng chuỗi lẫn dạng số;
- nhận trace id và span id dạng hex;
- nhận enum dạng số;
- nhận `bytesValue` dạng base64;
- bỏ qua key không biết.

Mọi phần tử trong một dòng SHALL có source là dòng đó. Dòng parse lỗi SHALL bị bỏ qua và được tính vào số cảnh báo.

#### Scenario: Batch nhiều record trong một dòng
- **WHEN** dòng 12 chứa một request có 3 logRecords
- **THEN** OtelInput có 3 record, cả ba đều có source là dòng 12

#### Scenario: timeUnixNano dạng chuỗi
- **WHEN** một record có `"timeUnixNano":"1730275200502000000"`
- **THEN** cột Timestamp hiện `08:00:00.502`, và ngày đầy đủ là `2024-10-30T08:00:00.502Z`

#### Scenario: Dòng hỏng
- **WHEN** một dòng mở đầu bằng `{"resourceLogs"` nhưng JSON bị cắt dở
- **THEN** dòng đó bị bỏ qua, các dòng khác vẫn được nạp, và status bar báo 1 cảnh báo

### Requirement: Fixture OTel
Repo SHALL có fixture OTLP/JSON trong `tests/fixtures/otel/` và script sinh ra nó. Fixture SHALL có:
- ít nhất 2 trace qua 3 service, trong đó có 1 trace lỗi và 1 span mồ côi;
- record thuộc đủ 6 nhóm severity;
- record thiếu severity text;
- body kiểu kvlist;
- exception;
- record không có trace;
- một dòng chứa nhiều record.

#### Scenario: Mở fixture
- **WHEN** người dùng tạo Static Log Space từ hai file trong `tests/fixtures/otel/`
- **THEN** nút OTel hiện, Trace List có 2 trace, và chế độ Logs có đủ 6 mức severity
