## Why

Search chỉ tô Match và nhảy qua từng Match, còn Filter ẩn dòng ngay trong Parsed View. Muốn đọc ngữ cảnh quanh một Match thì phải bỏ Filter và mất chỗ đang đọc. Muốn thấy mọi Match một lượt thì phải bấm next từng cái. Người dùng cần thấy cả log đầy đủ và các dòng đã thu hẹp trên cùng một màn hình, giống klogg (xem ADR-0008).

## What Changes

**Focus Pane**
- Pane mới rộng hết cửa sổ, nằm dưới view và dưới cả hai sidebar. Chiều cao kéo được. Có nút trên thanh Search và một phím tắt để bật/tắt. Trạng thái mở và chiều cao được nhớ theo app.
- Focus Pane liệt kê các dòng qua cả Filter lẫn Search Expression, mỗi dòng là raw line kèm số dòng, và Source File khi Log Space có nhiều file. Phrase khớp được tô. Search Expression rỗng hoặc Filter rỗng thì không giới hạn; cả hai rỗng thì pane trống và hiện gợi ý.
- Trong Raw View, Focus Pane chỉ dùng Search Expression, bỏ qua Filter.
- Dòng hiện tại của Focus Pane và selection của view luôn đi theo nhau: bấm một dòng trong pane thì view cuộn tới và chọn dòng đó; next/prev trong view thì pane chọn theo.
- Focus Pane hiện Marker và Level Marker. Khi view là Parsed View, pane cũng tô Highlight Rule. Pane tự cập nhật khi tail và khi sửa file scratch.
- Chép từ Focus Pane chỉ lấy các dòng pane đang liệt kê.

**Focus Tab**
- Focus Pane có một hoặc nhiều Focus Tab đánh số 1, 2, 3… Mỗi tab có Search Expression riêng. Ô Search trên toolbar luôn sửa Search Expression của tab đang mở.
- Nút "+" tạo tab mới lấy số lớn nhất cộng một. Số không bị đánh lại khi đóng tab khác. Nhãn tab là số kèm phần đầu của Search Expression. Tab cuối cùng không có nút đóng.
- View chỉ tô Match của tab đang mở; bộ đếm, next/prev và overview ruler cũng theo tab đó. Mọi tab dùng chung Filter của view.
- Mỗi tab nhớ dòng hiện tại của nó. Mở một tab thì view nhảy tới dòng đó.
- **BREAKING**: Focus Tab cùng Search Expression, Focus Find và dòng hiện tại được nhớ theo view như Filter. Trước đây Search Expression không được nhớ.

**Focus Find**
- Mỗi Focus Tab có ô Focus Find riêng trong pane. Ô này tìm và nhảy giữa các dòng tab đang liệt kê, viết giống Search Expression, và không thu hẹp tab. Kết quả chỉ được tô trong pane; nhảy tới kết quả thì view cuộn theo.

**Filter và Search**
- **BREAKING**: Filter không còn ẩn dòng trong Parsed View. View luôn hiện mọi dòng; Filter chỉ thu hẹp Focus Pane. Sidebar Filter vẫn chỉ hiện ở Parsed View.
- Trong Parsed View, chỉ dòng qua Filter mới là Match (giữ nguyên số đếm như cũ).
- Chép vùng chọn ở view lấy mọi dòng trong vùng chọn, vì view không còn dòng bị ẩn.

**Inspector**
- **BREAKING**: Inspector chỉ hiện text lưu trữ của dòng đang chọn, kèm các Continuation Line của nó. Bỏ tab Fields và Hex. Inspector hiện ở cả Raw View và Parsed View, và theo dòng đang chọn ở bất kỳ khung nào.

## Capabilities

### New Capabilities
- `focus-pane`: Focus Pane, Focus Tab, Focus Find, đồng bộ dòng hiện tại với view, bố cục, ghi nhớ, cập nhật khi tail/sửa.
- `inspector`: Inspector chỉ hiện text lưu trữ của dòng đang chọn, ở cả hai view.

### Modified Capabilities
- `log-search`: Search không ẩn dòng ở view nhưng lấp đầy Focus Tab; Search Expression thuộc về Focus Tab đang mở và được nhớ; Match trong Parsed View là dòng qua Filter.
- `log-filter`: Filter thu hẹp Focus Pane thay vì ẩn dòng ở Parsed View; ghi nhớ theo view nay gồm cả Focus Tab.

## Impact

- Phụ thuộc change `add-text-markers-and-highlight-rules` (Marker, Level Marker, Highlight Rule, chọn và chép text). Change đó nên được archive trước khi áp dụng change này.
- Code: `src/app/filter.rs`, `src/app/search.rs`, `src/app/raw.rs`, `src/app/selection.rs`, `src/app/text_select.rs`, `src/app/view/mod.rs` (bố cục), `src/ui/canvas.rs`, `src/ui/inspector.rs`, `src/ui/context_sidebar.rs`, `src/workspace/session.rs` / `state.rs` (lưu Focus Tab), module view mới cho Focus Pane.
- Dữ liệu lưu: thêm Focus Tab vào trạng thái theo view; trạng thái cũ không có Focus Tab thì mở với một tab trống.
- Docs: `CONTEXT.md` và ADR-0008 đã cập nhật.
