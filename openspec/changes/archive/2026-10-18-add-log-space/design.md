## Context

Hiện trạng liên quan:
- `Project` giữ thẳng danh sách file (`Project.files: Vec<LogFileDescriptor>`). Mỗi `LogFileDescriptor` mang Filter và Marker của file đó ([project.rs](src/workspace/project.rs)).
- Tab trỏ tới `TabTarget::LogFile { file_id }`. Đã có biến thể `ChronicleStream { file_ids }` cho view gộp, nhưng mới chỉ được vẽ tên trên tab bar, chưa có logic gộp ([working_set.rs](src/workspace/working_set.rs), [tab_bar.rs](src/app/view/tab_bar.rs)).
- Mỗi file được parse riêng (`parse_file_with_schema`). `LogEntry` có `index` (số dòng, bắt đầu từ 1), `timestamp: f64` và `raw_line` ([model.rs](src/model.rs)).
- Filter được dựng từ cột phát hiện khi nạp file ([filter_index.rs](src/filter_index.rs), [filter.rs](src/filter.rs)).
- Session lưu tại `~/.config/log_gpui/session.json` ([session.rs](src/workspace/session.rs)). Chọn file dùng `rfd`. Chưa có kéo thả và chưa có tail thật.
- gpui-component 0.7.3 có `Editor`/`EditorState` (nhiều dòng, `readonly`) và hỗ trợ `ExternalPaths` cho kéo thả.

Thuật ngữ và rule đã chốt trong [CONTEXT.md](CONTEXT.md) (Project, Log Space, Imported/Scratch Log Space, Source File, File View, Source Column, Quick Settings, Raw View, File Section, Parsed View; rule 17, 20, 22–29). Quyết định ghi tại [ADR-0006](docs/adr/0006-log-space-merged-timeline.md).

Change này phụ thuộc `redesign-search-and-filter`: dùng Filter, Search và hàm so khớp Unicode của change đó.

## Goals / Non-Goals

**Goals:**
- Log Space là đơn vị nhóm file, có hai loại Imported và Scratch, tạo bằng một dialog.
- Parsed View gộp dòng từ mọi file theo timestamp, có Source Column.
- File View cho từng file, có Filter riêng.
- Raw View nhiều File Section; sửa được với Scratch, chỉ đọc với Imported.
- Filter gắn với view, Marker gắn với dòng của file.

**Non-Goals:**
- Chuyển đổi session cũ.
- Tail thật cho file Imported. Change này chỉ quy định hành vi khi có tail (chèn theo timestamp, viewport không nhảy); phần đọc file đang ghi thêm làm ở change khác.
- Kéo thả để sắp xếp lại file.
- Đổi múi giờ hay chuẩn hóa timestamp giữa các file.
- Sửa file trong Parsed View, và sửa file Imported.
- Dialog cho việc tạo Project (vẫn một click như hiện tại).

## Decisions

### 1. Mô hình: `Project → LogSpace → LogFile`

```
Project
 └─ log_spaces: Vec<LogSpace>
      ├─ id, name, kind: Imported | Scratch
      ├─ file_order: Added | Name
      ├─ files: Vec<LogFile>        (file_id, display_name, path, added_seq, markers)
      ├─ filter: FilterState        (Filter chung của Log Space)
      └─ file_filters: Map<file_id, FilterState>   (Filter của từng File View)
```

- `kind` là trường của Log Space, không phải của file. Nhờ vậy rule "Imported và Scratch không trộn lẫn" được giữ bằng kiểu dữ liệu: `add_imported_files` chỉ hợp lệ khi `kind == Imported`, `new_scratch_file` chỉ hợp lệ khi `kind == Scratch`.
- Marker vẫn nằm trên file (`file_id` + số dòng). Gỡ file thì Marker đi theo.
- Bỏ `Project.files`, `LogFileDescriptor.filter`, `TabTarget::LogFile` và `ChronicleStream`.

*Phương án khác:* giữ `Project.files` và cho Log Space chỉ chứa danh sách `file_id`. Bị loại vì cùng một file có thể nằm trong hai Log Space, khiến chuyện Marker và gỡ file trở nên mập mờ.

### 2. Tab trỏ tới view, không trỏ tới file

```
TabTarget::LogSpace  { space_id }
TabTarget::FileView  { space_id, file_id }
TabTarget::InvestigationBoard { case_id }   (giữ nguyên)
```

`TabSessionState` thêm `mode: Raw | Parsed` và trạng thái thu gọn của từng File Section. Mở một target đã có tab thì kích hoạt tab đó.

### 3. Gộp theo timestamp bằng merge k đường

Mỗi file được parse riêng và cache theo `file_id`. View gộp là một danh sách tham chiếu `(file_idx, line_idx)` sắp theo khóa:

```
(timestamp_hiệu_dụng, thứ_tự_file, số_dòng)
```

- `timestamp_hiệu_dụng`: dòng tiếp nối kế thừa dòng cha. Dòng trước timestamp đầu tiên lấy timestamp đầu tiên của file. File không có timestamp nào nhận `+∞`, nên rơi xuống cuối và được gắn cờ.
- Từng file vốn đã gần như tăng dần theo thời gian, nên merge k đường bằng heap chạy O(n log k). Không cần sort lại toàn bộ.
- Dòng tiếp nối dùng chung khóa với dòng cha và có số dòng lớn hơn, nên luôn đi liền ngay sau dòng cha.
- Đổi `file_order` thì chỉ cần merge lại, không parse lại.

