# STATUS

## Đang làm
- Đã soạn thảo xong đề xuất giao thức **PROTOCOL.md phiên bản v0.1** cho Bước 3 (Quái và NPC State Machine).
- Dừng lại theo đúng quy tắc AGENTS.md (mục "Quy trình đổi giao thức": chỉ sửa PROTOCOL.md, mô tả đề xuất, KHÔNG viết code, rồi báo con người duyệt).
- Chờ con người duyệt nội dung đề xuất `PROTOCOL.md` v0.1 trước khi cập nhật crate `protocol/` và viết code logic server/client.

## Vừa xong
- Client Godot 4 có màn hình kết nối mặc định `127.0.0.1:7777`, gửi `Join`, vào bản đồ khi nhận `Welcome` và quay lại màn hình kết nối khi mất phiên.
- Client tạo danh sách nhân vật từ `PlayerJoined` / `PlayerLeft`, gắn camera theo nhân vật cục bộ và nội suy giữa hai `Snapshot` dựa trên `tick_rate`. WASD gửi hướng ngay khi đổi, kể cả khi nhả phím, và gửi lại sau ít nhất 100 ms khi giữ hướng. Vị trí lấy từ server.
- Bản đồ v0 dùng kích thước hiển thị tạm **1024 × 1024 px, tile 32 px**, chờ PROTOCOL v0.1 vì `Welcome.map` hiện chỉ là tên bản đồ. Biên di chuyển vẫn do server quyết định.
- Cách chạy: tại `D:\Deep-Night`, chạy `cargo run -p server`. Mở `client/project.godot` bằng Godot 4 và nhấn F5. Mở hai phiên với hai tên khác nhau để kiểm tra thấy nhau và di chuyển bằng WASD.
- Kiểm tra với Godot 4.4.1 headless và server thật tại `ws://127.0.0.1:7777`: hai client nhận `Welcome`, thấy người vào trước và người mới, đồng bộ vị trí sau khi di chuyển và nhả phím, xóa nhân vật khi rời, báo mất kết nối khi dừng server. WebSocket thử nghiệm xác nhận JSON sai bị bỏ qua, `Error` được hiển thị và lý do đóng được giữ khi quay lại màn hình kết nối. Chưa kiểm tra giao diện bằng cửa sổ Godot.
- `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test` (10 test) và `cargo run -p bot` (`DURATION_SECS=3`) đều thành công.

### Server Bước 1

- Triển khai Server WebSocket (`tokio-tungstenite`) đọc cổng từ biến môi trường `PORT` (mặc định 7777), bind `0.0.0.0`.
- Thiết kế kiến trúc Server Authoritative: tách riêng Network Task và Game Loop 20Hz qua channel `GameEvent`.
- Xử lý kết nối và xác thực:
  - Timeout 5 giây cho gói `Join`.
  - Kiểm tra độ dài tên (1-20 ký tự), lọc ký tự điều khiển.
  - Từ chối trùng tên đang online bằng gói `Error` ("Ten nguoi choi da ton tai").
  - Gán `player_id`, gửi `Welcome`, gửi danh sách người chơi online cho người mới, và broadcast `PlayerJoined` cho người khác; broadcast `PlayerLeft` khi người chơi ngắt kết nối.
- Game Loop 20Hz: gom `Input` mới nhất theo `seq`, tính toán vị trí theo `PLAYER_MOVE_SPEED` (200 px/s, chuẩn hóa khi đi chéo), kẹp trong biên map `[0.0, 1000.0]`, broadcast `Snapshot` mỗi tick.
- Kiểm tra an toàn dữ liệu mạng (Zero-trust, không panic, không unwrap/expect):
  - Kích thước frame tối đa 4096 bytes (gói lớn hơn bị từ chối bằng Error).
  - Rate limit tối đa 30 Input/s và 60 packets/s chống spam DoS.
  - Bỏ qua dữ liệu sai cú pháp JSON hoặc giá trị `dx, dy` ngoài `{-1, 0, 1}`.
  - Tất cả các điểm kiểm tra input đều có chú thích `// AUTH:`.
- Viết `bot/src/main.rs`: chạy đồng thời 3 bot di chuyển vòng quanh 30 giây (hơn 700 ticks), in log tọa độ; chạy kịch bản gửi gói rác (JSON lỗi, gói 5000 bytes, `dx=5`, trùng tên, gửi Input khi chưa Join) chứng minh server không sập.
- Viết 5 integration tests tự động trong `server/tests/server_test.rs`.
- Kết quả kiểm tra:
  - `cargo fmt --check`: PASSED.
  - `cargo clippy --workspace -- -D warnings`: PASSED (0 warnings).
  - `cargo test --workspace`: 10/10 tests passed (5 unit tests protocol, 5 integration tests server).
  - `target/debug/bot.exe`: 3 bot chạy 30s thành công, toàn bộ fuzz tests vượt qua.

## Yêu cầu cho agent kia
- Agent SERVER: Đề xuất mở rộng `Snapshot` để server gửi danh sách đầy đủ quái và NPC hiện diện ở mỗi tick. Client cần ID ổn định, loại hoặc mã hiển thị, tọa độ và trạng thái cần hiển thị. Đề nghị đặc tả kiểu dữ liệu, giá trị trạng thái và quy tắc xuất hiện, biến mất trong `PROTOCOL.md` để người duyệt trước khi viết code. Sau khi được duyệt, server phụ trách dữ liệu và State Machine. Client dùng snapshot để tạo, cập nhật và gỡ đối tượng trên màn hình.
- Agent SERVER: Khi từ chối tham gia hoặc đóng phiên do lỗi, hãy truyền lý do trong khung WebSocket Close cùng với gói `Error` hiện có để Godot vẫn hiện được chi tiết nếu cả hai khung đến trong cùng một lần poll.

## Bẫy đã gặp
- Godot 4.4.1 chỉ cho đọc hàng đợi nhận của `WebSocketPeer` khi còn ở `STATE_OPEN`. Với server thật, tên trùng bị từ chối nhưng client chỉ hiện “Máy chủ đã đóng kết nối trước khi vào bản đồ” vì khung Close không có lý do và gói `Error` đến ngay trước đó không đọc được.
- Bộ giải mã bỏ qua ID và tick vượt `2^53 - 1` vì kiểu số của JSON trong Godot có thể làm tròn và khiến hai ID khác nhau trùng nhau.
- Máy Windows ban đầu thiếu link.exe (MSVC) và GCC. Khi dùng Rust `x86_64-pc-windows-gnu`, cần dùng MinGW phiên bản MSVCRT (thay vì UCRT) để khớp với runtime CRT mà `rustc` liên kết, tránh lỗi `STATUS_ACCESS_VIOLATION`.
- `stream.next()` trong integration test nhận snapshot liên tục ở tần số 20Hz (mỗi 50ms), nên cần đọc có giới hạn số lần thay vì dùng vòng lặp timeout vô hạn.
