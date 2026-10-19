## Why

Khi một Remote File ghi rất nhiều dòng, Live Window đầy những dòng người dùng đã đọc xong, và dòng mới khó theo dõi giữa đống dòng cũ. Người dùng muốn **xoá sạch** những gì đang giữ để đọc các dòng tiếp theo cho gọn, và ngược lại, khi cần ngữ cảnh thì **nạp thêm** dòng cũ hơn từ Server theo từng phần, thay vì bị chặn ở Live Window (L4, L10) hay phải Open in full cả file.

## What Changes

- **Clear** (theo từng Remote File hoặc Remote Command, chỉ trong Remote Log Space): bỏ thật mọi dòng nó đang giữ khỏi bộ nhớ, ở mọi Generation đang giữ, kể cả Generation tự nạp từ Rotated File. File vẫn đọc tiếp, lệnh vẫn chạy; dòng mới hiện sau một Break "cleared". Không có gì trên Server bị đổi.
  - Remote Command (của `remote-command-source`): Clear không dừng hay chạy lại Run đang chạy. Restart sau đó vẫn bắt đầu Run mới như L29, và Resume By Time (L30) vẫn dùng timestamp của dòng cuối đã nhận, kể cả khi dòng đó đã bị Clear.
  - Lối vào: menu của Remote File hay Remote Command ở sidebar, header File Section trong Raw View, và phím tắt `Cmd+Shift+K` cho file hoặc lệnh của dòng đang chọn. "Clear all files" trên menu Remote Log Space là Clear từng file cùng lúc.
  - Dòng bị Clear được xử lý như dòng rời Live Window (L12): mất Marker, ra khỏi số đếm Filter, Filter Value hết dòng thì biến mất trừ khi đang tích. Khác L12: dòng hiện tại của Focus Tab, Match hiện tại và dòng chọn chuyển sang dòng còn giữ **ngay sau** trong timeline gộp, không nhảy về dòng cũ nhất.
  - Sau Clear, file **không tự nạp dòng cũ nữa**: nạp dần đang chạy bị dừng, không tự nạp Rotated File (L24 của `remote-full-read`). Chỉ Load more mới đưa dòng cũ về.
  - Clear chạy được khi Pause và khi mất kết nối. Không có undo; Load more đọc lại từ Server (chỉ với Remote File). Chỉ hỏi xác nhận khi phần bị Clear có Marker.
- **Load more** (chỉ Remote File, không có cho Remote Command): đọc thêm một số dòng cũ hơn dòng cũ nhất file đang giữ, đọc ngược từ Server, mỗi lần bấm một phần. Nút chính nạp **Load More Step** (mặc định 10.000, chỉnh trong Settings); nút kèm một menu chọn số dòng cho lần này: 1.000, 10.000, 100.000 hoặc tự nhập. Nút nằm ở Break đầu phần dòng của file ("left the window", "cleared", hay đầu phần dòng khi nạp dần đã dừng ở Live Window hoặc Starting Time), trên menu Remote File, và có phím `Cmd+Shift+↑` (nạp Load More Step).
  - Sau Load more, file có **Extended Window**: nó giữ mọi dòng từ dòng cũ nhất đã nạp trở về sau, dòng mới không đẩy dòng cũ ra, cho đến khi file giữ đủ **Full Read Limit** (của `remote-full-read`, mặc định 2.000.000). Khi đó dòng cũ nhất rời như L12, và Load more đổi thành "Open in full…".
  - "Back to Live Window" hoặc Clear kết thúc Extended Window. Đổi Live Window trong Settings không cắt file đang có Extended Window.
  - Load more sau Clear đưa lại chính các dòng đã Clear trước (đọc lại từ Server, không có Marker cũ), rồi đi tiếp về trước. Break "cleared" vẫn đứng ở chỗ đã Clear, kể cả khi dòng hai bên đã liền lại.
  - Load more dừng ở dòng 1 của Generation đang đọc. Sang Generation trước luôn qua nút **"Load earlier generation"** của `remote-full-read`, kể cả khi Rotated File không nén. Load more bỏ qua Starting Time.
  - Load more cần kết nối; khi mất kết nối nút bị tắt kèm lý do.
- Kích thước bộ nhớ hiện ra: tooltip của Remote File ở sidebar và header File Section ghi số dòng đang giữ và ước lượng bộ nhớ ("120.000 lines held, ≈160 MB"), và việc bỏ dòng (Clear, Back to Live Window) trả lại bộ nhớ thật (thu nhỏ vùng nhớ, dựng lại bản sao của view).

## Domain changes

Change này không sửa CONTEXT.md hay ADR vì các session khác đang làm song song. Khi áp dụng change, cần cập nhật CONTEXT.md như sau. Các quy tắc L4, L10, L12, L24 nói tới ở đây là bản đã được `remote-full-read` viết lại.

