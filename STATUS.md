# STATUS

## Đang làm
- Chờ Agent CLIENT triển khai hiển thị Quái vật và NPC trên client Godot 4 cho Bước 3.
- Phía Server và Bot test tập trung 1 người chơi cho Bước 3 đã hoàn thành xuất sắc.

## Vừa xong

### Server Bước 3 (Quái & NPC State Machine - Tập trung 1 người chơi)
- Hoàn thiện nâng cấp giao thức `PROTOCOL.md` lên **v0.1**: bổ sung `monsters` và `npcs` vào `Snapshot`, quy định các trạng thái FSM của quái vật (`Idle`, `Patrol`, `Chase`, `Attack`, `Return`, `Dead`) và NPC (`Idle`, `Patrol`, `Talk`).
- Thêm dữ liệu nội dung game trong `data/monsters.json` (Goblin, Sói xám với tầm aggro, tầm đánh, bán kính leash, chu kỳ hồi sinh) và `data/npcs.json` (Trưởng làng Galahad idle và Lính canh tuần tra waypoint).
- Triển khai State Machine cho Quái vật và NPC trong `server/src/entities.rs`:
  - Quái vật: Tự động tuần tra quanh điểm spawn, phát hiện người chơi trong tầm `aggro_radius`, đuổi theo ở tốc độ `chase_speed`, chuyển sang `Attack` khi áp sát trong tầm `attack_range`. Nếu người chơi chạy xa vượt bán kính `leash_radius`, quái lập tức bỏ đuổi, kích hoạt trạng thái `Return` quay về điểm spawn, hồi đầy máu và quay lại `Idle`/`Patrol`. Khi quái chết (`Dead`), ẩn khỏi snapshot sau 3 giây despawn và tự động hồi sinh sau `respawn_time`.
  - NPC: Tự động chuyển sang `Talk` khi người chơi ở cự ly gần (<= 50px), và quay trở lại trạng thái ban đầu (`Idle` hoặc `Patrol`) khi người chơi rời xa > 60px.
- Tích hợp `EntityManager` vào Game Loop 20Hz trong `server/src/lib.rs`: thu thập snapshot của quái và NPC phát sóng tới tất cả client đang kết nối.
- Hoàn thiện cơ chế gửi lý do đóng kết nối WebSocket Close frame: khi từ chối client hoặc gặp lỗi, server gửi gói `ServerMessage::Error` kèm khung WebSocket `Close` mang mã `Policy` (1008) và chuỗi lý do UTF-8, giúp client Godot 4.4.1 nhận được chính xác lý do từ chối.
- Cập nhật `bot/src/main.rs`: Mặc định chạy kịch bản tập trung 1 người chơi (`Knight_Arthur`):
  - Khởi đầu cạnh Trưởng làng Galahad, xác nhận NPC chuyển sang `Talk`.
  - Di chuyển rời xa Trưởng làng, xác nhận NPC trở lại `Idle`.
  - Tiến về bãi quái Goblin tại (300, 400), ghi nhận Goblin phát hiện người chơi và chuyển sang `Chase`, áp sát chuyển sang `Attack`.
  - Tháo chạy về phía đông nam vượt tầm leash, ghi nhận Goblin bỏ đuổi và chuyển sang `Return` quay về điểm spawn rồi trở về `Idle`.
  - Chạy tiếp 5 bài kiểm tra gói rác (fuzz tests): JSON lỗi, gói > 4096 bytes, `dx=5`, trùng tên người chơi đang online, gửi Input khi chưa Join. Toàn bộ test vượt qua, chứng minh server không sập.
- Bổ sung 2 integration test tự động trong `server/tests/server_test.rs`:
  - `test_single_player_monster_aggro_and_leash`: kiểm chứng FSM quái vật phản ứng với 1 người chơi (Aggro -> Chase -> Attack -> Leash Return -> Idle).
  - `test_single_player_npc_interaction_proximity`: kiểm chứng FSM NPC chuyển đổi `Talk` và `Idle` theo khoảng cách người chơi.
  - `test_duplicate_name_rejected`: kiểm chứng nhận được cả gói tin `Error` và khung WebSocket Close frame chứa đúng lý do lỗi.
