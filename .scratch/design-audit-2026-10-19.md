# Design audit — log_gpui (2026-10-19)

Audit theo Checklist Design, từng nhóm màn hình. Màn chính có ảnh chụp (session mẫu demo.log, 400 dòng); các dialog/panel audit từ source.

## Bảng log chính (Parsed view)

Mình xem ảnh chụp cửa sổ chính (w1.png, demo.log 400 dòng) và đọc source của canvas, cột, status bar, toolbar, menu chuột phải.

Checklist **Data Table (Web app)**: https://www.checklist.design/web-app/data-table
Checklist này viết cho bảng dữ liệu kiểu CRUD trên web. Parsed View là trình xem log chỉ đọc: thứ tự dòng chính là dữ liệu, và Search hay Filter chỉ thu hẹp Focus Pane chứ không đụng tới bảng (CONTEXT.md:97-98). Vì vậy mấy dòng ⚪ bên dưới là chủ ý thiết kế, không phải thiếu sót.

| | Item | Why |
|---|---|---|
| ⚪ | **Sortable columns** — Column headers that sort rows by that value on click, toggling ascending and descending | Header không sort được (canvas.rs:482-560 chỉ có pin và hover), và vậy là hợp lý: log phải đọc theo thứ tự dòng, cột `#` trong Gutter đã là thứ tự đó. Ai cần gom theo Level thì dùng Filter. |
| 🟡 | **Column visibility and order** — Controls to show or hide individual columns and drag to reorder them | Chỉ ẩn/hiện được Source Column qua Quick Settings (column.rs:173-176, spaces.rs:601), không kéo để đổi thứ tự cột được. Ảnh chụp cho thấy cột MODULE trống hoàn toàn với demo.log mà vẫn chiếm chỗ, người dùng không tắt nó đi được. Phần tốt là layout cột được lưu theo session (working_set.rs:46). |
| ⚪ | **Row selection and bulk actions** — Checkboxes on each row and a persistent action bar appearing when rows are selected | Dòng log chỉ đọc, không có gì để sửa hay xoá hàng loạt. Thay cho checkbox, app dùng text selection kéo qua nhiều dòng, rồi Mark with color hoặc ⌘C (marking.rs:14-19, text_select.rs:182). |
| ⚪ | **Row actions on hover** — Contextual actions (edit, delete, view) appearing when hovering over a row | App desktop nên dùng menu chuột phải (cell_menu.rs:8-50: Mark with color, Change/Remove marker, Highlight rows where…). Hàng log dày đặc mà hiện thêm icon khi hover thì càng khó đọc. |
| 🟢 | **Search and filter** — A search input for quick lookup alongside filter controls for narrowing by specific attributes. | Ô Search nằm ngay trên bảng, có placeholder ví dụ cú pháp và bộ đếm 0/0 kèm mũi tên prev/next (toolbar.rs:60-110, ảnh chụp). Filter theo giá trị cột nằm ở sidebar (filter.rs). Cả hai thu hẹp Focus Pane chứ không đụng bảng, đúng như định nghĩa domain. |
| ⚪ | **Pagination** — Controls to navigate between pages of results, with an option to choose how many rows show per page | Bảng dùng `uniform_list` ảo hoá và cuộn liên tục, cộng thêm minimap để nhảy nhanh (canvas.rs:233-250, 395-420). Chia trang sẽ làm đứt mạch đọc log. Tổng số dòng đã có ở status bar. |
| 🟢 | **Frozen columns** — The first column pinned so it remains visible when the user scrolls horizontally | Gutter (số dòng + level marker) luôn neo cố định (column.rs:224-227). Timestamp, Level, Module, Source thì pin được bằng icon ghim hiện khi hover header (canvas.rs:143-147, column.rs:86-95), và có đường chia khi đã cuộn ngang. |
| 🟡 | **Export action** — A way to download the visible or selected rows as CSV, spreadsheet, or another format | Mới có ⌘C để copy raw line của vùng chọn (text_select.rs:191, focus.rs:529). Không có lệnh xuất file, và menu chuột phải cũng không có mục Copy (cell_menu.rs). Muốn lấy 200 dòng ERROR mà Focus Pane đã lọc ra thì phải kéo chọn rồi dán tay. |
| 🟡 | **Empty and loading states** — The states shown when the table has no rows or when data is being fetched | Empty state có, kèm lời nhắn và nút hành động: "No Log Space open." / "This Log Space has no files." + Add files (raw_view.rs:194-219). Trạng thái loading thì mình không thấy: canvas không có nhánh nào cho lúc đang parse hoặc mở file. Riêng archive có notice "Reading archive…" (spaces.rs:310). |

**Ngoài checklist:**
- Status bar hiện **"0,400 events"** cho 400 dòng (ảnh chụp). Lỗi nằm ở `format!("{},{:03} events", total_count / 1000, total_count % 1000)` (status_bar.rs:38): dưới 1000 dòng nó luôn hiện "0,". Còn từ 1.000.000 dòng trở lên thì phần nghìn không có dấu phân cách (ví dụ "1200,000"). Dùng một hàm format số có dấu phân cách hàng nghìn thật sự là xong.
- Header cột Module hiện thành **"MOD…"** trong ảnh chụp. Trong khi đó comment ở column.rs:57 và 145 nói độ rộng được tính để "column headers never truncate". Nhiều khả năng khi cột trống thì độ rộng mặc định 100px hoặc min width bị thắng, nên tên cột bị cắt mất. Người dùng phải đoán đây là cột gì.

## Tìm kiếm, Match Navigator và Command Palette

