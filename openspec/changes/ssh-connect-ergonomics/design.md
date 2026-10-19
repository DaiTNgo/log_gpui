## Context

`add-remote-live-log` đã đưa vào SSH client nhúng (`russh` 0.64.1 + `russh-sftp` 3.0.1, tokio runtime riêng trong `RemoteHub`), danh sách Server lưu ở `servers.json`, `known_hosts.json` riêng, bí mật trong Keychain (`src/remote/secrets.rs`). Hiện trạng liên quan:

- `AuthMethod` chỉ có `Key { path }` và `Password` (`src/remote/server.rs`); `RemoteHub::authenticate` (`src/remote/hub.rs`) gọi `authenticate_password` / `authenticate_publickey` một lần và coi mọi kết quả không `success()` là "Authentication failed", nên server đòi `publickey,keyboard-interactive` hiện bị báo sai là sai mật khẩu.
- Thiếu bí mật thì `ConnectError::NeedsSecret` → UI hỏi rồi thử lại. Host key chưa biết thì `HostKeyRequest` gửi qua `async_channel` để UI hỏi. Đây là mẫu sẵn có để hỏi người dùng giữa lúc kết nối.
- Form Server (`src/app/server_dialog.rs`, `src/app/view/server_views.rs`) có ô đường dẫn key gõ tay, tạo được Jump Server mới ngay trong form. Bộ chọn file (`src/app/remote_picker.rs`) bắt đầu bằng chọn Server từ một danh sách phẳng.
- ADR 0009 cấm đọc `~/.ssh/config` và để agent/2FA cho sau. Change này nới cả hai (xem "Domain changes" trong proposal).

Change `remote-file-picker` đang sửa phần duyệt thư mục của bộ chọn file song song; change này chỉ chạm phần chọn Server và thêm ô Quick Connect.

## Goals / Non-Goals

**Goals:**
- Có Server dùng được trong vài giây với người đã có `~/.ssh/config`, ssh-agent hoặc 2FA.
- Biết cấu hình đúng hay sai ngay trong form, chỉ ra bước và tầng hỏng.
- Thêm một Remote File chỉ bằng một chuỗi gõ.
- Danh sách nhiều chục Server vẫn tìm được nhanh.

**Non-Goals:**
- Đọc `~/.ssh/config` khi kết nối, `Match`, `ProxyCommand`, `CertificateFile`, `LocalForward`/port forwarding, agent forwarding.
- Dùng `~/.ssh/known_hosts` của OpenSSH (vẫn chỉ `known_hosts.json` của app).
- Lưu câu trả lời Sign-in Prompt, tự điền OTP (TOTP secret).
- Chứng chỉ SSH (user certificate) và GSSAPI/Kerberos.
- Jump Server trong chuỗi Quick Connect (`-J`); người dùng sửa Server sau.
- Đồng bộ hai chiều với `~/.ssh/config` (app không bao giờ ghi file này).

## Decisions

### 1. Import là ảnh chụp, không phải liên kết sống
`src/remote/ssh_config.rs` đọc `~/.ssh/config` thành danh sách `ConfigHost { alias, host_name, user, port, identity_files, identity_agent, proxy_jump, unsupported: Vec<String> }` theo luật "giá trị đầu tiên thắng" của OpenSSH, duyệt mọi khối `Host` có pattern khớp alias (hỗ trợ `*`, `?`, `!`), xử lý `Include` (đường dẫn tương đối tính từ `~/.ssh`, có glob), token `%h`, `%u`, `%d`, `%%` và `~`. Khối `Match` bị bỏ qua và đưa vào "Skipped".

Mỗi Server import được ghi `origin: Some(Origin::SshConfig { alias })`. Refresh đọc lại, so trường của Server với `ConfigHost` hiện tại và hiện từng khác biệt để người dùng tích. App không ghi lại `~/.ssh/config` và không tự áp dụng thay đổi.

