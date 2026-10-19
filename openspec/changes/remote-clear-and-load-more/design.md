## Context

`add-remote-live-log` đã có:
- `RemoteFileSource` (`src/live/remote.rs`): Tail ~1.000 dòng cuối, rồi nạp ngược bằng `BackwardLines` (`src/live/backward.rs`) tới khi `mem.loaded` đạt Live Window, sau đó bỏ reader (`mem.backward = None`). `BackwardLines::new(end, block)` đọc ngược bắt đầu từ một byte `end` bất kỳ. Lệnh từ app qua `SourceCommand` (`Pause`, `Resume`, `SetWindow`, `Reconnect`, `Close`).
- `LiveBuffer` (`src/live/buffer.rs`) cạnh `ParsedFile` của mỗi file: `apply_tail`, `apply_backfill`, `apply_append`, `apply_break`, `trim_to_window`. `trim_to_window` `drain` các dòng đầu, gắn Break `LeftWindow` lên dòng đầu còn giữ, và chỉ cắt khi vượt 10% để không cắt từng dòng. App gọi nó sau mỗi lô dòng mới (`src/app/live.rs`), kèm cập nhật Filter (`filter_index.rs`), Marker (L12) và Focus Tab.
- Đo: ~1.320 byte bộ nhớ mỗi dòng, gồm cả bản sao của view.

`remote-full-read` (chưa áp dụng) thêm Rotated File là Generation cũ hơn (L24), tự nạp Rotated File mới nhất không nén khi lần nạp đầu còn chỗ, nút "Load earlier generation" ở Break đầu file, Starting Time (dừng nạp ngược theo timestamp), Full Read Limit (mặc định 2.000.000) và Open in full vào Remote Copy. Change này dựa trên các khái niệm đó và không định nghĩa lại chúng.

Hai yêu cầu của người dùng: **Clear** để đọc dòng mới cho gọn, và **Load more** để lấy thêm ngữ cảnh cũ theo từng phần. Load more mâu thuẫn với L4 ("không nạp vượt Live Window") và câu đầu L10.

## Goals / Non-Goals

**Goals:**
- Clear một Remote File, trả lại bộ nhớ thật, file vẫn live.
- Load more theo từng phần, đọc ngược từ Server, đi qua được dòng đã Clear và các Generation (qua Load earlier generation).
- Dòng đã nạp bằng Load more không bị dòng mới đẩy ra ngay, nhưng bộ nhớ vẫn có trần rõ ràng.
- Mọi hệ quả (Marker, Filter, Focus Tab, Search, timeline gộp) theo đúng ngữ nghĩa L12 có sẵn, chỉ khác ở chỗ cần khác.

**Non-Goals:**
- Trần bộ nhớ chung cho cả app ngoài Full Read Limit của từng file.
- Clear trong Imported hay Scratch Log Space (với Remote Copy, người dùng gỡ file hoặc dùng Filter/Time Range).
- Undo Clear từ bộ nhớ. Dòng đã Clear chỉ quay về bằng Load more từ Server.
- "Ẩn" dòng (Clear mềm). Muốn ẩn thì dùng Filter hoặc Time Range.
- Giữ "đảo" dòng cũ không liền với phần còn lại (xem quyết định 3).
- Load more cho Remote Command của `remote-command-source` (output đã qua không đọc lại được từ Server).
- Nhớ Clear hay Extended Window khi đóng hoặc mở lại (L10).

## Decisions

### 1. Clear là cắt tại Clear Point, không phải Pause hay Filter
`SourceCommand::Clear` đi tới `RemoteFileSource`, và app đồng thời xử lý phía view (bước 2). Phía nguồn:
- Bỏ `mem.backward` (dừng nạp dần) và đặt `mem.no_refill = true`: không tạo lại nạp dần, không tự nạp Rotated File (L33) kể cả khi lần nạp đầu chưa xong, Live Window còn chỗ, hay vừa kết nối lại.
- Clear Point = `follow_offset` lúc đó: ngay sau dòng trọn vẹn cuối cùng đã gửi. Dòng dở dang chưa có newline (cơ chế `replace_last` của `LiveBuffer`) thuộc **sau** Clear Point: nó bị bỏ khỏi view và hiện lại trọn vẹn dưới Break "cleared" khi đủ dòng. Vì vậy không dòng nào bị cắt đôi.
- Dòng tới sau đó đi như `Append` bình thường. `LiveBuffer` giữ một Break `Cleared` chờ (`pending`) và gắn nó lên dòng đầu tiên tới sau Clear.

