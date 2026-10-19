## Why

Remote Log Space đã chạy, nhưng để mở được một Remote File người dùng phải tự gõ lại từng Server và Jump Server mà `~/.ssh/config` đã có sẵn, không đăng nhập được bằng ssh-agent (1Password, Secretive, YubiKey) hay OTP/2FA, phải tạo Server trước rồi mới thêm được file, không biết cấu hình đúng hay sai cho tới khi mở file thật, và danh sách Server dài thì không có cách tìm. Các trình đọc log remote khác làm những việc này ngay từ đầu, nên đây là phần đang cản người dùng nhiều nhất.

## What Changes

- **Import từ `~/.ssh/config`**: người dùng chủ động import các `Host` (HostName, User, Port, IdentityFile, IdentityAgent, ProxyJump) thành Server và Jump Server của app. Chỉ đọc file lúc import hoặc lúc bấm "Refresh from ~/.ssh/config"; khi kết nối app vẫn chỉ dùng danh sách Server của chính nó và SSH client nhúng. Dòng không hỗ trợ (ProxyCommand, Match, wildcard Host…) được liệt kê là bị bỏ qua.
- **Đăng nhập bằng ssh-agent**: cách đăng nhập mới "Agent" dùng socket `SSH_AUTH_SOCK` của hệ thống hoặc một socket chọn riêng (1Password, Secretive, đường dẫn tuỳ ý). Khi agent cần người dùng chạm/duyệt, status hiện "Waiting for agent approval".
- **Keyboard-interactive (OTP/2FA)**: server hỏi gì thì app hiện đúng câu hỏi đó trong dialog cho người dùng trả lời, cả khi đây là bước thứ hai sau key/agent. Câu trả lời không bao giờ được lưu. Server cần keyboard-interactive không tự kết nối lại mà chờ người dùng bấm Connect/Reconnect.
- **Quick connect**: ô gõ `user@host[:port]:/path` (hoặc `<tên Server>:/path`) trong bộ chọn file trên Server để thêm Remote File ngay. Nếu chưa có Server khớp thì app thử ssh-agent trước, bị từ chối mới hỏi mật khẩu hoặc đường dẫn key, rồi tự tạo và lưu một Server mới (tên `user@host[:port]`), vì Remote File, host key và Keychain đều gắn với Server.
- **Test connection** trong form Server: thử cả chuỗi Jump Server với giá trị đang gõ (chưa lưu), báo từng bước (kết nối, host key, đăng nhập, SFTP) cho từng tầng.
- **Keyboard-interactive** là cách đăng nhập thứ tư trong form Server (cạnh key, mật khẩu, Agent), cho server chỉ cho đăng nhập qua câu hỏi. Ô file key vẫn là ô gõ đường dẫn, không có hộp chọn file.
- **Server Group và tag**: mỗi Server có thể thuộc một Server Group và có nhiều tag. Danh sách Servers và ô chọn Server trong bộ chọn file được nhóm theo Server Group và có ô tìm theo tên, địa chỉ, user, group, tag.

Không có thay đổi BREAKING: Server đã lưu giữ nguyên cách đăng nhập hiện tại.

## Capabilities

### New Capabilities
- `ssh-config-import`: import và refresh Server/Jump Server từ `~/.ssh/config`, ánh xạ các trường, xử lý trùng tên và dòng không hỗ trợ.

### Modified Capabilities
- `ssh-server`: (capability do `add-remote-live-log` đưa vào, chưa archive) yêu cầu "Đăng nhập bằng key hoặc mật khẩu" đổi để cho phép ssh-agent và keyboard-interactive và bỏ câu cấm đọc `~/.ssh/config`; thêm yêu cầu cho Agent, keyboard-interactive, Test connection, Server Group/tag và tìm Server.
- `remote-log-space`: thêm yêu cầu Quick connect để thêm Remote File mà không cần tạo Server trước; yêu cầu "Mở lại Project" thêm trường hợp Server cần keyboard-interactive.

## Domain changes

Không sửa trực tiếp CONTEXT.md và ADR 0009 trong change này (các session khác đang sửa song song); nội dung cần đổi khi gộp:

