## ADDED Requirements

### Requirement: Chế độ OTel của Log Space
Hệ thống SHALL hiện OtelInput của một Log Space trong OTel View, là chế độ thứ ba cạnh Raw View và Parsed View, với hai chế độ con Logs và Traces.
- Nút chuyển SHALL cho biết số record và số trace.
- Chế độ Traces SHALL chỉ chọn được khi có ít nhất một span.
- Chế độ Logs SHALL chỉ chọn được khi có ít nhất một record.

#### Scenario: Chỉ có span
- **WHEN** OtelInput có 9 span và không có record
- **THEN** OTel View mở ở chế độ Traces và nút Logs không bấm được

### Requirement: Bộ lọc OTel
OTel View SHALL có Bộ lọc OTel riêng gồm:
- severity tối thiểu (Mọi severity, ≥ INFO, ≥ WARN, ≥ ERROR);
- service (theo `service.name` của resource);
- chữ trong body (chứa chuỗi, không phân biệt hoa thường);
- traceId (khớp đúng, hoặc theo tiền tố từ 7 ký tự trở lên).

Filter, Search Expression, Focus Pane và Highlight Rule của Log Space SHALL không áp dụng trong OTel View.

Ở chế độ Traces:
- service và traceId SHALL lọc Trace List; một trace qua bộ lọc khi có ít nhất một span khớp;
- severity và chữ trong body SHALL chỉ lọc các chấm log trên chart.

#### Scenario: Lọc ≥ WARN
- **WHEN** người dùng chọn ≥ WARN ở chế độ Logs
- **THEN** chỉ các record có severity number từ 13 trở lên được hiện

#### Scenario: Record không có severity
- **WHEN** một record không có severity number, severity text cũng không đọc được, và bộ lọc đang là ≥ INFO
- **THEN** record đó bị ẩn

#### Scenario: Filter của Log Space không áp dụng
- **WHEN** Log Space có Filter `level = ERROR` và người dùng chuyển sang OTel
- **THEN** OTel View không bị Filter đó thu hẹp, và Filter vẫn còn nguyên khi quay về Parsed View

### Requirement: Bảng Logs
Chế độ Logs SHALL hiện mỗi record là một row, xếp theo thời gian (không có time thì dùng observed time; không có cả hai thì xếp cuối). Bảng SHALL có 5 cột cố định:
- Timestamp: giờ UTC `HH:MM:SS.mmm`, tooltip là ngày đầy đủ;
- Severity: đúng severity text;
- Service;
- Body: chuỗi; kvlist và mảng hiện thành JSON gọn trên một dòng;
- Trace: 7 ký tự đầu của trace id.

Màu chữ của Severity và Body SHALL theo severity number: từ 17 trở lên màu lỗi (FATAL như ERROR), 13–16 màu cảnh báo, dưới 9 màu mờ.

Người dùng SHALL thêm được cột từ bất kỳ key attribute nào của record, `resource.<key>`, `scope.name` và `scope.version`. Cột thêm SHALL bỏ đi được.

#### Scenario: Severity text trống
- **WHEN** một record có severity number 9 và không có severity text
- **THEN** ô Severity trống và chữ Body có màu thường

#### Scenario: FATAL
- **WHEN** một record có severity number 21 và severity text `FATAL`
- **THEN** ô Severity hiện `FATAL` bằng màu lỗi

#### Scenario: Thêm cột resource
- **WHEN** người dùng thêm cột `resource.k8s.pod.name`
- **THEN** bảng có thêm cột đó; record có resource không chứa key này thì ô để trống

### Requirement: TraceId dẫn sang trace
Bấm trace id của một record SHALL:
- mở trace đó ở chế độ Traces, chọn span gốc, nếu OtelInput có span của trace;
- nếu không, đặt bộ lọc traceId ở chế độ Logs.

Mọi trace id và span id SHALL có nút Copy chép đủ 32 hoặc 16 ký tự hex.

#### Scenario: Có span
- **WHEN** người dùng bấm `4bf92f3` trên một record
- **THEN** chế độ Traces mở Trace Chart của trace `4bf92f35…`

#### Scenario: Không có span
- **WHEN** người dùng bấm trace id của một record mà OtelInput không có span nào của trace đó
- **THEN** bộ lọc traceId được đặt và bảng Logs chỉ còn các record của trace đó

### Requirement: Trace List
Chế độ Traces SHALL mở đầu bằng Trace List. Mỗi row là một trace, gồm:
- root service · tên span gốc sớm nhất;
- thời điểm bắt đầu;
- thời lượng (end muộn nhất trừ start sớm nhất);
- số span;
- số span có status Error.

Danh sách SHALL xếp theo thời điểm bắt đầu, mới nhất ở trên. Chọn một row SHALL mở Trace Chart của trace đó.

#### Scenario: Trace lỗi
- **WHEN** một trace có 7 span, trong đó 4 span có status Error
- **THEN** row của trace đó hiện `7` span và `4` lỗi bằng màu lỗi

### Requirement: Cây span
Trace Chart SHALL dựng cây span theo parent span id, các span con xếp theo thời điểm bắt đầu. Span có parent span id không có trong OtelInput SHALL được coi là gốc và có nhãn "thiếu span cha".

#### Scenario: Span mồ côi
- **WHEN** span `x` có parent span id không có trong OtelInput
- **THEN** `x` hiện ở mức gốc, có nhãn "thiếu span cha"

### Requirement: Waterfall và flame chart
Trace Chart SHALL có hai dạng, chuyển qua lại bằng một nút:
- **Waterfall:** cột trái là cây span (thụt lề theo độ sâu, có chevron thu gọn), cột phải là thanh span theo thời gian kèm số mili giây.
- **Flame chart:** mỗi độ sâu một hàng, mỗi span một khối ghi tên và thời lượng.