Phía view (`LiveBuffer::clear`):
- `parsed.entries`, `lines`, `raw_breaks` của file bị xoá hết, rồi `shrink_to_fit` để trả bộ nhớ (không chỉ `drain`). Bản sao của view (merged rows) được dựng lại không có dòng của file đó.
- Ghi lại `ClearMark { generation, line_no, offset, time }` cho Load more về sau (quyết định 4).
- Thứ tự áp dụng giống `trim_to_window` (dùng chung một đường "bỏ dòng của file"): Filter index trừ số đếm, Marker bị bỏ khỏi bộ nhớ và khỏi session đã lưu, Match và Focus Tab được tính lại.

Break `Cleared` có nhãn "`<file>` cleared 10:42:11". Trong lúc file chưa có dòng mới:
- Raw View: File Section của file hiện chỉ Break đó, kèm "Waiting for new lines" và nút Load more.
- Parsed View và Focus Pane: không có row nào của file; Break chờ chỉ hiện khi có dòng đầu tiên, như Break hiện nay.

Clear thay thế Break "left the window" của file (không còn dòng cũ nào để nói "đã rời"). Break "cleared" ở lại chỗ đã Clear cả khi Load more đọc lại các dòng phía trên và dòng hai bên liền nhau: nó cho người dùng biết mình đã Clear ở đâu.

*Thay thế đã cân nhắc:*
- **Clear = ẩn dòng bằng một mốc thời gian** (như Time Range ngầm): bị loại vì người dùng muốn giải phóng RAM thật, và Filter đã làm được việc ẩn.
- **Clear = đóng rồi mở lại file ở cuối**: bị loại vì mở lại sẽ nạp dần lại đầy Live Window (L4), đúng thứ người dùng vừa bỏ.

### 2. Hệ quả của Clear: dùng lại L12, khác ở vị trí con trỏ
Bỏ dòng do Clear và do rời window đi chung một hàm `remove_lines(file, range, reason)` trong `src/app/live.rs`:

| Thứ | Rời Live Window (L12) | Clear |
|---|---|---|
| Marker | bỏ, không hiện lại | bỏ, không hiện lại, kể cả khi Load more đọc lại dòng đó |
| Filter Value | trừ số đếm; hết dòng thì biến mất trừ khi đang tích | như L12 |
| Cột > 500 giá trị | không nhận điều kiện mới | đếm lại; còn ≤ 500 thì nhận điều kiện mới trở lại (cả hai trường hợp) |
| Dòng hiện tại của Focus Tab | dòng cũ nhất còn giữ | dòng còn giữ ngay sau nó trong timeline gộp; không có thì dòng cuối cùng còn giữ; Log Space rỗng thì không có |
| Match hiện tại, dòng chọn, Inspector | như Focus Tab | như Focus Tab; Inspector trống nếu không còn dòng chọn |
| Break | "older lines left the window" | "cleared" |

Lý do khác ở con trỏ: rời window luôn bỏ dòng cũ nhất, nên "dòng cũ nhất còn giữ" cũng là dòng ngay sau. Clear bỏ cả một file giữa các file khác, nên nhảy về dòng cũ nhất của Log Space sẽ làm mất chỗ đang đọc. Quy tắc "ngay sau" cho kết quả giống L12 trong trường hợp rời window, nên có thể dùng chung về sau.

Search: Match nằm trong dòng bị bỏ ra khỏi tập Match. Match count, overview ruler và Focus Pane được tính lại cho file đó (không tính lại cả Log Space). Timeline gộp: chỉ row của file bị bỏ; row của file khác giữ nguyên thứ tự. Nếu Follow đang bật, view vẫn ở cuối; nếu tắt, view giữ row đang ở đầu màn hình nếu nó còn, không thì row còn giữ ngay sau.

