## Context

Hiện có ba loại Log Space (`LogSpaceKind` trong `src/workspace/log_space.rs`):

- **Imported**: file local, Archive Entry, Remote Copy.
- **Remote**: Remote File, Live Glob, Remote Command, đọc qua SSH.
- **Scratch**.

Change `line-offset-index` 4b.7 cho file local của Imported có Live/Pause và Follow. `LocalFileSource` (`src/live/local.rs`) phát hiện file bị đổi bằng cách so inode và kích thước sau mỗi lần poll, rồi phát Break `Rotated`, `Truncated` hoặc `Deleted`. Khi nhận Break đó, `apply_indexed_events` (`src/app/live.rs`) đọc lại cả file và bỏ hết dòng cũ.

Có hai chỗ hở:

- File bị ghi đè cùng inode mà dài bằng hoặc dài hơn trước thì không bị phát hiện. Các dòng cũ lúc đó đọc nhầm vào nội dung mới.
- App không biết được file vừa bị rotate hay bị thay. Hai việc này là cùng một thao tác với hệ điều hành.

Dialog New Log Space đang có thanh chọn `[ Local files | Remote | Scratch ]`. Thanh chọn này đến từ change `new-space-kind-switch` (đã archive nhưng chưa commit, là việc của người dùng). Change này dựng tiếp trên thanh chọn đó.

Marker có sẵn `line_hash`. `full_read.rs` đã dùng nó để giữ Marker khi đọc lại một Remote File.

## Goals / Non-Goals

**Goals:**

- Loại Log Space nói lên hành vi: Static là một bản cố định, Live là theo dõi. Nơi file nằm (local hay SSH) không còn quyết định loại.
- Static luôn hiện đúng nội dung file trên đĩa. Mọi thay đổi đều bị phát hiện, kể cả ghi đè cùng inode.
- Live giữ được dòng cũ khi file bị rotate hoặc replace, và ghi Break đúng lý do khi đoán được.
- Session cũ mở được mà không mất Log Space nào.

**Non-Goals:**

- Live Glob cho file local. Để sau; model không chặn việc này.
- Đổi một Log Space đã có từ Static sang Live hay ngược lại (xem Open Questions).
- Theo dõi server cho Remote Copy.
- Đổi cách đọc Archive, remote picker hay form Remote Command.

## Decisions

### 1. `LogSpaceKind { Static, Live, Scratch }` với serde alias

- `Static` mang `#[serde(alias = "Imported")]`, `Live` mang `#[serde(alias = "Remote")]`. Session cũ đọc được ngay, session mới ghi tên mới.
- Một file của Live space là local hay remote thì xem theo `LogFile.remote` (`Some` là Remote File). Không thêm cờ mới.
- Thay vì giữ tên cũ trong code và chỉ đổi nhãn: tên trong code sẽ lệch với CONTEXT.md, và `Remote` sẽ sai nghĩa khi Live space có file local.

### 2. Static: một Watch thay cho source live

- File local của Static không có `LocalFileSource`. Thay vào đó là một `FileWatch` nhẹ, chỉ chạy cho tab đang mở. Dormant Tab không poll.
- Mỗi lần poll, `FileWatch` đọc metadata: inode, kích thước, mtime.
- `IndexedFile` giữ một `Fingerprint` của phần đã index: inode, kích thước đã index, hash 4 KiB đầu và hash 4 KiB cuối. Fingerprint được cập nhật sau mỗi lần `extend()`.
- Khi metadata đổi, app so với Fingerprint:
  - Cùng inode, dài hơn, hai hash vẫn khớp: file chỉ được ghi thêm. Gọi `extend()`, không tự cuộn.
  - Mọi trường hợp khác (inode khác, ngắn đi, hash lệch, file bị xoá rồi có lại): đọc lại cả file ở nền như `read_indexed_again_if_stale` đang làm. Session được lưu trước, nên Filter được giữ.
