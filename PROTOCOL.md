# Giao thức Mạng (PROTOCOL) - Mini-MMORPG 2D

Phiên bản: **v0.1** (Đề xuất Bước 3: Quái và NPC)  
Nguồn sự thật duy nhất cho việc giao tiếp giữa Client và Server.

---

## 1. Quy ước chung

- **Tầng vận chuyển (Transport)**: WebSocket (`ws://` hoặc `wss://`).
- **Định dạng dữ liệu (Data Format)**: JSON (text frame), mã hóa UTF-8.
- **Phân biệt loại gói tin (Message Tagging)**: Dùng trường `"type"` ở cấp gốc của mỗi đối tượng JSON (tương ứng `#[serde(tag = "type")]` trong Rust).
- **Số phiên bản giao thức**: `v0.1`.
- **Nguyên tắc cốt lõi**: **SERVER AUTHORITATIVE**. Server nắm toàn quyền quyết định logic, trạng thái, vị trí thế giới, quái vật, NPC và AI. Client chỉ gửi input (ý định) và nhận snapshot để render.
- **Đóng kết nối**: Khi từ chối kết nối hoặc đóng phiên do lỗi (ví dụ: trùng tên, gói quá lớn, vi phạm rate limit), server gửi gói `Error` VÀ truyền mã Close (mã `1008 Policy Violation` hoặc `1000 Normal Closure`) kèm lý do bằng chuỗi UTF-8 trong khung Close của WebSocket để client Godot đọc được ngay lập tức.

---

## 2. Gói tin từ Client -> Server (`ClientMessage`)

### 2.1. `Join`
Yêu cầu tham gia vào thế giới game. Client phải gửi gói tin này đầu tiên ngay sau khi kết nối WebSocket thành công.

- **Các trường**:
  - `type`: `"Join"` (bắt buộc)
  - `name`: `string` - Tên hiển thị của người chơi.
- **Quy tắc tính hợp lệ**:
  - Độ dài chuỗi `name` từ 1 đến 20 ký tự (sau khi trim khoảng trắng).
  - Không chứa ký tự điều khiển (control characters) hoặc ký tự xuống dòng.
- **Ví dụ**:
  ```json
  {
    "type": "Join",
    "name": "Arthur"
  }
  ```

### 2.2. `Input`
Gửi ý định di chuyển của người chơi đến server.

- **Các trường**:
  - `type`: `"Input"` (bắt buộc)
  - `seq`: `integer` (`u64`) - Số thứ tự gói tin input (tăng dần). Dùng để client đối soát và server xác định thứ tự input.
  - `dx`: `integer` (`i8`) - Hướng di chuyển trục ngang X: `-1` (trái), `0` (đứng yên), `1` (phải).
  - `dy`: `integer` (`i8`) - Hướng di chuyển trục dọc Y: `-1` (lên), `0` (đứng yên), `1` (xuống).
- **Quy tắc tính hợp lệ**:
  - `dx` bắt buộc phải thuộc tập `{-1, 0, 1}`.
  - `dy` bắt buộc phải thuộc tập `{-1, 0, 1}`.
- **Ví dụ**:
  ```json
  {
    "type": "Input",
    "seq": 105,
    "dx": 1,
    "dy": 0
  }
  ```

---

## 3. Gói tin từ Server -> Client (`ServerMessage`)

### 3.1. `Welcome`
Server phản hồi sau khi chấp nhận gói `Join` của client.

- **Các trường**:
  - `type`: `"Welcome"` (bắt buộc)
  - `player_id`: `integer` (`u64`) - Mã ID duy nhất server cấp cho người chơi này trong phiên làm việc.
  - `tick_rate`: `integer` (`u32`) - Tần số tick của server (mặc định: `20` Hz).
  - `map`: `string` - Tên bản đồ người chơi đang ở (ví dụ: `"starter_village"`).
- **Ví dụ**:
  ```json
  {
    "type": "Welcome",
    "player_id": 1,
    "tick_rate": 20,
    "map": "starter_village"
  }
  ```

### 3.2. `PlayerJoined`
Thông báo cho các client khác trong thế giới biết có một người chơi vừa tham gia.

- **Các trường**:
  - `type`: `"PlayerJoined"` (bắt buộc)
  - `id`: `integer` (`u64`) - ID của người chơi vừa vào.
  - `name`: `string` - Tên của người chơi vừa vào.
- **Ví dụ**:
  ```json
  {
    "type": "PlayerJoined",
    "id": 2,
    "name": "Lancelot"
  }
  ```

### 3.3. `PlayerLeft`
Thông báo khi một người chơi thoát khỏi thế giới / ngắt kết nối.