Pause (L1): Clear vẫn chạy khi Pause vì người dùng chủ động. Nguồn đang không đọc, nên Clear Point là chỗ đã đọc tới; dòng ghi trong lúc Pause hiện dưới Break "cleared" khi tiếp tục.

Mất kết nối: Clear chỉ đụng bộ nhớ nên chạy được. Khi nối lại, đọc tiếp từ offset như L6; Break disconnected/reconnected vẫn được vẽ như thường, phía dưới Break "cleared".

Xoay vòng sau Clear: dòng mới của Generation mới tới như thường, có Break "rotated". Clear Point thuộc Generation cũ.

"Clear all files" trên menu Remote Log Space gửi Clear cho mọi file trong cùng một lần cập nhật view (một lần dựng lại timeline gộp).

### 3. Load more mở rộng một cửa sổ liền mạch (Extended Window), trần là Full Read Limit
Mỗi file có `HoldMode`:
- `Window`: giữ tối đa Live Window dòng mới nhất (như hiện nay).
- `Extended { oldest: LineId }`: giữ mọi dòng từ `oldest` trở về sau.

`trim_to_window` nhận giới hạn của file:
- `Window` → Live Window.
- `Extended` → Full Read Limit: không cắt cho tới khi file giữ quá Full Read Limit (+10% như hiện nay), rồi cắt dòng cũ nhất như L12 với Break "left the window".

"Back to Live Window" (menu Remote File và Break đầu file) cắt ngay xuống Live Window (không chờ 10%, giống khi giảm setting), `shrink_to_fit`, rồi về `Window`. Clear cũng về `Window`. Đổi setting Live Window không cắt file đang `Extended`.

*Thay thế đã cân nhắc:*
- **Load more chỉ nâng trần thêm N**: bị loại. Với file ghi 1.000 dòng/giây, 10.000 dòng vừa nạp bị đẩy ra sau 10 giây, đúng lúc người dùng đang đọc.
- **Ghim dòng đã nạp, cho dòng ở giữa rời đi**: bị loại. Sinh "đảo" dòng không liền nhau, Break ở giữa, và số đếm Filter, Focus Tab, Load more tiếp theo đều khó hiểu.
- **Ngừng cắt khi Follow tắt**: bị loại. Bộ nhớ không có trần, và Follow tự đổi theo cuộn (L2) nên trạng thái giữ dòng sẽ đổi mà người dùng không để ý.
- **Một setting trần riêng cho Load more**: không cần. Full Read Limit đã là "số dòng tối đa một lần đọc được giữ trong bộ nhớ", và người dùng đã được nói nó tốn bao nhiêu RAM. Xem câu hỏi mở 2.

Khi chạm trần, nút Load more và Load earlier generation đổi thành "Open in full…" (`remote-full-read`), đồng nhất với cách change đó xử lý window đầy.

### 4. Load more đọc từ đâu: Held Start theo từng Generation
Load more cần byte offset của dòng cũ nhất đang giữ. Hiện `LiveBuffer` không biết offset sau khi `trim_to_window` cắt.
- Mỗi dòng của file remote giữ thêm `byte_len: u32` (độ dài trên Server, gồm cả `\r\n` hay `\n`), do `BackwardLines` và reader xuôi báo. Tốn 4 byte/dòng (~0,3%).
- `LiveBuffer` giữ `held_start: { generation, line_no, offset }` của dòng cũ nhất. Khi Tail, offset là chỗ bắt đầu khối đã đọc. Khi `Backfill` và Load more, offset lùi theo các dòng đã nhận. Khi cắt, offset tiến thêm tổng `byte_len` của các dòng bị bỏ.
- Sau Clear, không còn dòng nào: Load more dùng `ClearMark.offset` (Clear Point) và `ClearMark.line_no`, nên đọc lại đúng các dòng vừa bị Clear, mới nhất trước.

