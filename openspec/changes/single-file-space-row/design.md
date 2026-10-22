## Quyết định

**D1. Một hàm quyết định chung.** Thêm `LogSpace::shows_group() -> bool` = `files.len() >= 2 || !globs.is_empty()`. Sidebar, chevron và menu đều dùng hàm này, để không có hai nơi tự đếm file. Tính lại mỗi lần render, nên glob khớp thêm file thì chevron tự hiện.

**D2. Chỉ là cách vẽ.** Space 1 file không bị đổi trong model. `expanded_spaces` không bị sửa khi chevron bị ẩn, nên khi có file thứ 2 thì trạng thái đã lưu dùng lại.

**D3. Space có Live Glob luôn là nhóm.** Header glob là nơi duy nhất hiện pattern, Stopped và waiting. Ẩn nó sẽ phải dời các trạng thái này lên dòng Space, tốn thêm thiết kế mà lợi ích nhỏ.

**D4. Action của file duy nhất.**
- Status bar: chỉ các nút gắn với live (Load more, Clear, Live/Pause, Back to Live Window), theo điều kiện hiện có của từng loại file.
- Menu chuột phải của Space: các action còn lại, gộp phẳng, không submenu "File".

**D5. Xóa file duy nhất.** Mục xóa trong menu của Space 1 file gọi cùng đường xóa Space hiện có, cộng dọn Remote Copy (`delete_copy_of`) và đóng tab (`close_tabs_for_file`). Scratch dùng lại xác nhận `DeleteScratchFile`, vì `LogSpace::remove_file` chặn xóa file scratch cuối.

## Rủi ro

- Status bar hẹp: nhiều nút có thể chật. Hai nút ít dùng (Back to Live Window, Clear) chỉ hiện khi áp dụng.
- Space Imported hoặc Remote đã có sẵn 0 file vẫn là trường hợp của hôm nay, không đổi.