- **Các trường**:
  - `type`: `"PlayerLeft"` (bắt buộc)
  - `id`: `integer` (`u64`) - ID của người chơi đã rời đi.
- **Ví dụ**:
  ```json
  {
    "type": "PlayerLeft",
    "id": 2
  }
  ```

### 3.4. `Snapshot`
Ảnh chụp trạng thái thế giới tại mỗi tick game (chu kỳ 20Hz), broadcast tới các client để cập nhật và nội suy hiển thị.

- **Các trường**:
  - `type`: `"Snapshot"` (bắt buộc)
  - `tick`: `integer` (`u64`) - Số thứ tự tick hiện tại của server.
  - `players`: `array` - Danh sách vị trí của tất cả người chơi đang kết nối:
    - `id`: `integer` (`u64`) - ID người chơi (dải 1 .. 9999).
    - `x`: `number` (`f32`) - Tọa độ X trên bản đồ.
    - `y`: `number` (`f32`) - Tọa độ Y trên bản đồ.
  - `monsters`: `array` - Danh sách quái vật đang hiện diện trên bản đồ:
    - `id`: `integer` (`u64`) - ID định danh thực thể quái (duy nhất, dải >= 10000, ổn định từ khi sinh ra tới khi chết).
    - `type_id`: `string` - Mã loại quái (ví dụ: `"goblin"`, `"wolf"`, ánh xạ tới cấu hình `data/monsters.json`).
    - `x`: `number` (`f32`) - Tọa độ X trên bản đồ.
    - `y`: `number` (`f32`) - Tọa độ Y trên bản đồ.
    - `state`: `string` - Trạng thái State Machine hiện tại: `"Idle"`, `"Patrol"`, `"Chase"`, `"Attack"`, `"Return"`, `"Dead"`.
    - `hp`: `integer` (`i32`) - Máu hiện tại.
    - `max_hp`: `integer` (`i32`) - Máu tối đa.
  - `npcs`: `array` - Danh sách NPC đang hiện diện trên bản đồ:
    - `id`: `integer` (`u64`) - ID định danh thực thể NPC (duy nhất, dải >= 20000).
    - `type_id`: `string` - Mã loại NPC (ví dụ: `"village_elder"`, `"guard"`, ánh xạ tới cấu hình `data/npcs.json`).
    - `name`: `string` - Tên hiển thị của NPC.
    - `x`: `number` (`f32`) - Tọa độ X trên bản đồ.
    - `y`: `number` (`f32`) - Tọa độ Y trên bản đồ.
    - `state`: `string` - Trạng thái State Machine hiện tại: `"Idle"`, `"Patrol"`, `"Talk"`.
- **Ví dụ**:
  ```json
  {
    "type": "Snapshot",
    "tick": 240,
    "players": [
      { "id": 1, "x": 120.5, "y": 85.0 },
      { "id": 2, "x": 150.0, "y": 92.5 }
    ],
    "monsters": [
      {
        "id": 10001,
        "type_id": "goblin",
        "x": 320.0,
        "y": 450.0,
        "state": "Patrol",
        "hp": 50,
        "max_hp": 50
      }
    ],
    "npcs": [
      {
        "id": 20001,
        "type_id": "village_elder",
        "name": "Trưởng làng",
        "x": 520.0,
        "y": 510.0,
        "state": "Idle"
      }
    ]
  }
  ```

### 3.4.1. Quy tắc xuất hiện, cập nhật và biến mất của Thực thể (Spawn/Despawn/Respawn)
1. **Danh sách đầy đủ (Full snapshot)**:
   - Tại mỗi tick 20Hz, server gửi toàn bộ các thực thể (quái, NPC) đang hoạt động trong thế giới.
2. **Khởi tạo và gỡ bỏ trên Client**:
   - Khi client nhận `Snapshot`:
     - Nếu gặp `id` chưa tồn tại trong danh sách của client -> Client tạo mới (instantiate) node tương ứng (dùng `type_id` để chọn sprite/scene).
     - Nếu `id` đã tồn tại -> Client cập nhật tọa độ đích để nội suy mượt mà (interpolation) và cập nhật diễn hoạt (animation) theo `state`.
     - Nếu một `id` trước đó có trong client nhưng không còn xuất hiện trong `Snapshot` -> Client giải phóng node đó (`queue_free()`).
