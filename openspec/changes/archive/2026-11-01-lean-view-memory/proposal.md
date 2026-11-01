## Why

Spec `line-index` (change `line-offset-index`) hứa rằng mở một file 3.000.000 dòng thì app chỉ dùng thêm "không quá khoảng 150MB". Số đo ngày 2026-11-01 cho thấy hiện tại là **308MB**, tức 103 B/dòng, gấp đôi lời hứa.

Đo bằng probe tạm `examples/zz_view_breakdown_probe.rs`. Điều kiện: bản release, malloc hệ thống, một Static Log Space có 1 file, chưa có Filter, Search hay Highlight Rule. Probe lấy từng field của `AppView` ra rồi xem malloc giải phóng bao nhiêu:

| Phần | MB | B/dòng | Là gì |
|---|---|---|---|
| `parsed_files` (Indexed) | 135,5 | 45 | Line Index, `LineMeta`, FilterIndex của file |
| `filter_index` của app | 74,4 | 25 | FilterIndex dựng lại trên các dòng đã merge. Với 1 file, nó giống hệt bản trong Indexed |
| `view.rows` | 33,6 | 11 | `ViewRows::Merged(Vec<u64>)`. Với 1 file, đây là `0..n` ghi ra thành mảng |
| `visible_indices` | 24,0 | 8 | Luôn là `0..n`, vì từ khi Filter chỉ thu hẹp Focus Pane, view luôn giữ mọi dòng |
| `filter_pass` | 24,0 | 8 | Mọi dòng, kể cả khi không có Filter nào |
| `highlight_paints` | 4,2 | 1,4 | Một byte cho mỗi dòng, kể cả khi không có rule nào |
| `histogram` | 4,2 | 1,4 | Class của mỗi dòng |

Lớp view của app (khoảng 165MB) tốn nhiều hơn chính cái index. Gần như toàn bộ phần này là dữ liệu lặp lại, hoặc là các list "mỗi dòng một phần tử" còn sót lại từ thời `Vec<LogEntry>`. Ngay trong phần Indexed, FilterIndex cũng giữ những thứ suy ra được: `effective_ts` (8 B/dòng) trùng với `LineMeta.ts_us`, và các cột Source, Stream, RequestId, TraceId lưu `row_ids` dù mọi dòng đều MISSING.

Memory này còn hạn chế Warm Tabs. Warm Tab Budget (512MB mặc định) được tính theo memory mà Indexed giữ. Indexed gọn lại thì giữ ấm được nhiều tab hơn, nên người dùng ít phải chờ index lại khi quay lại một tab.

## What Changes

Không có thay đổi nào người dùng thấy được. Mọi view, Filter, Search, Highlight, Time Histogram, Marker và Live đều cho kết quả như trước.

- **Log Space có 1 file dùng chung index của file.**
  - Thứ tự dòng là `ViewRows::File`, không ghi ra mảng.
  - FilterIndex của view là chính bản trong Indexed, dùng chung qua `Arc`, không dựng lại.
  - Khi file nhận dòng live, view lấy lại bản mà Indexed đã cập nhật. View không parse và append các dòng mới lần thứ hai.
- **Log Space nhiều file không chép lại cột của từng file.** FilterIndex của Log Space tra id giá trị qua `row_ids` của từng file, đổi sang id của Log Space bằng một bảng nhỏ cho mỗi file. Nó không dựng `row_ids` và `effective_ts` riêng cho các dòng đã merge.
- **Bỏ `visible_indices`.** Vị trí trong view chính là số dòng của view. Xoá luôn `src/ui/table_view.rs`, vì không còn ai dùng.
- **`filter_pass` rỗng nghĩa là "mọi dòng"** khi không có Filter: không giữ list nào.
- **`highlight_paints` không giữ byte nào cho mỗi dòng** khi không có Highlight Rule nào tô màu.
- **FilterIndex của file gọn lại.**
  - Một cột chỉ có `row_ids` từ dòng đầu tiên có giá trị. Cột không có giá trị nào thì không giữ gì cho từng dòng.
  - Timestamp hiệu dụng của mỗi dòng đọc từ `LineMeta.ts_us`, không lưu lại trong `effective_ts`.
- **`ViewRows` của File View nhận dòng live bằng cách tăng độ dài**, không ghi ra mảng. `ViewRows::Merged` được giữ trong `Arc`, nên một Scan hay một lần đếm Time Histogram không chép nó.
- **Một probe đo memory được giữ lại**: `examples/view_memory_probe.rs`, phát triển từ probe tạm. Nó báo memory theo từng phần cho một File View, một Log Space 1 file và một Log Space nhiều file.

Không nằm trong change này, để cho change sau:

- **Row Set** (Roaring) cho tập kết quả Filter, Matches, Marked rows, các dòng chưa đọc của text rule và Focus rows.
- **Live catch-up theo đúng Live Window** (L10, L12): khoảng trống lớn thì nhảy tới tail, bỏ `split_off` bậc hai và `rebuild()` mỗi đợt backfill.
- Class của Time Histogram (1,4 B/dòng).

## Capabilities

### New Capabilities
- `view-memory`: memory mà một view đang mở được dùng cho mỗi dòng, ngoài Row Cache: File View, Log Space 1 file, Log Space nhiều file, khi có và không có Filter hay Highlight Rule. Kèm cách đo.

### Modified Capabilities
<!-- Không có. Spec `line-index` (change `line-offset-index`, chưa archive) đã hứa ~150MB cho 3.000.000 dòng; change này làm cho lời hứa đó đúng mà không đổi nó. -->

## Impact

- **Code**:
  - `src/space_view.rs`: `build_space_view`, `merge_by_meta`, `build_file_view`.
  - `src/filter_index.rs`, `src/filter_index/chunk.rs`, `src/filter_index/merge.rs`: `row_ids` lười, thời gian lấy từ `LineMeta`, index của Log Space tra qua file.
  - `src/index/indexing.rs`, `src/index/file.rs`: `IndexedData.filter` thành `Arc<FilterIndex>`.
  - `src/index/view_rows.rs`: `Merged(Arc<Vec<u64>>)` và append cho `File`.
  - `src/app/mod.rs` và mọi chỗ đọc `visible_indices` (18 file), `filter_pass` (6 file) và `effective_ts()` (5 chỗ).
  - `src/app/filter.rs`, `src/app/live.rs`, `src/app/loading.rs`, `src/app/tabs.rs`, `src/app/highlights.rs`, `src/app/time_histogram.rs`, `src/app/go_to.rs`.
  - Xoá `src/ui/table_view.rs`.
- **Dependency**: không thêm.
- **Hành vi**: không đổi. Test hiện có phải giữ nguyên kết quả. Test mới so sánh view trước và sau trên cùng dữ liệu.
- **Liên quan**:
  - `line-offset-index` và `memory-budget` (mỗi change còn 2 task): không đụng phần còn lại của chúng.
  - `remote-local-cache`: Live Window của Remote File đi qua `CachedFile`, có view 1 file, nên được lợi như Log Space 1 file.
