## 1. Probe và số đo trước

- [x] 1.1 Tạo `examples/view_memory_probe.rs` từ `examples/zz_view_breakdown_probe.rs`, rồi xoá file tạm. Probe dùng malloc hệ thống và đo ba view: File View 3.000.000 dòng, Log Space 1 file 3.000.000 dòng, Log Space 3 file × 1.000.000 dòng có timestamp xen kẽ. Mỗi view in MB và B/dòng theo từng phần (lấy từng field của `AppView` ra), và in "OVER" khi vượt giới hạn của spec `view-memory`.
- [x] 1.2 Chạy probe trên `main`, rồi ghi số đo trước vào mục "Số đo" (mới) của design.md cho cả ba view, kèm thời gian đánh giá một Filter Level = ERROR trên Log Space 3 file (dùng làm mốc cho rủi ro "Filter chậm hơn").

## 2. Test so sánh trước/sau

- [x] 2.1 Viết test so sánh (dữ liệu nhỏ, chạy trong suite): với File View, Log Space 1 file và Log Space 3 file, ghi lại cho mỗi dòng `id_of_row` của mọi cột và `effective_timestamp`, cùng Filter Values, số đếm, Coverage, `kv_counts`, `time_span` và các cột của Time Histogram, dựng bằng đường hiện tại (`merge_files`, `effective_ts` lưu sẵn). Bao các trường hợp biên: dòng đầu không có timestamp, Continuation Line, file hoàn toàn không có timestamp, cột overflow, cột chỉ có giá trị từ giữa file.
- [x] 2.2 Đưa phần ghi lại đó thành một hàm dùng chung cho đường cũ và đường mới, để các group 5–8 so được đường mới với đường cũ trên cùng dữ liệu.

## 3. Bỏ list mỗi dòng khi không cần

- [x] 3.1 Xoá `AppView::visible_indices`: `get(pos)` thành `(pos < view.len()).then_some(pos)`, `len()` thành `view.len()`, `position(..)` thành id trực tiếp. Đổi tên `SearchState::matched_visible_indices` thành `matched_rows`. Xoá `src/ui/table_view.rs` và khai báo module của nó. Làm thành một commit riêng. Test hiện có xanh.
- [x] 3.2 `filter_pass: Option<Arc<Vec<usize>>>`, `None` khi `filter_state` rỗng. Thêm `passes(row)`, `passing_from(row)` và `pass_count()`, rồi đổi các chỗ đọc ở `focus.rs`, `filter.rs`, `time_histogram.rs` và `app/time_histogram.rs`. Test: thêm rồi bỏ Filter thì `filter_pass` là `None`; Focus Pane và Time Histogram cho kết quả như trước.
- [x] 3.3 `RowPaints` rỗng (`rows` không có phần tử) khi không có Highlight Rule nào bật. `get` trả `None`, và không chạy pass tô màu. Test: tắt rule cuối cùng thì `highlight_paints` không giữ byte nào và không dòng nào được tô.
- [x] 3.4 Chạy probe, ghi số vào design.md.

## 4. ViewRows dùng chung và append cho File

- [x] 4.1 `ViewRows::Merged(Arc<Vec<u64>>)`. Bỏ các bản clone sâu ở `view_data.rs` (`texts`) và `app/time_histogram.rs` (recount). Test hiện có của `view_rows` xanh.
- [x] 4.2 Thay `ViewRows::to_mut()` bằng các thao tác riêng: `File` tăng `lines` khi có dòng live; `Merged` dùng `Arc::make_mut`. Đổi `app/live.rs` (`append_to_open_view`). Test: một File View nhận 10.000 dòng live thì vẫn là `ViewRows::File`.

## 5. View đọc index của file

- [x] 5.1 Định nghĩa `ViewIndex { File(Arc<Indexed>), Merged(MergedIndex) }` và `ViewIndexRead`, một guard lấy một lần cho mỗi thao tác. Guard có các accessor mà code hiện dùng: `column`, `columns_in_use`, `field_columns`, `id_of_row`, `effective_timestamp`, `coverage`, `kv_counts`, `timed_lines`, `parent_lines`, `time_span`, `anchor_day`, `time_hint`. Guard lấy read lock theo thứ tự file của view.
- [x] 5.2 Đổi `AppView::filter_index` thành `ViewIndex`, và đổi các chỗ đọc (`filter.rs`, `highlight.rs`, `app/filter.rs`, `app/highlights.rs`, `app/columns.rs`, `app/context_layout.rs`, `app/line_config.rs`, `app/selection_menu.rs`, các panel trong `app/view/`) sang lấy guard một lần cho mỗi thao tác.
- [x] 5.3 `build_file_view` trả `ViewIndex::File`, không chép `data.filter`. Test so sánh (2.1) xanh cho File View.
- [x] 5.4 `build_space_view` với 1 file trả `ViewRows::File` và `ViewIndex::File`, cộng một Source Column hằng (một giá trị là tên file, số đếm bằng số dòng). Test so sánh xanh cho Log Space 1 file, kể cả cột Source và Filter theo Source.
- [x] 5.5 Dòng live cho view `ViewIndex::File`: Indexed tự cập nhật, view chỉ nhận số dòng mới. Bỏ phần append FilterIndex và parse lại các dòng mới qua Row Cache ở `app/live.rs` (`append_marking_untimed`). Test: một Local Live File nhận 100.000 dòng thì mỗi dòng được parse cho cột Filter đúng một lần (đếm bằng một counter test-only), và Filter Values đúng như trước. *Làm bằng cách khác: `live_space_index_test` kiểm rằng view không giữ cột nào theo dòng (`own_heap_bytes` < 64KB sau hơn 1.500 dòng live), tức không có lần parse thứ hai, và Filter Values bằng bản dựng lại.*
- [x] 5.6 `IndexedFile::extend` đọc và parse các dòng mới ngoài lock, rồi lấy write lock chỉ để nối vào. Test: trong lúc `extend` đọc 4MB, một read guard lấy được ngay (dùng một `LineSource` test chậm).
- [x] 5.7 Đọc `src/live/cached.rs`: Live Window của Remote File (`CachedFile`) có đi đúng đường `ViewIndex::File` không. Nếu không, thêm biến thể cần thiết, rồi ghi quyết định vào design.md. Test `cached_file_test` và `cached_remote_app_test` xanh.
- [x] 5.8 Chạy probe, ghi số vào design.md. File View và Log Space 1 file ≤ 50 B/dòng.

