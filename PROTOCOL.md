# Giao thức Mạng (PROTOCOL) - Mini-MMORPG 2D

Phiên bản: **v0**  
Nguồn sự thật duy nhất cho việc giao tiếp giữa Client và Server.

---

## 1. Quy ước chung

- **Tầng vận chuyển (Transport)**: WebSocket (`ws://` hoặc `wss://`).
- **Định dạng dữ liệu (Data Format)**: JSON (text frame), mã hóa UTF-8.
- **Phân biệt loại gói tin (Message Tagging)**: Dùng trường `"type"` ở cấp gốc của mỗi đối tượng JSON (tương ứng `#[serde(tag = "type")]` trong Rust).
- **Số phiên bản giao thức**: `v0`.
- **Nguyên tắc cốt lõi**: **SERVER AUTHORITATIVE**. Server nắm toàn quyền quyết định logic, trạng thái, vị trí thế giới. Client chỉ gửi input (ý định) và nhận snapshot để render.

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
  - `players`: `array` - Danh sách vị trí của tất cả người chơi:
    - `id`: `integer` (`u64`) - ID người chơi.
    - `x`: `number` (`f32`) - Tọa độ X trên bản đồ.
    - `y`: `number` (`f32`) - Tọa độ Y trên bản đồ.
- **Ví dụ**:
  ```json
  {
    "type": "Snapshot",
    "tick": 240,
    "players": [
      { "id": 1, "x": 120.5, "y": 85.0 },
      { "id": 2, "x": 150.0, "y": 92.5 }
    ]
  }
  ```

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