Mình đã đọc source trong `src/app/search.rs`, `src/search.rs`, `src/app/view/toolbar.rs`, `src/ui/focus_pane.rs`, `src/app/view/focus_pane.rs`, `src/ui/command_palette.rs` và `src/app/commands.rs`, và xem thêm ảnh chụp cửa sổ chính (demo.log, 400 dòng).

Audit theo checklist **Searchbar (Design system)**: https://www.checklist.design/design-system/searchbar

| | Item | Why |
|---|---|---|
| 🟢 | **Input field** — A clear container for a user to start typing in | Ô tìm kiếm chạy gần hết chiều ngang toolbar, cao 32px, có viền, bo góc và icon kính lúp (`toolbar.rs:47-62`). Trên ảnh chụp ô này nổi rõ ngay đầu view. |
| 🟢 | **Label or placeholder text** — Identify the purpose of the field is for them to search | Placeholder là "Search lines…  e.g. timeout && (db \|\| cache)" (`app/mod.rs:215`). Vừa nói ô dùng để làm gì, vừa đưa luôn ví dụ cú pháp `&&`/`\|\|`, đúng như tip của checklist. |
| ⚪ | **Quick links, autocomplete and suggestions** — As the user is typing, offer available links and phrases based on what they have entered so far | Search chạy lại theo từng phím gõ (`InputEvent::Change` → `run_search`, `app/mod.rs:86-96`). Bộ đếm, highlight và Focus Pane cập nhật ngay, nên chính kết quả đã là phản hồi. Log là văn bản tự do, không có bộ từ vựng cố định để gợi ý. |
| ⚪ | **Submit search button** — A visible link to submit search and view results | Không cần nút submit vì search chạy trực tiếp khi gõ. Enter và Shift+Enter dùng để nhảy tới Match sau hoặc trước (`app/mod.rs:297-304`). Mọi thứ chạy local, không có lỗi kết nối nên cũng không cần trạng thái loading. |
| 🟡 | **Previous searches** — Showing what a user has searched before can speed up their experience if they frequently search the same queries | Không có lịch sử search. Mỗi Focus Tab giữ Search Expression riêng và được lưu cùng session (`search.rs:6-8`), nên mở nhiều tab đã phần nào thay cho "search đã lưu". Danh sách query gần đây thì không có. |
| 🟡 | **Appropriate visibility** — Search should be directly linked to what you are looking for, whether it's searching across the entire platform or in a specific area | Ô search nằm ngay trên view đang mở, nên phạm vi "dòng của view này" là rõ. Tuy vậy, nó luôn sửa Search Expression của Focus Tab đang mở, mà ô search không hiện tab nào. Trên ảnh chụp không thấy Focus Pane, nên người dùng không biết mình đang sửa query của tab số mấy. |

Audit theo checklist **Search Results (Web app)**: https://www.checklist.design/web-app/search-results

| | Item | Why |
|---|---|---|
| 🟢 | **Search input** — A search field at the top of the results, pre-filled with the current query so it can be refined without starting over | Ô search luôn nằm phía trên view và Focus Pane. Query được ghi lại vào ô khi đổi tab hoặc khôi phục session (`sync_search_input`, `app/search.rs:32-41`), nên sửa tiếp được mà không phải gõ lại từ đầu. |
| 🟡 | **Result count** — How many results were found for the query | Có bộ đếm "cur/total" cạnh nút prev/next (`toolbar.rs:123-127`) và "Focus N of M" ở status bar. Nhưng ô trống và một query không có Match đều hiện "0/0" (`search.rs:43-50`). Thấy "0/0" thì người dùng không phân biệt được search đã chạy hay chưa, đúng chuyện mà tip của checklist cảnh báo. |
| 🟢 | **Result items** — Each result shown with enough to identify it — title, type, image, and a snippet of the matching content | Mỗi dòng trong Focus Pane có số dòng, ký hiệu level (ở Parsed View), tên Source File khi Log Space có nhiều file, và raw line có highlight đúng đoạn khớp (`search_ranges`, `ui/focus_pane.rs:480-555`). |
| 🟢 | **Result type indicators** — A label or icon marking what kind of item each result is (document, person, project, message) | Kết quả nào cũng là một dòng log, nên "loại" ở đây là level: ký hiệu level có màu (`ui/focus_pane.rs:491`, `:524-535`), cộng thêm cột Source File khi có nhiều file. |
| 🟢 | **Filters** — The ability to narrow results by category, date, status, or other relevant values | Filter thu hẹp Focus Pane theo giá trị cột, kể cả khoảng thời gian From/To (`app/filter.rs:27-29`). Raw View chỉ áp Search Expression, nhưng đó là chủ ý vì Raw View không có cột. |
| 🔴 | **No results state** — The state shown when a query returns no matches, ideally with suggestions for what to try instead | Gợi ý trong Focus Pane chỉ hiện khi chưa có query (`view/focus_pane.rs:75-77`). Có query mà không có Match thì `hint` là `None` và pane trống trơn (`ui/focus_pane.rs:291`). Dòng chữ "No matches" có trong `match_navigator.rs:31`, nhưng component đó không được render ở đâu. Gõ sai một chữ, người dùng chỉ thấy một vùng trống, không biết là không khớp hay app bị lỗi. |
| 🟡 | **Recent searches** — A list of the user's previous queries, shown when the search field is focused but empty | Focus vào ô search khi ô đang trống thì không hiện gì. Chỉ các Focus Tab đang mở là còn giữ query cũ, và phải mở pane, chuyển tab mới thấy. |