`SourceCommand::LoadMore { generation, end_offset, first_line_no, lines }` tới `RemoteFileSource`:
1. Mở đúng file của Generation đó (bước 5), tạo `BackwardLines::new(end_offset, BLOCK)`, đọc ngược `lines` dòng (hoặc tới byte 0).
2. Gửi `SourceEvent::Older { generation, lines, first_line_no, reached_start }`. App chèn lô vào phía trên dòng cũ nhất của file, dùng đường `apply_backfill` sẵn có (đã giữ vị trí đọc khi chèn phía trên), rồi đưa file sang `Extended`.
3. Số dòng được tính lùi từ `first_line_no`, nên là số thật. Nếu số dòng của Generation chưa biết (`wc -l` chưa xong), lô dùng số tạm như Backfill hiện nay và được đánh lại khi `TotalLines` tới.

Load more bỏ qua Starting Time (L35): người dùng đã chủ động muốn dòng cũ hơn. Break đầu file khi đó ghi "N lines before Starting Time loaded".

Lô đầu tiên của Load more dùng `stop_before` = `None` và không có giới hạn byte. Một dòng rất dài vẫn là một dòng.

### 5. Load more trên Generation cũ (sau xoay vòng hoặc từ Rotated File)
Dòng cũ nhất đang giữ có thể thuộc Generation cũ hơn file hiện tại: file đã xoay vòng trong lúc mở, hoặc đã nạp Rotated File (`remote-full-read`). Byte của Generation đó giờ nằm trong một Rotated File.
- `LiveBuffer` lưu `inode` của mỗi Generation khi bắt đầu đọc nó (nguồn đã có inode để nhận biết xoay vòng).
- Load more liệt kê Rotated File bằng `rotated.rs` của `remote-full-read` và chọn file có cùng inode.
- Không có file cùng inode (thường do logrotate đã nén nó): lấy Rotated File theo khoảng cách Generation (L24), kiểm tra dòng cũ nhất đang giữ nằm đúng chỗ (so `line_hash` của dòng tại `first_line_no`).
- Nếu khớp và file không nén: đọc ngược như bước 4.
- Nếu khớp và file nén: giải nén theo luồng từ đầu, giữ `lines` dòng cuối trước `first_line_no` trong ring buffer, như `remote-full-read` làm cho file nén. Mỗi lần bấm phải giải nén lại từ đầu; nút ghi rõ "Load more (re-reads 38 MB)".
- Nếu không khớp hoặc không thấy file: Load more dừng với Break "earlier lines of this generation are no longer on the Server" và đề nghị "Load earlier generation" nếu còn Rotated File cũ hơn.

### 6. Dừng ở đầu Generation, sang Generation trước qua Load earlier generation
Khi một lần Load more chạm byte 0 (`reached_start`), file giữ trọn Generation đó tới dòng 1. Break đầu file khi đó hiện nút "Load earlier generation" của `remote-full-read`, với tên và kích thước Rotated File kế tiếp, kể cả khi file đó không nén. Lý do: tự sang Generation trước thì người dùng không thấy ranh giới và có thể bắt đầu giải nén một file lớn mà không biết.

Lượng dòng của Load earlier generation:
- Live Window còn chỗ (file đang `Window`, ví dụ sau Clear): nạp vừa chỗ trống, như `remote-full-read` định nghĩa.
- Không còn chỗ, hoặc file đang `Extended`: nạp một Load More Step dòng cuối của Rotated File đó và đưa file sang `Extended`.

Sau đó Load more tiếp tục trong Generation vừa nạp.

### 7. Nút và nơi hiện
- **Break đầu phần dòng của file**: các Break "left the window", "cleared", "N lines since <Starting Time>", hoặc một Break "start of loaded lines" mới, chỉ hiện khi file chưa giữ tới dòng 1 của Generation cũ nhất. Break mang các nút:
  - "Load more (10,000)" kèm menu chọn số dòng, và "Back to Live Window" khi file đang `Extended`;
  - hoặc "Load earlier generation (…)" khi đã tới dòng 1;
  - hoặc "Open in full…" khi đã tới Full Read Limit.
