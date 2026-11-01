# OpenTelemetry logs: the standard schema and how existing viewers show it

## Question

We want OpenTelemetry (OTel) log support in this viewer. The UI comes first, built against a schema we take from the OTel standard; a parser comes later. This note answers two questions:

1. **The schema.** What does the OTel standard define for a log record: Log Data Model fields, OTLP/JSON encoding, the on-disk formats that Collector and SDK exporters write, and the semantic-convention attributes worth surfacing? Covers whether each is required and how stable it is.
2. **UI conventions.** How do existing OTel log viewers (.NET Aspire dashboard, Grafana Explore + Loki, SigNoz, otel-tui, Jaeger) lay out the list, show severity, group the detail panel, render nested values, filter by attribute, and link logs to traces?

Conventions: the sources were read from each repo's `main` branch on 2026-10-30, so line numbers refer to that day's `main`. At that time the latest tags were opentelemetry-specification v1.62.0, opentelemetry-proto v1.11.1 and semantic-conventions v1.44.0, from the GitHub releases API. Short repo prefixes are used:

- `SPEC/` = https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/
- `PROTO/` = https://github.com/open-telemetry/opentelemetry-proto/blob/main/
- `SEMCONV/` = https://github.com/open-telemetry/semantic-conventions/blob/main/docs/
- `ASPIRE/` = https://github.com/dotnet/aspire/blob/main/src/Aspire.Dashboard/
- `TUI/` = https://github.com/ymtdzzz/otel-tui/blob/main/tuiexporter/internal/
- `SIGNOZ/` = https://github.com/SigNoz/signoz/blob/main/frontend/src/
- `JAEGER/` = https://github.com/jaegertracing/jaeger-ui/blob/main/packages/jaeger-ui/src/components/TracePage/TraceTimelineViewer/SpanDetail/

Claims that I could not trace to a primary source are marked **UNVERIFIED**.

## TL;DR