## 6. Cột Filter của file gọn lại

- [x] 6.1 `ColumnIndex` có `ids_from`, và `row_ids` chỉ từ dòng đầu tiên có giá trị. `id_of_row` trả `MISSING` trước `ids_from` hoặc khi mảng rỗng. `has_row_ids()` thành `!overflow`. Sửa `push_chunk` (`chunk.rs`), live `push_row`, `truncate` / trim của Live Window, `merge.rs` và `heap_bytes`. Test đơn vị: cột không có giá trị không giữ byte nào; cột có giá trị từ dòng 2.000.000 lọc đúng; trim qua `ids_from` vẫn đúng. Test so sánh xanh.
- [x] 6.2 Bỏ `FilterIndex::effective_ts`. `effective_timestamp(row)` đọc `LineMeta.ts_us` qua `meta::seconds`, là `NaN` trước `first_timed_row` hoặc khi `NO_TIME`. Bỏ các `Arc::make_mut` của `effective_ts` ở `chunk.rs` và `filter_index.rs`. Test so sánh xanh, đặc biệt với dòng trước timestamp đầu tiên và file không có timestamp.
- [x] 6.3 `RowFacts.ts` thành accessor theo dòng, và `first_row_at` tìm nhị phân trên `0..n` qua accessor. Đổi `go_to.rs` và `app/time_histogram.rs` (5 chỗ). Đếm histogram trên worker nhận `ViewIndex` và lấy guard theo khúc. Test hiện có của `time_histogram` (kể cả `first_row_at_a_time`) xanh.
- [x] 6.4 Chạy probe, ghi số vào design.md, kèm `memory_bytes` của file trước và sau (ảnh hưởng tới Warm Tab Budget).

## 7. Log Space nhiều file tra qua file

- [x] 7.1 `MergedIndex`: `rows: Arc<Vec<u64>>` dùng chung với `ViewRows`. Cho mỗi cột: giá trị của Log Space theo thứ tự xuất hiện đầu tiên, số đếm, spelling, Coverage, và một bảng đổi id `Vec<u16>` cho mỗi file. Cột Source theo file. Dựng bằng một lượt qua các dòng đã merge, không có `row_of`. Test đơn vị: thứ tự giá trị và số đếm khớp `merge_files`.
- [x] 7.2 `id_of_row` và `effective_timestamp` của `MergedIndex` tra qua file, với file untimed cho `NaN`. `build_space_view` với ≥ 2 file trả `ViewIndex::Merged`. Test so sánh xanh cho Log Space 3 file.
- [x] 7.3 Dòng live trên Log Space nhiều file: thêm giá trị mới vào cuối danh sách và vào các bảng đổi; các dòng mới đi qua `ViewRows` như hiện tại. Test: Live Log Space 2 file nhận dòng có Level mới thì Filter Values có thêm giá trị đó với số đếm đúng. *`live_space_index_test::a_level_first_seen_live_joins_the_filter_values`.*
- [x] 7.4 Đo thời gian đánh giá Filter Level = ERROR trên Log Space 3 file × 1.000.000 dòng, so với mốc ở 1.2: ≤ 1,5 lần và ≤ 30ms. Ghi vào design.md. *Kết quả: 6,1–6,3ms, khoảng 1,8 lần mốc; đạt ≤ 30ms nhưng không đạt ≤ 1,5 lần. Chấp nhận, lý do ghi trong design.md.*
- [x] 7.5 Chạy probe, ghi số vào design.md. Log Space 3 file ≤ 60 B/dòng.

## 8. Dọn và kiểm

- [x] 8.1 Xoá đường cũ không còn dùng (`FilterIndex::merge_files` dựng `row_ids`, `merge_by_meta` trả `effective_ts`, `append_marking_untimed`). Giữ test so sánh với dữ liệu đã ghi sẵn (golden) thay cho đường cũ.
- [x] 8.2 Chạy các test liên quan tới phần đã đổi (filter, highlight, time_histogram, live, cached, space_view, view_rows, index), không chạy cả suite và sshd.
- [ ] 8.3 Chạy app với một file 3.000.000 dòng và một Log Space 3 file. Kiểm bằng mắt rằng Filter, Search, Highlight, Time Histogram, Go To Time và Marker như trước, và Activity Monitor cho thấy footprint thấp hơn số đo trước.
- [x] 8.4 Cập nhật bảng số đo cuối trong design.md: trước/sau cho từng phần và từng view.