- **Menu Remote File** (sidebar, header File Section): "Clear", "Load more", "Back to Live Window".
- **Menu Remote Log Space**: "Clear all files".
- **Phím tắt**: Clear file hoặc Remote Command của dòng đang chọn (`Cmd+Shift+K`; `Cmd+K` đã là Command Palette), Load more một Load More Step cho file của dòng đang chọn (`Cmd+Shift+↑`). Cả hai cũng có trong Command Palette.
- **Chọn số dòng**: nút Load more là nút tách đôi. Phần chính nạp Load More Step ("Load more (10,000)"). Mũi tên mở menu 1.000 / 10.000 / 100.000 / "Custom…" (ô nhập số, 1 – Full Read Limit); lựa chọn chỉ áp dụng cho lần bấm đó, không đổi setting. Menu Remote File có cùng các lựa chọn dưới "Load more". Số nào vượt chỗ còn lại tới Full Read Limit thì ghi "(up to N)".
- **Đang đọc**: nút đổi thành "Loading…" kèm Cancel. Cancel giữ phần đã nhận. Chỉ một Load more mỗi file một lúc.
- **Mất kết nối**: Load more tắt, tooltip "Disconnected".
- **Tooltip** của Remote File ở sidebar và header File Section: "120,000 lines held (≈160 MB) · Extended". Con số ≈ lấy từ 1.320 byte/dòng đã đo, không đo bộ nhớ thật.

Không thêm gì vào status bar: L7 chỉ dành status bar cho trạng thái kết nối, Follow và Pause. Tiến độ Load more trên file nén thì hiện ở đó, như `remote-full-read` làm với Load earlier generation.

### 8. Load More Step
Setting `load_more_step`, mặc định **10.000**, chỉnh trong Settings (1.000 – 1.000.000), ghi kèm "≈13 MB per step". Đó là số dòng của phần chính nút Load more và của phím tắt; menu của nút cho chọn số khác cho từng lần (quyết định 7). Lô đọc không vượt quá chỗ còn lại tới Full Read Limit. Không có trần chung cho cả app: nhiều file `Extended` cùng lúc thì người dùng thấy bộ nhớ của từng file trong tooltip và dùng Back to Live Window.

### 9. Trả bộ nhớ thật
`Vec::drain` không trả vùng nhớ. Sau Clear và Back to Live Window:
- `shrink_to_fit` cho `entries`, `lines`, các vector của `filter_index`, Match cache của Search và Focus Tab;
- dựng lại bản sao của view.

Trim thường (rời window từng 10%) giữ nguyên cách hiện tại, không `shrink_to_fit`, vì vùng nhớ sẽ được dùng lại ngay.

Allocator của hệ điều hành có thể không trả trang cho OS ngay. Task 7.3 đo (macOS, `cargo run --release --example clear_rss`, dùng `footprint`, chỉ tính bộ nhớ của cache một file, chưa tính bản sao view):

| Bước | Bộ nhớ |
|---|---|
| Mới khởi động | 2 MB |
| Giữ 100.000 dòng | 99 MB (≈1 KB/dòng) |
| Sau Clear | 16 MB (trả gần hết) |
| Giữ 500.000 dòng | 316–377 MB |
| Sau Back to Live Window (còn 100.000 dòng) | ≈273 MB |

Clear trả gần hết bộ nhớ vì thay hẳn các vector. Back to Live Window thì chỉ trả được phần nhỏ: còn ≈273 MB trong khi 100.000 dòng mới nạp chỉ ≈99 MB. `shrink_to_fit` không đủ, và thử sao chép phần còn lại sang vùng nhớ mới cũng không khá hơn (đã bỏ). Nguyên nhân chưa rõ; cần tìm hiểu thêm trước khi hứa với người dùng là Back to Live Window "trả lại bộ nhớ".

