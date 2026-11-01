## Context

Lý do và số đo nằm trong proposal. Tóm tắt: một Log Space 1 file 3.000.000 dòng làm malloc dùng thêm 308MB. Indexed chiếm 135MB; lớp view của app chiếm khoảng 165MB, gần như toàn bộ là dữ liệu lặp lại hoặc list mỗi dòng không cần thiết.

Hiện trạng liên quan:

- **`IndexedData { file: FileIndex, filter: FilterIndex, max_chars }`** nằm sau `RwLock` trong `Indexed`. Indexing và `IndexedFile::extend` ghi vào đó.
- **`build_file_view`** chép `data.filter.clone()` sang view. Trong FilterIndex, `effective_ts` là `Arc` nhưng `row_ids` của mọi cột bị chép. Live append sau đó gọi `Arc::make_mut` lên `effective_ts`, nên copy cả mảng.
- **`build_space_view`** luôn chạy `merge_by_meta`, kể cả với 1 file. Hàm đó cho ra `ViewRows::Merged(Vec<u64>)` và một `effective_ts` mới. Sau đó `FilterIndex::merge_files` dựng `row_ids` mới cho mọi cột trên các dòng đã merge, cộng một bảng tạm `row_of` 4 B/dòng.
- **Các chỗ đọc FilterIndex của view** đã đi qua accessor theo dòng: `ColumnIndex::id_of_row(row)`, `FilterIndex::effective_timestamp(row)`, cùng metadata cột (`values`, `coverage`, `kv_counts`). Riêng Time Histogram và Go To Time lấy cả slice qua `effective_ts()` (5 chỗ), và `RowFacts.ts: &[f64]`.
- **`visible_indices`** luôn là `0..n` (`apply_filters_and_refresh`: "The view keeps every line: Filter only narrows the pane"). `matched_visible_indices` vì thế đã là số dòng.
- **`filter_pass`** là `Arc<Vec<usize>>`, còn Time Histogram đã nhận `Option<&[usize]>`, với `None` nghĩa là không có Filter.
- **`LineMeta.ts_us`** là timestamp hiệu dụng tính bằng µs. Các dòng trước timestamp đầu tiên lấy timestamp đó, trong khi `effective_ts` của FilterIndex cho các dòng này là `NaN`. Đổi đơn vị bằng `meta::seconds`.

Ràng buộc: không hành vi nào người dùng thấy được phép đổi. Không thêm dependency. Không đụng phần còn lại của `line-offset-index`, `memory-budget` và `remote-local-cache`.

## Goals / Non-Goals

**Goals:**
- File View và Log Space 1 file 3.000.000 dòng: ≤ 150MB (spec `view-memory`). Ước tính sau change: khoảng 25–30 B/dòng.
- Log Space nhiều file: ≤ 60 B/dòng. Thứ tự merge (8 B/dòng) là phần duy nhất view giữ cho từng dòng.
- Không parse dòng live hai lần (Indexed một lần, FilterIndex của app một lần nữa).
- Có probe giữ lại để đo, và test so sánh trước/sau.

**Non-Goals:**
- Row Set (Roaring) cho kết quả Filter, Search, Marker, Unread, Focus rows: để change sau. Change này chỉ làm cho trường hợp "không có Filter" không tốn gì, và giữ API đủ gọn để Row Set thay vào sau.
- Live catch-up theo Live Window, `split_off` bậc hai, `rebuild()` của `CachedFile`.
- Class của Time Histogram, Row Cache, render.
- Thu nhỏ `LineMeta` (16 B) hay Line Index.

## Decisions

### 1. View đọc index của file qua một lần khoá cho mỗi thao tác, không chép

FilterIndex của view trở thành một enum `ViewIndex`:

```
ViewIndex
├─ File(Arc<Indexed>)          File View, Log Space 1 file: đọc thẳng IndexedData
└─ Merged(MergedIndex)         Log Space nhiều file (quyết định 3)
```