- Kết quả kiểm tra:
  - `cargo fmt --check`: PASSED.
  - `cargo clippy --workspace --tests -- -D warnings`: PASSED (0 warnings).
  - `cargo test --workspace`: 12/12 tests passed (5 unit tests protocol, 7 integration tests server).
  - Chạy thực tế `target/debug/server.exe` và `target/debug/bot.exe`: kịch bản 1 người chơi và bộ fuzz tests chạy mượt mà, đầy đủ log chuyển đổi trạng thái.

### Client Bước 1
- Client Godot 4 có màn hình kết nối mặc định `127.0.0.1:7777`, gửi `Join`, vào bản đồ khi nhận `Welcome` và quay lại màn hình kết nối khi mất phiên.
- Client tạo danh sách nhân vật từ `PlayerJoined` / `PlayerLeft`, gắn camera theo nhân vật cục bộ và nội suy giữa hai `Snapshot` dựa trên `tick_rate`. WASD gửi hướng ngay khi đổi, kể cả khi nhả phím, và gửi lại sau ít nhất 100 ms khi giữ hướng. Vị trí lấy từ server.

## Yêu cầu cho agent kia (Agent CLIENT)
- Phía Server đã hoàn tất toàn bộ logic State Machine cho Quái vật và NPC trong Bước 3.
- Giao thức `Snapshot` đã có 2 trường `monsters: Vec<MonsterSnapshot>` và `npcs: Vec<NpcSnapshot>` (xem chi tiết trong `PROTOCOL.md` v0.1).
- Người chơi mới xuất hiện tại `(500, 500)` ngay cạnh Trưởng làng Galahad tại `(520, 510)`. Ở khoảng cách này, Trưởng làng sẽ có `state: "Talk"`.
- Bãi quái Goblin gần nhất ở `(300, 400)`. Khi người chơi tiếp cận (khoảng cách <= 180px), Goblin sẽ chuyển sang `state: "Chase"` và đuổi theo người chơi, sau đó chuyển sang `state: "Attack"` khi khoảng cách <= 35px.
- Agent CLIENT chỉ cần:
  1. Cập nhật giải mã gói `Snapshot` để lấy thêm danh sách `monsters` và `npcs`.
  2. Tạo scene / node hiển thị NPC (hiển thị tên, trạng thái thoại/bong bóng chat khi `state == "Talk"`).
  3. Tạo scene / node hiển thị Quái vật (thanh máu `hp / max_hp`, hoạt ảnh theo `state`: Idle/Patrol/Chase/Attack/Return/Dead).
  4. Nội suy tọa độ `x, y` của quái và NPC mượt mà giữa các snapshot như đã làm với người chơi.

## Bẫy đã gặp
- Khi bot fuzz test kiểm tra trùng tên chạy độc lập (Fuzz 4), nếu gửi tên một bot không online thì server chấp nhận thay vì từ chối. Cần tạo 1 kết nối client thành công trước rồi cho client thứ 2 thử đăng ký cùng tên đó.
- Godot 4.4.1 chỉ cho đọc hàng đợi nhận của `WebSocketPeer` khi còn ở `STATE_OPEN`. Server hiện đã gửi kèm khung WebSocket Close frame mã Policy (1008) kèm lý do dạng văn bản để client hiển thị đúng thông báo lỗi ngay cả khi socket đóng lập tức.
- Bộ giải mã bỏ qua ID và tick vượt `2^53 - 1` vì kiểu số của JSON trong Godot có thể làm tròn và khiến hai ID khác nhau trùng nhau.
- Máy Windows ban đầu thiếu link.exe (MSVC) và GCC. Khi dùng Rust `x86_64-pc-windows-gnu`, cần dùng MinGW phiên bản MSVCRT (thay vì UCRT) để khớp với runtime CRT mà `rustc` liên kết, tránh lỗi `STATUS_ACCESS_VIOLATION`.
- `stream.next()` trong integration test nhận snapshot liên tục ở tần số 20Hz (mỗi 50ms), nên cần đọc có giới hạn số lần thay vì dùng vòng lặp timeout vô hạn.
- Clippy cảnh báo `explicit_counter_loop` khi dùng biến `mut seq` ngoài vòng lặp; cảnh báo `unnecessary_map_or` khi có thể thay thế bằng `is_some_and` hoặc `is_none_or`.