### 10. Clear trên Remote Command
Remote Command (`remote-command-source`) là thành viên thứ hai của Remote Log Space, đọc output của một Run qua cùng `LiveBuffer` và `SourceEvent`. Clear dùng cùng `LiveBuffer::clear` và `remove_lines` như Remote File:
- `RemoteCommandSource` nhận `SourceCommand::Clear` nhưng **không** dừng, gửi signal hay chạy lại Run. Clear Point là sau dòng trọn vẹn cuối cùng của output đã nhận; dòng dở dang hiện lại trọn vẹn dưới Break "cleared", giống file.
- Nguồn không có nạp ngược, nên `no_refill` không có gì để dừng.
- Resume By Time (L30) lấy timestamp của dòng cuối **đã nhận**, không phải dòng đang giữ. Nguồn giữ timestamp đó riêng (không đọc từ `ParsedFile`), nên vẫn đúng khi mọi dòng đã bị Clear.
- Restart, Restart on exit và nối lại sau Clear bắt đầu Run như L29, với Break của change đó, phía dưới Break "cleared".
- Stream Column và số đếm Filter của nó được trừ như mọi cột khác.
- Không có Load more: output đã qua của một Run không đọc lại được từ Server. Break "cleared" của Remote Command không có nút, File Section rỗng chỉ ghi "Waiting for output", và menu không có "Load more" hay "Back to Live Window".

## Risks / Trade-offs

- **[Extended Window ăn RAM]** Nhiều file cùng `Extended` tới Full Read Limit có thể dùng vài GB, và người dùng đã chọn không có trần chung. → Tooltip ghi bộ nhớ ước lượng của từng file; Back to Live Window chỉ một cú bấm; Full Read Limit chỉnh được.
- **[Offset lệch]** Nếu `byte_len` sai (ví dụ dòng bị thay khi `replace_last`, hay encoding lạ), Load more đọc lệch. → `byte_len` lấy từ byte thật của reader, không từ text đã decode. Lô Load more đầu tiên kiểm tra dòng ngay dưới `end_offset` có `line_hash` khớp dòng cũ nhất đang giữ. Không khớp thì báo lỗi, không chèn lô.
- **[File bị cắt ngắn/ghi đè trên Server]** Byte trước Clear Point có thể đã đổi. → Cùng kiểm tra `line_hash` như trên, rồi Break "earlier lines of this generation are no longer on the Server".
- **[Giải nén lại mỗi lần bấm]** Load more trong Generation nén phải giải nén từ đầu mỗi lần. → Nút ghi số byte phải đọc lại. Người dùng có thể tăng Load More Step hoặc dùng Open in full.
- **[Clear nhầm]** Không có undo. → Load more đưa lại dòng từ Server. Không hỏi xác nhận, vì Clear là thao tác nhanh người dùng muốn làm nhiều lần; Marker là thứ duy nhất mất hẳn, và nếu file có Marker trong phần bị Clear thì hiện xác nhận "Clear will remove N Markers".
- **[Phối hợp với `remote-full-read`]** Change này sửa requirement "Load earlier generation khi Live" của change đó. → Archive change này sau `remote-full-read`; nếu change đó đổi tên requirement thì cập nhật delta.

## Migration Plan

- Không đổi dữ liệu session. Setting `load_more_step` có `#[serde(default)]` là 10.000.
- Clear và Extended Window chỉ sống trong bộ nhớ; mở lại Project mở file như L3/L4.
- Rollback: gỡ change; không còn dữ liệu nào phải dọn.
- Archive sau `add-remote-live-log` và `remote-full-read`.

## Open Questions

Không còn câu hỏi mở. Đã chốt với người dùng:
- Clear áp dụng cho cả Remote Command: bỏ dòng đang giữ, Run vẫn chạy; Load more không áp dụng cho Remote Command (quyết định 10). Change này sở hữu hành vi đó; `remote-command-source` chỉ tham chiếu.
- Load more có menu chọn số dòng (1.000 / 10.000 / 100.000 / tự nhập), mặc định Load More Step 10.000 chỉnh trong Settings; không có "Load more to time…" hay Shift+click (quyết định 7, 8).
- Không có trần bộ nhớ chung cho cả app; chỉ Full Read Limit cho từng file (quyết định 3).
- Chỉ hỏi xác nhận khi phần bị Clear có Marker (Risks).
- Phím tắt: `Cmd+Shift+K` cho Clear, `Cmd+Shift+↑` cho Load more (quyết định 7).
- Break "cleared" ở lại chỗ đã Clear, kể cả khi Load more làm dòng hai bên liền lại (quyết định 1).