Mỗi thao tác (một lần đánh giá Filter, một lần tô Highlight, một lần đếm histogram, vẽ panel Filter) lấy một `ViewIndexRead` **một lần**: read guard của các file liên quan cộng các bảng của view. Thao tác đó đọc qua cùng các accessor như hôm nay: `column(key)`, `id_of_row(row)`, `effective_timestamp(row)`, `coverage`, `kv_counts`, `time_span`… Khi file nhận dòng live, `Indexed` tự cập nhật một lần, view chỉ cần biết số dòng mới. Bỏ đường `filter_index.append_marking_untimed` cùng bước parse lại các dòng mới qua Row Cache ở `app/live.rs`.

*Thay thế đã cân nhắc:*
- **`Arc<FilterIndex>` dùng chung, ghi bằng `Arc::make_mut`.** Đơn giản, nhưng mỗi lần Indexed append khi view còn giữ `Arc` sẽ chép cả index. Đó chính là lỗi `effective_ts` hiện tại, chỉ lan sang mọi cột.
- **Cột lưu theo khúc `Arc<[u16; 64K]>`** (persistent): clone rẻ và append chỉ chép khúc cuối. Tốt, nhưng thêm một cấu trúc dữ liệu mới, mà vẫn giữ hai "bản" index về mặt khái niệm. Có thể dùng sau, nếu khoá gây tranh chấp.
- **Khoá theo từng dòng** (`id_of_row` tự lấy lock): quá đắt trong vòng lặp 3.000.000 dòng.

### 2. Log Space 1 file là một File View có Source Column hằng

`build_space_view` với đúng 1 file trả `ViewRows::File { lines }` và `ViewIndex::File`. Source Column của Log Space hiện tên file cho mọi dòng. Nó trở thành một **cột hằng**: một giá trị, số đếm bằng số dòng, `id_of_row` luôn trả id đó, không lưu gì cho từng dòng. Thứ tự, timestamp, Continuation Line và cờ untimed y như merge của 1 file, vì merge 1 file là phép đồng nhất. Có test so sánh với đường merge cũ.

*Thay thế đã cân nhắc:* tối ưu riêng `merge_by_meta` cho 1 file nhưng vẫn ra `Merged`. Cách đó vẫn tốn 8 B/dòng mà không được gì.

### 3. Log Space nhiều file tra qua cột của file, đổi id bằng bảng nhỏ

`MergedIndex` giữ:
- `rows: Arc<Vec<u64>>`, dùng chung với `ViewRows::Merged` (quyết định 6).
- Cho mỗi cột: danh sách giá trị của Log Space theo thứ tự xuất hiện đầu tiên trên các dòng đã merge (như `merge_files` hôm nay), số đếm, spelling và Coverage đã cộng dồn, và **cho mỗi file một bảng `Vec<u16>` đổi id của file sang id của Log Space** (≤ `MAX_DISTINCT` phần tử).
- Cột Source là cột "theo file": id của dòng là id của file.

`id_of_row(row)` = `unpack(rows[row])` → `(file, line)` → id của file ở `line` → bảng đổi. `effective_timestamp(row)` = `ts_us` của `LineMeta` của file đó, `NaN` nếu file untimed (đúng như `merge_by_meta`). Thứ tự giá trị vẫn tính bằng một lượt qua các dòng đã merge, nhưng không còn dựng `row_of` (4 B/dòng tạm thời). Dòng live mới chỉ có thể thêm giá trị mới vào cuối danh sách và vào các bảng đổi.

*Thay thế đã cân nhắc:* giữ `row_ids` đã merge (2 B/dòng/cột). Đọc nhanh hơn một chút, nhưng với 6 cột cố định cộng các cột field, đó là 12–20 B/dòng chép lại thứ mà file đã có. Cái giá của cách tra là một `unpack` cộng một lần tra bảng mỗi dòng mỗi lần đánh giá, ước chừng ≤ 10ms cho 3.000.000 dòng. Sẽ đo ở task 7.

### 4. Bỏ `visible_indices`; vị trí là số dòng

Xoá field. Mọi `visible_indices.get(pos)` thành `(pos < view.len()).then_some(pos)`, mọi `.len()` thành `view.len()`, và `position(|&i| i == x)` thành `Some(x)`. Đổi tên `matched_visible_indices` thành `matched_rows`, nghĩa giữ nguyên. Xoá `src/ui/table_view.rs` (không còn ai dùng). Việc này cơ học, khoảng 18 file, nên làm thành một commit riêng để dễ review.