**Ngoài checklist:**
- Command Palette không có ô để gõ lọc lệnh: nó chỉ render một danh sách 21 lệnh cố định để click (`ui/command_palette.rs:178-211`). Lệnh càng nhiều thì càng mất thời gian dò. Đây lại chính là nơi người dùng quen gõ vài chữ để chạy lệnh.
- Lệnh palette "Search: Find in Log ⌘F" gọi `toggle_search` (`commands.rs:12`). Hàm này không focus ô search mà còn bật panel Filter (`app/search.rs:11-18`), còn `activate_search` thì có focus. Kết quả là người dùng chọn "Find" rồi vẫn phải click vào ô mới gõ được. Ngoài ra hai nút prev/next ở toolbar chỉ có icon, không có tooltip hay phím tắt (`toolbar.rs:106-122`).

## Filter Panel và Highlights Panel

Tôi đánh giá từ source, không có screenshot. Highlights Panel tô màu dòng chứ không lọc dòng, nên bảng dưới chỉ chấm Filter Panel. Phần Highlights nằm ở mục "Ngoài checklist".

Audit theo checklist Filtering items (Flows): https://www.checklist.design/flows/filtering-items

Đây là app desktop, không phải web. Ở đây "collection" chính là Focus Pane: Filter chỉ thu hẹp Focus Pane trong Parsed View, còn view chính vẫn giữ đủ mọi dòng (`src/app/filter.rs:68-69`, CONTEXT.md mục **Filter**).

| | Item | Why |
|---|---|---|
| 🟡 | **Show action near item collection** — Place above or beside the collection it affects, using a recognizable icons and/or label. | Có nút "+ Add filter" ghi rõ chữ (`filter_panel.rs:306`), nhưng nút nằm ở context sidebar bên phải, còn kết quả hiện ở Focus Pane full-width phía dưới. Trong Focus Pane chỉ có câu hint "…or pick a Filter" (`app/view/focus_pane.rs:75-76`) trỏ ngược lại, và hint này biến mất ngay khi có filter. Nút lại là `ghost().xsmall()`, nên có dễ thấy hay không thì phải có screenshot mới nói được. |
| 🟡 | **Show available filter options** — When the action is triggered, filter options can either be shown on the same page for immediate feedback, or on another page. This should be dependent on the amount of filtering that is possible, and whether you think the user is likelier to tweak filters ongoing, or apply several and then view results. It's also worthwhile considering a filtering priority order, with the most common options filtered sitting at the front. | Option hiện ngay trong sidebar và mỗi lần tick là lọc lại liền (`filter.rs:139-142`), nên đã hợp với kiểu chỉnh liên tục. Cái thiếu là thứ tự ưu tiên: value xếp theo thứ tự xuất hiện đầu tiên trong file, không theo count (`filter_index.rs:218-228`). Một cột có tới 500 checkbox (`MAX_DISTINCT`) mà không có ô tìm trong danh sách value thì value hay dùng có thể nằm tận cuối. |
| 🟢 | **Consider different filter types** — The standard filter is a multi-select option picker. But certain properties can benefit from a different way of managing the filtering. There's sliders, checkboxes, dropdowns and others to consider. Each property should be considered, asking yourself what feels like the easiest way to change this value. | Có đủ loại: checkbox multi-select cho value (`filter_panel.rs:98`), Custom Value "contains" gõ tay, công tắc Hide Mode "Hide the chosen values" (`:209`), Time Range From/To gõ được mọi dạng thời gian và báo "! invalid time" khi gõ sai (`:279-283`). Cột quá 500 value thì bị khoá và có ghi lý do. |
| 🟢 | **Show active filters clearly when applied** — On the item collection page, display which filters are currently applied. You can also choose to have a high level active state applied to the filter action to imply it is in use, if you find showing all applied filters is too cluttered. | Mỗi cột đang lọc có card riêng, kèm tag số value đã chọn và tag "hide" (`filter_panel.rs:172-173`). Header section Filter có badge đếm số điều kiện đang bật, nên thu gọn section vẫn thấy (`ui/context_sidebar.rs:182`, `filter.rs:124`). Chỉ hơi thiếu ở chỗ card thu gọn chỉ hiện con số, không hiện value nào đang được chọn. |
| 🟢 | **Provide easy filter removal** — Let users clear individual filters or all filters at once to allow easier refining of their results. | Gỡ được ở mọi mức: nút X trên từng card (`filter_panel.rs:186-193`), X trên từng chip Custom Value (`:248`), bỏ tick từng value, và "Clear all" chỉ hiện khi có filter (`:315`). Time Range không có nút xoá riêng, phải xoá chữ hoặc dùng Clear all. |
| 🟢 | **Show result count** — Not a must have, but this can be handy where it is meaningful for users to see what the total results value has reduced to. It will help users feel if their result is too narrow or broad, directing them to either add or remove filters. | Status bar hiện "Focus N of total" (`app/view/status_bar.rs:35`). Mỗi checkbox value cũng kèm count, ví dụ "ERROR  (42)" (`filter_panel.rs:108`). |
| 🔴 | **Empty state** — It's possible that a combination of filters can product no results. Explain this clearly and suggest adjusting or clearing filters to see results. | Khi có filter mà không dòng nào khớp thì `hint` là `None` (`app/view/focus_pane.rs:74-76`), và Focus Pane render một `div` trống (`ui/focus_pane.rs:280-292`). Người dùng không biết pane đang lỗi, đang load hay thật sự 0 kết quả, cũng không có gợi ý nới hoặc xoá filter. |