*Phương án khác:* nối mọi dòng rồi sort ổn định. Đơn giản nhưng O(n log n) trên toàn bộ, và chậm hơn rõ rệt khi chèn thêm file.

### 4. Hợp cột theo tên, chỉ mục Filter trên tập đã gộp

Schema của Log Space là hợp các schema của từng file, giữ thứ tự cột của file đầu tiên, rồi thêm cột mới theo thứ tự file. Source Column là một cột ảo đứng trước MESSAGE, giá trị là `display_name` của file.

Chỉ mục giá trị Filter (ngưỡng 500, Distinct Count) được tính trên tập dòng gộp. Source Column là Filterable Column như các cột khác. File View dùng chỉ mục của riêng file đó.

### 5. Raw View: danh sách ảo hóa các File Section

Raw View là một danh sách cuộn dọc. Mỗi phần tử là header của File Section (chevron, tên, thư mục) hoặc một khối nội dung.
- **Imported**: vẽ dòng chỉ đọc bằng danh sách ảo hóa, dùng lại cách render của canvas. Không cần editor, vì file có thể rất lớn.
- **Scratch**: mỗi File Section dùng một `EditorState` của gpui-component. File scratch do người dùng gõ nên nhỏ, dùng editor thật là hợp lý.

Search trong Raw View chạy trên `raw_line` của mọi dòng và bỏ qua Filter. Nút trước/sau đi xuyên qua các section.

*Kết quả spike (task 1.4, gpui-component/gpui-base 0.7.3):* `EditorState` có hiển thị số dòng (`set_line_number`), `Editor::readonly(bool)`, tô sáng khoảng tùy ý qua `TextDecoration`/`RangeDecoration` (`create_decorations_collection`), `set_value` và `set_cursor_position`. Đủ cho Scratch: dùng `EditorState` cho mỗi File Section, không cần phương án thay thế.

### 6. File scratch nằm cạnh session

```
~/.config/log_gpui/spaces/<space_id>/untitled.log
                                     untitled-2.log
```

Cùng thư mục gốc với `session.json`, để cả dữ liệu của ứng dụng nằm một chỗ. Lưu sau 500 ms ngừng gõ, dùng lại cơ chế debounce của `SessionPersistence`; ⌘S lưu ngay. Xóa Log Space hoặc xóa file scratch thì xóa file thật trong thư mục này. Save As chỉ ghi bản sao ra ngoài.

### 7. Marker khi sửa dòng scratch

Mỗi Marker lưu số dòng và hash nội dung dòng lúc đặt. Sau mỗi lần lưu, Marker nào có hash không còn khớp nội dung ở số dòng đó thì bị bỏ. Như vậy rule "sửa dòng thì mất Marker" được thực hiện mà không cần theo dõi từng thao tác sửa.

Hệ quả: chèn dòng phía trên cũng làm lệch nội dung, nên Marker phía dưới cũng bị bỏ. Đây là đánh đổi chấp nhận được với file scratch nhỏ.

### 8. Dialog tạo Log Space

Đây là một modal trong `context_view` (đã có sẵn backdrop và modal). Trạng thái thuần dữ liệu được tách riêng để test không cần gpui:

```
NewLogSpaceForm { name, files: Vec<PendingFile>, error }
  can_create()  = !name.trim().is_empty() && !project.has_space_named(name.trim())
  kind()        = if files.is_empty() { Scratch } else { Imported }
  PendingFile   { path, status: Ok | Binary | Duplicate }
```

Thả một thư mục thì lấy file con ở cấp một. Một file bị coi là binary nếu 8 KB đầu có byte `NUL` hoặc không phải UTF-8 hợp lệ.

## Risks / Trade-offs

- [Timestamp của các file khác định dạng hoặc khác múi giờ, xếp sai thứ tự] → So theo đồng hồ của từng file, không đổi múi giờ (rule 19). File không parse được timestamp nào bị đưa xuống cuối và gắn cờ, để người dùng thấy rõ.
- [Log Space nhiều file lớn tốn bộ nhớ, vì mỗi file đều được parse và giữ trong cache] → Bảng gộp chỉ giữ cặp chỉ số `(u32, u32)`; nội dung dòng nằm trong cache của từng file.
- [Hai tab cùng hiện một file scratch (Log Space và File View)] → Cả hai đọc chung một bộ đệm theo `file_id`. Sửa ở tab nào thì tab kia nhận nội dung mới sau khi lưu.
- [Editor của gpui-component không đáp ứng Raw View] → Task 1.1 kiểm tra trước; có phương án thay thế ở Quyết định 5.
- [Mất session cũ khi nâng cấp] → Đã chấp nhận: không chuyển đổi dữ liệu cũ. Session cũ bị bỏ qua khi bump version.
- [Change `redesign-search-and-filter` chưa archive] → Archive change đó trước, rồi mới archive change này, để delta `log-filter` và `log-search` áp lên đúng spec.

## Open Questions

- Hiện có chấm màu riêng cho mỗi file trong Source Column và trên sidebar không, và màu lấy từ đâu?
- Giới hạn số file hoặc tổng dung lượng trong một Log Space? Hiện chưa có.
