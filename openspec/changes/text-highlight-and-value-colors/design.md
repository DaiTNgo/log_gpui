## Context

Hiện có ba cách tô màu (ADR 0007, 0008):

- **Marker** (`src/marker.rs`, `src/app/marking.rs`): màu trên một khoảng byte của một dòng, lưu trong `LogFile` theo dòng và Generation, hiện ở Raw View và Parsed View, có vạch trên overview ruler.
- **Highlight Rule** (`src/highlight.rs`, `src/app/highlights.rs`): điều kiện trên giá trị một cột, tô nhạt cả row trong Parsed View, thuộc `Project.highlight_rules`.
- **Search Highlight** (`src/search_color.rs`): màu của từng Phrase trong Focus Pane. Hue sinh từ Phrase đã đổi về chữ thường bằng FNV-1a (`hue_for_key`), cố định qua mọi phiên; `phrase_rgb` đặt saturation/lightness theo theme để chữ đọc rõ.

Thanh màu nổi (`ColorBar`, `ColorTarget` trong `src/app/color.rs`, vẽ ở `src/app/view/color_bar.rs`) là lối vào chung cho Marker (Parsed và Raw) và màu của Highlight Rule. Menu chuột phải của ô (`src/ui/cell_menu.rs`) có "Change color" / "Remove marker".

Trong canvas (`src/ui/canvas.rs`), nền của row được chồng theo thứ tự trang → Highlight Rule → hàng đang chọn → Match; Marker và vùng chọn vẽ trên text qua `CellCtx`.

## Goals / Non-Goals

**Goals:**
- Một thanh màu, hai phạm vi: Marker (vị trí) và Text Highlight (mọi chỗ trùng).
- Value Color tự động, không cần cấu hình, ổn định qua mọi phiên.
- Thứ tự lớp màu rõ ràng, chữ luôn đọc được.
- Không đổi Index, Filter, định dạng file log; session cũ vẫn mở được.

**Non-Goals:**
- Đổi màu hay tắt Value Color theo cột hoặc theo giá trị.
- Text Highlight bằng regex hay Search Expression.
- Danh sách quản lý Text Highlight, hay vạch Text Highlight trên overview ruler.
- Value Color trong Raw View (xem Open Questions).

## Decisions

### 1. Text Highlight là khái niệm riêng, không phải "Marker có scope"
Marker khóa theo (file, Generation, dòng, byte range) và có luật riêng khi dòng rời Live Window (L12) hay khi mở lại Remote Copy (ADR 0011). Text Highlight khóa theo chuỗi và sống ở Project. Gộp chung một kiểu dữ liệu chỉ thêm một cờ phạm vi kèm hai bộ luật. Vì vậy chúng chỉ chung lối vào (thanh màu); dữ liệu, lưu trữ và vẽ là riêng.

- Kiểu mới `TextHighlight { text: String, key: String /* chữ thường */, color: HighlightColor, seq: u64 }` trong module mới `src/text_highlight.rs`.
- `Project.text_highlights: Vec<TextHighlight>` với `#[serde(default)]`. `seq` tăng mỗi lần tạo hoặc đổi màu, dùng để quyết định cái nào vẽ trên khi chồng nhau.
- Trùng `key` thì đổi màu, không thêm cái mới.

*Đã cân nhắc:* lưu Text Highlight theo Log Space. Bị loại vì một id thường đi qua nhiều Log Space của cùng một hệ thống, và Highlight Rule cũng đã thuộc Project.

### 2. So khớp: chữ thường, chuỗi con, không regex
Tìm mọi lần xuất hiện của `key` trong `raw_line` đã đổi về chữ thường, không chồng lấn trong cùng một Text Highlight. Kết quả là danh sách `(byte range, color, seq)` theo byte của text đã lưu. Đổi về chữ thường phải giữ được ánh xạ byte về text gốc (chỉ fold ASCII, hoặc dùng cùng cách fold với `Matcher` của Search để nhất quán; xem Risks).

Kết quả được tính lúc vẽ cho các dòng đang hiện, không lưu vào Index. Số dòng hiện mỗi khung hình nhỏ và số Text Highlight thường dưới vài chục, nên chi phí không đáng kể. Có thể dùng `aho-corasick` nếu đo thấy chậm, nhưng không thêm từ đầu.