*Thay thế đã cân nhắc:*
- Đọc config mỗi lần kết nối (như OpenSSH): Server đổi hành vi ngầm, khó giải thích khi lỗi, phải cài đặt cả `Match exec`. Loại.
- Crate `ssh2-config`/`ssh_config`: chất lượng và cách xử lý `Include`/first-match không đồng đều, mà phần cần dùng nhỏ (một tokenizer theo dòng + so pattern). Tự viết, có test theo các ví dụ trong `man ssh_config`.

### 2. Chọn cách đăng nhập khi import
Thứ tự: `IdentityAgent` có (khác `none`) → `Agent { socket: <đó> }`; `IdentityFile` có file tồn tại → `Key { path }` với file tồn tại đầu tiên, kèm `preferred_key` là `<path>.pub` nếu có; không có gì → `Agent { socket: System }`. Lý do để Agent làm mặc định: đây đúng là điều OpenSSH làm khi không có `IdentityFile` (thử agent rồi key mặc định), và người dùng 1Password/Secretive gần như luôn có agent.

### 3. `AuthMethod::Agent` và socket
```
enum AgentSocket { System, OnePassword, Secretive, Path(String) }
AuthMethod::Agent { socket: AgentSocket, preferred_key: Option<String> /* đường dẫn .pub hoặc fingerprint SHA256 */ }
```
`System` lấy `SSH_AUTH_SOCK` của tiến trình app lúc kết nối (app mở từ Finder vẫn nhận socket của launchd). `OnePassword` = `~/Library/Group Containers/2BUA8C4S2C.com.1password/t/agent.sock`, `Secretive` = `~/Library/Containers/com.maxgoedjen.Secretive.SecretAgent/Data/socket.ssh`; form hiện preset nào đang có socket tồn tại.

Xác thực: `russh::keys::agent::client::AgentClient::connect_uds(socket)` → `request_identities()` → lọc theo `preferred_key` nếu có → với từng key gọi `handle.authenticate_publickey_with(user, key, hash, &mut agent)`. Dừng ở key đầu tiên thành công hoặc khi server trả `partial_success`. Thử tối đa 5 key để tránh `MaxAuthTries` (mặc định 6) cắt kết nối; quá thì báo kèm gợi ý đặt "Preferred key". Mỗi lần ký có timeout 60 s; trong lúc chờ, hub gửi `ConnectProgress::WaitingForAgent { server_name }` lên UI.

*Thay thế đã cân nhắc:* tự đọc private key mà agent đang giữ — không thể (agent không xuất key) và trái mục đích của 1Password/Secretive/YubiKey.

### 4. Vòng xác thực theo `remaining_methods`
`authenticate` đổi thành một vòng nhỏ:
1. Chạy cách chính (Key / Password / Agent / KeyboardInteractive).
2. Nếu `AuthResult::Success` → xong.
3. Nếu `Failure { partial_success: true, remaining_methods }` và có `keyboard-interactive` → sang bước Sign-in Prompt. Có `password` mà cách chính không phải Password → hỏi mật khẩu (dùng Keychain nếu có).
4. Ngược lại → `AuthFailed` với thông báo theo cách đã dùng.

Keyboard-interactive: `authenticate_keyboard_interactive_start(user, None)` → mỗi `InfoRequest { name, instructions, prompts }` được gửi lên UI qua channel mới `SignInRequest { server_id, server_name, name, instructions, prompts: Vec<(String, bool /*echo*/)>, reply: oneshot::Sender<Option<Vec<String>>> }` (cùng mẫu với `HostKeyRequest`); `None` là huỷ. Lượt có `prompts` rỗng được trả lời rỗng ngay, không hiện dialog. Câu trả lời chỉ sống trong bộ nhớ của lượt đó, được ghi đè bằng `zeroize` sau khi gửi.

