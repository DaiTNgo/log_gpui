## 1. Schema OtelInput

- [ ] 1.1 Tạo module `src/otel/` với `model.rs`: `OtelInput`, `Resource`, `Scope`, `OtelSpan`, `OtelRecord`, `SpanKind`, `SpanStatus`, `SpanEvent`, `SpanLink`, `AnyValue`, `Attrs`, `LineRef`, `TraceId`/`SpanId` (hiện hex, id toàn số 0 coi là không có)
- [ ] 1.2 Viết hàm gộp nhiều OtelInput: nối spans và logs, gộp Resource/Scope trùng, đánh lại chỉ số; loại phần tử có `source` rỗng và đếm cảnh báo; test đơn vị
- [ ] 1.3 Viết hàm severity: tên ngắn theo số (TRACE…FATAL4), dùng text khi không có số (bằng bộ nhận level đang có), phân loại màu (lỗi ≥17, cảnh báo 13–16, mờ <9); test đơn vị
- [ ] 1.4 Viết hàm định dạng thời gian UTC (`HH:MM:SS.mmm` và RFC3339 đến mili giây), fallback sang observed time; chuyển AnyValue thành chuỗi gọn (kvlist/mảng ra JSON gọn, bytes ra hex); test đơn vị

## 2. Adapter và bộ nạp OTLP/JSON

- [ ] 2.1 `src/otel/adapter.rs`: trait `OtelAdapter`, `AdapterFile` (đọc dòng qua đường đọc của app, có Generation, chạy được với Archive Entry và Remote Copy), `Progress`, `CancelToken`, danh sách adapter đăng ký tĩnh
- [ ] 2.2 `src/otel/otlp_json.rs`: struct serde cho `resourceLogs`/`resourceSpans` (camelCase, int64 dạng chuỗi hoặc số, id hex, enum số, `bytesValue` base64, bỏ qua key lạ); mỗi phần tử có `LineRef` là dòng của nó; dòng lỗi bị bỏ qua và đếm cảnh báo
- [ ] 2.3 Test bộ nạp với ví dụ hai record trong bản nghiên cứu, một dòng batch nhiều record, timeUnixNano dạng chuỗi và dạng số, dòng hỏng, file không có OTLP (cho OtelInput rỗng)
- [ ] 2.4 Trong `src/app/`, chạy adapter nền cho Log Space đang mở (ADR-0014): khi mở, khi Indexing xong, khi thêm hoặc gỡ file, khi file đổi (gộp các lần đổi trong 1 giây); lượt mới huỷ lượt cũ; tiến độ và cảnh báo hiện ở status bar; giữ OtelInput của lượt xong gần nhất
- [ ] 2.5 Giải phóng OtelInput khi tab thành Dormant, dựng lại khi tab được chọn lại

## 3. Fixture

- [ ] 3.1 `scripts/gen_otel_fixture.py`, chỉ dùng thư viện chuẩn, sinh `tests/fixtures/otel/spans.otlp.jsonl` và `logs.otlp.jsonl` theo design §12
- [ ] 3.2 Commit fixture; thêm test tích hợp: nạp hai file thì có 2 trace, đủ 6 nhóm severity, 1 span mồ côi, chấm log gắn đúng span

## 4. Truy vấn trên OtelInput

- [ ] 4.1 `src/otel/query.rs`: chỉ mục traceId → spans và (traceId, spanId) → records; dựng cây span theo parent, đánh dấu span mồ côi là gốc; các dòng của Trace List (gốc, bắt đầu, thời lượng, số span, số lỗi), xếp mới nhất trước
- [ ] 4.2 Bộ lọc OTel: severity tối thiểu, service, chữ trong body, traceId (khớp đúng hoặc tiền tố ≥7); áp dụng cho bảng Logs, Trace List và chấm log theo design §9; test đơn vị
- [ ] 4.3 Thứ tự bảng Logs theo thời gian, cùng thời gian thì giữ thứ tự source, không có thời gian thì xếp cuối; test đơn vị

## 5. Chế độ OTel và trạng thái

