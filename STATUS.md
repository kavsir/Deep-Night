# STATUS

## Đang làm
- Chờ con người duyệt `PROTOCOL.md` phiên bản v0 (Bước 0 trong ROADMAP).

## Vừa xong
- Khởi tạo Cargo workspace gồm `server/`, `protocol/`, `bot/` (skeleton biên dịch và chạy được) và thư mục `client/` với `README.md`.
- Viết `PROTOCOL.md` phiên bản v0 với đầy đủ các gói: `Join`, `Input`, `Welcome`, `PlayerJoined`, `PlayerLeft`, `Snapshot`, `Error`, giới hạn kích thước gói 4KB, rate limit 30 inputs/s, quy tắc xử lý dữ liệu sai.
- Cài đặt crate `protocol/` với serde tagged `type`, hàm kiểm tra tính hợp lệ nghiệp vụ (`is_valid()`) và 5 unit tests pass 100%.
- Kiểm tra toàn bộ workspace: `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test --workspace`, `cargo run -p server`, `cargo run -p bot` đều thành công.

## Yêu cầu cho agent kia
- Agent CLIENT: Chờ duyệt `PROTOCOL.md` v0 trước khi triển khai kết nối WebSocket trong thư mục `client/`.

## Bẫy đã gặp
- Máy Windows ban đầu thiếu link.exe (MSVC) và GCC. Khi dùng Rust `x86_64-pc-windows-gnu`, cần dùng MinGW phiên bản MSVCRT (thay vì UCRT) để khớp với runtime CRT mà `rustc` liên kết, tránh lỗi `STATUS_ACCESS_VIOLATION`.
- `rusqlite` (bundled) cần C compiler biên dịch mã nguồn C SQLite. Đã chuẩn bị sẵn môi trường build cho Bước 5 theo ROADMAP.
