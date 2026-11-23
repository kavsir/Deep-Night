# Giao thức Mạng (PROTOCOL) - Mini-MMORPG 2D

Phiên bản: **v0.2** (Đề xuất Bước 3: Chiến đấu, Kinh nghiệm, Túi đồ)  
Nguồn sự thật duy nhất cho việc giao tiếp giữa Client và Server.

---

## 1. Quy ước chung

- **Tầng vận chuyển (Transport)**: WebSocket (`ws://` hoặc `wss://`).
- **Định dạng dữ liệu (Data Format)**: JSON (text frame), mã hóa UTF-8.
- **Phân biệt loại gói tin (Message Tagging)**: Dùng trường `"type"` ở cấp gốc của mỗi đối tượng JSON (tương ứng `#[serde(tag = "type")]` trong Rust).
- **Số phiên bản giao thức**: `v0.2`.
- **Nguyên tắc cốt lõi**: **SERVER AUTHORITATIVE**. Server nắm toàn quyền quyết định logic, trạng thái, vị trí thế giới, quái vật, NPC, tính toán sát thương, kinh nghiệm, túi đồ và tỷ lệ rơi đồ. Client chỉ gửi input (ý định) và nhận snapshot/stats để render.
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

### 2.3. `Attack` (v0.2)
Gửi ý định tấn công một mục tiêu (quái vật) từ người chơi.

- **Các trường**:
  - `type`: `"Attack"` (bắt buộc)
  - `seq`: `integer` (`u64`) - Số thứ tự gói tin để chống gửi trùng lặp.
  - `target_id`: `integer` (`u64`) - Mã định danh của quái vật muốn tấn công.
- **Quy tắc tính hợp lệ**:
  - `target_id` phải tồn tại trong danh sách quái đang sống trên server (`hp > 0`, state khác `"Dead"`).
  - Khoảng cách từ vị trí người chơi tới quái vật phải $\le$ `PLAYER_ATTACK_RANGE` (60.0px).
  - Tuân thủ thời gian hồi chiêu đòn đánh `PLAYER_ATTACK_COOLDOWN` (0.6 giây, tức tối đa ~1.67 đòn/giây). Nếu gửi nhanh hơn sẽ bị server bỏ qua.
  - Người chơi phải đang sống (`hp > 0`).
- **Ví dụ**:
  ```json
  {
    "type": "Attack",
    "seq": 106,
    "target_id": 10001
  }
  ```

### 2.4. `UseItem` (v0.2)
Yêu cầu sử dụng một vật phẩm tiêu hao trong túi đồ.

- **Các trường**:
  - `type`: `"UseItem"` (bắt buộc)
  - `slot`: `integer` (`u32`) - Vị trí ô trong túi đồ muốn sử dụng (dải từ `0` đến `19`).
- **Quy tắc tính hợp lệ**:
  - `slot` phải nằm trong khoảng `[0, MAX_INVENTORY_SLOTS - 1]`.
  - Ô đồ phải có vật phẩm và số lượng $> 0$.
  - Vật phẩm phải thuộc loại có thể sử dụng (ví dụ: `consumable` như `"potion_hp"`).
  - Tuân thủ thời gian hồi chiêu dùng vật phẩm `ITEM_USE_COOLDOWN` (1.0 giây).
  - Người chơi phải đang sống (`hp > 0`).
