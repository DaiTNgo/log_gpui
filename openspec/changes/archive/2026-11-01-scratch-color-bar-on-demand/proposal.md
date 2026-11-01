## Why

Trong Raw View của một file scratch đang sửa được, thanh màu tự bật lên mỗi khi vùng chọn của editor khác rỗng: kéo chuột, Shift+mũi tên, Cmd+A đều mở nó. Ở đó người dùng chọn text gần như luôn để sửa hoặc copy. Marker trên dòng scratch cũng mất ngay khi sửa dòng (rule 26). Vì vậy thanh màu tự bật chủ yếu làm phân tâm. Nó cũng làm hai chế độ của Raw View cư xử khác nhau: danh sách chỉ đọc mở thanh theo một cử chỉ (nhả chuột sau khi kéo), còn editor mở theo trạng thái vùng chọn.

## What Changes

- Trong editor của scratch (Raw View sửa được), chọn text không còn tự mở thanh màu, dù chọn bằng chuột hay bàn phím.
- Thêm lệnh **"Mark: Color Selection…"** (mặc định `cmd-shift-h`, đổi được trong Keymap, có trong Command Palette). Lệnh mở thanh màu cho vùng chọn hiện tại ở bất kỳ view nào có vùng chọn: editor scratch, danh sách Raw chỉ đọc, Parsed View. Trong editor scratch, đây là cách để tô Marker hoặc tạo Text Highlight.
- Bấm (đặt con trỏ) lên một Marker hay Text Highlight có sẵn trong editor scratch vẫn mở thanh để đổi màu hoặc gỡ, như hiện nay.
- Danh sách Raw chỉ đọc và Parsed View giữ nguyên: nhả chuột sau khi kéo chọn vẫn tự mở thanh.

## Capabilities

### New Capabilities

(không có)

### Modified Capabilities

- `text-marker`: thanh màu nổi không còn tự mở khi chọn text trong editor scratch; thêm lệnh mở thanh màu cho vùng chọn hiện tại.
- `raw-view`: nói rõ cách tô Marker / Text Highlight trong Raw View sửa được (chỉ qua lệnh), và việc bấm lên màu có sẵn vẫn mở thanh.

## Impact

- `src/app/raw.rs` (khoảng dòng 720–760): đoạn đồng bộ thanh màu theo vùng chọn của editor thôi tạo `ColorTarget::RawMark` từ vùng chọn khác rỗng; giữ phần `RawRecolor` / `RecolorTextHighlight` khi con trỏ đứng trên màu có sẵn.
- `src/keymap.rs`: thêm `Command::ColorSelection` với id, tên và phím mặc định.
- `src/app/color.rs`, `src/app/raw_list.rs`: hàm xử lý lệnh, mở `ColorBar` cho vùng chọn của view đang hiện (editor, list hoặc Parsed).
- `CONTEXT.md`: thêm một câu vào mục **Marker** về việc tô trong scratch đang sửa.
- Không đổi dữ liệu lưu, không có migration.