**ADR 0009** — thêm **ADR 0010** "Servers may be imported from ~/.ssh/config; agent and keyboard-interactive sign-in", sửa một phần 0009:
- Vẫn giữ SSH client nhúng và danh sách Server, known_hosts riêng của app.
- `~/.ssh/config` được **import theo yêu cầu**, không được đọc khi kết nối. Lý do: giữ đúng tinh thần của 0009 (hành vi không phụ thuộc cấu hình máy lúc chạy) mà vẫn đỡ việc gõ lại. Phương án đã cân nhắc: đọc `~/.ssh/config` mỗi lần kết nối như OpenSSH — bị loại vì Server sẽ đổi hành vi ngầm khi file đổi, và phải cài đặt cả `Match`, `Include`, token… mới đúng.
- Bỏ hai câu trong Consequences ("Aliases, ProxyJump… are not used" và "Servers that only allow agent or 2FA sign-in cannot be reached"); thay bằng: ProxyCommand, `Match` và các tuỳ chọn ngoài danh sách ánh xạ không được hỗ trợ; known_hosts của OpenSSH vẫn không được dùng.

**CONTEXT.md — glossary**:
- **Server**: sửa "how to sign in" thành "how to sign in (key, password, or ssh-agent, optionally followed by questions the server asks such as a one-time code)"; thêm "A Server may belong to one Server Group and carry tags."
- Thêm **Server Group**: "A named heading that Servers are listed under in the Servers list and the Server chooser, such as `prod` or `staging`. A Server belongs to at most one." _Avoid_: Folder, environment, cluster.
- Thêm **Quick Connect**: "Adding a Remote File by typing `user@host[:port]:/path` in one field. It reuses a matching Server or saves a new one." _Avoid_: Ad-hoc connection, temporary server.
- Thêm **Sign-in Prompt**: "A question the server asks while signing in (keyboard-interactive), such as a password or a one-time code, shown to the user as asked. Answers are never stored." _Avoid_: 2FA dialog, OTP box.

**CONTEXT.md — rules** (L16–L19 thuộc change này; L20–L31 thuộc ba change song song):
- L16. **Config Is Imported, Not Followed**: Servers can be imported from `~/.ssh/config` and refreshed on request; the app never reads it while connecting, and an imported Server is an ordinary Server the user can edit.
- L17. **Sign-in Prompts Need The User**: Answers to Sign-in Prompts are never stored. A Server that needs them is never reconnected by itself — after a lost connection or on reopening a Project it waits for the user to press Connect.
- L18. **Quick Connect Saves A Server**: A Remote File always belongs to a saved Server. Quick Connect reuses the Server matching the typed user, host and port (or the typed Server name) and otherwise saves a new one when the file is added.
- L19. **Agent Keys Stay In The Agent**: With ssh-agent sign-in the app never sees or stores the private key; it only asks the agent to sign.
- Sửa L6 **Reconnect Without Loss**: thêm "unless the Server needs Sign-in Prompts (L17)".

## Impact

- Code: `src/remote/server.rs` (`AuthMethod::Agent`, `group`, `tags`, `origin`), `src/remote/hub.rs` (xác thực agent, keyboard-interactive, partial success, test connection), module mới `src/remote/ssh_config.rs` (đọc và ánh xạ `~/.ssh/config`), module mới phân tích chuỗi Quick connect; không đụng keepalive/trạng thái kết nối theo Server (thuộc change `remote-command-source`), `src/app/server_dialog.rs` và `src/app/view/server_views.rs` (form, Test connection, import dialog, group/tag, ô tìm), `src/app/remote_picker.rs` và `src/app/view/remote_dialogs.rs` (ô Quick connect, ô tìm Server), `src/app/remote.rs` (trạng thái "Sign-in required", không tự kết nối lại Server cần prompt).
- Dependency: không thêm crate mới dự kiến — `russh` 0.64.1 đã có `AgentClient` (unix socket) và keyboard-interactive; parser `~/.ssh/config` tự viết (phần con nhỏ).
- Dữ liệu lưu: `servers.json` thêm các trường có `#[serde(default)]` (`group`, `tags`, `origin`, agent socket); không cần migrate.
- Phối hợp: change `remote-file-picker` cũng sửa bộ chọn file; change này chỉ thêm ô Quick connect và ô tìm/nhóm Server ở phần chọn Server, không đổi phần duyệt thư mục.