- **Ví dụ**:
  ```json
  {
    "type": "UseItem",
    "slot": 0
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
  - `players`: `array` - Danh sách trạng thái của tất cả người chơi đang kết nối:
    - `id`: `integer` (`u64`) - ID người chơi (dải 1 .. 9999).
    - `x`: `number` (`f32`) - Tọa độ X trên bản đồ.
    - `y`: `number` (`f32`) - Tọa độ Y trên bản đồ.
    - `hp`: `integer` (`i32`) - (v0.2) Máu hiện tại của người chơi.
    - `max_hp`: `integer` (`i32`) - (v0.2) Máu tối đa của người chơi.
    - `level`: `integer` (`u32`) - (v0.2) Cấp độ của người chơi.
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
  - `combat_events`: `array` - (v0.2, tùy chọn/mặc định rỗng) Danh sách sự kiện đòn đánh xảy ra trong tick này:
    - `attacker_id`: `integer` (`u64`) - ID bên tấn công (người chơi hoặc quái).
    - `target_id`: `integer` (`u64`) - ID bên nhận sát thương.
    - `damage`: `integer` (`i32`) - Lượng sát thương gây ra.
    - `is_crit`: `boolean` - Đòn đánh chí mạng hay không.
    - `target_hp`: `integer` (`i32`) - Máu còn lại của mục tiêu sau đòn đánh.
- **Ví dụ**:
  ```json
  {
    "type": "Snapshot",
    "tick": 240,
    "players": [
      { "id": 1, "x": 120.5, "y": 85.0, "hp": 100, "max_hp": 100, "level": 1 },
      { "id": 2, "x": 150.0, "y": 92.5, "hp": 85, "max_hp": 100, "level": 1 }
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
        "name": "Trưởng làng Galahad",
        "x": 520.0,
        "y": 510.0,
        "state": "Idle"
      }
    ],
    "combat_events": [
      {
        "attacker_id": 1,
        "target_id": 10001,
        "damage": 15,
        "is_crit": false,
        "target_hp": 35
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

### 3.5. `PlayerStats` (v0.2)
Gói tin cập nhật bảng thuộc tính chi tiết gửi riêng cho client người chơi khi tham gia (`Welcome`) hoặc khi chỉ số thay đổi (nhận kinh nghiệm, lên cấp, thay đổi trang bị).

- **Các trường**:
  - `type`: `"PlayerStats"` (bắt buộc)
  - `level`: `integer` (`u32`) - Cấp độ hiện tại.
  - `exp`: `integer` (`u64`) - Điểm kinh nghiệm hiện có.
  - `max_exp`: `integer` (`u64`) - Điểm kinh nghiệm cần để lên cấp tiếp theo.
  - `hp`: `integer` (`i32`) - Máu hiện tại.
  - `max_hp`: `integer` (`i32`) - Máu tối đa.
  - `attack_power`: `integer` (`i32`) - Sức tấn công vật lý cơ bản.
  - `defense`: `integer` (`i32`) - Điểm phòng thủ giảm sát thương.
- **Ví dụ**:
  ```json
  {
    "type": "PlayerStats",
    "level": 2,
    "exp": 120,
    "max_exp": 200,
    "hp": 120,
    "max_hp": 120,
    "attack_power": 18,
    "defense": 5
  }
  ```

### 3.6. `Inventory` (v0.2)
Gói tin gửi toàn bộ danh sách vật phẩm trong túi đồ của người chơi khi tham gia thế giới hoặc khi có biến động túi đồ (nhặt vật phẩm rơi từ quái, sử dụng bình máu, vứt bỏ).

- **Các trường**:
  - `type`: `"Inventory"` (bắt buộc)
  - `slots`: `array` - Danh sách các ô đồ đang có vật phẩm:
    - `slot`: `integer` (`u32`) - Chỉ số ô đồ từ `0` đến `19` (`MAX_INVENTORY_SLOTS = 20`).
    - `item_id`: `string` - Mã định danh vật phẩm (ví dụ: `"potion_hp"`, `"goblin_ear"`).
    - `count`: `integer` (`u32`) - Số lượng vật phẩm trong ô đó.
- **Ví dụ**:
  ```json
  {
    "type": "Inventory",
    "slots": [
      { "slot": 0, "item_id": "potion_hp", "count": 5 },
      { "slot": 1, "item_id": "goblin_ear", "count": 3 }
    ]
  }
  ```

### 3.7. `Error`
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

### 3.8. Đặc tả Cơ chế Chiến đấu, Kinh nghiệm & Túi đồ (v0.2)

1. **Cơ chế Tấn công & Sát thương (Combat)**:
   - **Người chơi tấn công quái vật**:
     - Client gửi gói `Attack { seq, target_id }`.
     - Server kiểm tra: `target_id` tồn tại, khoảng cách tới quái $\le$ `PLAYER_ATTACK_RANGE` (60px), và người chơi không trong thời gian hồi đòn `PLAYER_ATTACK_COOLDOWN` (0.6s).
     - Công thức sát thương: $\text{damage} = \max(1, \text{attack\_power} \pm \text{variance})$.
     - Quái vật bị trừ máu và lập tức chuyển FSM sang trạng thái `Chase` hoặc `Attack` nhắm vào người chơi vừa đánh.
     - Sinh sự kiện `CombatEvent` đưa vào `Snapshot` tick đó để client hiển thị số nhảy sát thương.
   - **Quái vật tấn công người chơi**:
     - Khi quái ở FSM `Attack` và cự ly $\le \text{attack\_range}$, mỗi chu kỳ 1.0 giây quái gây sát thương lên người chơi mục tiêu: $\text{damage} = \max(1, \text{monster\_attack} - \text{defense})$.
     - Sinh `CombatEvent` đưa vào `Snapshot`.
   - **Người chơi tử vong & Hồi sinh**:
     - Khi người chơi bị đánh cạn máu (`hp <= 0`): Người chơi chuyển sang trạng thái bất động trong 3 giây.
     - Sau 3 giây, server hồi sinh người chơi tại tọa độ mặc định (`SPAWN_X`, `SPAWN_Y`) với 100% máu.

2. **Cơ chế Kinh nghiệm & Thăng cấp (EXP & Leveling)**:
   - Khi quái vật bị hạ gục (`hp <= 0`): Người chơi kết liễu quái vật nhận được điểm kinh nghiệm tương ứng với loại quái (`goblin`: 25 EXP, `wolf`: 50 EXP).
   - Công thức EXP lên cấp: $\text{max\_exp} = \text{level} \times 100$.
   - Khi $\text{exp} \ge \text{max\_exp}$:
     - Cấp độ `level` tăng 1.
     - Trừ lượng EXP đã đạt: $\text{exp} = \text{exp} - \text{max\_exp}$.
     - Tăng chỉ số: `max_hp` tăng thêm $+20$, `attack_power` tăng thêm $+3$.
     - Hồi phục đầy đủ 100% máu hiện tại.
     - Server phát gói `PlayerStats` mới tới client.

3. **Cơ chế Túi đồ & Rơi vật phẩm (Inventory & Drops)**:
   - Mỗi người chơi sở hữu túi đồ gồm `MAX_INVENTORY_SLOTS = 20` ô (đánh số từ `0` đến `19`).
   - Khi quái chết, server tính toán tỷ lệ rơi đồ theo bảng cấu hình trong `data/items.json`:
     - Yêu tinh (Goblin): 60% rơi 1 `potion_hp`, 80% rơi 1 `goblin_ear`.
     - Sói xám (Wolf): 50% rơi 1 `potion_hp`, 90% rơi 1 `wolf_pelt`.
   - Vật phẩm rơi được đưa trực tiếp vào túi đồ của người chơi:
     - Ưu tiên cộng dồn (stack) vào ô có sẵn cùng loại vật phẩm (nếu chưa vượt `max_stack`).
     - Nếu không thể stack, đưa vào ô trống đầu tiên có chỉ số nhỏ nhất.
     - Nếu túi đồ đầy cả 20 ô: vật phẩm bị bỏ qua (thông báo túi đầy).
   - Server gửi gói `Inventory` cập nhật tới client.
   - Khi người chơi gửi gói `UseItem { slot }`:
     - Nếu vật phẩm tại ô đó là `"potion_hp"`: Hồi 40 HP (không vượt quá `max_hp`), trừ 1 số lượng vật phẩm. Nếu số lượng về 0, giải phóng ô đồ.
     - Cooldown giữa 2 lần dùng vật phẩm là `ITEM_USE_COOLDOWN` (1.0 giây).
     - Gửi cập nhật `Inventory` và `PlayerStats` tới client.

---

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

2. **Hằng số Chiến đấu & Túi đồ (v0.2)**:
   - `PLAYER_ATTACK_RANGE`: **60.0 pixels** (tầm đánh cận chiến của người chơi).
   - `PLAYER_ATTACK_COOLDOWN`: **0.6 giây** (khoảng thời gian tối thiểu giữa 2 lần ra đòn).
   - `ITEM_USE_COOLDOWN`: **1.0 giây** (khoảng thời gian tối thiểu giữa 2 lần dùng vật phẩm).
   - `MAX_INVENTORY_SLOTS`: **20** ô (túi đồ cá nhân).
   - `RESPAWN_DELAY`: **3.0 giây** (thời gian chờ hồi sinh sau khi tử vong).

3. **Kích thước gói tin tối đa (Max Packet Size)**:
   - Client -> Server: Tối đa **4096 bytes (4 KB)** cho mỗi frame văn bản WebSocket.
   - Bất kỳ frame nào vượt quá giới hạn này sẽ bị hủy bỏ ngay lập tức và server ngắt kết nối client vi phạm.

4. **Tần suất gói tin tối đa (Rate Limit)**:
   - Gói `Input`: Tối đa **30 gói / giây** cho mỗi client.
   - Gói `Attack`: Tối đa **5 gói / giây** cho mỗi client.
   - Gói `UseItem`: Tối đa **3 gói / giây** cho mỗi client.
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
Định nghĩa danh sách các loại quái, thuộc tính chiến đấu và cấu hình spawn:
```json
[
  {
    "type_id": "goblin",
    "name": "Yêu tinh",
    "max_hp": 50,
    "attack_power": 8,
    "defense": 2,
    "exp_reward": 25,
    "drops": [
      { "item_id": "potion_hp", "chance": 0.6, "min": 1, "max": 1 },
      { "item_id": "goblin_ear", "chance": 0.8, "min": 1, "max": 1 }
    ],
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
    "attack_power": 14,
    "defense": 4,
    "exp_reward": 50,
    "drops": [
      { "item_id": "potion_hp", "chance": 0.5, "min": 1, "max": 1 },
      { "item_id": "wolf_pelt", "chance": 0.9, "min": 1, "max": 1 }
    ],
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

### 6.3. `data/items.json` (v0.2)
Định nghĩa danh mục vật phẩm trong game:
```json
[
  {
    "id": "potion_hp",
    "name": "Bình máu nhỏ",
    "type": "consumable",
    "heal_amount": 40,
    "max_stack": 20,
    "description": "Hồi phục 40 điểm sinh lực khi sử dụng."
  },
  {
    "id": "goblin_ear",
    "name": "Tai yêu tinh",
    "type": "material",
    "heal_amount": 0,
    "max_stack": 99,
    "description": "Chiến lợi phẩm từ yêu tinh, có thể bán hoặc dùng làm nhiệm vụ."
  },
  {
    "id": "wolf_pelt",
    "name": "Da sói xám",
    "type": "material",
    "heal_amount": 0,
    "max_stack": 99,
    "description": "Bộ da sói dày, có thể bán hoặc dùng chế tạo áo giáp."
  }
]
```