**Ngoài checklist:**
- Form "New rule" của Highlights có danh sách value đặt `max_h(px(160.0)).overflow_y_hidden()` (`highlights_panel.rs:171`). Value nào vượt quá 160px bị cắt mất, không cuộn tới được, nên cột nhiều value thì không chọn được phần dưới. Nên đổi sang cuộn được, giống danh sách bên Filter.
- Tag số trên mỗi rule Highlights (`highlights_panel.rs:119`) chỉ là một con số trơn, không ghi "matches" và không có tooltip. Nút X xoá rule (`:131`) và X xoá card filter (`filter_panel.rs:189`) cũng chỉ có icon, không có tooltip, trong khi các nút ở Focus Pane đều có tooltip.

## Settings

Mình đọc source của `src/ui/settings.rs` và `src/app/view_settings.rs` vì không có screenshot.

Đối chiếu với checklist Settings (Web app): https://www.checklist.design/web-app/settings

Đây là app desktop chạy local, không có tài khoản, không thu phí và không có backend người dùng. Vì vậy các mục ⚪ bên dưới không phải lỗi, chỉ là không áp dụng cho loại sản phẩm này.

| | Item | Why |
|---|---|---|
| 🟢 | **Structure** — Organising settings controls into logical categories e.g. account, notifications, security, billing. _Tip: Consider elevating the most commonly changed settings rather than the most important_ | Có 4 nhóm General / Appearance / Raw View / Keymap ở thanh trái (`settings.rs:45-68`). Ô search lọc qua mọi nhóm theo cả tên lẫn mô tả, và báo "No settings match." khi không có kết quả (`settings.rs:117-159`). Riêng nhóm General chỉ có Live Window, một setting dành cho Remote File (`settings.rs:231-233`), nên cái tên "General" hơi rộng so với nội dung. |
| ⚪ | **Account details** — The fields where users update their name, email address, and profile photo | App không có khái niệm tài khoản người dùng, nên không có tên, email hay ảnh để sửa. |
| ⚪ | **Security details** — The ability to change the password, two-factor authentication and other security information. _Tip: These fields should require re-authentication to save changes_ | App không có mật khẩu của chính nó. Mật khẩu SSH nằm trong Keychain (`remote/secrets.rs:1-3`) và được quản lý ở dialog Server, gồm cả xoá server và quên host key (`app/server_dialog.rs:280-289`). Đó là đúng chỗ, không cần chuyển vào Settings. |
| ⚪ | **Notification preferences** — Controls for which notifications the user receives and through which channel, grouped by type (product updates, reminders, billing) | App không gửi thông báo qua kênh nào (email, push…), nên không có gì để chọn kênh. |
| ⚪ | **Billing** — Managing payment method, upgrading or cancelling a payment — this could also be a preview of this information with a link directly to the billing page if separate | Đây là công cụ local, không có gói trả phí. |
| 🟡 | **Additional preferences (if applicable)** — Language, timezone, date format, and appearance settings like dark mode | Appearance đã có row density, minimap, cột source và màu match (`settings.rs:250-312`). Còn thiếu lựa chọn sáng/tối: theme bị code cứng là `ThemeMode::Dark` (`main.rs:11`, `main.rs:25`). Ai cần nền sáng, ví dụ khi đọc log dưới nắng hoặc khi share màn hình, hiện không có cách nào đổi. |
| ⚪ | **Danger zone** — Destructive actions like account deletion, clearly separated from the rest of settings. _Tip: Include a confirmation step for any destructive actions, with details on what will be lost if the user continues_ | Không có tài khoản để xoá. Mọi setting đều đổi qua lại được, và không có hành động phá huỷ nào nằm trong Settings. |

**Ngoài checklist:**
- Các ô màu trong "Search match color" và "Focus Find match color" chỉ là `div` tô màu, không có tên hay tooltip (`settings.rs:373-385`). Ô đang chọn chỉ khác ở màu viền, `foreground` thay cho `border` (`settings.rs:380`). Với ô 20px thì người mù màu khó biết mình đang chọn hue nào, và trạng thái đang chọn có thể khó thấy. Cái này cần screenshot mới chắc được.
- "Show source file" bị disable khi tab hiện tại không phải Log Space (`settings.rs:294`). Mô tả có ghi "Applies to a Log Space", nhưng không nói vì sao switch lúc này bị khoá. Vì thế người mở Settings từ một File View sẽ thấy một switch xám mà không rõ phải làm gì để bật nó.

## Dialogs (New Space, Confirm, Notice, Host key, Secret, Servers, Server form)

Mình chỉ đọc source, chưa có screenshot. Phần lớn item ở ba checklist này hỏi dialog có gì và chạy ra sao, nên đọc code là trả lời được. Notice là thanh báo phía trên status bar chứ không phải modal, nên ở checklist Modal chỉ được nhắc khi cần.

Đối chiếu với checklist Modal (Design system): https://www.checklist.design/design-system/modal

