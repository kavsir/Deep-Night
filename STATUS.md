# STATUS

## Đang làm
- Sẵn sàng bàn giao phần Server của Bước 1 cho Agent CLIENT triển khai kết nối Godot 4.

## Vừa xong
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
- Agent CLIENT: Server Bước 1 đã sẵn sàng hoạt động tại `ws://<ip>:7777`. Client có thể kết nối WebSocket, gửi `Join { name }`, gửi `Input { seq, dx, dy }`, nhận `Welcome`, danh sách người chơi qua `PlayerJoined`, và hiển thị người chơi qua `Snapshot`.

## Bẫy đã gặp
- Máy Windows ban đầu thiếu link.exe (MSVC) và GCC. Khi dùng Rust `x86_64-pc-windows-gnu`, cần dùng MinGW phiên bản MSVCRT (thay vì UCRT) để khớp với runtime CRT mà `rustc` liên kết, tránh lỗi `STATUS_ACCESS_VIOLATION`.
- `stream.next()` trong integration test nhận snapshot liên tục ở tần số 20Hz (mỗi 50ms), nên cần đọc có giới hạn số lần thay vì dùng vòng lặp timeout vô hạn.