Ở cả hai dạng:
- màu SHALL theo `service.name` và ổn định giữa các lần mở;
- span có status Error SHALL có viền màu lỗi;
- chọn một span SHALL hiện nó trong OTel Detail;
- thu gọn một nhánh SHALL ẩn các span con của nó.

#### Scenario: Chuyển dạng giữ trạng thái
- **WHEN** người dùng chọn span `charge`, thu gọn `cart.load`, rồi chuyển từ Waterfall sang Flame
- **THEN** flame chart vẫn chọn `charge` và không hiện các span con của `cart.load`

### Requirement: Phóng to theo thời gian
Trace Chart SHALL có minimap vẽ toàn bộ trace.
- Kéo chuột trên minimap SHALL đặt cửa sổ thời gian; chart chỉ vẽ khoảng đó, và các thanh bị cắt ở hai mép.
- "Bỏ phóng to" SHALL trả về toàn trace.
- Kéo một đoạn ngắn hơn 1% chiều dài minimap SHALL không đổi cửa sổ.

#### Scenario: Phóng to
- **WHEN** người dùng kéo trên minimap từ 250ms tới 560ms
- **THEN** trục thời gian của chart chạy từ 250ms tới 560ms

### Requirement: Log trên span
Trace Chart SHALL vẽ mỗi record có cùng trace id và span id với một span thành một chấm trên thanh của span đó. Chấm đặt tại time của record, màu theo severity. Chấm của record bị bộ lọc severity hoặc chữ trong body loại ra SHALL không được vẽ. Bấm một chấm SHALL chuyển sang chế độ Logs và chọn record đó.

#### Scenario: Bấm chấm lỗi
- **WHEN** người dùng bấm chấm đỏ trên span `charge`
- **THEN** chế độ Logs hiện, và record `charge failed: card_declined` được chọn và cuộn tới

### Requirement: OTel Detail
OTel View SHALL có panel OTel Detail bên phải, hiện record hoặc span đang chọn theo nhóm. Mỗi nhóm có số đếm và gập được; Resource và Scope gập sẵn.

- **Record:**
  - nhóm Record: severity dạng `TÊN (text) · số`, time UTC đầy đủ, observed time, event name, body, attributes;
  - nhóm Trace: trace id, span id, flags;
  - nhóm Exception;
  - nhóm Resource;
  - nhóm Scope.
- **Span:**
  - đầu panel: tên, kind, status, thời lượng, độ lệch so với đầu trace;
  - các nhóm Attributes · Events · Links · Logs · Trace · Resource · Scope.
- **Exception:** các attribute `exception.*` của record SHALL được chuyển từ Record sang nhóm Exception. Stacktrace SHALL hiện monospace, nhiều dòng.
- **Kvlist và mảng** SHALL hiện dạng cây, mở sẵn khi có tối đa 10 khoá. Bytes SHALL hiện hex.

#### Scenario: Record có exception
- **WHEN** người dùng chọn record có `exception.type`, `exception.message` và `exception.stacktrace`
- **THEN** OTel Detail có nhóm Exception với 3 mục này, và nhóm Record không còn chúng

#### Scenario: Logs của span
- **WHEN** người dùng chọn span `charge`, span có 2 record
- **THEN** nhóm Logs liệt kê 2 record đó; bấm một record thì nhảy sang chế độ Logs và chọn nó

### Requirement: Xem dòng gốc
OTel Detail SHALL có nút "Xem dòng gốc".
- Bấm nút SHALL chuyển Log Space sang Parsed View và chọn dòng đầu tiên của source đầu tiên, không thay đổi Filter, Search hay Time Range (rule 41).
- Khi source có nhiều khoảng dòng, nút SHALL mở menu liệt kê từng khoảng dạng `file:dòng–dòng`.

#### Scenario: Nhảy về dòng gốc
- **WHEN** record có source `payment.log` dòng 405–409 và người dùng bấm "Xem dòng gốc"
- **THEN** Parsed View hiện, và dòng 405 của `payment.log` được chọn và cuộn tới

### Requirement: OTel View không đặt Marker và không sửa giá trị
OTel View SHALL hiện giá trị của OtelInput, không phải text đang lưu. Người dùng SHALL không đặt được Marker trong OTel View. Copy SHALL chép giá trị đang hiện.

#### Scenario: Chọn chữ trong OTel View
- **WHEN** người dùng chọn chữ trong một ô Body của OTel View
- **THEN** thanh màu Marker không hiện, và ⌘C chép đúng chuỗi đã chọn

### Requirement: Nhớ trạng thái OTel View
Hệ thống SHALL nhớ cho mỗi Log Space và khôi phục khi mở lại:
- chế độ OTel và chế độ con;
- Bộ lọc OTel và các cột đã thêm;
- trace đang mở, dạng chart, cửa sổ phóng to, các nhánh đã thu gọn;
- phần tử đang chọn.

Nếu trace hoặc phần tử đã nhớ không còn sau khi adapter chạy lại, view SHALL quay về Trace List hoặc bỏ chọn, không báo lỗi.

#### Scenario: Mở lại Project
- **WHEN** người dùng đang mở trace `4bf92f35…` dạng Flame, rồi đóng và mở lại Project
- **THEN** Log Space mở ở OTel, chế độ Traces, trace `4bf92f35…` dạng Flame

#### Scenario: Trace đã mất
- **WHEN** trace đã nhớ không còn trong OtelInput sau khi file đổi
- **THEN** chế độ Traces hiện Trace List