**Thuật ngữ mới (nhóm Live):**
- **Clear**: bỏ mọi dòng một Remote File hoặc Remote Command đang giữ khỏi bộ nhớ trong khi nó vẫn đọc tiếp hoặc chạy tiếp; dòng đến sau hiện dưới một Break "cleared". Không đổi gì trên Server. _Avoid_: reset, wipe, delete, truncate (Truncated là việc xảy ra với file trên Server).
- **Clear Point**: chỗ Clear cắt: ngay sau dòng trọn vẹn cuối cùng đã nhận lúc Clear, trong file trên Server hoặc trong output của Run. Dòng sau Clear Point là dòng mới; dòng trước nó chỉ quay về bằng Load more.
- **Load More**: đọc thêm một số dòng (mặc định một Load More Step) cũ hơn dòng cũ nhất một Remote File đang giữ, từ chính Generation đó trên Server. _Avoid_: load earlier (đó là "Load earlier generation" sang Rotated File), scroll back, history.
- **Load More Step**: số dòng Load More đọc khi người dùng không chọn số khác cho lần đó, một giá trị chung cho cả app, mặc định 10.000. _Avoid_: page size, chunk.
- **Extended Window**: trạng thái của một Remote File sau Load More: file giữ mọi dòng từ dòng cũ nhất đã nạp trở về sau, không để dòng mới đẩy dòng cũ ra, cho đến khi giữ đủ Full Read Limit. Kết thúc bằng "Back to Live Window" hoặc Clear. _Avoid_: bigger buffer, history mode, pinned lines.

**Thuật ngữ đổi nghĩa:**
- **Live Window**: thêm "Một Remote File có Extended Window thì không bị Live Window giới hạn cho đến khi trở lại Live Window."
- **Break**: thêm "…hoặc người dùng đã Clear file đó. Break 'cleared' ở lại chỗ đã Clear, kể cả khi Load More làm dòng hai bên liền lại." Break ở đầu phần dòng của một Remote File có thể mang nút Load More hoặc Load earlier generation.
- **Full Read Limit** (của `remote-full-read`): thêm "và là số dòng tối đa một Remote File có Extended Window được giữ."

**Quy tắc đổi:**
- **L4 Remote Opens At The End**: câu "Không nạp dòng vượt quá Live Window" đổi thành "Không tự nạp dòng vượt quá Live Window; chỉ Load More (L34) mới nạp vượt."
- **L10 Live Is A Window, Not An Archive**: câu đầu đổi thành "Một Remote Log Space có thể kết nối bao lâu tuỳ ý, nhưng mỗi file chỉ giữ Live Window của nó, hoặc Extended Window khi người dùng đã Load More (L34); dòng cũ nhất rời đi khi có dòng mới vượt giới hạn đó." Câu "Nothing received is kept after the Log Space is closed" giữ nguyên: Clear và Extended Window không được nhớ khi đóng hay mở lại.
- **L12 Leaving The Window**: thêm "Dòng bị Clear cũng mất Marker, thôi được đếm cho Filter Value như vậy, nhưng Break là 'cleared' (L32), và dòng hiện tại của Focus Tab chuyển sang dòng còn giữ ngay sau nó." Một dòng rời Extended Window khi chạm Full Read Limit được xử lý đúng như rời Live Window.
- **L13 Window Per Remote File**: thêm "Extended Window cũng là của riêng từng Remote File: Load More trên một file không đổi giới hạn của file khác."
- **L1 Pause Is Per Log Space**: thêm "Clear và Load More vẫn chạy khi Pause, vì người dùng chủ động yêu cầu; dòng ghi trong lúc Pause hiện sau Break 'cleared' khi tiếp tục."
- **15 Filter Counts Ignore Other Filters**: thêm "…và trừ đi khi dòng bị Clear. Một cột đã vượt 500 giá trị riêng biệt mà sau khi dòng rời hoặc bị Clear còn không quá 500 thì nhận điều kiện mới trở lại."