### 5. `filter_pass: Option<Arc<Vec<usize>>>`, và `None` nghĩa là mọi dòng

`None` khi `filter_state` rỗng. Các chỗ đọc đã có sẵn ngữ nghĩa này: Time Histogram nhận `Option<&[usize]>`, còn Focus Pane rỗng khi không có Filter và không có matcher. Bọc trong vài hàm (`passes(row)`, `passing_from(row)`, `pass_count()`) để Row Set sau này chỉ phải thay phần thân. Highlight dùng cùng cách: `RowPaints` không có rule nào bật thì `rows` rỗng, và `get` trả `None`. Không chạy pass tô màu khi không có rule nào tô.

### 6. `ViewRows::Merged(Arc<Vec<u64>>)`, và File nhận dòng bằng cách tăng độ dài

Clone một view hay gửi dòng sang Scan hoặc đếm histogram chỉ còn clone `Arc`. Khi có dòng live:
- `File { lines }` tăng `lines`. Bỏ `to_mut()` ghi ra mảng.
- `Merged` dùng `Arc::make_mut`. View là chủ duy nhất trong lúc append (Scan và đếm chạy xong hoặc bị huỷ khi rows đổi), nên không copy. Nếu có holder khác, copy một lần là đúng ngữ nghĩa.

### 7. `row_ids` lười, bắt đầu từ dòng đầu tiên có giá trị

`ColumnIndex` giữ `ids_from: usize` cùng `row_ids` chỉ cho các dòng từ `ids_from`. Cột chưa có giá trị nào thì `row_ids` rỗng. `id_of_row(row)` trả `MISSING` khi `row < ids_from` hoặc khi `row_ids` rỗng. `has_row_ids()` đổi nghĩa thành "lọc được theo id" (`!overflow`), không còn là "mảng khác rỗng". Hai chỗ gọi (`filter.rs:352`, `highlight.rs:189`) dùng nó đúng theo nghĩa này. `heap_bytes` tính theo `row_ids.capacity()`. Khi Live Window trim các dòng đầu, `ids_from` giảm trước, rồi mới cắt mảng.

### 8. Timestamp hiệu dụng đọc từ `LineMeta`, không lưu `effective_ts`

Với file, `effective_timestamp(row)` = `seconds(meta[row].ts_us)`, trừ khi `row < first_timed_row` hoặc `ts_us == NO_TIME`, khi đó là `NaN`. Đây đúng là giá trị `effective_ts` hôm nay, được kiểm bằng test so sánh trên các file có dòng đầu không có timestamp, có Continuation Line và hoàn toàn không có timestamp. FilterIndex giữ `first_timed_row`.

Các chỗ cần slice thì đổi:
- `RowFacts.ts: &[f64]` thành một accessor theo dòng, `ts(row) -> f64`, qua `ViewIndexRead`.
- `first_row_at` tìm nhị phân trên `0..n` qua accessor, thay cho `partition_point` trên slice.
- Đếm histogram trên worker thread nhận `ViewIndex` (các `Arc`) và tự lấy read guard theo khúc, thay cho clone `Arc<Vec<f64>>`.

### 9. `IndexedFile::extend` không giữ write lock trong lúc đọc và parse

Khi view đọc thẳng `IndexedData` (quyết định 1), một write lock dài sẽ chặn UI. Hiện `extend` giữ write lock suốt lúc đọc đĩa và chạy `index_more`, tới 4MB. Change này đổi thành: đọc và parse các dòng mới vào một phần riêng **ngoài** lock, rồi lấy write lock chỉ để nối phần đó vào (`FileIndex`, `MetaTable`, các cột).

### 10. Probe giữ lại

`examples/view_memory_probe.rs` phát triển từ `zz_view_breakdown_probe.rs` (bỏ file tạm). Nó dùng malloc hệ thống (`malloc_zone_statistics`) và đo ba loại view: File View, Log Space 1 file, Log Space 3 file × 1.000.000. Với mỗi loại, nó lấy từng phần của `AppView` ra để in MB và B/dòng, rồi in "OVER" nếu vượt giới hạn của spec. Probe không chạy trong test suite vì cần bản release và tốn vài giây.