- Khi một tab từ Dormant được mở lại, app so Fingerprint một lần trước khi hiện. File đã đổi thì đọc lại.
- File bị xoá: tab vẫn hiện nội dung cũ qua handle đang mở, kèm trạng thái "File deleted". Khi file có lại thì đọc lại.
- Thay vì bỏ qua thay đổi (Static là cố định thật sự): người dùng đã chốt rằng Imported bị replace thì phải cập nhật. "Static" nghĩa là không có Live/Pause, Follow hay Generation, chứ không phải đóng băng nội dung.

### 3. Static: Marker sau khi đọc lại

- Sau khi đọc lại, mỗi Marker được so `line_hash` với dòng cùng số dòng, giống `full_read.rs`. Khớp thì giữ Marker. Không khớp, hoặc dòng không còn, thì bỏ Marker.
- Có Marker bị bỏ thì hiện notice, ví dụ "3 Markers no longer match the file".
- Marker không có `line_hash` (dữ liệu cũ) thì được giữ theo số dòng.

### 4. Static: Reload cho Remote Copy

- Remote Copy có lệnh "Reload" trong menu của file. Lệnh này chạy lại `full_read` cho Remote File và các Rotated File đã chọn, theo cùng Starting Time.
- Marker được giữ theo quyết định 3.
- Không có poll SSH nào. Static space không giữ SSH session.

### 5. Live: Generation cho file local

- File local của Live có `LocalFileSource` như hiện nay.
- Mỗi Generation là một `IndexedFile` cùng handle `stay_open` của nó. Thứ tự gộp giữ (Generation, dòng), giống Remote File.
- Break có thêm hai `BreakKind` mới:

| Quan sát | Break | Dòng cũ |
|---|---|---|
| Inode mới, handle cũ có nlink > 0 (bị đổi tên) | `Rotated` | giữ, đọc qua handle cũ |
| Inode mới, handle cũ có nlink = 0 (bị xoá) | `Replaced` (mới) | giữ, đọc qua handle cũ |
| Cùng inode, ngắn đi | `Truncated` | bỏ: text không còn trên đĩa |
| Cùng inode, hash đầu hoặc cuối lệch | `Rewritten` (mới) | bỏ: text không còn trên đĩa |
| File bị xoá | `Deleted`, rồi `Rotated` hoặc `Replaced` khi file có lại | giữ tới khi rời tab |

- Kiểm tra hash dùng cùng `Fingerprint` của quyết định 2, trong `IndexedFile::extend()`.
- nlink được đọc bằng `fstat` trên handle cũ ngay lúc thấy inode mới. Windows không có nlink nên luôn ghi `Rotated`.
- Đoán sai thì chỉ có nhãn của Break sai. Không mất dòng và không lẫn nội dung.
- Thay vì đọc lại cả file như 4b.7 đang làm: làm vậy thì mất dòng cũ, đúng thứ mà người dùng chọn Live để giữ.

### 6. Live: handle của Generation cũ

- Một file đã bị xoá vẫn chiếm dung lượng đĩa chừng nào app còn mở nó.
- Khi tab thành Dormant, chỉ Generation hiện tại được giữ. Các Generation cũ được đóng, và dòng của chúng bị bỏ khỏi view.
- Mở lại tab thì đọc lại Generation hiện tại. Generation cũ không quay lại.
- Mọi handle đi qua `HandleBudget::shared()`. Hết handle thì Generation cũ nhất bị đóng trước.

### 7. Live: thao tác chỉ của Remote File

- Live/Pause và Follow áp cho mọi file của space. Pause dừng cả file local lẫn Remote File.
- Load more, Open in full, Live Window, Back to Live Window, "N lines held" và Clear chỉ hiện khi space có ít nhất một Remote File, và chỉ tác động lên Remote File.
  - Clear bỏ các dòng đang giữ của Remote File. Dòng của file local vẫn còn.
- Trong File View của một file local, các mục này không hiện.

### 8. Dialog: `[ Static | Live | Scratch ]`

- `NewSpaceKind { Static, Live, Scratch }` thay cho `{ Local, Remote, Scratch }`.
- Nấc Static dùng `NewLogSpaceForm` như nấc Local hiện nay.
- Nấc Live có hai danh sách:
  - danh sách local, là một `NewLogSpaceForm` thứ hai;
  - danh sách remote của `RemoteState`.
