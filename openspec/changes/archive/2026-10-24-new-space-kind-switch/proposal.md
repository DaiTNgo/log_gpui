## Why

Dialog New Log Space đoán loại Log Space từ những gì người dùng đã chọn: có file local thì Imported, có Remote File thì Remote, không có gì thì Scratch ("Create empty"). Người dùng không thấy mình đang tạo loại nào, phần local và phần remote nằm chồng lên nhau và phải có câu nhắc riêng để giải thích vì sao không trộn được. Chọn loại trước, rõ ràng, sẽ bỏ được cả lớp đoán ngầm lẫn các câu nhắc đó.

## What Changes

- Dialog có thanh chọn ba nấc `[ Local files | Remote | Scratch ]` (segmented control) ngay dưới tiêu đề, kèm một dòng mô tả nấc đang chọn. Dialog luôn mở ở nấc Scratch, không nhớ lần trước.
- Mỗi nấc giữ danh sách riêng; gạt nấc chỉ ẩn danh sách kia, không xóa và không hiện số đếm. Nút Create chỉ tạo từ nấc đang hiện.
- **BREAKING**: bỏ việc suy loại từ danh sách file. Nấc Local hoặc Remote chưa có gì thì nút Create bị tắt; Scratch chỉ tạo được từ nấc Scratch ("Create empty" không còn).
- Bỏ hai câu nhắc không trộn local/remote (`MIX_HINT_LOCAL`, `MIX_HINT_REMOTE`).
- Tên gợi ý đi theo nấc đang hiện khi người dùng chưa tự gõ tên; nấc Scratch gợi ý "Scratch" (thêm số nếu trùng).
- ⌘1/⌘2/⌘3 chọn nấc; Tab dừng ở thanh chọn và ←/→ đổi nấc.
- Thả file vào dialog đang mở: tự gạt sang Local và thêm vào danh sách.
- **BREAKING**: thả file vào cửa sổ khi dialog chưa mở tạo ngay một Imported Log Space chứa mọi file đã thả (thư mục được mở rộng, Archive được đọc xong mới tạo), tên theo file đầu tiên. Nếu có file nào không dùng được thì không tạo, mà mở dialog ở nấc Local với danh sách và lỗi từng file.

## Capabilities

### New Capabilities

(không có)

### Modified Capabilities

- `log-space`: dialog New Log Space chọn loại bằng thanh chọn ba nấc thay cho suy loại từ file; điều kiện bật Create theo từng nấc; thả file vào cửa sổ tạo Imported Log Space ngay.

## Impact

- `src/app/view/dialogs.rs`: bố cục dialog, thanh chọn (`TabBar` kiểu `Segmented` của gpui-component 0.7.3), nội dung từng nấc, bỏ MIX_HINT.
- `src/app/spaces.rs`: trạng thái nấc trong `NewSpaceDialog`, tên gợi ý theo nấc, `new_space_create` theo nấc, `drop_paths_on_window` tạo ngay hoặc mở dialog.
- `src/workspace/new_space.rs`: nhãn và điều kiện Create theo nấc, tên gợi ý cho Scratch.
- `src/app/keyboard.rs`: ⌘1–3 khi dialog mở.
- `src/app/remote_picker.rs`, `src/app/remote_command.rs`: thêm vào danh sách Remote không còn chặn khi có file local.
- Test: `tests/new_space_dialog_test.rs`, `tests/new_space_test.rs`, `tests/archive_test.rs` và các test đang dựa vào "Create empty" hoặc chặn trộn local/remote.
- `CONTEXT.md`: định nghĩa Scratch/Imported Log Space nhắc cách tạo.