## Risks / Trade-offs

- **[Tranh chấp lock giữa UI và Indexing/live]** → Quyết định 9 rút ngắn write lock xuống thời gian nối. Read guard lấy theo thao tác, không theo frame. Nếu vẫn thấy giật, chuyển sang cột theo khúc (thay thế của quyết định 1).
- **[Deadlock khi một thao tác cần guard của nhiều file]** → `ViewIndexRead` luôn lấy read guard theo thứ tự file của view, và không bao giờ lấy write lock khi đang giữ nó. Writer (Indexing, `extend`) chỉ khoá đúng file của mình.
- **[Kết quả lệch ở các trường hợp biên]**: dòng trước timestamp đầu tiên, file untimed, Continuation Line, cột overflow, Live Window trim. → Trước khi xoá đường cũ, có test so sánh: dựng view bằng đường cũ (`merge_files`, `effective_ts` lưu sẵn) và đường mới trên cùng dữ liệu, so từng dòng `id_of_row`, `effective_timestamp`, Filter Values, Coverage và histogram. Đường cũ chỉ bị xoá khi test xanh.
- **[Đánh giá Filter trên Log Space nhiều file chậm hơn]** do tra gián tiếp. → Đo ở task 7. Ngưỡng chấp nhận: ≤ 1,5 lần thời gian hiện tại và ≤ 30ms cho 3.000.000 dòng.
- **[Đổi tên/xoá field chạm nhiều file]** gây xung đột với các change đang mở (`add-swimlane`, `otel-view`, `text-highlight-and-value-colors`). → Làm bước xoá `visible_indices` thành commit riêng, sớm, để các nhánh khác rebase một lần.

## Migration Plan

Không có dữ liệu lưu nào đổi: session, Project, `keymap.json` và Remote Cache giữ nguyên. Thứ tự làm, mỗi bước đều chạy được và đo được bằng probe:

1. Probe giữ lại, ghi số đo trước.
2. Bỏ `visible_indices`, `filter_pass = None`, paints rỗng (≈ 52MB).
3. `ViewRows` Arc và append cho File.
4. `ViewIndex::File` cho File View và Log Space 1 file, live không parse hai lần, `extend` ngoài lock (≈ 108MB).
5. `row_ids` lười, rồi timestamp từ `LineMeta` (phần Indexed, ≈ 40MB).
6. `MergedIndex` cho Log Space nhiều file.
7. Test so sánh xanh thì xoá đường cũ, đo lại, cập nhật số trong design.

Rollback: mỗi bước là một commit tách được. Không bước nào đổi định dạng lưu.

## Open Questions

- Spec `line-index` của `line-offset-index` (chưa archive) có scenario "≤ khoảng 150MB". Khi archive cả hai change, có nên gộp scenario đó vào `view-memory`, hay giữ cả hai?
- Live Window của Remote File đi qua `CachedFile`. Nó có đi đúng đường `ViewIndex::File` không, hay cần một biến thể riêng cho chuỗi segment của Remote Cache? Cần đọc `src/live/cached.rs` khi tới task 4.
- Class của Time Histogram (1,4 B/dòng) có thể suy ra từ `LineMeta.level` theo cùng cách. Có làm luôn ở đây không? Hiện để ngoài phạm vi.

## Số đo

Đo bằng `examples/view_memory_probe.rs` (bản release, malloc hệ thống, mỗi view một tiến trình riêng, 3.000.000 dòng). Đơn vị: MB malloc dùng thêm, kèm B/dòng.

### Trước (main, 2026-11-01)

| Phần | File View | Log Space 1 file | Log Space 3 file |
|---|---|---|---|
| **Tổng khi mở** | **237,6 (79,2)** | **308,3 (102,8)** | **283,5 (94,5)** |
| `visible_indices` | 24,0 | 24,0 | 24,0 |
| `filter_pass` | 24,0 | 24,0 | 24,0 |
| `highlight_paints` | 4,2 | 4,2 | 4,2 |
| `histogram` | 4,2 | 4,2 | 4,2 |
| `filter_index` (app) | 36,1 | 74,4 | 69,6 |
| `view` (`ViewRows`) | 0 | 33,6 | 33,6 |
| `parsed_files` (Indexed) | 135,5 | 135,5 | 114,7 |
| còn lại | 9,6 | 8,4 | 9,2 |