Hub ghi nhớ trong bộ nhớ (và trong `servers.json` qua cờ `needs_sign_in_prompt: bool`, cập nhật mỗi lần kết nối thành công) Server nào đã cần Sign-in Prompt. Khi tự kết nối lại hoặc mở lại Project, hub được gọi với `interactive: false`; gặp Server có cờ này (hoặc server bất ngờ đòi keyboard-interactive) thì trả `ConnectError::SignInRequired { server_id }` thay vì hỏi. UI (`src/app/remote.rs`) hiện "Sign-in required: <server>" + Connect/Reconnect; bấm thì gọi lại với `interactive: true`. Vòng tự kết nối lại (1s…30s) dừng ngay khi gặp lỗi này thay vì thử tiếp 5 phút.

### 5. Test connection dùng chung đường kết nối, không dùng chung session
`RemoteHub::test_chain(chain: Vec<Server>, secrets: TypedSecrets) -> impl Stream<TestStep>` dùng lại `connect_chain`, nhưng:
- chain lấy từ giá trị form (Server và Jump Server chưa lưu dùng id tạm);
- session không vào bảng session dùng chung, và bị đóng khi xong hay khi huỷ (drop task);
- sau đăng nhập tầng cuối thử mở SFTP rồi đóng;
- mỗi bước phát `TestStep { hop, step: Tcp | HostKey | Auth | Sftp, result: Ok(detail) | Err(msg), elapsed }`.

Host key được trust trong lúc test được ghi vào `known_hosts.json` như bình thường (đó là quyết định của người dùng, không phụ thuộc việc lưu Server). Secret gõ trong lúc test giữ trong `TypedSecrets` của form; chỉ ghi Keychain khi Save với Remember.

*Thay thế đã cân nhắc:* lưu Server rồi kết nối thật — trái mong muốn "thử trước khi lưu" và để lại Server rác khi thử sai.

### 6. Ô file key là ô gõ đường dẫn
Không có hộp chọn file (người dùng đã chốt): ô key giữ là ô gõ đường dẫn như hiện nay, `~` được mở rộng. Key sai hoặc thiếu passphrase được phát hiện khi Test connection hoặc khi kết nối, với các lỗi sẵn có (`NeedsSecret`, "Could not read the key …").

### 7. Quick Connect: luôn là một Server đã lưu
Bộ phân tích `src/remote/quick.rs`:
```
parse("ops@[fe80::1]:2222:/var/log/x") -> QuickTarget { user: Some("ops"), host: "fe80::1", port: Some(2222), path: "/var/log/x" }
```
Luật tách: dạng `ssh://` theo URL; ngược lại tách `user@` ở `@` đầu tiên, host là `[...]` hoặc tới dấu `:` đầu tiên; nếu phần sau là toàn chữ số và theo sau bởi `:` thì là port; phần còn lại là path, phải bắt đầu bằng `/` hoặc `~/`. `~/` được đổi thành home của user trên server bằng SFTP `realpath(".")`.

Giải Server theo thứ tự trong spec. Server mới được dựng trong bộ nhớ (`Server` có id tạm, `origin: Some(Origin::QuickConnect)`), kết nối và duyệt như Server thật, và chỉ được ghi vào `ServerStore` trong `picker_confirm` khi có ít nhất một Remote File của nó được thêm. Agent bị từ chối → bộ chọn hiện form đăng nhập nhỏ (mật khẩu hoặc gõ đường dẫn key) ngay trong bộ chọn. Thứ tự đã chốt: luôn thử Agent trước rồi mới hỏi.

*Thay thế đã cân nhắc:* Remote File "không cần Server" (chỉ lưu `user@host:port` trong Remote File). Loại vì Remote Log Space lưu theo `server_id`, host key, Keychain account, L15 và "Sửa Server khi đang kết nối" đều dựa vào Server; một loại Remote File thứ hai sẽ nhân đôi mọi chỗ đó. Server tự lưu, sửa và xoá được thì người dùng không mất gì.

