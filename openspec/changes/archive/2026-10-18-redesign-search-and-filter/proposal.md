## Why

Ô Search dùng chung query language với Filter (`level == "error" && module contains "db"`), giống một câu SQL, nên người dùng phải học cú pháp chỉ để tìm một từ khóa. Filter thì dựa trên các hàng cố định (`level`, `module`, `message`, `request_id`) và danh sách module gán cứng, nên không phản ánh file đang mở. Khái niệm "View" còn bị dùng cho ba thứ khác nhau (panel View, `SavedView`, layout cột).

Debug cần hai việc rõ ràng: tìm nhanh text trên một dòng, và thu hẹp các dòng theo giá trị có thật trong file. Thiết kế lại hai chức năng này làm mỗi cái có một vai trò, và bỏ phần thừa.

## What Changes

**Search (text-only)**
- Search chỉ khớp text trên toàn bộ `raw_line`, không theo trường hay cột.
- Search Expression gồm các Search Term nối bằng `&&`, `||` và `()`; `&&` ưu tiên hơn `||`. Các chữ liền nhau là một Phrase, mọi chuỗi khoảng trắng đều thỏa một dấu cách; `"..."` là lối thoát tùy chọn.
- Không phân biệt hoa thường (theo Unicode), không bỏ dấu, không regex, không phủ định, không nút match case. `and`/`or` chỉ là chữ thường.
- Biểu thức gõ dở hoặc sai cú pháp: giữ Match của lần hợp lệ trước và hiện chỉ báo lỗi.
- Search chỉ tìm trên các dòng Filter đang hiển thị; không đếm Match bị Filter ẩn.

**Filter (theo cột phát hiện từ file)**
- Khi nạp file, phát hiện các cột và giá trị riêng biệt của từng cột.
- Cột lọc được: tối đa 500 giá trị riêng biệt, trừ `timestamp` và `message`/payload. Cột quá 500 hiện mờ kèm số đếm và lý do. Khi chọn cột để thêm Filter, mỗi cột hiện số giá trị riêng biệt.
- Mỗi cột lọc được có danh sách checkbox giá trị (kèm số dòng), và "Add value" để nhập giá trị chứa chuỗi (không regex). Giá trị chỉ khác hoa/thường được gộp thành một checkbox.
- Trong cùng cột các giá trị là OR; giữa các cột là AND. **Hide Mode** theo cột đổi "chỉ hiện" thành "ẩn".
- Dòng không có giá trị ở cột đó không bao giờ khớp: bị ẩn khi chỉ hiện giá trị đã chọn, vẫn hiện trong Hide Mode.
- `timestamp` có **Time Range** riêng (Từ/Đến), nhập theo dạng hiển thị trong cột Timestamp; dòng không có timestamp kế thừa dòng phía trên.
- Số đếm không đổi theo Filter khác; khi tail, giá trị mới được thêm vào cuối danh sách.
- Mỗi file log có Filter và Marker riêng, tự động nhớ và khôi phục khi mở lại file. Search Expression không được nhớ.
- Filter và Search dùng chung một hàm so khớp chuỗi Unicode.

**Bỏ**
- **BREAKING**: `SavedView` (không còn preset theo project) và `saved_view_test`.
- **BREAKING**: panel "View" trong context sidebar (density, nút ghim, module focus). Ghim cột vẫn có trên Column Header, kéo đổi độ rộng và Auto-fit vẫn giữ; density chuyển sang settings.
- **BREAKING**: menu nhanh trên giá trị level/module.
- **BREAKING**: ô Search không còn nhận cú pháp `field operator value`; Filter không còn regex và các hàng toán tử theo trường; Filter cũ đã lưu bị bỏ khi nạp session cũ.
- Parser/tokenizer/validator field-query, `language_service`, `completion_provider`, và `src/query/` (AST, `QueryEngine`) khi không còn tham chiếu.

## Capabilities

### New Capabilities
- `log-search`: Search text-only trên raw line với `&&`, `||`, `()`, Phrase, so khớp Unicode, giữ Match cũ khi biểu thức lỗi, highlight, điều hướng Match, chỉ tìm trên dòng đang hiển thị.
- `log-filter`: Filter theo cột phát hiện từ file (checkbox giá trị, Add value, Hide Mode, ngưỡng 500), Time Range cho `timestamp`, Filter và Marker riêng cho từng file.

### Modified Capabilities
<!-- openspec/specs/ hiện chưa có spec nào, nên không có capability sẵn để sửa. -->

## Impact

- **Code xóa:** `src/workspace/saved_view.rs` (giữ lại và chuyển `search_across_files`, `nearest_timestamp_index` sang file khác), `src/app/view/view_panel.rs`, menu nhanh (`QuickLogMenuKind` trong [filter_popover.rs](src/ui/filter_popover.rs)), `src/query/` (parser, validator, `language_service`, `completion_provider`, AST, engine) khi không còn tham chiếu.
- **Code mới:** module Search Expression (parser và matcher), hàm so khớp Unicode dùng chung, mô hình Filter mới (trạng thái theo cột và Time Range), bước phát hiện cột và giá trị khi nạp file.
- **Code sửa:** [src/search.rs](src/search.rs), [src/filter.rs](src/filter.rs), [src/infrastructure/search_engine.rs](src/infrastructure/search_engine.rs), [src/infrastructure/filter_engine.rs](src/infrastructure/filter_engine.rs), [src/app/search.rs](src/app/search.rs), [src/app/view/toolbar.rs](src/app/view/toolbar.rs), [src/ui/context_sidebar.rs](src/ui/context_sidebar.rs), [src/ui/filter_popover.rs](src/ui/filter_popover.rs), `src/workspace/` (session, project, working_set, state), [src/app/mod.rs](src/app/mod.rs), [src/parser/text.rs](src/parser/text.rs).
- **Test:** xóa test field-query và `saved_view_test`; sửa `filter_test`, `project_test`, `session_test`, `workspace_test`; thêm test cho Search, Filter mới, Time Range và nhớ theo file.
- **Dữ liệu đã lưu:** manifest session cần bump version; `saved_views` và filter cũ bị bỏ khi đọc.
- **Tài liệu:** [CONTEXT.md](CONTEXT.md) đã cập nhật; [ADR-0004](docs/adr/0004-search-is-text-only.md) và [ADR-0005](docs/adr/0005-filter-from-detected-columns.md) ghi lại quyết định.
- **Dependency:** có thể cần `unicode-normalization` (chuẩn hóa NFC); không cần crate khác.