Filter Level = ERROR trên mọi dòng của Log Space 3 file: 3,2ms (750.000 dòng qua). Đây là mốc cho task 7.4.

### Tiến độ theo nhóm

| Sau | File View | Log Space 1 file | Log Space 3 file | Filter (3 file) |
|---|---|---|---|---|
| Trước | 237,6 (79,2) | 308,3 (102,8) | 283,5 (94,5) | 3,2ms |
| Nhóm 3 (`visible_indices`, `filter_pass`, paints) | 185,4 (61,8) | 253,7 (84,6) | 231,3 (77,1) | 3,5ms |
| Nhóm 4+5+7 (view đọc index của file, `SpaceIndex`) | 149,3 (49,8) | 148,1 (49,4) | 161,7 (53,9) | 6,1ms |
| Nhóm 6 (`row_ids` lười, thời gian từ `LineMeta`) | **82,2 (27,4)** | **81,1 (27,0)** | **111,4 (37,1)** | 6,3ms |

Sau nhóm 6, Indexed của mỗi file còn 68,4MB cho 3.000.000 dòng (22,8 B/dòng, trước là 45,2), đúng mức spike `line-offset-index` đã đo (21–23 B/dòng). `memory_bytes` giảm theo, nên cùng Warm Tab Budget giữ ấm được khoảng gấp đôi số file. Phần còn lại của Log Space 3 file là thứ tự merge (33,6MB, 8 B/dòng cộng dung lượng dư), là dữ liệu thật.

### Khác với thiết kế khi làm

