## 1. Tài liệu và quyết định (đã xong)

- [x] 1.1 Chốt tiếng Việt Mức 1 (so khớp đúng Unicode, không bỏ dấu)
- [x] 1.2 Filter dùng cùng quy tắc Unicode với Search
- [x] 1.3 Viết ADR-0004 (Search text-only) và ADR-0005 (Filter theo cột phát hiện, bỏ Saved View)
- [x] 1.4 Cập nhật `CONTEXT.md` (thuật ngữ và quy tắc 7–21)

## 2. So khớp Unicode dùng chung

- [x] 2.1 Thêm module so khớp Unicode: chuẩn hóa NFC, gấp hoa/thường (kể cả `đ/Đ`), `eq`, `contains`, `find_all` trả offset byte trên chuỗi gốc, đường nhanh cho dòng toàn ASCII
- [x] 2.2 Thêm crate `unicode-normalization` vào `Cargo.toml`
- [x] 2.3 Test: `ĐÃ` khớp `đã`, NFC và NFD khớp nhau, `loi` không khớp `lỗi`, offset đúng với ký tự đa byte

## 3. Parser Search Expression (module mới)

- [x] 3.1 Tạo module Search Expression riêng với AST `Phrase`, `And`, `Or`
- [x] 3.2 Tokenizer theo `char` cho `&&`, `||`, `(`, `)`, `"..."` và chữ; không coi `and`/`or` là toán tử
- [x] 3.3 Parser theo ngữ pháp trong `design.md` (`&&` ưu tiên hơn `||`)
- [x] 3.4 Báo lỗi cú pháp: ngoặc chưa đóng, toán tử thiếu vế, `"` chưa đóng, Phrase liền kề nhóm ngoặc không có toán tử
- [x] 3.5 Test parser cho mọi scenario trong `specs/log-search/spec.md`, gồm ca gõ dở và chữ Việt

## 4. Matcher Search và nối vào UI

- [x] 4.1 Compile Search Expression một lần thành matcher: Phrase khớp tuần tự, mọi chuỗi khoảng trắng thỏa một dấu cách, dùng module ở mục 2
- [x] 4.2 Trả highlight cho mọi lần xuất hiện của mỗi Phrase, đúng offset byte trên chuỗi gốc
- [x] 4.3 Đổi `SearchState` sang matcher mới; giữ biểu thức hợp lệ gần nhất và lỗi hiện tại; khi lỗi giữ danh sách Match và highlight cũ; khi rỗng xóa Match và lỗi
- [x] 4.4 Search chỉ chạy trên `visible_indices` (không đếm dòng bị Filter ẩn)
- [x] 4.5 Cập nhật `SearchEngine` trong `infrastructure/search_engine.rs`
- [x] 4.6 Cập nhật chỉ báo lỗi trong `app/view/toolbar.rs`; bỏ completion provider của ô Search trong `app/mod.rs`
- [x] 4.7 Giữ điều hướng Match (kế tiếp, trước đó, quay vòng, vị trí trên tổng số)
- [x] 4.8 Không lưu Search Expression vào session
- [x] 4.9 Test matcher và `SearchState` theo `specs/log-search/spec.md`

## 5. Phát hiện cột và mô hình Filter

- [x] 5.1 Bước phát hiện khi nạp file: danh sách cột lọc được và bảng giá trị (gộp hoa/thường, dạng hiển thị phổ biến nhất, số dòng); dừng thu thập khi vượt 500 và đánh dấu "> 500"; loại `timestamp` và `message`/payload
- [x] 5.2 Gán id nhỏ (`u16`) cho giá trị và lưu id theo dòng và theo cột, với id riêng cho "thiếu giá trị"
- [x] 5.3 `FilterState` mới theo file: theo cột `{values, custom, hide}` và `TimeRange { from, to }`
- [x] 5.4 Đánh giá Filter bằng so sánh id: `custom` quy về tập id bằng cách duyệt bảng giá trị của cột; OR trong cột, AND giữa cột và Time Range; dòng thiếu giá trị không khớp; Hide Mode đảo
- [x] 5.5 Timestamp hiệu dụng: dòng không có timestamp kế thừa dòng phía trên; dòng đầu file luôn hiện
- [x] 5.6 Parse ô Từ/Đến theo dạng hiển thị trong cột Timestamp, so sánh theo đồng hồ của file
- [x] 5.7 Khi tail: thêm giá trị mới vào cuối bảng (chưa tích), cộng số đếm, giữ điều kiện đã bật khi cột vượt 500
- [x] 5.8 Test phát hiện cột với log text, JSON và logcat/syslog; test Filter theo `specs/log-filter/spec.md`

## 6. Giao diện Filter

- [x] 6.1 Làm lại mục Filter trong context sidebar: chọn cột kèm Distinct Count (cột quá 500 hiện mờ kèm lý do)
- [x] 6.2 Danh sách checkbox giá trị kèm số dòng, nút "Add value", công tắc Hide Mode theo cột
- [x] 6.3 Ô Từ/Đến cho Time Range kèm gợi ý nhỏ nhất và lớn nhất của file
- [x] 6.4 Hiển thị các điều kiện đang bật (chip) và cho gỡ từng điều kiện
- [x] 6.5 Quyết định cách phân biệt trên UI giữa giá trị bằng (checkbox) và giá trị chứa chuỗi (Custom Value)

## 7. Lưu theo file

- [x] 7.1 Kiểm tra Marker hiện được lưu ở đâu và gom Marker theo file nếu chưa
- [x] 7.2 Lưu `FilterState` và Marker theo file (khóa theo `LogFileDescriptor`) trong session manifest và khôi phục khi mở lại file
- [x] 7.3 Tăng version manifest; khi đọc version cũ thì bỏ `saved_views` và Filter cũ, mở được bình thường
- [x] 7.4 Test `session_test`, `project_test`, `workspace_test` theo `specs/log-filter/spec.md`

## 8. Bỏ phần không còn dùng

- [x] 8.1 Bỏ panel View trong context sidebar, danh sách module cố định, `app/view/view_panel.rs`; chuyển density sang settings
- [x] 8.2 Bỏ menu nhanh trên giá trị level/module (`QuickLogMenuKind` và `trigger_search_query` gọi từ đó)
- [x] 8.3 Xóa `SavedView`; chuyển `search_across_files` và `nearest_timestamp_index` sang file khác; bỏ `saved_view_test`
- [x] 8.4 Xác nhận bằng tìm kiếm rằng `src/query/` không còn tham chiếu, rồi xóa parser, validator, `language_service`, `completion_provider`, AST, engine, `src/infrastructure/` và các test field-query. `schema.rs` được `parser/text.rs` dùng nên chuyển sang `src/parser/schema.rs` (bỏ `for_log_fields`); bỏ crate `lsp-types`
- [x] 8.5 Chạy `cargo build` và `cargo test`, sửa mọi lỗi còn sót

## 9. Kiểm tra cuối

- [x] 9.1 Chạy toàn bộ `cargo test` và `cargo clippy`
- [ ] 9.2 (chưa làm: chưa kiểm tra bằng mắt trên cửa sổ app; chỉ mới build và chạy test) Chạy app thử Search (hợp lệ, gõ dở, chữ Việt), Filter (checkbox, Add value, Hide Mode, Time Range, dòng thiếu giá trị, tail) và mở lại file để kiểm tra khôi phục
- [x] 9.3 Đo hiệu năng (release, 300k dòng): dựng index 17ms, áp Filter 1,4ms, Search 80–160ms
- [ ] 9.4 Rà soát lại các spec so với hành vi thực tế