| | Item | Why |
|---|---|---|
| 🟢 | **Title** — Clear, simple text explaining the action of the modal | Dialog nào cũng có tiêu đề nói rõ đang làm gì: "New Log Space", "Delete “name”?", "Trust {server}?", "Password for {name}", "Servers" (dialogs.rs:139, 231; remote_dialogs.rs:58; server_views.rs:132). |
| 🟢 | **Actionable item** — A button or link to continue or close the event | Nút chính nói đúng việc sẽ xảy ra. Ví dụ "Create with 3 files" / "Create remote with 1 file" (dialogs.rs:47–53), "Delete" màu danger, "Trust", "Connect", "Save". Không có nút "OK" chung chung. |
| 🟡 | **Close action** — A way to exit the modal | Dialog nào cũng có nút Cancel hoặc Done. Nhưng Escape chỉ đóng được New Space và Confirm (keyboard.rs:17–30). Host key, Secret và Servers không có Escape, và bấm ra nền cũng không đóng dialog nào (`occlude()`, dialogs.rs:25). |
| 🟡 | **Responsiveness** — Consider the size of the modal on different device sizes, and whether a modal is suitable on all | Mobile không áp dụng, nhưng cửa sổ desktop kéo nhỏ được. Các dialog dùng chiều rộng cố định 400–560px, không có `max_w` hay scroll cho cả khung (dialogs.rs:131, server_views.rs:124). Server form khi mở thêm Jump Server khá cao, nên trên cửa sổ hẹp hoặc thấp sẽ bị cắt. |
| 🟢 | **Background change behind modal** — Darken, blur or lighten - change the background behind the modal to bring focus to it | Có lớp nền đen mờ `rgba(0x00000088)` phủ toàn màn hình, dùng chung cho mọi modal (dialogs.rs:20–30). |
| 🟢 | **Description** — Incase they require more information to understand how to make their decision | Những chỗ cần giải thích đều có. Confirm nói rõ "Your log files on disk are not touched." hay scratch file sẽ bị xoá (dialogs.rs:232–236). Host key ghi rõ địa chỉ kèm fingerprint (remote_dialogs.rs:60–67). |

Đối chiếu với checklist Submitting a form (Flows): https://www.checklist.design/flows/submitting-a-form

| | Item | Why |
|---|---|---|
| 🟢 | **Show button to submit** — Below the form fields, a button to submit the information needs to be present. You can change the copy to fit the form e.g. the button can say “Subscribe” if someone is providing their email address to receive emails. | Nút chính nằm dưới cùng bên phải, chữ khớp với việc của từng form: "Create with N files", "Save", "Connect" (dialogs.rs:208, server_views.rs:238, remote_dialogs.rs:136). |
| 🟡 | **Show loading state after submission** — The user must see the form is in the process of being submitted. Note: also make sure your hover state is considered before they press the button! | New Space làm tốt: file đang đọc hiện "Reading {name}…" và nút Create bị khoá cho tới khi đọc xong (new_space.rs:73, 305). Còn Connect ở Secret thì đóng dialog ngay sau khi bấm (remote.rs:143), trong dialog không có trạng thái đang kết nối. Hover state thì phải có screenshot mới biết. |
| 🟡 | **Show success message when it submits** — The form was submitted! Let’s communicate that back to the user with a clear success message. | Thành công chỉ thấy gián tiếp: Server mới hiện trong list, Log Space mới được mở ra. Riêng Secret thì sau khi đóng dialog không có câu nào xác nhận đã kết nối được (remote.rs:150–153). |
| 🟡 | **If it doesn't, show an error message** — Sometimes, things don’t work out. If the form can’t submit, because of invalid information or another error, that also needs to be shown. | Server form báo lỗi ngay trong dialog (server_dialog.rs:256–276). Nhưng `SecretPrompt.error` luôn được gán `None` (remote.rs:123) và không chỗ nào set lại. Vì vậy chỗ hiển thị lỗi ở remote_dialogs.rs:119 không bao giờ hiện, gõ sai mật khẩu sẽ không thấy lỗi nào trong dialog. New Space đóng dialog trước khi tạo (spaces.rs:126, 132), nên nếu tạo thất bại thì lỗi chỉ hiện ở thanh Notice và những gì đã nhập bị mất. |
| 🟡 | **An error may occur because of the wrong information** — If the criteria for a text field isn’t met, the form can fail to submit due to that error, and must be detailed. | Câu báo lỗi cụ thể và nói cần làm gì ("Enter an address", "The Jump Server chain loops back on itself", server.rs:89–96). Có hai chỗ hở. Form chỉ hiện một lỗi mỗi lần, ở cuối form chứ không cạnh field sai. Và Port nhập sai không bị báo mà âm thầm thành 22 (server_dialog.rs:201). |

Đối chiếu với checklist Showing input error (Flows): https://www.checklist.design/flows/showing-input-error

| | Item | Why |
|---|---|---|
| 🟡 | **Keep the input in default state** — The text field should be checked for errors only after the information has been entered. | New Space làm đúng: tên để trống không bị coi là lỗi (new_space.rs:295–302). Server form thì ngược lại: vừa mở "Add Server" là đã hiện đỏ "Enter a name", vì lỗi được tính ngay mỗi lần render (server_views.rs:146). |
| 🟡 | **Allow user to enter information** — Let the user type without interruptions and submit the information aka don't assess as changes are made. | Người dùng vẫn gõ bình thường, nhưng cả hai form đều kiểm tra lỗi theo từng ký tự. Server form có lỗi đỏ đổi liên tục khi gõ, New Space báo trùng tên ngay giữa lúc đang gõ (dialogs.rs:45, server_views.rs:146–147). |
| 🟡 | **Signal error after loss of focus** — After the input has lost focus (user has clicked another element), the field should be assessed. Looks like we have an error here! In this case, explain why the error happened, and what is required to resolve it. For accessibility, combine the text with a visual icon that indicates an error was found. | Câu lỗi có nói cần làm gì. Nhưng không có xử lý theo blur, chỉ có chữ đỏ `text_xs` mà không có icon (dialogs.rs:148, server_views.rs:227), và chính field cũng không chuyển sang trạng thái lỗi (viền đỏ). Riêng Server form để lỗi ở cuối form, nên không biết field nào đang sai. |
| 🟡 | **Return to default state upon reattempt** — Once the user focuses on the field again, the error message should disappear. If there is an error again, the process will simply repeat until they are able to continue. | Lỗi chỉ biến mất khi giá trị đã hợp lệ, không phải khi focus lại vào field. Lỗi từ lần Save trước (`dialog.error`) được ưu tiên hiện thay cho lỗi đang có, và chỉ bị xoá khi Cancel hoặc mở form mới (server_dialog.rs:67, 82–84; server_views.rs:146). Nên sửa xong field rồi mà câu lỗi cũ vẫn còn đó. |