- **Khoá (quyết định 1).** View vẫn đọc qua read guard như thiết kế, nhưng `Indexed.data` đổi sang `parking_lot::RwLock` và view lấy `read_recursive()`. Lý do: trong lúc giữ guard, Filter hoặc Highlight đọc text dòng qua `IndexedFile::entry`, và hàm đó lại khoá cùng file. RwLock của std ưu tiên writer, nên một Indexing đang chờ ghi sẽ làm lần đọc lồng này deadlock. `parking_lot` 0.12.5 đã có sẵn trong cây phụ thuộc qua gpui; Cargo.toml chỉ thêm một dòng, không có crate mới. **Mọi** lần đọc index của file đều dùng `read_recursive()`, kể cả lần đọc lồng bên trong (`entry`, `len`, `meta`, `LineSpans`). Bản đầu chỉ đổi phía view, nên vẫn deadlock được; code review bắt ra lỗi này. Test `indexed_file_test::a_row_read_while_the_index_is_held_does_not_wait_for_a_writer` treo với `read()` thường và qua với `read_recursive()`.
- **File được đọc lại thành một Line Index mới** (Remote Cache rebuild không báo structural). `SpaceIndex` và `ViewIndex::File` từng giữ `Indexed` cũ, làm sai Filter của Remote Command (test `cached_remote_app_test`). Đã thêm `AppView::sync_view_index`, chạy đầu `apply_filters_and_refresh` và đầu mỗi lần append live: so con trỏ `Indexed` của từng file với `parsed_files`, rồi thay (File View) hoặc dựng lại (Log Space).
- **Log Space 1 file không cần Source Column hằng riêng.** Nó dùng `SpaceIndex` như Log Space nhiều file, với `ViewRows::File` và một `ColumnMap::Source` một phần tử: không có mảng theo dòng nào.
- **Task 6.3 (thời gian theo dòng cho histogram và Go To) làm luôn ở nhóm 5**, vì Log Space không còn slice `effective_ts`. `RowFacts.ts` là `&dyn RowTimes`. Đếm histogram trên worker nhận `ViewIndex` và `ViewRows` (clone `Arc`), rồi tự lấy guard.
- **`effective_ts` chỉ còn ở `FilterIndex` dựng từ entry** (`build`, `build_merged`, đường Owned và test). Index của file đọc qua `IndexedData::effective_timestamp`, được kiểm bằng `indexing_test` so từng dòng với `FilterIndex::build` từ entry.
- **Filter trên Log Space 3 file: 6,1–6,3ms, so với mốc 3,2–3,5ms (khoảng 1,8 lần).** Dưới ngưỡng 30ms nhưng trên mức 1,5 lần đặt ở rủi ro. Đã gộp bảng đổi id vào bảng điều kiện theo từng file (`RowTable`): 7,3ms xuống còn 6,1ms. Phần còn lại là một lần tra `rows[row]` cho mỗi dòng. Chấp nhận, vì Filter chỉ chạy khi điều kiện đổi.
- **`extend` đọc file khi không giữ lock nào** (quyết định 9 đầy đủ). Chỉ phần xem index đang có và việc đọc lại dòng cuối bị cắt là chạy dưới một read lock ngắn. Test kiểm rằng index ghi được (`try_write`) trong lúc đọc.
- **Merge không còn `row_of`** (task 7.1). Một lượt qua các dòng đã merge tìm dòng đầu của mỗi file và của mỗi giá trị, rồi dừng khi đã thấy hết.
- **`filter_pass` là enum `FilterPass { All { rows }, Rows(..) }`** chứ không phải `Option<Arc<Vec<usize>>>` (quyết định 5). `len()` vẫn là số dòng qua, nên các chỗ đọc và test giữ nguyên.
- **`has_row_ids()` vẫn là `!overflow && row_ids.len() == rows`** (quyết định 7 ghi chỉ `!overflow`). `RowIdList::len()` tính cả các dòng chưa có id, nên điều kiện độ dài vẫn đúng mà vẫn bắt được cột chưa theo kịp số dòng.
- **Có thêm `ViewIndex::Owned`** cho giá trị đọc được trong lúc file còn đang được đọc. Trong lúc đó, append live dựng lại view thay vì thêm tại chỗ; chỉ chậm hơn, người dùng không thấy khác.
- **File View của file live chưa có timestamp.** Trước change, các dòng mới được đánh dấu không có thời gian chỉ khi file chưa có timestamp nào. Khi timestamp đầu tiên tới, lane đổi kiểu và view được dựng lại. `IndexedData::effective_timestamp` cũng không cho dòng nào thời gian khi file chưa có timestamp (`first_timed_row` là `None`). Sau khi có timestamp, nó cho đúng như bản dựng lại. Hai cách cho cùng kết quả.
- **Đã xoá** `FilterIndex::append_marking_untimed`, `drop_rows`, `AppView::rebuild_filter_index` (không ai gọi) và `src/ui/table_view.rs` (không được khai báo là module).

### Test

- Mới: `view_index_golden_test` (golden ghi từ code của `main` trong một worktree tạm, cho File View, Log Space 1 file, Log Space 3 file; gồm cả các cột của Time Histogram có và không có Filter), `live_space_index_test` (Level mới tới khi live; Log Space live không giữ cột riêng theo dòng, so với bản dựng lại), unit test `RowIdList`, `filter_change_test` (không có Filter / rule thì không giữ gì), `live_incremental_test::a_file_view_taking_live_lines_lists_no_rows`, `indexed_file_test::reading_new_lines_leaves_the_index_readable`.
- 1.559 test liên quan qua (khoảng 120 file test). Riêng `raw_list_test::a_drag_selects_text_to_paint_a_marker` đỏ, nhưng nó đỏ cả trên `main`: test chờ color bar mở khi chọn text, điều mà change Selection Menu đã bỏ. Chưa chạy test cần sshd.
- Chưa kiểm bằng mắt trong app (task 8.3).
- Tìm thấy khi viết test, có từ trước trên `main`, **đã sửa luôn ở đây**: dòng dài nhất tính khi append live khác với bản dựng lại. Bản append so độ dài message (`payload`), còn `find_longest_row` so độ dài cả dòng theo Line Index. Giờ cả hai dùng cùng thước đo và cùng cách chọn khi bằng nhau (`longest_key`), và bản append không còn đọc từng dòng mới chỉ để đo. Test: `live_incremental_test::the_longest_row_added_in_place_is_the_one_a_rebuild_picks`.