- Create ở nấc Live cần ít nhất một mục ở một trong hai danh sách.
- Nhãn Create gộp cả hai, ví dụ "Create with 1 file and 2 remote files".
- Archive không thêm vào nấc Live được: nó hiện trong danh sách kèm cảnh báo "Archives can only be added to a Static Log Space".
- Thả file vào dialog đang mở: ở nấc Live thì thêm vào danh sách local của Live, ở nấc khác thì gạt sang Static. Thả vào cửa sổ khi không có dialog nào mở thì tạo Static.
- Thay vì một nấc thứ tư "Live local": hai nấc theo hành vi là điều người dùng đã chốt, và Live trộn được cả hai nguồn.

### 9. CONTEXT.md (áp sau khi việc của change `new-space-kind-switch` được commit)

- **Imported Log Space** đổi thành **Static Log Space**: một Log Space là bản cố định của file local, Archive Entry hoặc Remote Copy. Nó hiện nội dung file như đang có trên đĩa và đọc lại khi file đổi; nó không có Live/Pause, Follow hay Generation. _Avoid_: Imported space, snapshot space.
- **Remote Log Space** đổi thành **Live Log Space**: một Log Space theo dõi file đang được ghi, gồm file local và Remote File, trên một timeline. _Avoid_: Remote space, tail space.
- **Remote File**: "a file of a Remote Log Space" đổi thành "a file on a Server, in a Live Log Space".
- **Remote Copy**: "added to an Imported Log Space" đổi thành "added to a Static Log Space"; thêm câu về Reload.
- Thêm **Local Live File**: một file local trong Live Log Space. Nó có Generation như Remote File nhưng không có Live Window, Load more hay Clear. _Avoid_: tailed file, local remote.
- Các định nghĩa đang nhắc "Imported" hay "Remote Log Space" (Archive Entry, Live Glob, Live Window…) được đổi theo.

## Risks / Trade-offs

- [Người dùng có Imported space chứa file local mà họ đang theo dõi (4b.7) sẽ thấy nó thành Static, mất Live/Pause] → Notice một lần sau khi mở session cũ: "Imported Log Spaces are now Static. Create a Live Log Space to follow files." 4b.7 mới có trên nhánh chính, chưa phát hành.
- [Hash 4 KiB đầu và cuối không bắt được ghi đè chỉ đổi ở giữa file mà giữ nguyên kích thước] → Chấp nhận, giống klogg. mtime đổi mà Fingerprint khớp thì không làm gì.
- [`copytruncate` làm mất text của Generation cũ ngay trên đĩa] → Break `Truncated` nói rõ chuyện đó. Dòng cũ không cứu được, vì app không giữ text trong memory (đó là điều line-offset-index nhắm tới).
- [Giữ handle của file đã xoá làm đầy đĩa khi log rất lớn] → Đóng khi tab thành Dormant (quyết định 6), và có giới hạn `HandleBudget`.
- [Hai danh sách local, của nấc Static và nấc Live, dễ gây nhầm] → Mỗi nấc giữ danh sách riêng như thanh chọn hiện nay, không hiện số đếm của nấc bị ẩn.
- [Status bar của Live space trộn local và remote phức tạp hơn] → Có test cho từng tổ hợp: chỉ local, chỉ remote, trộn.

## Migration Plan

1. Đổi `LogSpaceKind` kèm serde alias. Session cũ mở được, rồi được ghi lại bằng tên mới.
2. Mỗi bước sau là một commit, test xanh sau từng bước.
3. Rollback: revert các commit. Session đã ghi bằng tên mới sẽ không mở được bằng bản cũ. Muốn rollback an toàn thì bản cũ phải nhận alias ngược lại; vì app chưa phát hành, change này không làm việc đó.

## Open Questions

- Có cần lệnh "Convert to Live" / "Convert to Static" trên một Log Space đã có không? Tạm thời là không. Muốn đổi thì tạo space mới.
- Một Live space trộn file local với file SSH: nếu SSH mất kết nối thì có nên Pause cả file local không? Tạm thời là không. Mỗi file theo trạng thái của chính nó.
