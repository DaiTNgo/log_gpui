## Context

`add-focus-pane` đặt Focus Pane dưới cửa sổ chính, với state `FocusTabs` nằm trong `AppView` và hàm đồng bộ `set_current_line(LineRef, origin)`. App hiện chỉ mở một cửa sổ (`src/main.rs`: `Root::new(AppView)`). gpui cho phép mở nhiều cửa sổ trong cùng một `App`, và các `Entity` dùng chung được giữa các cửa sổ. Riêng `InputState` của gpui-component gắn với `Window` nơi nó được tạo.

## Goals / Non-Goals

**Goals:**
- Chuyển Focus Pane qua lại giữa cửa sổ chính và một Focus Window.
- Không nhân đôi state: một `FocusTabs`, một dòng hiện tại.

**Non-Goals:**
- Nhớ Focus Window, vị trí hay kích thước của nó qua lần khởi động.
- Nhiều Focus Window, hoặc Focus Window gắn cố định với một view.
- Filter trong Focus Window.

## Decisions

### 1. State vẫn ở `AppView`, Focus Window chỉ là một view mỏng
Thêm `FocusWindowView { app: Entity<AppView>, search_input, find_input }` làm root của cửa sổ mới (bọc trong `Root`). Nó render bằng cùng hàm render Focus Pane của `add-focus-pane`, với dữ liệu đọc từ `app`. Mọi hành động (bấm dòng, đổi tab, next/prev, Focus Find) gọi `app.update(...)`, nên đồng bộ hai cửa sổ dùng lại đúng `set_current_line`.
- Phương án khác là chuyển `FocusTabs` thành một `Entity` riêng mà hai cửa sổ cùng quan sát. Cách đó sạch hơn về sở hữu nhưng phải viết lại phần đã cài ở `add-focus-pane`; không cần cho một cửa sổ phụ duy nhất.
- Hàm render Focus Pane phải nhận một `Entity<AppView>` cùng dữ liệu đã chuẩn bị sẵn, không dùng `cx.listener` của `AppView`, để chạy được trong `Context<FocusWindowView>`.

### 2. Ô nhập riêng cho mỗi cửa sổ, đồng bộ qua state
`InputState` gắn với cửa sổ, nên Focus Window có `search_input` và `find_input` của riêng nó. Khi `AppView` đổi Search Expression hay Focus Find, nó đánh dấu "dirty". Mỗi cửa sổ, trong `render` của mình, đặt lại giá trị ô nhập khi khác với state, giống `sync_search_input` hiện tại. Gõ ở ô nào cũng ghi vào `FocusTab` của `AppView`.

### 3. Vòng đời
- `AppView.focus_window: Option<WindowHandle<Root>>`. "Move to window" mở cửa sổ khi chưa có; khi đã có thì `activate_window`.
- Kích thước mặc định khoảng 1000×600, đặt giữa màn hình chứa cửa sổ chính.
- "Move back" gọi `remove_window`. Đóng bằng nút của hệ điều hành đi qua `on_window_should_close` / `on_release`; cả hai đều đặt `focus_window = None` và `cx.notify()` cửa sổ chính. Chiều cao Focus Pane trong `FocusPaneState` không bị đụng tới, nên khi về thì giữ nguyên chiều cao cũ.
- Đóng cửa sổ chính thì đóng luôn Focus Window, trong handler đóng của cửa sổ chính.
- Không ghi gì vào app state; khởi động luôn có `focus_window = None`.

### 4. Cửa sổ chính khi đã tách
`render` của cửa sổ chính bỏ tay kéo và Focus Pane khi `focus_window.is_some()`. Nút bật/tắt Focus Pane (`⌘J`) khi đó đưa Focus Window lên trước thay vì ẩn.

### 5. Phím tắt trong Focus Window
Focus Window có `on_key_down` riêng cho: `⌘F` (đặt con trỏ vào ô Search), Enter/Shift+Enter (next/prev Match), phím next/prev của Focus Find, `⌘C` (chép dòng đang chọn trong pane), và `⌘W` (Move back).

## Risks / Trade-offs

- [Render Focus Pane đang viết với `cx.listener` của `AppView`] → Tách thành hàm nhận `Entity<AppView>` ngay trong change này. Cả cửa sổ chính lẫn Focus Window dùng chung hàm đó.
- [Cửa sổ chính không tự render lại khi Focus Window đổi state] → Mọi thay đổi đều đi qua `app.update(...)` và gọi `cx.notify()`. Focus Window `observe` `app` để render lại.
- [Focus Window mở trên màn hình đã bị rút] → Không áp dụng, vì không nhớ vị trí.

## Migration Plan

Không có dữ liệu mới. Rollback chỉ cần bỏ code.
