# STATUS

## Đang làm
- Bước 3 trong ROADMAP (Chiến đấu, kinh nghiệm, túi đồ):
  - Đã soạn thảo đề xuất nâng cấp giao thức `PROTOCOL.md` lên **v0.2** (bổ sung các gói `Attack`, `UseItem`, `PlayerStats`, `Inventory`, mảng `combat_events` trong `Snapshot`, cơ chế tính sát thương, kinh nghiệm, túi đồ 20 ô, cấu hình `data/items.json`).
  - ĐANG DỪNG LẠI và chờ con người duyệt `PROTOCOL.md` theo đúng quy trình đổi giao thức tại `AGENTS.md`. Chưa viết code server/client ở bước này.

## Vừa xong

### Client Bước 2 (Hiển thị Quái vật & NPC trên Godot 4)
- Cập nhật [client/server_packet.gd](file:///d:/Deep-Night/client/server_packet.gd): Giải mã và kiểm tra kiểu an toàn mảng `monsters` (id, type_id, x, y, state, hp, max_hp) và `npcs` (id, type_id, name, x, y, state) từ bản tin `Snapshot` v0.1.
- Nâng cấp [client/world.gd](file:///d:/Deep-Night/client/world.gd):
  - Hiển thị Quái vật: Hình khối đa giác phân biệt cho từng loài (Goblin: thoi xanh lá `#40c057`, Sói xám: đa giác đỏ thẫm `#e03131`), thanh máu HP nền xám thanh đo xanh lá co giãn theo tỷ lệ máu, nhãn trạng thái FSM đổi màu động (`[Đuổi theo]` màu cam, `[Tấn công!]` màu đỏ rực, `[Quay về]` màu xanh ngọc, `[Tuần tra]` màu xanh dương nhạt, `[Đã gục]` mờ 50%).
  - Hiển thị NPC: Hình khiên gia huy xanh dương (`Trưởng làng Galahad`, `Lính canh`) kèm tên hiển thị có viền chữ rõ nét và huy hiệu thoại `💬 Trò chuyện` màu vàng kim tự động xuất hiện khi người chơi ở cự ly $\le 50\text{px}$.
  - Nội suy chuyển động mượt mà (linear interpolation) cho toàn bộ Quái vật và NPC giữa các snapshot 20Hz.
  - Quản lý vòng đời thực thể: tự động tạo node khi thực thể mới xuất hiện, giải phóng node (`queue_free()`) khi thực thể biến mất khỏi snapshot.
- Cập nhật [client/main.gd](file:///d:/Deep-Night/client/main.gd): Chuyển tiếp mảng `monsters` và `npcs` từ `Snapshot` vào `world.gd`.
- Viết [client/test_runner.gd](file:///d:/Deep-Night/client/test_runner.gd) kiểm thử tự động trên Godot 4.4.1 headless:
  - Test 1: Giải mã chính xác gói `Snapshot` v0.1 chứa quái và NPC.
  - Test 2: Khởi tạo, cập nhật trạng thái FSM, co giãn thanh máu, ẩn/hiện huy hiệu thoại và giải phóng node trong `World`.
  - Test 3: Kết nối trực tiếp vào server đang chạy tại `ws://127.0.0.1:7777`, Join và nhận đầy đủ dữ liệu quái/NPC.
  - Kết quả kiểm thử: **TOÀN BỘ TEST GODOT CLIENT PASSED**.

### Server Bước 2 (Quái & NPC State Machine - Tập trung 1 người chơi)
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
- Bổ sung 2 integration test tự động trong `server/tests/server_test.rs`: 12/12 tests passed toàn workspace.

## Yêu cầu cho agent kia
- Không có. Cả hai phía Server và Client đã hoàn thành đồng bộ Bước 2.

## Bẫy đã gặp
- Khi bot fuzz test kiểm tra trùng tên chạy độc lập (Fuzz 4), nếu gửi tên một bot không online thì server chấp nhận thay vì từ chối. Cần tạo 1 kết nối client thành công trước rồi cho client thứ 2 thử đăng ký cùng tên đó.
- Godot 4.4.1 chỉ cho đọc hàng đợi nhận của `WebSocketPeer` khi còn ở `STATE_OPEN`. Server hiện đã gửi kèm khung WebSocket Close frame mã Policy (1008) kèm lý do dạng văn bản để client hiển thị đúng thông báo lỗi ngay cả khi socket đóng lập tức.
- Bộ giải mã bỏ qua ID và tick vượt `2^53 - 1` vì kiểu số của JSON trong Godot có thể làm tròn và khiến hai ID khác nhau trùng nhau.
- Máy Windows ban đầu thiếu link.exe (MSVC) và GCC. Khi dùng Rust `x86_64-pc-windows-gnu`, cần dùng MinGW phiên bản MSVCRT (thay vì UCRT) để khớp với runtime CRT mà `rustc` liên kết, tránh lỗi `STATUS_ACCESS_VIOLATION`.
- `stream.next()` trong integration test nhận snapshot liên tục ở tần số 20Hz (mỗi 50ms), nên cần đọc có giới hạn số lần thay vì dùng vòng lặp timeout vô hạn.
- Clippy cảnh báo `explicit_counter_loop` khi dùng biến `mut seq` ngoài vòng lặp; cảnh báo `unnecessary_map_or` khi có thể thay thế bằng `is_some_and` hoặc `is_none_or`.