**Quy tắc mới:**
- **L32 Clear Frees, Reading Goes On**: Clear chỉ có trong Remote Log Space và áp dụng cho từng Remote File hoặc Remote Command. Nó bỏ khỏi bộ nhớ mọi dòng đang giữ, ở mọi Generation, trong khi file vẫn đọc tiếp và Run vẫn chạy; dòng sau Clear Point hiện dưới một Break "cleared". Không có undo; dòng đã Clear của Remote File chỉ quay về bằng Load More, không kèm Marker cũ; dòng đã Clear của Remote Command không quay về. Clear chỉ hỏi xác nhận khi phần bị Clear có Marker.
- **L33 Cleared Files Do Not Refill**: Sau Clear, file không bao giờ tự nạp dòng cũ hơn Clear Point: nạp dần dừng lại và không tự nạp Rotated File. Chỉ Load More mới nạp dòng cũ.
- **L34 Load More Extends The Window**: Load More chỉ có cho Remote File. Nó đọc số dòng người dùng chọn (mặc định Load More Step) cũ hơn dòng cũ nhất file đang giữ, từ Server, và đưa file vào Extended Window: mọi dòng từ dòng cũ nhất đã nạp trở về sau được giữ cho đến khi file giữ đủ Full Read Limit, rồi dòng cũ nhất rời như L12. "Back to Live Window" và Clear kết thúc Extended Window. Load More sau Clear đọc lại các dòng đã Clear trước.
- **L35 Load More Stays In Its Generation**: Load More dừng ở dòng 1 của Generation đang đọc; sang Generation trước luôn là "Load earlier generation" (L24), kể cả với Rotated File không nén. Load More bỏ qua Starting Time. Khi file đã giữ đủ Full Read Limit, Load More và Load earlier generation nhường chỗ cho Open in full.

Số quy tắc L32–L35 thuộc change này; L16–L19, L20–L23, L24–L27 và L28–L31 thuộc `ssh-connect-ergonomics`, `remote-file-picker`, `remote-full-read` và `remote-command-source`.

**ADR:** nên thêm ADR 0012 "Load More Extends A Contiguous Window Instead Of Pinning Old Lines" (design.md, quyết định 3) khi áp dụng change. Nó sửa phần "Live chỉ là cửa sổ" của ADR 0009 và bổ sung ADR 0011 (đọc trọn vẫn đi qua Remote Copy; Load More chỉ là đọc thêm có giới hạn trong Live).

## Capabilities

### New Capabilities
- `remote-clear`: Clear một Remote File hoặc Remote Command (và Clear all files), Clear Point, Break "cleared", không tự nạp lại, bỏ dòng thật khỏi bộ nhớ, hệ quả cho Marker, Filter Value, Focus Tab, Search, dòng chọn và timeline gộp, Clear khi Pause hoặc mất kết nối.
- `remote-load-more`: Load More với số dòng chọn được (mặc định Load More Step), Extended Window và Full Read Limit, Back to Live Window, Load More sau Clear, dừng ở đầu Generation rồi chuyển sang Load earlier generation, giữ vị trí đọc, số dòng thật, hiện số dòng và bộ nhớ đang giữ.

### Modified Capabilities
- `live-log`: "Live Window" thêm Extended Window và việc Clear; "Dòng rời Live Window" thêm dòng rời Extended Window và khác biệt của Clear.
- `remote-rotated-files` (của change `remote-full-read`): "Load earlier generation khi Live" vẫn hiện khi Live Window đã đầy (nạp một Load More Step, đưa file vào Extended Window), và chỉ đổi thành "Open in full…" khi file đã giữ đủ Full Read Limit.

Lưu ý: `live-log` mới có trong change `add-remote-live-log`, `remote-rotated-files` mới có trong change `remote-full-read`; cả hai chưa được archive. Requirement về Remote Command trong `remote-clear` dùng thuật ngữ của change `remote-command-source`. Change này cần được archive sau `add-remote-live-log`, `remote-full-read` và `remote-command-source`.

## Impact

- Code: `src/live/buffer.rs` (`clear`, Clear Point, độ dài byte của từng dòng giữ, Extended Window trong `trim_to_window`), `src/live/backward.rs` (đọc ngược bắt đầu từ một byte offset bất kỳ), `src/live/remote.rs` và `src/live/source.rs` (lệnh `Clear`, `LoadMore { lines }`, `BackToWindow`; `BreakKind::Cleared`; dừng nạp dần sau Clear; tìm lại Generation cũ qua inode), `src/filter_index.rs` (bỏ dòng ở giữa/toàn bộ một file, cột trở lại dưới 500), `src/marker.rs`, `src/search.rs`, `src/app/live.rs`, `src/app/focus.rs`, `src/app/view/status_bar.rs`, `src/app/view/mod.rs` (nút trên Break, header File Section), `src/ui/settings.rs` (Load More Step).
- Dùng lại của `remote-full-read`: Rotated File, `rotated.rs`, Load earlier generation, Full Read Limit, Open in full.
- Không có dependency mới. Không đổi dữ liệu lưu, trừ setting `load_more_step` (`#[serde(default)]`).
- Phối hợp: `remote-command-source` — change này sở hữu hành vi Clear trên Remote Command (bỏ dòng đang giữ, Run vẫn chạy); change kia chỉ tham chiếu. Load More không áp dụng cho Remote Command. Archive change này sau `remote-command-source`.