- **Every Log Data Model field is optional.** The data model document is **Stable**. `EventName` was promoted from the `event.name` attribute to a top-level field in Dec 2024 (spec PR #4320) and stabilized in Apr 2025 (PR #4475). The `event.name` attribute is now **deprecated** in semconv. (`SPEC/logs/data-model.md` L8, L180-451; commit history of that file)
- **Severity has two fields.** `SeverityNumber` takes 1-24, in six ranges of four (TRACE, DEBUG, INFO, WARN, ERROR, FATAL), and 0 means unspecified. `SeverityText` is the source's original string. The spec recommends displaying **`SHORT (SeverityText)`**, e.g. `INFO (Informational)`, putting short names in filter dropdowns, comparing by number, and treating values of 17 and above as errors. **The spec defines no colors**; it only says UIs "may display such errors in a different color". (`SPEC/logs/data-model.md` L260-390)
- **Timestamps.** If you can show only one time, use `Timestamp` and fall back to `ObservedTimestamp`. (`SPEC/logs/data-model.md` L200-204)
- **OTLP/JSON** (Stable for logs) follows the proto3 JSON mapping with four OTLP-specific rules:
  - keys are lowerCamelCase only;
  - `traceId`/`spanId` are **hex**, not base64;
  - enums are **integers only**;
  - 64-bit ints, including `timeUnixNano`, are **decimal strings**, but readers must also accept numbers.

  `bytesValue` stays base64 because the hex rule covers only trace and span IDs. Receivers MUST ignore unknown fields. (`PROTO/docs/specification.md` L447-492; `PROTO/README.md` L57-69)
- **On-disk formats.**
  - The spec's File Exporter (status **Development**) defines JSON Lines: each line is a whole `LogsData`, i.e. `{"resourceLogs":[...]}`, holding **many records per line**. (`SPEC/protocol/file-exporter.md`)
  - The Collector `fileexporter` (**alpha** for logs) writes one line per batch with `format: json`. `format: proto` and `zstd` produce length-prefixed binary framing instead. (fileexporter README L5-6, L65-132)
- **There is no standardized flat, single-record JSON.** The spec says the stdout exporter's format "is unspecified and can vary between implementations" (`SPEC/logs/sdk_exporters/stdout.md`). In practice:
  - Go `stdoutlog` writes PascalCase JSON objects, one record each;
  - JS `ConsoleLogRecordExporter` uses `console.dir` (not JSON);
  - .NET writes multi-line `LogRecord.X: value` text;
  - Java offers plain text, one `ResourceLogs` per line, or a full `{"resourceLogs":...}` line.

  A viewer should treat OTLP/JSON Lines as the standard input and any flat form as our own, ad hoc format.
- **Semantic conventions to surface.**
  - Stable: `service.name`, `service.namespace`, `service.version`, `service.instance.id`, `deployment.environment.name`, `k8s.namespace.name`, `k8s.pod.name`, `k8s.container.name`, `exception.type`, `exception.message`, `exception.stacktrace`, `code.function.name`, `code.file.path`, `code.line.number`.
  - Release candidate: `process.pid`.
  - Development: `host.name`, `log.file.path`, `log.record.uid`.
  - Deprecated: `code.function`, `code.filepath`, `code.lineno`, `deployment.environment`, `event.name`.
- **Common list columns** across viewers: **Timestamp, Severity/Level, Service (resource), Message/Body, Trace ID**. Aspire and otel-tui use exactly this set, plus Actions in Aspire and Event Name in otel-tui.
- **Common detail grouping:** the record itself (level, message/body, attributes), then trace context (trace ID, span ID), exceptions, Resource, and Scope. Aspire's headings are `Log entry`, `Context`, `Exception`, `Resource`. otel-tui mirrors the OTLP tree: `Resource`, then `Scopes`, then `LogRecord`.
- **Common trace links:** a trace-ID cell or field is a link that opens the trace view, filtered to the span where possible. The reverse link from a trace or span shows "logs for this trace" (Aspire `View Logs`, Grafana `Logs for this span`, otel-tui's timeline log pane).

## 1. Log Data Model fields

Source: `SPEC/logs/data-model.md`, document status **Stable** (L8). The field table is at L163-176, and the details at L180-451.

| Field | Type | Required? | Notes | Cite |
|---|---|---|---|---|
| `Timestamp` | uint64 ns since epoch | optional ("may be missing if the source timestamp is unknown") | Time at the source. | L180-186 |
| `ObservedTimestamp` | uint64 ns | "SHOULD be set once the event is observed by OpenTelemetry" in the spec; the proto comment says MUST | Collection time. For a single-time display: use `Timestamp` if present, else `ObservedTimestamp`. | L188-204; `PROTO/opentelemetry/proto/logs/v1/logs.proto` L144-159 |
| `TraceId` | bytes (16) | optional | W3C trace id. All-zero or wrong-length values are invalid, and the empty string in OTLP/JSON is invalid. | L208-215; logs.proto L188-199 |
| `SpanId` | bytes (8) | optional | "If SpanId is present TraceId SHOULD be also present." | L217-223; logs.proto L201-213 |
| `TraceFlags` | byte | optional | W3C flags; only SAMPLED is defined. In OTLP it is the low 8 bits of `flags` (fixed32): mask with `LOG_RECORD_FLAGS_TRACE_FLAGS_MASK = 0xFF`. | L225-232; logs.proto L123-134, L181-186 |
| `SeverityText` | string | optional | The original level string. If missing, the short name of `SeverityNumber` "may be used as a substitution". | L236-244 |
| `SeverityNumber` | number (enum) | optional | 1-24 (see §1.1); 0 = unspecified. | L246-271; logs.proto L87-121 |
| `Body` | AnyValue | optional | A string (may be multi-line) or structured data; MUST support AnyValue. | L392-402 |
| `Resource` | Resource | optional | Fixed per source; may be recorded once per batch. SHOULD follow resource semantic conventions. | L404-417 |
| `InstrumentationScope` | name, version, attributes (+ droppedAttributesCount) | optional | | L419-425; `PROTO/opentelemetry/proto/common/v1/common.proto` L105-123 |
| `Attributes` | Attribute collection (`map<string, AnyValue>`) | optional | Varies per occurrence. Exception info, if present, MUST follow the exception semantic conventions. | L427-443 |
| `EventName` | string | optional | Identifies the event class and SHOULD uniquely identify its structure. "A log record with a non-empty event name is an Event." | L445-451; logs.proto L215-227 (`[Since v1.5.0]`) |

**Status of EventName.** The data model has no per-field status markers, so `EventName` is Stable along with the rest of the document. The commit history of `specification/logs/data-model.md` shows "Change `event.name` attribute into top-level event name field (#4320)" (2024-12-06) and then "Stabilize EventName (#4475)" (2025-04-11). (GitHub commits API for that path.) In the Logs API, the `Emit` and `Enabled` parameters accept Event Name as optional (`SPEC/logs/api.md` L125, L143). The API's separate "Ergonomic API" is Development (L183).

**AnyValue** (`SPEC/common/README.md` L41-75, Stable; `common.proto` L28-51). An AnyValue is one of:

- a string, bool, int64 or double;
- a byte array;
- an array of AnyValue;
- a `map<string, AnyValue>` with unique, case-sensitive keys;
- an empty value.

Nesting can be arbitrarily deep. Empty values, zero, empty strings and empty arrays "are considered meaningful and MUST be stored". The proto `oneof` also has `string_value_strindex` (field 8, Alpha, Profiles only). Non-profiles receivers should treat it as a non-fatal issue and ignore it (common.proto L39-51). The spec commit "Stabilize complex AnyValue types and related attribute limits (#4794)" (2026-01-07) means arrays and maps as attribute values are now stable.

**Display rule for nested values (useful for our UI).** `SPEC/common/README.md` L102-188 defines a string form for "protocols that do not natively support some of the AnyValue types". It is a good default for rendering a nested Body or attribute on one line:

- strings appear as-is, without quotes;
- bools and numbers appear as JSON;
- NaN and Infinity appear bare;
- bytes appear as base64;
- empty appears as the empty string;
- arrays and maps appear as JSON, with nested strings quoted and empty values as `null`.

### 1.1 Severity: ranges, short names, display

- Ranges (L262-269): 1-4 TRACE, 5-8 DEBUG, 9-12 INFO, 13-16 WARN, 17-20 ERROR, 21-24 FATAL. `SeverityNumber=0` MAY represent unspecified (L271).
- Short names (L334-363): `TRACE, TRACE2, TRACE3, TRACE4, DEBUG..DEBUG4, INFO..INFO4, WARN..WARN4, ERROR..ERROR4, FATAL..FATAL4`. The proto enum names are `SEVERITY_NUMBER_TRACE`…`SEVERITY_NUMBER_FATAL4` (logs.proto L87-121).
- Mapping from a source format (L273-302): if a source has one level in a range, use the smallest value in that range (e.g. "Informational", the only info-like level, maps to 9). A record with no severity "may" be shown distinctly or treated as INFO (9).
- Error semantics (L315-330): "If `SeverityNumber` is present and has a value of ERROR (numeric 17) or higher then it is an indication that the log record represents an erroneous situation… (e.g. UIs may display such errors in a different color…)".
- Displaying (L365-383): when one record is displayed, show both fields. The recommended combined string is the short name followed by `SeverityText` in parentheses, e.g. **INFO (Informational)**. If the text is missing, show only the short name. **Dropdowns** should list the short names, not the distinct `SeverityText` values.
- Comparing (L385-390): compare by `SeverityNumber`; `0` MAY be special-cased.
- **Colors: not standardized.** The spec mentions color only in the "may display … in a different color" sentence above. Each viewer chooses its own palette (§5).

## 2. OTLP/JSON encoding

Maturity: OTLP `logs/*` and `collector/logs/*` are **Stable** in both binary and JSON forms (`PROTO/README.md` L62-69).

Rules (`PROTO/docs/specification.md` §"JSON Protobuf Encoding", L447-492):

- The base is the proto3 JSON mapping, with these deviations:
  - `traceId` and `spanId` are "case-insensitive hex-encoded strings; they are not base64-encoded". Example: `"traceId": "5B8EFFF798038103D269B633813FC60C"` (L453-460).
  - Enum fields "MUST be encoded as integer values … enum name strings MUST NOT be used" (L462-468). So `"severityNumber": 9`, never `"SEVERITY_NUMBER_INFO"`.
  - Receivers "MUST ignore message fields with unknown names" (L470-473).
  - Keys are lowerCamelCase, and the original snake_case names "are not valid" (L475-481).
- 64-bit integers (`int64`, `uint64`, `fixed64`, …) "are encoded as decimal strings, and either numbers or strings are accepted when decoding". This applies to `timeUnixNano` and `observedTimeUnixNano` (L482-487). `intValue` inside AnyValue is int64, so it is also a string (`"intValue": "10"` in `PROTO/examples/logs.json`).
- `bytesValue` is not covered by the hex exception, so the standard proto3 JSON mapping (base64) applies. **UNVERIFIED** by an OTLP example (none of the examples use `bytesValue`). This follows from the text, which limits the exception to `traceId`/`spanId`.

Nesting and field names (from `logs.proto`):

- `LogsData { resourceLogs[] }` (L38-46). The OTLP/HTTP request `ExportLogsServiceRequest` has the same single field.
- `ResourceLogs { resource, scopeLogs[], schemaUrl }` (L48-65). `schemaUrl` here applies only to `resource`.
- `ScopeLogs { scope, logRecords[], schemaUrl }` (L68-84). `schemaUrl` applies to the scope and its records.
- `LogRecord` fields in JSON (L136-227): `timeUnixNano`, `observedTimeUnixNano`, `severityNumber`, `severityText`, `body`, `attributes`, `droppedAttributesCount`, `flags`, `traceId`, `spanId`, `eventName`. Field 4 is reserved.
- `Resource { attributes[], droppedAttributesCount }`; `InstrumentationScope { name, version, attributes[], droppedAttributesCount }` (common.proto L105-123).
- KeyValue: `{"key": "...", "value": { <one of> stringValue | boolValue | intValue | doubleValue | arrayValue{values[]} | kvlistValue{values[KeyValue]} | bytesValue }}` (common.proto L28-100).
- Attribute keys within one record MUST be unique (logs.proto L174-177).

### 2.1 Canonical example (one request, two records)

Record 1 and the resource and scope are taken verbatim from `PROTO/examples/logs.json` (https://github.com/open-telemetry/opentelemetry-proto/blob/main/examples/logs.json). The following are **added by us** to show the remaining fields:

- the extra resource attributes and `schemaUrl` (the version is illustrative);
- all of record 2: an ERROR event with `flags`, `eventName`, a `kvlistValue` body, exception and code attributes, and `droppedAttributesCount`.

Field shapes follow §2.

```json
{
  "resourceLogs": [
    {
      "resource": {
        "attributes": [
          { "key": "service.name", "value": { "stringValue": "my.service" } },
          { "key": "service.version", "value": { "stringValue": "1.4.2" } },
          { "key": "deployment.environment.name", "value": { "stringValue": "staging" } },
          { "key": "host.name", "value": { "stringValue": "web-01" } }
        ]
      },
      "schemaUrl": "https://opentelemetry.io/schemas/1.37.0",
      "scopeLogs": [
        {
          "scope": {
            "name": "my.library",
            "version": "1.0.0",
            "attributes": [
              { "key": "my.scope.attribute", "value": { "stringValue": "some scope attribute" } }
            ]
          },
          "logRecords": [
            {
              "timeUnixNano": "1544712660300000000",
              "observedTimeUnixNano": "1544712660300000000",
              "severityNumber": 10,
              "severityText": "Information",
              "traceId": "5B8EFFF798038103D269B633813FC60C",
              "spanId": "EEE19B7EC3C1B174",
              "body": { "stringValue": "Example log record" },
              "attributes": [
                { "key": "string.attribute", "value": { "stringValue": "some string" } },
                { "key": "boolean.attribute", "value": { "boolValue": true } },
                { "key": "int.attribute", "value": { "intValue": "10" } },
                { "key": "double.attribute", "value": { "doubleValue": 637.704 } },
                { "key": "array.attribute", "value": { "arrayValue": { "values": [ { "stringValue": "many" }, { "stringValue": "values" } ] } } },
                { "key": "map.attribute", "value": { "kvlistValue": { "values": [ { "key": "some.map.key", "value": { "stringValue": "some value" } } ] } } }
              ]
            },
            {
              "timeUnixNano": "1544712661500000000",
              "observedTimeUnixNano": "1544712661500120000",
              "severityNumber": 17,
              "severityText": "Error",
              "traceId": "5B8EFFF798038103D269B633813FC60C",
              "spanId": "EEE19B7EC3C1B174",
              "flags": 1,
              "eventName": "my.library.checkout.failed",
              "body": {
                "kvlistValue": {
                  "values": [
                    { "key": "message", "value": { "stringValue": "checkout failed" } },
                    { "key": "cart.items", "value": { "intValue": "3" } }
                  ]
                }
              },
              "attributes": [
                { "key": "exception.type", "value": { "stringValue": "System.InvalidOperationException" } },
                { "key": "exception.message", "value": { "stringValue": "Payment provider timed out" } },
                { "key": "exception.stacktrace", "value": { "stringValue": "System.InvalidOperationException: Payment provider timed out\n   at Shop.Checkout.Pay() in /src/Checkout.cs:line 42" } },
                { "key": "code.function.name", "value": { "stringValue": "Shop.Checkout.Pay" } },
                { "key": "code.file.path", "value": { "stringValue": "/src/Checkout.cs" } },
                { "key": "code.line.number", "value": { "intValue": "42" } }
              ],
              "droppedAttributesCount": 1
            }
          ]
        }
      ]
    }
  ]
}
```

In a `.jsonl` file this whole object is **one line** (about 2.2 KB when compacted). The spec's own logs example in `SPEC/protocol/file-exporter.md` §Examples has the same shape: four lines, each a `resourceLogs` request with two `logRecords`, and empty strings for `traceId`/`spanId` in the second record.

## 3. On-disk and console output formats

### 3.1 Spec: OTLP File Exporter (JSON Lines)

`SPEC/protocol/file-exporter.md` has status **Development** and calls itself "a placeholder". The rules:

- Output is a JSON Lines file: UTF-8, "Each line is a valid JSON value", separator `\n`, preferred extension `jsonl`.
- "There is no guarantee that the data in the file is ordered", including that timestamps increase monotonically.
- The encoding is OTLP JSON. "Only top-level objects, `TracesData`, `MetricsData`, and `LogsData` are supported", and "Files must contain exactly one type of data".

So each line is a `LogsData` holding any number of records. The UI must expect many records per line and must not assume they arrive in time order.

### 3.2 Collector `fileexporter` (opentelemetry-collector-contrib)

Source: https://github.com/open-telemetry/opentelemetry-collector-contrib/blob/main/exporter/fileexporter/README.md. Its stability is **alpha** for traces, metrics and logs and **development** for profiles (L5-6). The README says "there is no guarantee that exact field names will remain stable" (L34). Behavior:

- **`format`** (default `json`) is `json` or `proto` (L65). **`encoding`** lets an encoding extension replace the built-in marshaler (L66).
- **Framing** (File Format table, L117-132):
  - `json`, no compression: "Newline-delimited OTLP JSON, **one line per batch**".
  - `proto`: "Length-prefixed messages", where each message is preceded by a 4-byte big-endian uint32 length.
  - `zstd` without the `exporter.file.nativeCompression` gate: each batch is compressed on its own and then length-prefixed, which is "Not readable by `zstd -d`".
  - `zstd` with the gate: a standard zstd stream that contains newline-delimited JSON or length-prefixed proto.
- **Compression:** only `zstd` (L68, L106-115), with levels 1, 3, 6 or 11 (L69-76).
- **Rotation** applies only when `rotation:` is configured, and is size-based (`max_megabytes` default 100, `max_backups` default 100, `max_days`, `localtime`). The rotated file is renamed `data-2022-09-14T05-02-14.173-size.json`, and a new file is created at the original `path` (L58-63, L91-104). `append: true` cannot be combined with rotation (L67).
- **`group_by.resource_attribute`** writes one file per resource-attribute value (L86-89, L134-140).
- Reading back: the `otlpjsonfilereceiver` "only reads uncompressed, newline-delimited OTLP JSON" (L22).

So "one ExportLogsServiceRequest per line" holds for `format: json` without compression, where a line is one batch. It does not hold for proto or zstd.

### 3.3 Collector `debug` exporter

Source: https://github.com/open-telemetry/opentelemetry-collector/blob/main/exporter/debugexporter/README.md. The verbosity levels are:

- `basic` (default) prints one summary line per batch (L68-75);
- `normal` prints "about one line for each telemetry record", and "is not a strict rule" (L78-81);
- `detailed` prints multi-line output per record (L95-97).

"The output formats for all verbosity levels is not guaranteed and may be changed at any time" (L157). Not a parse target.

### 3.4 SDK stdout/console exporters: no standard flat form

The spec (`SPEC/logs/sdk_exporters/stdout.md`, status Stable for the exporter's existence) says: "The exporter's output format is unspecified and can vary between implementations". Docs should warn "The output format is not standardized". For a standardized stdout format it points to the File Exporter, i.e. OTLP JSON Lines. What each SDK actually emits:

| SDK | Output per record | Source |
|---|---|---|
| Go `stdoutlog` | One JSON object per record, via `json.Encoder.Encode`, so newline-terminated. `WithPrettyPrint` indents it with tabs. Keys are **PascalCase**: `Timestamp`, `ObservedTimestamp`, `EventName`, `Severity`, `SeverityText`, `Body`, `Attributes`, `TraceID`, `SpanID`, `TraceFlags`, `Resource`, `Scope`, `DroppedAttributes`. Values use Go's own `attribute.Value`/`resource` JSON shapes, not OTLP's. | https://github.com/open-telemetry/opentelemetry-go/blob/main/exporters/stdout/stdoutlog/record.go L18-32; `exporter.go` L32-34, L75 |
| JS `ConsoleLogRecordExporter` | `console.dir(obj, {depth: 3})`: Node's inspect format, **not JSON**, multi-line. The object has `resource.attributes`, `instrumentationScope`, `timestamp` (microseconds), `traceId`, `spanId`, `traceFlags`, `severityText`, `severityNumber`, `eventName`, `body`, `attributes`. | https://github.com/open-telemetry/opentelemetry-js/blob/main/packages/sdk-logs/src/export/ConsoleLogRecordExporter.ts L52-68, L80 |
| .NET `ConsoleLogRecordExporter` | Multi-line text, `LogRecord.Timestamp:`, `LogRecord.TraceId:`, …, `LogRecord.Attributes (Key:Value):`, then the instrumentation scope and resource blocks. | https://github.com/open-telemetry/opentelemetry-dotnet/blob/main/src/OpenTelemetry.Exporter.Console/ConsoleLogRecordExporter.cs L52-171 |
| Java `SystemOutLogRecordExporter` | One text line: `ISO-time SEVERITY [eventName: …] 'body' : traceId spanId [scopeInfo: name:version] {attributes}`. | https://github.com/open-telemetry/opentelemetry-java/blob/main/exporters/logging/src/main/java/io/opentelemetry/exporter/logging/SystemOutLogRecordExporter.java L65-98 |
| Java `OtlpJsonLoggingLogRecordExporter` | OTLP JSON, "Each log line will include a single `ResourceLogs`", without the `resourceLogs` wrapper. | `exporters/logging-otlp/.../OtlpJsonLoggingLogRecordExporter.java` L18, L35 |
| Java `OtlpStdoutLogRecordExporter` (internal/experimental) | OTLP JSON. By default `wrapperJsonObject = true` wraps output in an outer `"resourceLogs"` object, i.e. the File Exporter format. | `exporters/logging-otlp/.../internal/logs/OtlpStdoutLogRecordExporterBuilder.java` L30, L39-45; `OtlpStdoutLogRecordExporter.java` L57-66 |

**Conclusion:** no standardized flat single-record JSON exists. The only standard file or stream form is OTLP JSON Lines (`LogsData` per line). If we want a flat "one record per line" form for the UI, it is **our own schema**. Base it on the data model's field names and on the AnyValue→string rules in §1.

## 4. Semantic conventions worth surfacing

Source: the attribute registry in `SEMCONV/registry/attributes/*.md`. The stability badge is read from each attribute's row.

| Attribute | Where | Stability | Cite (file:line) |
|---|---|---|---|
| `service.name` | Resource | Stable | `service.md` L21 |
| `service.namespace` | Resource | Stable | `service.md` L22 |
| `service.version` | Resource | Stable | `service.md` L23 |
| `service.instance.id` | Resource | Stable | `service.md` L20 |
| `deployment.environment.name` | Resource | Stable | `deployment.md` L17 |
| `deployment.environment` | — | **Deprecated**, replaced by `deployment.environment.name` | `deployment.md` L62 |
| `host.name` | Resource | Development | `host.md` L27 |
| `process.pid` | Resource | Release Candidate | `process.md` L36 |
| `k8s.namespace.name` / `k8s.pod.name` / `k8s.container.name` | Resource | Stable | `k8s.md` L50 / L74 / L20 |
| `exception.type` / `exception.message` / `exception.stacktrace` | Log attributes | Stable | `exception.md` L19 / L17 / L18 |
| `code.function.name` / `code.file.path` / `code.line.number` | Log attributes | Stable | `code.md` L19 / L18 / L20 |
| `code.function` / `code.filepath` / `code.lineno` | — | **Deprecated**. `code.filepath` is replaced by `code.file.path` and `code.lineno` by `code.line.number`. `code.function` (and `code.namespace`) should be folded into the fully-qualified `code.function.name`. | `code.md` L48-51 |
| `log.file.path` | Log attributes | Development | `log.md` L41 |
| `log.record.uid` | Log attributes | Development (example: a ULID) | `log.md` L53 |
| `event.name` | — | **Deprecated**: "The value of this attribute MUST now be set as the value of the EventName field on the LogRecord" | `event.md` L14 |

Exceptions in logs (`SEMCONV/exceptions/exceptions-logs.md`, Stable except where marked):

- `exception.message` is "Required if `exception.type` is not set", and `exception.type` is required if the message is not set (L189-209).
- The spec data model requires exception data to follow these conventions when present (`SPEC/logs/data-model.md` L437-443).

UI implication: show legacy keys (`code.filepath`, `event.name`, `deployment.environment`) as fallbacks for the stable ones, since older SDKs still emit them. Aspire does this for EventName: "EventName from the LogRecord field takes precedence over the legacy attribute" (`ASPIRE/Otlp/Model/OtlpLogEntryData.cs` L53-54).

## 5. UI conventions in existing OTel log viewers

### 5.1 .NET Aspire dashboard (Structured logs page)

Source code: `ASPIRE/Components/Pages/StructuredLogs.razor`, `Components/Controls/StructuredLogDetails.razor(.cs)`. Docs: https://aspire.dev/dashboard/explore/ (formerly learn.microsoft.com).

- **List columns**, in order (`StructuredLogs.razor` L108-142; English headers from `Resources/StructuredLogs.resx`): **Resource**, **Level**, **Timestamp**, **Message**, **Trace**, **Actions**.
  - The Resource cell has a 5px left border in a color generated per resource (`ColorGenerator…GetColorVariableByKey`, L109).
  - Columns are resizable and reorderable through `GridColumnManager`. Clicking a row opens details (L105).
- **Severity:** the number is mapped to .NET `LogLevel`:
  - 1-4 → Trace, 5-8 → Debug, 9-12 → Information, 13-16 → Warning, 17-20 → Error, 21-24 → Critical, anything else → None (`Otlp/Model/OtlpLogEntryData.cs` L95-104).
  - The cell shows the LogLevel **name** as text, plus an icon only for errors and warnings: a filled ErrorCircle in Error color for Error/Critical, a Warning icon in Warning color for Warning (`Components/ResourcesGridColumns/LogLevelColumnDisplay.razor`).
  - `SeverityText` is not displayed. `IsError` means Error or Critical (`OtlpLogEntry.cs` L27-28).
- **Message:** the Body converted to a string (`OtlpLogEntryData.cs` L46-48). Nested values become **compact JSON strings** (ArrayValue/KvlistValue → `ToJsonString`) and bytes become **hex** (`Otlp/Model/OtlpHelpers.cs` L174-189). A text visualizer can open the message or a value formatted as JSON/XML (`TextVisualizerFormat`, `StructuredLogDetails.razor.cs` L80, L113).
- **Detail panel:**
  - Toolbar: `Resource: <name>`, `Timestamp: <time>`, a filter textbox and an actions menu.
  - Then an accordion (`StructuredLogDetails.razor`; headings from `Resources/ControlsStrings.resx`), each section with a count and expanded by default:
    1. **Log entry**: `Level`, `Message`, then all remaining log attributes.
    2. **Context**: `Category` (= scope name), `EventName` (if set), `TraceId`, `SpanId`.
    3. **Exception**: shown only if any attribute starts with `exception.`; those attributes are moved here.
    4. **Resource**: the resource attributes.

    (Grouping logic in `StructuredLogDetails.razor.cs` L72-115.)
  - Scope version and attributes have no section.
- **Trace links:**
  - The Trace column shows the trace ID **shortened to 7 chars** as a link to the trace detail URL with the span (`StructuredLogs.razor` L122-133; `src/Shared/Otlp/OtlpHelpers.cs` L28 `ShortenedIdLength = 7`).
  - In details, `TraceId` and `SpanId` render as buttons that navigate to the trace (`TraceIdButtonValue`, `SpanIdButtonValue`; `StructuredLogDetails.razor.cs` L118-129).
  - Reverse link: the trace details page has a **View Logs** button that opens Structured logs filtered to that trace (aspire.dev "Trace details").
- **Filtering:**
  - A **Level** dropdown sets a minimum level ("Minimum log level filter", `StructuredLogs.resx`).
  - The text search covers service, level and message (docs).
  - A filter dialog works on "any log property". Its conditions for strings are Equals, Contains, NotEqual and NotContains; numbers and dates get `>=, >, <=, <, =, !=` (`Components/Dialogs/FilterDialog.razor.cs` L63-86).
  - Known pseudo-fields include `log.message`, `log.category`, `log.traceid`, `log.spanid`, `log.level`, `log.eventname` and `log.timestamp` (`Model/Otlp/KnownStructuredLogFields.cs` L8-16).
  - The actions menu offers View details, Log message and View JSON (`Model/StructuredLogMenuBuilder.cs` L89-115).

### 5.2 Grafana Explore with Loki (OTel data)

How OTLP lands in Loki determines what Grafana shows:

- By default 17 resource attributes become **index labels**, with dots changed to underscores (e.g. `service_name`). The list includes `service.name`, `service.namespace`, `service.instance.id`, `deployment.environment.name`, `k8s.namespace.name`, `k8s.pod.name` and `k8s.container.name`. (https://grafana.com/docs/loki/latest/send-data/otel/)
- Other resource attributes, all scope data and all LogRecord fields except the body and timestamp go into **structured metadata**.
- The **Body becomes the log line**, with non-string bodies stringified, and nested attributes are flattened with `_`.
- In Loki's source the metadata keys are `scope_name`, `scope_version`, `scope_dropped_attributes_count`, `observed_timestamp`, `severity_number`, `severity_text`, `dropped_attributes_count`, `flags`, `trace_id`, `span_id` and `event_name` (https://github.com/grafana/loki/blob/main/pkg/loghttp/push/otlplabels/labels.go L17-22, ~L196-327).

UI (https://grafana.com/docs/grafana/latest/explore/logs-integration/):

- **List:** one line per log, with an optional timestamp (hidden / ms / ns) and a colored level. Common labels are shown once above the list.
- **Level colors** come from a `level` label: critical = purple, error = red, warning = yellow, info = blue, debug = gray, trace = light blue, unknown = gray. Each color also accepts syslog numbers and aliases such as `crit` and `eror`. The docs do not say that `severity_number` is used for color. Whether OTel `severity_text` feeds the level is **UNVERIFIED** from this page (Loki may populate `detected_level`).
- **Log details:** expand the line, inline or in a sidebar. There are sections **Fields** and **Links**. For Loki, fields are grouped as **Indexed Labels**, **Parsed fields** and **Structured Metadata**.
  - Each field has a positive filter (filter for value) and a negative filter (filter out value). Both add a label matcher to the query, and Grafana shows when a filter is already active.
  - An eye icon shows only the selected fields in the list, and a stats icon shows ad-hoc stats for the field.
- **OTel-aware line display**, a newer preview feature (https://grafana.com/whats-new/2026-01-05-enhanced-display-for-opentelemetry-log-lines/):
  - A line counts as OTel "when it contains OpenTelemetry-related fields, such as `severity_number`".
  - Shown inline: thread name, exception type, exception message, body and log attributes.
  - Kept in log details: `k8s_*`, `cloud_*`, `host_*` and `service_*` resource attributes, plus severity, trace and span IDs, timestamps and flags.
  - The Grafana version for this feature is **UNVERIFIED**; the page does not say.
- **Trace links:**
  - **Derived fields** on the Loki data source turn a value into a link that "appears in the log details". The type is **Regex in log line** or **Label**, there is an **Internal link** toggle pointing at a Tempo data source, the query is `${__value.raw}`, and `urlDisplayLabel` sets the label, e.g. "View Trace" (https://grafana.com/docs/grafana/latest/datasources/loki/configure/).
  - Reverse link: Tempo's **Trace to logs** adds a "Logs for this span" link on spans. It can filter by trace ID (recommended) or by span ID, with a time shift around the span (https://grafana.com/docs/grafana/latest/datasources/tempo/configure-tempo-data-source/configure-trace-to-logs/).

### 5.3 SigNoz Logs Explorer

Docs: https://signoz.io/docs/product-features/logs-explorer/. Source: `SIGNOZ/`.

- **List:** three formats, **Raw**, **Default** and **Column**. Default shows "Timestamp and Body", and "Edit columns" adds attribute columns.
  - Each row has a **colored severity bar** 3px wide. The color family is chosen by `SeverityNumber` first, falling back to `SeverityText` (`components/Logs/LogStateIndicator/utils.ts` "give priority to the severityNumber"). Families: TRACE = forest (green), DEBUG = aqua, INFO = robin (blue), WARN = amber, ERROR = cherry (red), FATAL = sakura (pink). Shades within a family follow the casing of the text variant (`LogStateIndicator.styles.scss` L1-121; `LogStateIndicator.tsx` L50-68).
  - The frequency chart is colored by severity (docs).
- **Detail drawer:**
  - The header has the timestamp, prev/next arrows, "Copy log" and "Copy link to log".
  - A **highlights** strip shows badges for SERVICE (`service.name`), SEVERITY (`severity_text`, colored like the bar), NAMESPACE (`service.namespace`), ENVIRONMENT (`deployment.environment`, the deprecated key), TRACE ID and SPAN ID (`components/LogDetail/LogHighlights/config.tsx`).
  - Tabs: **Overview**, **Context** (about 10 logs before and after) and **Metrics** (`components/LogDetail/index.tsx` L335-353).
  - Overview has a Pretty/JSON toggle. Pretty shows `timestamp`, `body`, `attributes` and `resource` as a tree, with a JSON body expanded into nested keys (docs).
- **Filtering:** each field's ⋯ menu has "Filter for value", "Filter out value", "Group by field", "Replace filters with this value" and "Copy Value". There is also a quick-filters side panel (docs).
- **Trace link:** the TRACE ID badge is a router link to `/trace/<traceId>` (`components/LogDetail/LogHighlights/TraceIdField.tsx` L9-18).

### 5.4 otel-tui

Source: `TUI/` (terminal UI built on tview).

- **List columns** (`tui/component/table/log.go` L9-46): **Trace ID**, **Service Name**, **Timestamp**, **Severity**, **Event Name**, **RawData** (the Body as a string).
  - In the per-trace timeline the Trace ID column is dropped (L48-80).
  - Severity is the raw `SeverityText` (`telemetry/store.go` L144-146). The number is not shown in the list and there is **no color**; only the header row is yellow.
  - **Gap:** Event Name is read from the deprecated `event.name` attribute, not from the top-level field (store.go L148-154).
- **Detail:** an expandable tree that mirrors OTLP nesting (`tui/component/page/log/detail.go` L60-150):
  - `Log` → `Resource` (dropped attributes count, schema url, `Attributes`, sorted).
  - `Resource` → `Scopes` → `<scope name>` (schema url, version, dropped count, `Attributes`).
  - `Resource` → `LogRecord`: trace id, span id, timestamp, observed timestamp, body, `severity: <text> (<number>)`, flags, dropped attributes count, `Attributes`.
  - Values are `key: value` with `pcommon.Value.AsString()`, so maps and arrays render as JSON text (`layout/attributes.go` L12-22).
  - A separate **Body** pane shows the full body (`page/log/body.go`), and `y` copies the body (README).
- **Filter:** a single text box, "Filter by service or body (/)", does a substring match on `service.name + " " + body` (`page/log/table.go` L43-47; `store.go` L318-334). There are no per-attribute filters.
- **Trace links:**
  - In the detail tree, the trace id node gets a "(🔗)" prefix and becomes selectable when spans for that trace are cached. Selecting it opens the trace timeline (`detail.go` L101-111).
  - The timeline page has a **Logs** pane listing the trace's logs. `A` toggles between "all logs of the trace" and "logs of the selected span" (`page/timeline/log.go` L37-59).

### 5.5 Jaeger

- Jaeger UI's "logs on span" are **span events**, not OTel log records. The accordion label is `Logs` in legacy terms and `Events` when OTel terms are on (`JAEGER/AccordionEvents.tsx` L126-145). The same toggle switches `Tags`/`Attributes` and `Process`/`Resource` (`JAEGER/index.tsx` L70-71).
  - The title shows "(n of m)" when the timeline is zoomed (L127-145).
- Attribute values that look like JSON objects or arrays are parsed and rendered as an **interactive JSON tree**. The tree starts expanded when the object has 10 or fewer keys (`JAEGER/AttributesTable.tsx` L20-30, L73-100). This is a useful pattern for nested AnyValue.
- I found no OTel log-record view in jaeger-ui. Whether Jaeger v2 stores OTLP logs at all is **UNVERIFIED** (not checked against Jaeger docs).

### 5.6 Honeycomb

Not investigated (optional per brief). Nothing here is sourced.

## 6. Synthesis for our UI

**Default column set**, the intersection of Aspire, otel-tui, the SigNoz Default view and the Grafana line:

1. **Timestamp**: `Timestamp`, falling back to `ObservedTimestamp`.
2. **Severity**: a short name derived from `SeverityNumber`, falling back to `SeverityText`. Use a per-range color or icon.
3. **Service**: the `service.name` resource attribute. Aspire and SigNoz give it a per-service color accent.
4. **Message/Body**: a string, with nested values shown as compact JSON.
5. **Trace**: a short trace ID rendered as a link.

Optional extras: Event Name (otel-tui), Actions (Aspire), and user-added attribute columns (SigNoz, Grafana).

**Severity presentation:**

- Show the spec's short name (TRACE…FATAL4) in the list and in dropdowns, and `SHORT (SeverityText)` in details (spec §1.1).
- Colors are app-chosen. The common pattern is error = red, warn = amber/yellow, info = blue, debug = gray/aqua, trace = light blue/green, fatal = purple/pink, unknown = gray (Grafana, SigNoz).
- Aspire uses icons for errors and warnings only.
- Treat a number of 17 or more as an error.

**Detail-panel grouping.** The common order is record first, then context, then exceptions, then provenance:

1. **Record**: severity, body/message (full, multi-line, with a JSON tree for kvlist/array), event name, then log **Attributes**.
2. **Trace context**: trace ID, span ID, flags, each with a link.
3. **Exception**: the `exception.*` attributes, pulled out of Attributes (Aspire). The stack trace should be monospace and multi-line.
4. **Resource**: attributes, plus schemaUrl and dropped count (otel-tui).
5. **Scope**: name, version, attributes (otel-tui; Aspire shows only the name, labelled "Category").

Per-section counts and a filter box over all fields (Aspire) are cheap wins. Nested values: compact JSON inline, expandable JSON tree on demand (Jaeger, SigNoz).

**Filter-by-attribute affordances:**

- A minimum-severity dropdown (Aspire, Grafana level filter).
- Free-text search over body and service (all four tools).
- Per-field **filter for value / filter out value** from the detail panel (Grafana, SigNoz).
- A structured field / condition / value filter (Aspire's dialog).

**Trace-link affordances:**

- Clicking a trace ID opens the trace view, focused on the span when a span ID exists (Aspire, SigNoz, otel-tui, Grafana derived field).
- The reverse direction shows logs for this trace, with a toggle to narrow to this span (Aspire View Logs, Grafana "Logs for this span", otel-tui `A` toggle).
- Since we have no trace view yet, the natural first step is "filter logs by this trace ID / span ID".