- [ ] 5.1 Thêm `ViewMode::Otel` trong `src/workspace/working_set.rs`; nút chuyển ba lựa chọn chỉ hiện ở Log Space có OtelInput khác rỗng, không hiện trong File View (`src/app/raw.rs`); quay về Parsed khi OtelInput mất
- [ ] 5.2 State của OTel View: chế độ con Logs/Traces, bộ lọc, cột thêm, trace đang mở, dạng chart, cửa sổ phóng to, nhánh thu gọn, phần tử đang chọn
- [ ] 5.3 Lưu và khôi phục state đó qua `src/workspace/session.rs`; về Trace List hoặc bỏ chọn khi trace hay phần tử không còn
- [ ] 5.4 Thanh Bộ lọc OTel (`src/ui/otel/filter_bar.rs`) cùng nút chuyển Logs/Traces có số đếm, và tắt chế độ không có dữ liệu

## 6. Chế độ Logs

- [ ] 6.1 `src/ui/otel/logs_table.rs`: bảng ảo hoá gồm Timestamp · Severity · Service · Body · Trace, màu theo severity, tooltip ngày đầy đủ
- [ ] 6.2 Nút thêm và bỏ cột từ key attribute, `resource.<key>`, `scope.name` và `scope.version`
- [ ] 6.3 Bấm trace id: mở trace nếu có span, nếu không thì đặt bộ lọc traceId; nút Copy chép đủ hex
- [ ] 6.4 Chọn chữ và Copy chép giá trị đang hiện, không hiện thanh Marker

## 7. Chế độ Traces

- [ ] 7.1 `src/ui/otel/trace_list.rs`: Trace List, chọn một row để mở chart, nút quay về danh sách
- [ ] 7.2 Lớp layout dùng chung cho hai dạng chart: cây đã thu gọn, cửa sổ thời gian, vị trí các thanh; test đơn vị cho việc cắt ở mép cửa sổ
- [ ] 7.3 `src/ui/otel/trace_chart.rs` dạng Waterfall: cột cây (thụt lề, chevron, nhãn "thiếu span cha"), thanh span màu theo service (Highlight Palette, hash ổn định), viền lỗi, số mili giây, row ảo hoá
- [ ] 7.4 Dạng Flame chart: hàng theo độ sâu, khối có tên và thời lượng, gộp các khối hẹp dưới 1px, nút chuyển giữ nguyên lựa chọn và nhánh thu gọn
- [ ] 7.5 `src/ui/otel/minimap.rs`: vẽ toàn trace, kéo để đặt cửa sổ (bỏ qua đoạn kéo dưới 1%), nút "Bỏ phóng to"
- [ ] 7.6 Chấm log trên thanh span theo severity, áp bộ lọc severity và chữ; bấm chấm thì sang Logs, chọn và cuộn tới record

## 8. OTel Detail

- [ ] 8.1 `src/ui/otel/detail.rs`: panel bên phải đổi được độ rộng, các nhóm gập được có số đếm (Resource và Scope gập sẵn)
- [ ] 8.2 Detail của record: Record, Trace (copy và link), Exception (chuyển `exception.*` sang, stacktrace monospace), Resource, Scope
- [ ] 8.3 Detail của span: tên, kind, status, thời lượng, độ lệch; Attributes, Events (exception), Links (mở span đích), Logs (nhảy tới record), Trace, Resource, Scope
- [ ] 8.4 Hiện AnyValue dạng cây (mở sẵn khi tối đa 10 khoá), bytes ra hex, chuỗi giữ xuống dòng
- [ ] 8.5 Nút "Xem dòng gốc": chuyển sang Parsed View và chọn dòng đầu của source (rule 41); có menu khi source có nhiều khoảng

## 9. Domain docs và kiểm tra

- [ ] 9.1 Cập nhật `CONTEXT.md`: thêm các thuật ngữ OTel View, OtelInput, Adapter, Trace List, Trace Chart, OTel Detail và quy tắc 45–47 (theo proposal)
- [ ] 9.2 Viết `docs/adr/0016-otel-view-is-a-third-view-over-adapter-built-records.md` (theo design, quyết định 1–3)
- [ ] 9.3 `cargo test` và `cargo clippy` sạch; chạy app với fixture và kiểm tra bằng tay từng scenario của spec `otel-view` (chuyển dạng chart, phóng to, chấm log, Xem dòng gốc, nhớ trạng thái khi mở lại Project)
