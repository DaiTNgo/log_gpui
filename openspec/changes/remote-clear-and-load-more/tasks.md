## 1. Nền tảng trong LiveBuffer

- [x] 1.1 Thêm `byte_len: u32` cho mỗi dòng của file remote, lấy từ byte thật của `BackwardLines` và reader xuôi (gồm `\n` hay `\r\n`); test với file CRLF và dòng UTF-8 nhiều byte
- [x] 1.2 Thêm `held_start { generation, line_no, offset }` vào `LiveBuffer`, cập nhật khi Tail, Backfill, Append, trim; test offset sau nhiều lần trim khớp vị trí thật trong file
- [x] 1.3 Lưu `inode` của mỗi Generation khi bắt đầu đọc nó
- [x] 1.4 Thêm `HoldMode { Window, Extended }`; `trim_to_window` nhận giới hạn theo `HoldMode` (Live Window hay Full Read Limit); test file `Extended` không bị cắt khi có dòng mới tới khi vượt Full Read Limit
- [x] 1.5 Thêm `BreakKind::Cleared` (nhãn "cleared") và Break "start of loaded lines"

## 2. Clear

- [x] 2.1 `LiveBuffer::clear`: bỏ mọi dòng và Break của file, `shrink_to_fit`, ghi `ClearMark { generation, line_no, offset, time }`, đặt Break `Cleared` chờ; dòng dở dang (`replace_last`) thuộc sau Clear Point
- [x] 2.2 `SourceCommand::Clear` trong `RemoteFileSource`: bỏ `mem.backward`, đặt `no_refill`, không tự nạp Rotated File; test nạp dần dừng và không chạy lại sau khi kết nối lại
- [x] 2.3 Gộp đường bỏ dòng của trim và Clear thành `remove_lines(file, range, reason)` trong `src/app/live.rs`: Filter index (số đếm, Filter Value hết dòng, cột trở lại ≤ 500), Marker (bộ nhớ và session), Search Match, Focus Tab, dòng chọn, Inspector
- [x] 2.4 Dòng hiện tại của Focus Tab / Match / dòng chọn khi Clear chuyển sang dòng còn giữ ngay sau trong timeline gộp; test với hai file xen nhau
- [x] 2.5 Dựng lại bản sao view của timeline gộp không có row của file đã Clear, giữ vị trí cuộn (Follow bật: ở cuối; tắt: giữ row trên cùng hoặc row ngay sau)
- [x] 2.6 Raw View: File Section rỗng hiện Break "cleared", "Waiting for new lines" và nút Load more
- [x] 2.7 Clear khi Pause và khi mất kết nối; test dòng ghi trong lúc Pause hiện dưới Break "cleared" <!-- Pause tested; Clear while disconnected is handled in the source loop but has no test -->
- [x] 2.8 Hỏi xác nhận khi phần bị Clear có Marker ("Clear will remove N Markers")
- [x] 2.9 Lối vào: menu Remote File và Remote Command ở sidebar, header File Section, Command Palette, phím `Cmd+Shift+K`; "Clear all files" trên menu Remote Log Space (một lần cập nhật view); không có trong Imported/Scratch

## 3. Load more

- [x] 3.1 `SourceCommand::LoadMore { generation, end_offset, first_line_no, lines }` và `SourceEvent::Older { generation, lines, first_line_no, reached_start }`; đọc bằng `BackwardLines::new(end_offset, BLOCK)`
- [x] 3.2 Kiểm tra `line_hash` của dòng ngay dưới `end_offset` khớp dòng cũ nhất đang giữ; không khớp thì không chèn và vẽ Break "earlier lines of this generation are no longer on the Server"
- [x] 3.3 App chèn lô `Older` phía trên qua đường `apply_backfill`, giữ vị trí đọc, đánh số dòng thật lùi từ `first_line_no` (số tạm nếu chưa có `TotalLines`), rồi chuyển file sang `Extended`
- [x] 3.4 Load more sau Clear đọc từ `ClearMark.offset`; Break "cleared" giữ nguyên chỗ; Marker cũ không quay lại
- [ ] 3.5 Generation cũ: tìm Rotated File theo inode qua `rotated.rs`, rồi theo khoảng cách Generation + `line_hash`; file nén thì giải nén theo luồng giữ N dòng cuối trước `first_line_no`, nút ghi số byte phải đọc lại
- [ ] 3.6 `reached_start`: Break đầu file đổi sang "Load earlier generation" của `remote-full-read`; nút đó nạp vừa chỗ trống khi file là `Window` còn chỗ, còn lại nạp Load More Step và chuyển sang `Extended`
- [ ] 3.7 Giới hạn Full Read Limit: lô chỉ đọc phần còn lại; đủ giới hạn thì nút thành "Open in full…"
- [ ] 3.8 Load more bỏ qua Starting Time; Break ghi "N lines before Starting Time loaded"
- [x] 3.9 Trạng thái nút: "Loading…" + Cancel (giữ phần đã nhận), một Load more mỗi file một lúc, tắt khi Disconnected
- [x] 3.10 Lối vào: nút trên Break đầu file, menu Remote File, Command Palette, phím `Cmd+Shift+↑` (nạp Load More Step)
- [x] 3.11 Nút tách đôi: phần chính nạp Load More Step, menu 1.000 / 10.000 / 100.000 / "Custom…" (1 – Full Read Limit) chỉ cho lần đó; cùng lựa chọn trong menu Remote File; ghi "(up to N)" khi gần Full Read Limit

## 4. Clear trên Remote Command

- [ ] 4.1 `RemoteCommandSource` nhận `SourceCommand::Clear` mà không dừng hay chạy lại Run; dùng chung `LiveBuffer::clear` và `remove_lines`
- [ ] 4.2 Giữ timestamp của dòng cuối đã nhận trong nguồn (không đọc từ `ParsedFile`) để Resume By Time đúng sau Clear; test Clear rồi mất kết nối với preset chưa sửa
- [ ] 4.3 Remote Command không có Load more / Back to Live Window: Break "cleared" không nút, File Section rỗng ghi "Waiting for output"

## 5. Back to Live Window

- [x] 5.1 "Back to Live Window" trên Break đầu file và menu file: cắt ngay xuống Live Window, `shrink_to_fit`, về `Window`
- [x] 5.2 Đổi setting Live Window không cắt file `Extended`; Clear đưa file về `Window`

## 6. Settings và hiển thị

- [x] 6.1 Setting `load_more_step` (mặc định 10.000, 1.000–1.000.000, `#[serde(default)]`) trong Settings, ghi "≈13 MB per step"
- [x] 6.2 Tooltip Remote File ở sidebar và header File Section: "N lines held (≈X MB)" kèm "· Extended"
- [ ] 6.3 Tiến độ Load more trên Generation nén ở status bar, dùng chung với Load earlier generation

## 7. Kiểm thử

- [x] 7.1 Test tích hợp với sshd cục bộ (theo cách test remote live hiện có): Clear rồi ghi thêm dòng, Load more sau Clear đọc lại đúng dòng, Load more qua xoay vòng, chạm đầu Generation
- [x] 7.2 Test `filter_index` khi bỏ toàn bộ một file khỏi Log Space nhiều file, và cột trở lại ≤ 500 giá trị
- [x] 7.3 Đo RSS trước và sau Clear một file 100.000 dòng, và sau Back to Live Window từ 500.000 dòng; ghi kết quả vào design.md
- [ ] 7.4 Smoke GUI: Clear, Load more ba lần, Back to Live Window trên một Remote Log Space hai file, kiểm tra vị trí đọc không nhảy