3. **Vòng đời của Quái**:
   - Khi máu `hp <= 0`, quái chuyển sang trạng thái `"Dead"`.
   - Server vẫn giữ quái ở trạng thái `"Dead"` trong 3 giây (60 ticks) để client hiển thị hoạt ảnh chết (death animation).
   - Hết 3 giây, server loại bỏ quái khỏi `Snapshot` và bắt đầu đếm thời gian hồi sinh (`respawn_time` trong `data/monsters.json`).
   - Sau khi hết thời gian hồi sinh, quái xuất hiện trở lại tại điểm spawn với đầy đủ máu và trạng thái `"Idle"`.

### 3.4.2. Đặc tả State Machine của Quái (Monster FSM)
Mỗi quái vật chạy một Finite State Machine độc lập trên server tại mỗi tick:
- **`Idle`**: Đứng yên tại vị trí spawn hoặc nghỉ chân giữa các lần đi dạo. Sau khoảng thời gian chờ (idle timer), quái chuyển sang `Patrol`. Nếu phát hiện người chơi trong tầm `aggro_radius`, chuyển sang `Chase`.
- **`Patrol`**: Đi dạo ngẫu nhiên hoặc tuần tra quanh điểm spawn ban đầu (trong phạm vi `patrol_radius`). Khi tới điểm đích, quay lại `Idle`. Nếu phát hiện người chơi trong tầm `aggro_radius`, chuyển sang `Chase`.
- **`Chase`**: Đuổi theo người chơi gần nhất theo đường thẳng.
  - Nếu khoảng cách tới mục tiêu <= `attack_range`: chuyển sang `Attack`.
  - Nếu mục tiêu chạy xa khỏi điểm spawn ban đầu quá cự ly `leash_radius` hoặc người chơi ngắt kết nối: chuyển sang `Return`.
- **`Attack`**: Dừng lại và thực hiện đòn đánh vào người chơi. Khi mục tiêu di chuyển ra ngoài `attack_range` nhưng vẫn trong `leash_radius`: quay lại `Chase`.
- **`Return`**: Quái không đuổi nữa, quay đầu chạy về điểm spawn ban đầu với tốc độ cao hơn và hồi đầy máu. Khi về tới điểm spawn: chuyển sang `Idle`.
- **`Dead`**: Quái bị tiêu diệt, không di chuyển, chờ despawn.

### 3.4.3. Đặc tả State Machine của NPC (NPC FSM)
- **`Idle`**: Đứng yên tại vị trí spawn (ví dụ đứng trước quầy hoặc cổng làng).
- **`Patrol`**: Di chuyển tuần tra qua lại giữa các điểm waypoint cố định được cấu hình trong `data/npcs.json`.
- **`Talk`**: Khi có người chơi đứng trong phạm vi tương tác (khoảng cách < 40px), NPC quay mặt về phía người chơi và chuyển sang `Talk`. Khi người chơi rời đi, NPC quay lại `Idle` hoặc tiếp tục `Patrol`.

### 3.5. `Error`
Thông báo lỗi từ server gửi tới client (ví dụ: dữ liệu không hợp lệ, lỗi đăng nhập).

- **Các trường**:
  - `type`: `"Error"` (bắt buộc)
  - `message`: `string` - Nội dung thông báo lỗi.
- **Ví dụ**:
  ```json
  {
    "type": "Error",
    "message": "Ten nguoi choi khong hop le"
  }
  ```

---

## 4. Giới hạn kỹ thuật và Hằng số thế giới (Limits & Constants)

1. **Hằng số thế giới & Di chuyển**:
   - `TICK_RATE`: **20 Hz** (chu kỳ mỗi tick: 50ms).
   - `PLAYER_MOVE_SPEED`: **200.0 pixels / giây** (tương đương `10.0 pixels / tick` khi di chuyển đơn trục; khi di chuyển chéo sẽ chuẩn hóa vector với độ dài 1.0 để vận tốc không bị tăng lên $\sqrt{2}$).
   - `MAP_MIN_X`: **0.0**
   - `MAP_MAX_X`: **1000.0**
   - `MAP_MIN_Y`: **0.0**
   - `MAP_MAX_Y`: **1000.0**
   - `SPAWN_X`: **500.0**
   - `SPAWN_Y`: **500.0**

2. **Kích thước gói tin tối đa (Max Packet Size)**:
   - Client -> Server: Tối đa **4096 bytes (4 KB)** cho mỗi frame văn bản WebSocket.
   - Bất kỳ frame nào vượt quá giới hạn này sẽ bị hủy bỏ ngay lập tức và server ngắt kết nối client vi phạm.

3. **Tần suất gói tin tối đa (Rate Limit)**:
   - Gói `Input`: Tối đa **30 gói / giây** cho mỗi client (server chạy ở 20Hz, ngưỡng 30 gói/giây cho phép bù đắp biến thiên độ trễ mạng - network jitter).
   - Gói `Join`: Tối đa **1 lần** cho mỗi phiên kết nối.
   - Tổng tần suất gói tin từ một client: Không vượt quá **60 gói / giây**.