**Ngoài checklist:**
- Nút "Delete" trong list Servers xoá ngay, không hỏi lại (server_views.rs:90–94 → server_dialog.rs:280). Trong khi đó xoá Log Space hay scratch file thì có Confirm. Nên dùng chung Confirm cho việc xoá Server.
- "Forget host key" là thao tác thành công, nhưng câu báo được đưa vào `dialog.error` (server_dialog.rs:298) nên hiện bằng màu danger (server_views.rs:101). Người dùng sẽ tưởng là có lỗi.

## Empty state, Tab bar và Sidebar/Project switcher

Mình đọc source (`raw_view.rs`, `tab_bar.rs`, `tabs.rs`, `sidebar.rs`, `space_menu.rs`, `cell_menu.rs`, `project_switcher.rs`, `view/mod.rs`, `dialogs.rs`, `keyboard.rs`) và screenshot `w1.png`. Screenshot chỉ có một tab "Live" đang mở với demo.log, không menu nào đang mở.

Audit theo checklist **Empty State (Web app)**: https://www.checklist.design/web-app/empty-state

| | Item | Why |
|---|---|---|
| 🔴 | **Illustration or icon** — A visual that signals the empty state and gives the screen some personality, rather than feeling broken | `render_empty_state` (raw_view.rs:212-219) chỉ có một dòng chữ muted và một nút, không có icon. Vùng giữa cửa sổ to và trống nên nhìn dễ tưởng app bị lỗi. Nên thêm một icon riêng cho từng trường hợp, ví dụ `Layers` cho Log Space trống và `FolderOpen` cho lúc chưa mở gì. |
| 🟡 | **Clear heading** — A short, plain-language title naming what's missing | Câu "No Log Space open." / "This Log Space has no files." (raw_view.rs:196, 204) nói đúng cái đang thiếu. Có điều đây chỉ là text muted cỡ thường, không có tiêu đề nổi hơn. |
| 🔴 | **Supporting description** — A brief explanation of what belongs in this space, most useful for first-time users | Không có câu nào giải thích Log Space là gì hay nên cho file nào vào. Người mới mở app sẽ không hiểu "Log Space" khác gì một thư mục hay một file log. |
| 🟢 | **Primary action** — A CTA pointing toward the next step: creating, importing, connecting etc | Nút primary đổi theo ngữ cảnh: "Add files" mở thẳng dialog thêm file vào đúng space đó, còn "New Log Space" mở dialog tạo space (raw_view.rs:197-209). Cả hai tạo ra item đầu tiên chứ không dẫn sang chỗ chung chung. |
| 🔴 | **Zero state vs. no-results state** — A distinction between a screen that is empty because nothing has been created versus one that returned no search or filter results | Khi Filter lọc ra 0 dòng, `LogCanvas::render` (canvas.rs:100) vẫn vẽ header với vùng trống, không có nhánh nào cho `visible_indices` rỗng. Người dùng thấy bảng trắng và không có nút xoá hay nới Filter ngay tại chỗ. Bộ đếm "0/0" trên screenshot chỉ là số match của Search, không giúp gì cho trường hợp này. |
| 🟡 | **Error state variant** — A separate variant for when content failed to load, as opposed to genuinely being empty | Khi đọc file lỗi thì có thông báo riêng "… was not found" / "Could not read …" (tabs.rs:24-36), hiện ở notice banner phía dưới (dialogs.rs:284). Nhưng vùng giữa không có biến thể lỗi nào, nên dễ bị đọc nhầm thành log rỗng. |

Audit theo checklist **Tabs (Design system)**: https://www.checklist.design/design-system/tabs

| | Item | Why |
|---|---|---|
| 🟢 | **Labels** — Name of each tab | Tab Log Space mang tên space, tab File View mang dạng "file · space" (tab_bar.rs:21-31), còn Investigation Board có thêm icon Flag. Đủ để đoán trong tab có gì. Label "file · space" hơi dài, nhưng tên file là thứ cần thấy nên chấp nhận được. |
| 🟢 | **Content area** — Where the content for the active tab is displayed | Nội dung nằm ngay dưới strip. Tab active lấy cùng màu nền với main view nên trông như nối liền với nội dung bên dưới (tab_bar.rs:53-55, screenshot). |
| 🟡 | **Style** — How the active tab and inactive tabs differentiate visually, as well as the tab container overall | Tab active dùng màu main view, tab inactive tối hơn (`l *= 0.55`) và chữ muted (tab_bar.rs:47-48, 69-72). Cách làm hợp lý, nhưng screenshot chỉ có một tab nên chưa kiểm được độ tương phản giữa active và inactive trên nền tối. Code cũng chưa bỏ border dưới của tab active như comment ở dòng 53 nói (border_b_1 nằm trên container). |
| ⚪ | **Item order** — Consider the arrangement of tabs to be ordered by popularity or familiarity | Đây là working set kiểu editor: tab mới được thêm vào cuối theo thứ tự mở (working_set.rs:157). Thứ tự này mới là cái người dùng quen, nên xếp theo độ phổ biến không có ý nghĩa ở đây. |
| 🟡 | **States** — Default, active, hover are the key states | Đủ default và active. Hover chỉ đổi màu chữ từ muted sang foreground mà không đổi nền (tab_bar.rs:71), trên nền tối sẽ khá khó thấy. Nút đóng chỉ hiện khi hover tab inactive (dòng 84-85), cái đó ổn. |