### 3. Value Color dùng lại hash của Phrase
`value_rgb(value, tone)` = `phrase_rgb(hue_for_key(&casefold(value)), tone)`, rồi điều chỉnh lightness để màu chữ đủ tương phản với nền trang (Value Color là màu chữ, còn Phrase là màu nền). FNV-1a đã cố định trong `search_color.rs`, nên màu ổn định qua mọi phiên và mọi máy.

Một hệ quả có chủ ý: cùng một chuỗi có cùng hue ở Value Color và ở Phrase của Search. Người dùng search `T-12` sẽ thấy Match cùng tông với màu chữ của ô `T-12`.

**Cột được tô:** quyết định theo cột của Log Format, không theo dữ liệu, nên không phải đếm giá trị:
- loại timestamp, message/payload (`MESSAGE_COLUMN`), level;
- loại cột có `FieldType::Number | Boolean | Json`;
- còn lại (`String`, `Enum`, các cột không rõ kiểu) được tô.

*Đã cân nhắc:* dùng lại Filterable Column (tối đa 500 giá trị). Bị loại vì cột định danh như `request_id` thường vượt 500, mà đó lại là use case chính.

### 4. Tránh màu Text Highlight
Khi vẽ, nếu hue của giá trị cách hue của một màu Text Highlight của Project dưới `MIN_HUE_GAP`, đẩy hue theo `HUE_STEP` tới chỗ trống gần nhất (cùng thuật toán như `phrase_hues_following`). Màu Custom lấy hue từ RGB. Hệ quả là màu của một giá trị có thể đổi khi người dùng thêm Text Highlight. Chấp nhận được, vì nó vẫn ổn định khi tập Text Highlight không đổi.

### 5. Thứ tự lớp: tách nền và màu chữ
Nền và màu chữ là hai kênh khác nhau, nên thứ tự lớp được định nghĩa riêng cho từng kênh:

```
Nền (trên thắng):   Marker > Match của Search (Focus Pane) > Text Highlight > hàng chọn > Highlight Rule > trang
Màu chữ:            có nền Marker/Text Highlight → chọn theo tương phản (text_tone_on)
                    ngược lại, Value Colors bật và cột được tô → Value Color
                    ngược lại → màu chữ thường
```

Match nằm trên Text Highlight vì trong Focus Pane, việc người dùng đang search là ngữ cảnh gần nhất. Quy ước "người dùng tô thì có nền, app tô thì chỉ đổi màu chữ" vẫn giữ đúng.

Trong code, `CellCtx` nhận thêm `text_highlights: &[(Range<usize>, HighlightColor)]` (đã giải quyết chồng lấn theo `seq`) và `value_color: Option<Hsla>` cho mỗi ô. Hàm vẽ text của ô gộp các khoảng Marker, Text Highlight và Match thành các run, giống cách Marker đang làm. Raw View (`src/app/raw.rs` / `raw_list.rs`) nhận cùng danh sách khoảng Text Highlight.

### 6. Thanh màu có phạm vi (phương án A, Alt làm phím tắt)
Thanh màu vẫn tự hiện khi nhả chuột sau khi kéo chọn (`finish_text_selection`). Phía trên hàng màu có một nút chọn hai phần: **"Chỉ dòng này"** | **"Mọi chỗ “<text>” · N"**. Mặc định là "Chỉ dòng này" mỗi lần thanh mở, không nhớ lựa chọn lần trước. Như vậy một lần chọn text để copy không vô tình tô cả file.

```
┌──────────────────────────────────────────────┐
│ [ Chỉ dòng này ][ Mọi chỗ “conn-7f3a” · 4 ]  │
│ ■ ■ ■ ■ ■ ■  Palette  ◐                      │
└──────────────────────────────────────────────┘
```

- **Alt+bấm** một màu thì tạo Text Highlight ngay, bất kể nút chọn đang ở đâu. Tooltip của các ô màu ghi rõ phím này.
- `N` là số dòng của view đang mở có chứa đoạn text đó. Việc đếm chạy ở background, theo cùng cách Search đếm Match (ADR 0014), và phần "· N" chỉ hiện khi đếm xong. File lớn hay đang đếm thì nút chỉ ghi "Mọi chỗ “<text>”". Đổi vùng chọn hoặc đóng thanh thì hủy lần đếm đang chạy.
- Text dài bị cắt giữa bằng "…" để thanh không quá rộng.
- "Mọi chỗ" bị tắt khi vùng chọn qua nhiều dòng hoặc chỉ có khoảng trắng, có tooltip giải thích. Lúc đó Alt+bấm tạo Marker như bấm thường.
- Thanh mở để đổi màu (`RecolorMarker`, `RecolorTextHighlight`, `Rule`) không có nút phạm vi.