### 8. Server Group, tag và tìm
`Server` thêm `group: Option<String>` và `tags: Vec<String>` (cả hai `#[serde(default)]`, đã trim, tag không trùng). Group không có bảng riêng: danh sách group là tập `group` của các Server, nên group rỗng tự biến mất và đổi tên group = sửa các Server (form có ô group với gợi ý từ group đã có). Trạng thái thu gọn group lưu trong settings app theo tên group.

Tìm: một hàm thuần `server_matches(&Server, &str) -> bool` dùng cùng cách gập chữ của Search (Unicode case-fold, không bỏ dấu, rule 8/8a), tách truy vấn theo khoảng trắng, mỗi từ phải là chuỗi con của ít nhất một trong tên, `host`, `user`, group, tag. Một widget danh sách Server dùng chung (`ServerList` view: group heading + ô tìm + chọn bằng phím) cho dialog Servers, ô chọn Server của bộ chọn file và ô chọn Jump Server.

*Thay thế đã cân nhắc:* group lồng nhau / thư mục. Loại (người dùng chốt một tầng + tag): một tầng đủ cho prod/staging/khách hàng, còn tag lo phần cắt ngang.

### 9. Dữ liệu lưu
`servers.json` thêm, đều có default: `group`, `tags`, `origin` (`None` | `SshConfig { alias }` | `QuickConnect`), `needs_sign_in_prompt`, và biến thể `AuthMethod::Agent`. Bản cũ của app đọc file có `Agent` sẽ lỗi parse → giữ nguyên quy ước rollback: xem Migration.

## Risks / Trade-offs

- **[Agent ký chậm hoặc cần chạm]** → timeout 60 s riêng cho bước ký, status "Waiting for agent approval", vòng tự kết nối lại không đếm thời gian chờ chạm vào backoff.
- **[Agent có nhiều key → server cắt vì `MaxAuthTries`]** → thử tối đa 5 key, ưu tiên `preferred_key`; thông báo lỗi gợi ý đặt Preferred key; import điền sẵn từ `IdentityFile`.
- **[`SSH_AUTH_SOCK` của app mở từ Finder khác của terminal]** → preset 1Password/Secretive và đường dẫn tuỳ chọn; form hiện socket thực tế đang dùng.
- **[Server đòi OTP làm vỡ tính "tự kết nối lại" (L6)]** → chủ ý: không bật dialog khi người dùng không ở đó; Break và đọc tiếp từ byte vẫn giữ khi người dùng bấm Reconnect. Ghi vào CONTEXT thành L17 (proposal).
- **[Config dùng tính năng không hỗ trợ]** → liệt kê rõ trong "Skipped"; `ProxyCommand` không tích sẵn; không âm thầm tạo Server sai.
- **[Quick Connect tạo nhiều Server rác]** → chỉ lưu khi có file được thêm; khớp Server sẵn có theo tên và theo user/host/port trước.
- **[Test connection trust host key]** → giống kết nối thật; chấp nhận vì người dùng đã thấy fingerprint.
- **[Hai change cùng sửa bộ chọn file]** → change này chỉ đụng phần chọn Server và một ô Quick Connect phía trên; nếu `remote-file-picker` thay cấu trúc bộ chọn thì task Quick Connect gắn ô vào vị trí tương ứng.

## Migration Plan

- `servers.json` cũ đọc được không đổi (mọi trường mới có default; Server cũ giữ `Key`/`Password`).
- Rollback: bản cũ không đọc được `AuthMethod::Agent`/`KeyboardInteractive`. Trước khi ghi Server đầu tiên dùng cách đăng nhập mới, app sao lưu `servers.json` thành `servers.v1.json` một lần; gỡ change thì khôi phục file đó.

## Open Questions

Không còn câu hỏi mở. Đã chốt với người dùng: không có hộp chọn file key; Keyboard-interactive là cách đăng nhập thứ tư trong form; mỗi Server một Server Group (không lồng) cộng tag; Quick Connect tới host mới thử Agent trước rồi mới hỏi mật khẩu/key. Keepalive và trạng thái kết nối theo Server thuộc change `remote-command-source`, không làm ở đây.