Audit theo checklist **Dropdown Menu (Design system)**: https://www.checklist.design/design-system/dropdown-menu. Các menu được audit là: menu "…" của Project, menu chuột phải của Log Space và file ở sidebar, và context menu của ô trong Parsed View.

> ⚠️ Ba trong tám mục này là về cách menu trông ra sao (anatomy, destructive styling, positioning), mà screenshot không có menu nào đang mở. Mình thấy được code nhưng không biết trên màn hình nó hiện thế nào. Một screenshot lúc mở menu chuột phải của Log Space sẽ trả lời được.

| | Item | Why |
|---|---|---|
| 🟢 | **Trigger affordance** — The element that opens the menu: an overflow icon, chevron button, or right-click target | Project có nút ghost `Ellipsis` hiện khi hover (sidebar.rs:174-178), còn row Log Space và row file mở menu bằng chuột phải (space_menu.rs:296, 344). Đây là kiểu quen thuộc của sidebar trên desktop. |
| 🟢 | **Menu item anatomy** — The structure of each action: label, optional leading icon, optional trailing shortcut, optional destructive styling | Mọi item đều chỉ có label, không menu nào trộn item có icon với item không icon. Item nào mở dialog thì có "…" ở cuối ("Add files…", "Save As…"), và "Sort files by" dùng dấu check (space_menu.rs:151-166). |
| 🟢 | **Section dividers and groups** — Dividers and/or labels that break related actions into sections once the menu gets large | Có separator trước Delete (space_menu.rs:174), trước "Remove from Sidebar" (sidebar.rs:199) và trước nhóm highlight rule trong cell menu (cell_menu.rs:42). |
| 🟡 | **Destructive item styling** — Visual differentiation — typically the danger colour — for actions that cannot be undone | Delete được đặt cuối menu, sau divider (space_menu.rs:174-178, 229-238), đúng vị trí. Nhưng nó vẫn là `PopupMenuItem::new` thường, không có màu danger, nên nhìn không khác gì "Rename" ngay bên trên. |
| 🟢 | **Nested submenu** — A secondary menu triggered by a parent item, used for grouping related but distinct actions | "Sort files by" là submenu một cấp, có hai lựa chọn Added/Name (space_menu.rs:151). Đúng mức nên dùng. |
| 🟡 | **Disabled items** — Menu items that are visible but cannot be triggered, indicating an unavailable action | Có dùng trạng thái disabled: "Mark with color" khi chưa chọn text, "Reveal…" khi không có path, Delete với scratch file cuối cùng (cell_menu.rs:20, space_menu.rs:194, 231). Nhưng không có tooltip nói lý do. Ngược lại, "Remove from Sidebar" khi chỉ còn một project vẫn bật mà bấm vào thì không làm gì (projects.rs:79-80). |
| ❔ | **Positioning and overflow** — The menu's placement relative to its trigger, repositioning automatically to stay within the viewport | Phần này do `PopupMenu` của gpui_component lo chứ code app không quyết định. Cần screenshot lúc mở menu sát mép cửa sổ mới biết. |
| ⚪ | **Keyboard shortcut display** — Shortcut hints aligned to the trailing edge of the item label | Không action nào trong các menu này có phím tắt trong `keyboard.rs` (Rename, Delete, Add files…, Remove…), nên không có gì để hiện. |

Còn một mục ❔ (Positioning and overflow). Một screenshot lúc mở menu chuột phải của Log Space ở gần mép dưới cửa sổ sẽ trả lời được.

**Ngoài checklist:**
- Project Switcher (Cmd+P) đặt nút "×" gỡ project sát ngay icon bút chì đổi tên (project_switcher.rs:98-119). `remove_project_at` chạy luôn, không hỏi xác nhận và không có màu danger, nên bấm trượt một chút là mất project khỏi danh sách.
- Phần lớn Project Switcher không đặt màu: viền, nền panel và chữ "ACTIVE" đều không có token theme (project_switcher.rs:71-79, 155-162), nên khả năng cao trông khác hẳn sidebar và các dialog dùng `cx.theme()`. Overlay cũng hardcode `rgba(0x00000088)`.

## Remote file picker, Download và Live status

**Loading (Design system)**: https://www.checklist.design/design-system/loading

⚠️ Hai trong năm mục ở đây (Accessibility, Visuals) là về hình thức. Ảnh chụp chỉ có màn hình chính ở trạng thái "Live". Picker, hộp Download và các trạng thái Connecting/Reconnecting thì mình chỉ đọc được code, chưa thấy chúng hiện ra trên màn hình thế nào.

