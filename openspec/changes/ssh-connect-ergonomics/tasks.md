## 1. Mô hình Server

- [ ] 1.1 `Server` thêm `group`, `tags`, `origin` (`SshConfig { alias }` | `QuickConnect`), `needs_sign_in_prompt` với `#[serde(default)]`; `AuthMethod` thêm `Agent { socket, preferred_key }` và `KeyboardInteractive`; test đọc `servers.json` cũ
- [ ] 1.2 Sao lưu `servers.json` thành `servers.v1.json` một lần trước khi ghi Server đầu tiên dùng cách đăng nhập mới

## 2. Xác thực trong RemoteHub

- [ ] 2.1 Đổi `authenticate` thành vòng theo `remaining_methods`/`partial_success`; sửa lỗi báo "Authentication failed" khi server còn đòi bước sau
- [ ] 2.2 Đăng nhập Agent: `AgentClient::connect_uds`, lọc theo `preferred_key`, thử tối đa 5 key, timeout ký 60 s, `ConnectProgress::WaitingForAgent`; lỗi "ssh-agent not available at …" và "no agent key accepted"
- [ ] 2.3 Preset socket System / 1Password / Secretive / Path, phát hiện socket nào tồn tại
- [ ] 2.4 Keyboard-interactive: channel `SignInRequest` (mẫu của `HostKeyRequest`), trả lời rỗng cho lượt không có câu hỏi, huỷ = dừng kết nối Server đó, xoá câu trả lời khỏi bộ nhớ sau khi gửi
- [ ] 2.5 Cờ `needs_sign_in_prompt`, tham số `interactive`; `ConnectError::SignInRequired`; vòng tự kết nối lại dừng ngay khi gặp lỗi này
- [ ] 2.6 Test tích hợp với `sshd` cục bộ (`scripts/` hiện có): agent (`ssh-agent` tạm), `AuthenticationMethods publickey,keyboard-interactive`, Jump Server có keyboard-interactive

## 3. UI đăng nhập

- [ ] 3.1 Dialog Sign-in Prompt: tên Server, name/instructions của server, ô che theo cờ echo, Cancel
- [ ] 3.2 Trạng thái "Sign-in required: <server>" với Connect/Reconnect trên status bar; mở lại Project không tự hỏi
- [ ] 3.3 Status "Waiting for agent approval: <server>"

## 4. Form Server

- [ ] 4.1 Chọn cách đăng nhập Key / Password / Agent / Keyboard-interactive; với Agent hiện socket preset và Preferred key
- [ ] 4.2 `RemoteHub::test_chain` theo design (session riêng, bước Tcp/HostKey/Auth/Sftp theo từng tầng, huỷ được)
- [ ] 4.3 Nút "Test connection" và vùng kết quả theo tầng/bước trong form; secret gõ khi test chỉ vào Keychain khi Save + Remember
- [ ] 4.4 Ô Server Group (gợi ý từ group đã có) và ô tag trong form

## 5. Import từ ~/.ssh/config

- [ ] 5.1 `src/remote/ssh_config.rs`: tokenizer theo dòng (`key value`, `key=value`, ngoặc kép, comment), `Host` pattern (`*`, `?`, `!`), first-value-wins, `Include` có glob, token `%h %u %d %%`, `~`; khối `Match` vào Skipped; test theo ví dụ `man ssh_config`
- [ ] 5.2 Ánh xạ `ConfigHost` → Server: HostName, User, Port, IdentityFile (file tồn tại đầu tiên + `.pub` làm preferred key), IdentityAgent, mặc định Agent; cảnh báo "Key file not found"
- [ ] 5.3 ProxyJump → chuỗi Jump Server: dùng alias/Server có sẵn hoặc tạo Server cho tầng `[user@]host[:port]`; `none`; từ chối vòng lặp và quá 4 tầng
- [ ] 5.4 Dialog import: danh sách có ô tích, cảnh báo (ProxyCommand không tích sẵn, Name already used + sửa tên tại dòng), phần Skipped, ô Server Group, nút Import
- [ ] 5.5 Refresh: so trường của Server có `origin = SshConfig`, hiện khác biệt có ô tích, alias mới, "No longer in ~/.ssh/config"; áp dụng qua đường "Sửa Server khi đang kết nối"

## 6. Quick Connect

- [ ] 6.1 `src/remote/quick.rs`: phân tích `[user@]host[:port]:path`, `ssh://…`, IPv6 trong `[]`, `~/`; lỗi "Expected user@host[:port]:/path"; test bảng các dạng chuỗi
- [ ] 6.2 Giải Server: tên → user/host/port → Server tạm `origin = QuickConnect` với Agent
- [ ] 6.3 Ô Quick Connect trong bộ chọn file (phối hợp với change `remote-file-picker`): kết nối, đường dẫn/glob/chưa tồn tại như ô gõ tay, `~/` qua SFTP `realpath`
- [ ] 6.4 Agent bị từ chối → form đăng nhập nhỏ (mật khẩu hoặc gõ đường dẫn key; luôn sau khi thử Agent) trong bộ chọn; chỉ lưu Server khi `picker_confirm` có file của nó

## 7. Server Group và tìm

- [ ] 7.1 `server_matches` thuần dùng cách gập chữ của Search; test nhiều từ, địa chỉ, tag, dấu tiếng Việt
- [ ] 7.2 View `ServerList` dùng chung: heading group theo chữ cái + "Ungrouped" cuối, thu gọn (nhớ trong settings app), ô tìm, Enter chọn kết quả đầu, "No matching servers"
- [ ] 7.3 Dùng `ServerList` trong dialog Servers, ô chọn Server của bộ chọn file và ô chọn Jump Server

## 8. Hoàn tất

- [ ] 8.1 Cập nhật CONTEXT.md và thêm ADR theo mục "Domain changes" của proposal (L16–L19 và ADR 0010)
- [ ] 8.2 `cargo test` và `cargo clippy` sạch; `openspec validate ssh-connect-ergonomics` qua