---

## 5. Quy tắc Server xử lý dữ liệu sai (Server Validation & Error Handling)

Server áp dụng triệt để nguyên tắc **Zero Trust** đối với dữ liệu từ client:

1. **Tuyệt đối không panic**:
   - Mọi thao tác giải mã (deserialization) và xử lý input phải dùng Result/Option; không dùng `unwrap()` hoặc `expect()`.
   - Nếu dữ liệu không phân tích cú pháp (parse) được thành JSON hoặc thiếu trường: server gửi gói `Error` và ngắt kết nối.

2. **Kiểm tra trạng thái phiên (Session State Check)**:
   - Khi vừa kết nối, client ở trạng thái `Unauthenticated`.
   - Client **phải** gửi gói `Join` hợp lệ trước. Nếu client gửi `Input` khi chưa `Join`, server từ chối và ngắt kết nối (`// AUTH: Client must join before sending inputs`).
   - Nếu client đã `Join` thành công mà tiếp tục gửi thêm gói `Join`: server từ chối hoặc bỏ qua (`// AUTH: Duplicate join rejected`).

3. **Kiểm tra giá trị nghiệp vụ (Business Validation)**:
   - `name`: Cắt bỏ khoảng trắng đầu/cuối. Nếu rỗng, dài hơn 20 ký tự, hoặc chứa ký tự điều khiển: gửi gói `Error` ("Ten nguoi choi khong hop le") và đóng kết nối.
   - `dx`, `dy`: Phải thuộc `{-1, 0, 1}`. Nếu nằm ngoài khoảng này (ví dụ `dx = 10` do client hack tốc độ): server lập tức loại bỏ gói `Input` này (`// AUTH: Invalid direction values, input dropped`), không cập nhật vị trí.
   - `seq`: Nếu `seq` nhận được nhỏ hơn hoặc bằng `seq` trước đó quá xa (gói tin đến trễ do mạng): server bỏ qua gói cũ.

4. **Xử lý vi phạm Rate Limit và Flooding**:
   - Nếu client gửi vượt quá 30 `Input`/giây: server drop các gói thừa trong giây đó.
   - Nếu client liên tục spam quá 60 gói/giây: server coi là hành vi tấn công từ chối dịch vụ (DoS), ngắt kết nối ngay lập tức.

---

## 6. Cấu trúc dữ liệu nội dung game trong `data/`

Tất cả nội dung quái và NPC được định nghĩa tĩnh trong thư mục `data/` theo nguyên tắc của AGENTS.md (Server Authoritative):

### 6.1. `data/monsters.json`
Định nghĩa danh sách các loại quái và cấu hình spawn:
```json
[
  {
    "type_id": "goblin",
    "name": "Yêu tinh",
    "max_hp": 50,
    "move_speed": 100.0,
    "chase_speed": 150.0,
    "aggro_radius": 180.0,
    "attack_range": 35.0,
    "leash_radius": 300.0,
    "respawn_time": 10.0,
    "spawns": [
      { "spawn_x": 300.0, "spawn_y": 400.0, "patrol_radius": 80.0 },
      { "spawn_x": 700.0, "spawn_y": 300.0, "patrol_radius": 100.0 }
    ]
  },
  {
    "type_id": "wolf",
    "name": "Sói xám",
    "max_hp": 80,
    "move_speed": 120.0,
    "chase_speed": 180.0,
    "aggro_radius": 220.0,
    "attack_range": 40.0,
    "leash_radius": 350.0,
    "respawn_time": 15.0,
    "spawns": [
      { "spawn_x": 200.0, "spawn_y": 700.0, "patrol_radius": 120.0 },
      { "spawn_x": 800.0, "spawn_y": 750.0, "patrol_radius": 100.0 }
    ]
  }
]
```

### 6.2. `data/npcs.json`
Định nghĩa danh sách các NPC thân thiện:
```json
[
  {
    "type_id": "village_elder",
    "name": "Trưởng làng Galahad",
    "spawn_x": 520.0,
    "spawn_y": 510.0,
    "behavior": "idle",
    "dialogue": "Chào mừng hiệp sĩ đến với ngôi làng!"
  },
  {
    "type_id": "village_guard",
    "name": "Lính canh",
    "spawn_x": 450.0,
    "spawn_y": 500.0,
    "behavior": "patrol",
    "move_speed": 60.0,
    "waypoints": [
      { "x": 450.0, "y": 500.0 },
      { "x": 550.0, "y": 500.0 }
    ]
  }
]
```