| | Item | Why |
|---|---|---|
| 🟡 | **Visual indicator** — A clear representation that content is loading or in progress of change | Chỗ nào cũng chỉ báo bằng chữ: picker hiện dòng "Loading…" (`server_views.rs:333-335`), status bar hiện label "Connecting"/"Reconnecting" (`status_bar.rs:47`). Download có ghi chú "so the bar has a total" (`remote.rs:420`) nhưng màn hình chỉ có dòng "X of Y" (`server_views.rs:491-495`), không vẽ thanh tiến trình nào. Thêm progress bar cho Download và một icon/spinner cạnh label kết nối là đủ. |
| 🟡 | **Text** — Explaining the loading state | Download nói rất rõ: "Downloading 2 of 3: app.log" kèm số byte (`server_views.rs:487-495`). Trạng thái kết nối cũng cụ thể ("Waiting for file", "Waiting to connect"). Picker thì chỉ có chữ "Loading…" chung chung, và dùng cùng chữ đó khi đang kiểm tra path/glob gõ tay (`remote_picker.rs:208`). Nên nói rõ đang làm gì, ví dụ "Listing /var/log…" hoặc "Matching *.log…". |
| 🟢 | **Time** — Determine how long the time between two actions must be to require a loading component | Chỉ báo chỉ hiện khi có việc thật qua mạng (SFTP list, resolve glob, tải file, kết nối SSH), toàn là những việc có thể mất vài giây. Lúc dữ liệu đã có sẵn thì không có gì chen vào. |
| ❔ | **Accessibility** — Ensure your loading state can be clearly seen | "Loading…" là chữ `text_sm` màu muted trong khung có viền (`server_views.rs:332-334`). Còn label kết nối là nút ghost `xsmall` (`status_bar.rs:47`). Trong ảnh, "Live" đọc được nhưng trông y như nút Pause/Follow bên cạnh. Ảnh này không có trạng thái Connecting/lỗi, nên không biết các trạng thái đó có nổi bật hơn không. |
| ⚪ | **Visuals** — Entertain the user with an illustration during the loading state | Đây là công cụ đọc log cho dev, chờ thường chỉ vài giây và người dùng vẫn thao tác được (picker, log đang chạy). Thêm illustration ở đây chỉ làm rối. |

Ảnh chụp lúc picker đang "Loading…", hộp Download đang tải, và status bar ở trạng thái Connecting hoặc Error là đủ để chốt dòng Accessibility.

**Search Results (Web app)**: https://www.checklist.design/web-app/search-results
Picker không phải trang kết quả tìm kiếm. Nó là một trình duyệt thư mục, và "truy vấn" duy nhất là ô path/glob. Glob khớp được bao nhiêu file thì đưa thẳng hết vào danh sách đã chọn chứ không hiện thành một danh sách kết quả, nên vài dòng dưới đây được đọc theo cách đó.

| | Item | Why |
|---|---|---|
| 🟡 | **Search input** — A search field at the top of the results, pre-filled with the current query so it can be refined without starting over | Ô path/glob có placeholder rõ ràng (`remote_picker.rs:91`) nhưng nằm dưới danh sách, không ở trên. Thêm thành công là ô bị xoá luôn (`remote_picker.rs:234`), nên muốn sửa glob vừa gõ thì phải gõ lại từ đầu. Khi không khớp thì chữ vẫn được giữ lại, chỗ đó ổn. |
| 🟡 | **Result count** — How many results were found for the query | Nút "Add N files" (`server_views.rs:461`) chỉ đếm tổng số file đã chọn dồn lại từ trước. Glob khớp bao nhiêu file thì không hiện ở đâu cả, mà file trùng còn bị bỏ qua không báo (`remote_picker.rs:229`), nên gõ `*.log` xong không biết mình vừa thêm mấy file. |
| 🟢 | **Result items** — Each result shown with enough to identify it — title, type, image, and a snippet of the matching content | Mỗi dòng có tên, dung lượng và giờ sửa (`server_views.rs:370-372`). File đã chọn hiện kèm tên server, dạng `server:path` (`server_views.rs:385`). Với file log thì chừng đó là đủ để nhận ra. |
| 🟡 | **Result type indicators** — A label or icon marking what kind of item each result is (document, person, project, message) | Thư mục chỉ khác file ở dấu "/" cuối tên và việc không có checkbox (`server_views.rs:351`, `366`). Không có icon thư mục/file, nên lướt nhanh dễ nhầm. Thêm icon folder/file là xong. |
| ⚪ | **Filters** — The ability to narrow results by category, date, status, or other relevant values | Glob ở ô path (`*.log`) đã làm đúng việc lọc theo tên, cũng là cách lọc tự nhiên nhất với file log. Lọc theo ngày hay trạng thái không thêm được gì nhiều cho một trình chọn file. |
| 🟡 | **No results state** — The state shown when a query returns no matches, ideally with suggestions for what to try instead | Glob không khớp thì có lỗi đỏ "No file matches …", và khi gõ sai thì có gợi ý ("Enter an absolute path, starting with /", "… is a folder: open it and pick its files"). Nhưng thư mục rỗng thì chỉ là một khung trống cao 240px, không có chữ nào (`server_views.rs:332-375`), nhìn không biết đang trống hay đang lỗi. |
| 🔴 | **Recent searches** — A list of the user's previous queries, shown when the search field is focused but empty | Không lưu path/glob hay thư mục nào đã dùng trước đó. Lần nào mở picker cũng bắt đầu lại từ home (`remote_picker.rs:71`, `107`), nên người hay mở `/var/log/app/*.log` phải đi lại từ đầu mỗi lần. |

**Ngoài checklist:**
- Phần nạp các dòng cũ của Remote File chạy ngầm (L4, `live.rs:426`) mà không có chỉ báo nào. `SourceStatus` không có trạng thái riêng cho việc này nên status bar vẫn ghi "Live", và người dùng cuộn lên sẽ thấy dòng cũ tự xuất hiện mà không hiểu vì sao.
- Lúc picker đang tải thư mục mới, nó không xoá danh sách cũ (`remote_picker.rs:111-117`). Dòng "Loading…" hiện chồng lên trên danh sách của thư mục trước, các dòng cũ vẫn bấm được, và đường dẫn ở thanh trên vẫn là thư mục cũ.