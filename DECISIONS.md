# DECISIONS (chỉ thêm dòng mới, không xóa/sửa dòng cũ)

- 001: Server Rust + client Godot 4, giao tiếp WebSocket JSON. Lý do: Godot có WebSocketPeer sẵn, JSON dễ debug.
- 002: Server authoritative, tick 20Hz.
- 003: Lưu trữ SQLite (rusqlite), nội dung game bằng JSON trong data/.
- 004: Gói tin WebSocket định dạng JSON dùng trường "type" phân biệt loại gói (serde tag = "type"). Phiên bản protocol ban đầu là v0.
- 005: Giới hạn kích thước gói client -> server tối đa 4096 bytes (4KB), rate limit Input tối đa 30 gói/giây từ mỗi client để chống spam và bù jitter.
- 006: Thêm crate futures-util để sử dụng StreamExt và SinkExt trên WebSocketStream.
- 007: Mở rộng gói Snapshot trong PROTOCOL v0.1 với 2 mảng "monsters" và "npcs", mỗi thực thể có ID ổn định và trạng thái State Machine để client hiển thị hoạt ảnh và nội suy mượt mà.