Về code: `ColorBar` thêm `scope: MarkScope { Line, Everywhere }` và `everywhere_count: Option<usize>`. Thêm các đích `ColorTarget::TextHighlight { text }` và `ColorTarget::RecolorTextHighlight { key }`. `apply_bar_color(color, alt, cx)` chuyển `MarkSelection`/`RawMark` sang tạo Text Highlight khi `scope == Everywhere || alt`.

*Đã cân nhắc:* **B**, hai hàng màu (Marker / Text Highlight). Luôn một cú bấm, nhưng thanh to gấp đôi và hiện ra mỗi lần chọn text. **C**, chỉ Alt+bấm. Thanh không đổi, nhưng tính năng bị giấu. A cộng Alt giữ thanh gọn, tính năng nhìn thấy được, và người quen tay vẫn có lối tắt.

### 7. Menu chuột phải
`cell_menu` (Parsed View) tìm Text Highlight dưới byte được bấm, dùng cùng hàm so khớp lúc vẽ, lấy cái có `seq` lớn nhất. Nếu có thì thêm "Change highlight color" và "Remove highlight". Mục của Marker và Text Highlight đứng riêng, nên gỡ Marker không đụng tới Text Highlight bên dưới. Raw View không có menu chuột phải: ở đó bấm lên Marker mở thanh để đổi màu hoặc gỡ, nên bấm lên một Text Highlight (không có Marker) cũng mở thanh `RecolorTextHighlight { raw: true }` có nút "Remove highlight". "Remove highlight" gỡ Text Highlight khỏi Project, tức là gỡ mọi chỗ của nó; không cần lệnh "Remove all" riêng.

```
Mark with color
───────────────────────────
■ Change color
  Remove marker
───────────────────────────
■ Change highlight color
  Remove highlight
```

### 8. Công tắc
Thêm `workspace_state.show_value_colors: bool` (serde default `true`), một hàng trong Settings cạnh Search Highlight, và một lệnh Keymap để bật/tắt.

## Risks / Trade-offs

- [Value Color làm Parsed View rối, nhất là cột String gần như không lặp như URL] → Chỉ đổi màu chữ, lightness cố định theo theme, có công tắc toàn app. Theo dõi trong thực tế, nếu cần thì thêm luật loại cột sau.
- [Fold chữ thường làm lệch byte với ký tự không phải ASCII] → Dùng cùng cách fold với Search (`Matcher`), có test cho chữ có dấu tiếng Việt và ký tự đổi độ dài khi fold.
- [Màu chữ không đủ tương phản trên row đã được Highlight Rule tô] → Row tint của Highlight Rule nhạt (`Weight::Row`). Test tương phản cho 10 hue × 2 theme; nếu dưới ngưỡng thì dùng màu chữ thường.
- [Màu hash trùng nhau giữa hai giá trị khác nhau] → Không tránh được với 360 hue. Chấp nhận như lnav; Value Color chỉ là gợi ý thị giác, không phải định danh.
- [Text Highlight quá ngắn (`a`, `1`) tô kín màn hình] → Đó là việc người dùng chủ động làm, gỡ được ngay bằng chuột phải. Không chặn.
- [Chi phí vẽ khi có nhiều Text Highlight] → Chỉ so khớp các dòng đang hiện. Nếu đo thấy chậm thì chuyển sang `aho-corasick`.

## Migration Plan

Không cần migration: `Project.text_highlights` và `show_value_colors` có serde default. Session cũ mở ra không có Text Highlight và Value Colors bật. Muốn rollback thì bỏ hai trường; serde bỏ qua trường lạ khi đọc.

## Open Questions

- Value Color có nên hiện trong Raw View không? Parser đã giữ khoảng ký tự của từng trường (ADR 0007), nên làm được. Change này để Raw View giữ text như lưu, không có màu chữ tự động, giống việc Highlight Rule chỉ ở Parsed View.
