## Context

Thanh màu (`ColorBar`, `ColorTarget` trong `src/app/color.rs`) có ba lối vào trong Raw View và Parsed View:

```
Parsed View      kéo chọn, nhả chuột    → ColorTarget::MarkSelection      (color.rs)
Raw list (đọc)   finish_raw_selection() → ColorTarget::RawMark            (raw_list.rs:234)
Raw editor       mỗi lần render, đọc vùng chọn của editor:                (raw.rs ~720-760)
  (scratch)        vùng chọn khác rỗng       → RawMark          ← bỏ cái này
                   con trỏ trên Marker       → RawRecolor       ← giữ
                   con trỏ trên Text Highlight → RecolorTextHighlight ← giữ
                   không có gì               → đóng thanh (nếu đang mở cho Raw)
```

Lối vào của editor đi theo trạng thái: nó xem vùng chọn mỗi lần render chứ không bắt sự kiện chuột. Vì vậy chọn bằng bàn phím cũng mở thanh. Lệnh của app nằm trong `src/keymap.rs` (`Command`, id, tên, phím mặc định); người dùng đổi phím qua `keymap.json`.

## Goals / Non-Goals

**Goals:**
- Chọn text trong editor scratch không mở thanh màu.
- Vẫn tô được Marker / tạo Text Highlight trong editor scratch, bằng một lệnh có phím tắt.
- Lệnh đó dùng được ở mọi view có vùng chọn, để mở lại thanh lỡ đóng.

**Non-Goals:**
- Không đổi cách tự mở thanh ở Parsed View và Raw list.
- Không thêm menu chuột phải cho Raw View.
- Không đổi việc sửa dòng làm mất Marker.

## Decisions

**1. Bỏ nhánh `RawMark` trong đoạn đồng bộ của editor, giữ các nhánh recolor.**
Đoạn code ở `raw.rs` chỉ còn tìm Marker / Text Highlight dưới con trỏ khi vùng chọn rỗng. Khi vùng chọn khác rỗng, nó coi như "không có gì muốn mở". Nhưng nó không được đóng thanh đã mở bằng lệnh cho đúng vùng chọn đó (xem quyết định 3).
Phương án khác: một setting bật/tắt tự mở. Không chọn vì thêm một công tắc cho một hành vi gần như ai cũng muốn tắt khi sửa.

**2. Lệnh `Command::ColorSelection`, id `marker.color_selection`, tên "Mark: Color Selection…", mặc định `cmd-shift-h`.**
`cmd-shift-h` chưa dùng trong `keymap.rs`. "h" gợi tới highlight; `cmd-shift-m` đã dành cho Highlight Rules. Lệnh tìm vùng chọn của view đang hiện, theo thứ tự: editor scratch đang có focus → Raw list → Parsed View. Nó dựng đúng `ColorTarget` mà lối vào tự động của view đó sẽ dựng (`RawMark` hoặc `MarkSelection`), rồi mở thanh. Vị trí: trong editor dùng chỗ cố định mà editor vốn dùng (giữa, phía trên); ở Raw list và Parsed View dùng vị trí chuột, vì vùng chọn ở đó luôn được kéo bằng chuột.
Phương án khác: Alt+nhả chuột. Không chọn vì không dùng được bằng bàn phím, và Alt trong editor có thể bị dùng cho việc khác.

**3. Thanh mở bằng lệnh sống tới khi vùng chọn đổi.**
Thanh nhớ nó được mở cho vùng chọn nào (đã có trong `pieces` của `RawMark`). Đoạn đồng bộ của editor đóng thanh khi vùng chọn hiện tại khác vùng đó hoặc rỗng, và để yên khi vẫn giống. Escape và bấm ra ngoài vẫn đóng thanh như trước.

**4. Lệnh phải tới được app khi editor có focus.**
`cmd-s` (`SaveScratch`) đã chạy được khi đang gõ trong editor, nên lệnh mới đi cùng đường dispatch đó. Nếu editor nuốt mất `cmd-shift-h` thì phải chặn phím đó trước editor; cần kiểm tra khi làm.

## Risks / Trade-offs

- [Người dùng quen tô ngay trong scratch thấy mất tính năng] → Lệnh có trong Command Palette; thanh vẫn mở khi bấm lên màu có sẵn. Có thể thêm gợi ý phím tắt trong status bar sau, không làm trong change này.
- [`cmd-shift-h` trùng phím của bộ gõ hoặc hệ thống trên một số máy] → Đổi được trong `keymap.json`.
- [Vị trí thanh khi mở bằng bàn phím: không có vị trí chuột] → Editor dùng chỗ cố định như trước; Raw list và Parsed View dùng vị trí chuột lúc bấm phím, có thể xa vùng chọn nếu chuột đã đi chỗ khác.
- [Ở Parsed View, Escape xóa luôn vùng chọn] → Không mở lại thanh sau Escape được; đây là hành vi sẵn có, không đổi trong change này.

## Open Questions

- Có nên hiện một gợi ý nhỏ "⌘⇧H to color" khi chọn trong editor scratch không? Tạm thời không làm.
