## Why

Điều tra một sự cố thường cần đọc cùng lúc nhiều file liên quan (`api.log`, `worker.log`, `db.log`) theo đúng thứ tự thời gian. Hiện mỗi tab chỉ mở một file, nên người dùng phải chuyển tab và tự dò timestamp. Người dùng cũng muốn dán một đoạn log chưa có file thật rồi xem nó ở dạng đã parse, nhưng ứng dụng chỉ mở được file có sẵn trên đĩa.

## What Changes

**Log Space**
- Thêm tầng **Log Space** giữa Project và file: Project chứa nhiều Log Space, mỗi Log Space chứa nhiều file.
- Log Space có hai loại và không bao giờ trộn lẫn:
  - **Imported Log Space**: các file có sẵn được kéo thả hoặc chọn khi tạo. File chỉ đọc, ứng dụng không bao giờ sửa file trên đĩa.
  - **Scratch Log Space**: tạo chỉ với tên. Ứng dụng tạo file rỗng `untitled.log` trong thư mục dữ liệu của ứng dụng để người dùng tự gõ hoặc dán log vào. Có thể thêm `untitled-2.log`, `untitled-3.log`…
- Thêm và gỡ file tự do sau khi tạo, qua menu chuột phải. Imported chỉ thêm bằng import; Scratch chỉ thêm file scratch rỗng.
- Thứ tự file trong một Log Space là thứ tự thêm vào hoặc theo tên. Thứ tự này dùng cho sidebar, File Section, và khi hai dòng trùng timestamp.

**Dialog tạo Log Space**
- Gồm ô tên, vùng kéo thả file kèm nút Browse, nút Create và Cancel.
- Create chỉ bật khi tên (đã trim) không rỗng và không trùng với Log Space khác trong cùng Project.
- Không có file thì tạo Scratch, có từ một file trở lên thì tạo Imported. Không có radio chọn loại.
- Thư mục được mở rộng một cấp và chỉ lấy file text. File binary bị loại kèm cảnh báo. Được bỏ từng file khỏi danh sách trước khi bấm Create.

**Màn chính: Parsed View và Raw View**
- **Parsed View** là canvas hiện tại (cột, Filter, Search, Marker). Mở một Log Space thì các dòng từ mọi file được **gộp và xếp xen kẽ theo timestamp**. Các cột là hợp các cột phát hiện được, gộp theo tên.
- Thêm **Source Column** ghi tên file của từng dòng. Cột ghim được và lọc được. Bật tắt trong popover **Quick Settings** mới trên toolbar. Mặc định hiện khi Log Space có từ hai file trở lên.
- **File View**: mở riêng một file của Log Space, có Filter riêng. Marker gắn theo dòng nên dùng chung với Log Space.
- **Raw View** hiện text đúng như lưu trên đĩa. Mỗi file là một **File Section** thu gọn và mở rộng được, xếp chồng theo kiểu multi-buffer. Raw View luôn hiện mọi dòng và mọi section, bỏ qua Filter. Chỉ Search và Marker áp dụng; Search chỉ tô sáng, không ẩn gì.
- Nút chuyển Raw/Parsed trên toolbar:
  - Imported mở ở Parsed; Raw View chỉ đọc.
  - Scratch mở ở Raw nếu rỗng, nếu không thì nhớ chế độ dùng lần cuối. Chỉ sửa được trong Raw View. Chuyển sang Parsed thì parse lại.
- Scratch tự lưu sau khoảng 500 ms ngừng gõ, có ⌘S. Save As ghi một bản sao ra vị trí người dùng chọn.

**Sidebar và tab**
- Sidebar hiện các Log Space có chevron để mở danh sách file, nút **+** để mở dialog, và menu chuột phải (Rename, Add files… / New scratch file, Sort files by, Delete; trên file: Remove from Log Space, Reveal in Finder, Rename, Save As…, Delete).
- Mỗi tab là một Log Space hoặc một File View. Mở lại thì nhảy tới tab đã có.

**Thay đổi so với change `redesign-search-and-filter`**
- **BREAKING**: Filter không còn gắn với từng file mà gắn với view: một Filter chung cho mỗi Log Space, một Filter riêng cho mỗi File View. Marker gắn với một dòng của một file.
- **BREAKING**: density chuyển từ settings sang Quick Settings.
- Search trong Raw View tìm trên mọi dòng, vì Raw View không áp dụng Filter.
- **BREAKING**: `Project.files` và `TabTarget::LogFile` được thay bằng Log Space; session cũ không được chuyển đổi.

## Capabilities

### New Capabilities
- `log-space`: Log Space trong Project, hai loại Imported và Scratch, dialog tạo, thêm/gỡ file, thứ tự file, sidebar và menu chuột phải, tab theo Log Space hoặc File View.
- `merged-timeline`: Parsed View của Log Space: gộp dòng theo timestamp, hợp cột theo tên, Source Column, Quick Settings, File View.
- `raw-view`: Raw View với File Section, chuyển Raw/Parsed, sửa file scratch, tự lưu, Save As, Marker khi sửa dòng.

### Modified Capabilities
- `log-filter`: Filter gắn với Log Space hoặc File View thay vì với file. Marker gắn với dòng của file. Density chuyển sang Quick Settings.
- `log-search`: Search trong Raw View tìm trên mọi dòng vì Raw View không áp dụng Filter.

> `log-filter` và `log-search` hiện nằm trong change `redesign-search-and-filter` chưa archive. Cần archive change đó trước change này.

## Impact

- **Mô hình dữ liệu:** thêm Log Space vào [project.rs](src/workspace/project.rs) thay cho `Project.files`; đổi `TabTarget` trong [working_set.rs](src/workspace/working_set.rs) (bỏ `LogFile` và `ChronicleStream`, thêm Log Space và File View); [session.rs](src/workspace/session.rs) bump version, không đọc session cũ; [state.rs](src/workspace/state.rs).
- **Nạp và gộp:** [parser](src/parser/mod.rs) dùng cho từng file, thêm bước gộp theo timestamp; [filter_index.rs](src/filter_index.rs) và [filter.rs](src/filter.rs) tính trên tập dòng đã gộp; [model.rs](src/model.rs) cần biết Source File của mỗi dòng.
- **UI:** dialog tạo Log Space mới (kéo thả và [rfd](src/app/commands.rs)); [sidebar.rs](src/ui/sidebar.rs), [project_node.rs](src/ui/project_node.rs), [file_menu.rs](src/ui/file_menu.rs); [canvas.rs](src/ui/canvas.rs) thêm Source Column; [toolbar.rs](src/app/view/toolbar.rs) thêm nút Raw/Parsed và Quick Settings; [tab_bar.rs](src/app/view/tab_bar.rs), [tabs.rs](src/app/tabs.rs); thành phần Raw View mới (editor nhiều File Section).
- **Lưu trữ:** thư mục dữ liệu của ứng dụng chứa file scratch theo Log Space.
- **Test:** sửa `project_test`, `session_test`, `workspace_test`, `filter_test`; thêm test cho gộp theo timestamp, hợp cột, điều kiện bật Create, Marker khi sửa dòng scratch.
- **Tài liệu:** [CONTEXT.md](CONTEXT.md) đã cập nhật; [ADR-0006](docs/adr/0006-log-space-merged-timeline.md) ghi lại quyết định.
