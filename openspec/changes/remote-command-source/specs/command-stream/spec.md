## ADDED Requirements

### Requirement: Stream của dòng
Mỗi dòng của Remote Command SHALL giữ Stream mà nó đến từ: `stdout` hoặc `stderr`. Dòng của Remote File, file local và file scratch SHALL NOT có Stream. Stream SHALL là của chính dòng và SHALL NOT được Continuation Line thừa kế từ Parent Line.

#### Scenario: Stack trace ra stderr sau dòng stdout
- **WHEN** lệnh in `2026-10-19T10:02:03Z GET /a 500` ra stdout rồi `  at handler (a.js:3)` không có timestamp ra stderr
- **THEN** dòng thứ hai là Continuation Line của dòng thứ nhất nhưng có Stream `stderr`

### Requirement: Số dòng stderr trong Gutter
Hệ thống SHALL vẽ số dòng của dòng có Stream `stderr` bằng màu stderr, có tooltip "stderr", trong Parsed View, Raw View và Focus Pane. Màu này SHALL NOT thay màu nền của Highlight Rule hay màu của Marker, và SHALL NOT thay đổi text đã lưu của dòng.

#### Scenario: Dòng stderr có Highlight Rule
- **WHEN** một dòng stderr khớp Highlight Rule màu vàng và có một Marker xanh
- **THEN** row có nền vàng, Marker xanh hiện trên phần text đã đánh dấu, và số dòng trong Gutter có màu stderr

#### Scenario: Copy dòng stderr
- **WHEN** người dùng copy một dòng stderr
- **THEN** đoạn copy chỉ có text của dòng, không có chữ "stderr"

### Requirement: Stream Column
Hệ thống SHALL có Stream Column trong Parsed View của Log Space có ít nhất một Remote Command, hiện `stdout` hoặc `stderr` cho dòng của Remote Command và để trống cho dòng của file. Stream Column SHALL ẩn mặc định, SHALL được bật hoặc tắt trong Quick Settings, và SHALL NOT pin được.

#### Scenario: Bật Stream Column
- **WHEN** người dùng bật Stream Column trong Quick Settings của "Checkout" có một Remote File và một Remote Command
- **THEN** dòng của Remote Command hiện `stdout` hoặc `stderr` trong cột đó, còn dòng của Remote File để trống

### Requirement: Filter theo Stream
Khi Log Space có ít nhất một Remote Command, Filter SHALL có Stream như một Filterable Column với Filter Value `stdout` và `stderr`, dù Stream Column đang ẩn hay hiện, đếm theo các dòng đang giữ như mọi cột. Dòng không có Stream SHALL NOT khớp Filter Value nào của Stream, và SHALL vẫn hiện khi Stream ở Hide Mode.

#### Scenario: Chỉ xem stderr
- **WHEN** người dùng tích `stderr` dưới Stream
- **THEN** Focus Pane chỉ liệt kê các dòng stderr của Remote Command, không có dòng của Remote File

#### Scenario: Ẩn stdout
- **WHEN** người dùng bật Hide Mode cho Stream và tích `stdout`
- **THEN** Focus Pane liệt kê dòng stderr và mọi dòng của Remote File

### Requirement: Highlight Rule theo Stream
Highlight Rule SHALL dùng được Stream làm cột điều kiện như mọi cột, và áp dụng theo Stream của chính dòng.

#### Scenario: Tô dòng stderr
- **WHEN** người dùng tạo Highlight Rule "Stream = stderr" màu đỏ nhạt
- **THEN** mọi row có Stream `stderr` có nền đỏ nhạt trong Parsed View, còn row `stdout` và row của file thì không

### Requirement: Search bỏ qua Stream
Search và Focus Find SHALL chỉ khớp raw line và SHALL NOT khớp chữ `stdout` hay `stderr` của Stream.

#### Scenario: Tìm chữ stderr
- **WHEN** người dùng tìm `stderr` và không dòng nào có chữ đó trong text
- **THEN** không có Match nào, kể cả với dòng có Stream `stderr`
