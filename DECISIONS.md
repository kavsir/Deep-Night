# DECISIONS (chỉ thêm dòng mới, không xóa/sửa dòng cũ)

- 001: Server Rust + client Godot 4, giao tiếp WebSocket JSON. Lý do: Godot có WebSocketPeer sẵn, JSON dễ debug.
- 002: Server authoritative, tick 20Hz.
- 003: Lưu trữ SQLite (rusqlite), nội dung game bằng JSON trong data/.
- 004: Gói tin WebSocket định dạng JSON dùng trường "type" phân biệt loại gói (serde tag = "type"). Phiên bản protocol ban đầu là v0.
- 005: Giới hạn kích thước gói client -> server tối đa 4096 bytes (4KB), rate limit Input tối đa 30 gói/giây từ mỗi client để chống spam và bù jitter.
- 006: Thêm crate futures-util để sử dụng StreamExt và SinkExt trên WebSocketStream.
- 007: Mở rộng gói Snapshot trong PROTOCOL v0.1 với 2 mảng "monsters" và "npcs", mỗi thực thể có ID ổn định và trạng thái State Machine để client hiển thị hoạt ảnh và nội suy mượt mà.
- 008: Tập trung hoàn thiện vòng lặp trải nghiệm 1 người chơi trước (single-player loop: di chuyển, kích hoạt hội thoại NPC qua khoảng cách gần, quái vật phát hiện aggro -> chase -> attack -> leash return) trước khi mở rộng logic tranh chấp aggro nhiều người chơi.
- 009: Khi server gửi thông báo lỗi `ServerMessage::Error`, đồng thời gửi khung đóng WebSocket Close frame với mã `Policy` (1008) chứa đúng lý do lỗi bằng UTF-8 để client Godot 4.4.1 nhận và hiển thị thông báo lỗi chính xác ngay cả khi socket đóng lập tức.
- 010: Client Godot 4 hiển thị thực thể Quái vật (Goblin: thoi xanh lá; Sói xám: đa giác đỏ thẫm) kèm thanh máu HP và nhãn trạng thái FSM đổi màu động (Chase: cam, Attack: đỏ, Return: xanh ngọc, Dead: mờ); NPC mang hình gia huy xanh dương kèm huy hiệu thoại "💬 Trò chuyện" nổi bật khi người chơi ở cự ly gần; tất cả thực thể được nội suy vị trí mượt mà giữa các Snapshot.
- 011: Đề xuất PROTOCOL v0.2 cho Bước 3: Bổ sung ClientMessage::Attack { seq, target_id }, ClientMessage::UseItem { slot }, ServerMessage::PlayerStats, ServerMessage::Inventory, mảng combat_events trong Snapshot để truyền số sát thương nhảy; danh mục vật phẩm và tỷ lệ rơi đồ trong data/items.json và data/monsters.json.
- 012: Hoàn thiện Bước 3: Client gửi lệnh tấn công bằng phím Space hoặc chuột (tự động nhắm quái vật gần nhất trong phạm vi 60px) và phím 1-6 để dùng vật phẩm từ túi đồ. Gói Welcome luôn là bản tin đầu tiên trả về sau khi Join, theo sau là danh sách người online, PlayerStats và Inventory. Số sát thương nhảy (floating combat text) xuất hiện tức thì và bay lên mờ dần trong 0.8 giây.
