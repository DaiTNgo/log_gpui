## Why

Focus Pane nằm dưới cửa sổ chính nên bị giới hạn chiều cao, và chia màn hình với view. Khi có nhiều dòng cần đọc, hoặc khi có màn hình thứ hai, người dùng muốn Focus Pane to và rộng hơn mà vẫn giữ view đầy đủ ở cửa sổ chính.

## What Changes

- Nút "Move to window" trên Focus Pane (và lệnh trong command palette) chuyển Focus Pane sang một **Focus Window**: một cửa sổ hệ điều hành riêng, đổi kích thước được, kéo sang màn hình khác được.
- Khi Focus Window mở, Focus Pane chỉ có ở đó. Cửa sổ chính bỏ Focus Pane, và view chiếm lại toàn bộ chiều cao.
- Focus Window luôn theo view đang mở ở cửa sổ chính. Đổi tab ở cửa sổ chính thì Focus Window đổi theo.
- Focus Window gồm ô Search của Focus Tab đang mở, dải Focus Tab, Focus Find và danh sách dòng. Filter vẫn ở sidebar của cửa sổ chính.
- Đồng bộ dòng hiện tại giữa view và Focus Pane hoạt động như cũ, qua hai cửa sổ.
- Nút "Move back" trên Focus Window, hoặc đóng Focus Window, đưa Focus Pane về dưới cửa sổ chính, với chiều cao như trước khi tách.
- Focus Window không được nhớ: khởi động lại app thì Focus Pane luôn ở cửa sổ chính.

## Capabilities

### New Capabilities
- `focus-window`: tách Focus Pane ra cửa sổ riêng, nội dung, đồng bộ với cửa sổ chính, đưa về.

### Modified Capabilities

## Impact

- Phụ thuộc change `add-focus-pane`: cần cài và archive trước.
- Đây là lần đầu app mở hơn một cửa sổ. Code chịu ảnh hưởng: `src/main.rs` / `src/app/mod.rs` (mở và đóng cửa sổ), module render Focus Pane (dùng chung cho hai nơi), `src/app/view/mod.rs` (bỏ Focus Pane khi đã tách), keymap và command palette.
- Không đổi dữ liệu lưu.
