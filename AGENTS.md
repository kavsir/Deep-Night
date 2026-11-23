# Dự án: Mini-MMORPG 2D bối cảnh hiệp sĩ trung cổ

Mục tiêu hiện tại: 4-5 người chơi, server chạy trên máy cá nhân (truy cập qua Tailscale).

## Công nghệ
- Server: Rust (tokio, tokio-tungstenite, serde, serde_json, rusqlite, tracing).
- Client: Godot 4.x, GDScript. Dùng cú pháp Godot 4 (@onready, await, CharacterBody2D,
  @export, signal.connect). TUYỆT ĐỐI không dùng cú pháp Godot 3.
- Giao thức: WebSocket, gói tin JSON, định nghĩa trong PROTOCOL.md (nguồn sự thật duy nhất).
- Lưu trữ: SQLite (data/game.db). Nội dung game (NPC, quái, vật phẩm, sự kiện): JSON trong data/.

## Cấu trúc
- server/    : Rust, binary server (game loop, NPC AI, sự kiện, lưu trữ)
- protocol/  : Rust, crate định nghĩa gói tin (dùng chung cho server và bot)
- bot/       : Rust, binary bot test
- client/    : Godot 4 (scene, UI, input, hiển thị)
- data/      : JSON nội dung game
- Tài liệu   : AGENTS.md, PROTOCOL.md, ROADMAP.md, DECISIONS.md, STATUS.md

## Kiến trúc (BẮT BUỘC)
1. SERVER AUTHORITATIVE: server quyết định vị trí, sát thương, vật phẩm, drop, kinh nghiệm.
   Client chỉ gửi INPUT (hướng di chuyển, yêu cầu tấn công/dùng đồ) và hiển thị snapshot.
   Không bao giờ tin vị trí, sát thương, số lượng do client gửi. Mọi gói từ client phải
   được kiểm tra hợp lệ (tồn tại, trong tầm, đủ điều kiện, rate limit).
2. NPC, quái, sự kiện thế giới, đồng hồ ngày đêm chỉ chạy trên server.
3. Server chạy theo tick cố định 20Hz: nhận input -> cập nhật thế giới -> gửi snapshot.
   Task mạng và game loop tách riêng, nối bằng channel.
4. NPC dùng State Machine. Mỗi loại NPC/quái là một bản ghi dữ liệu trong data/.

## Vùng sở hữu (KHÔNG sửa ngoài vùng của mình)
- Agent SERVER: server/, bot/, data/
- Agent CLIENT: client/
- Vùng chung: protocol/ và PROTOCOL.md (chỉ đổi theo quy trình bên dưới)
- Chỉ con người sửa AGENTS.md.
- Cần thay đổi ngoài vùng: ghi yêu cầu vào STATUS.md (mục "Yêu cầu cho agent kia") rồi dừng,
  không tự sửa.

## Quy trình đổi giao thức
1. Chỉ sửa PROTOCOL.md, mô tả đề xuất, KHÔNG viết code, rồi báo con người duyệt.
2. Sau khi được duyệt: cập nhật crate protocol/, rồi mỗi agent làm phía mình đúng theo
   PROTOCOL.md. Tăng số phiên bản giao thức ở đầu PROTOCOL.md.
3. Không bao giờ thêm/đổi trường gói tin mà không ghi trong PROTOCOL.md.

## Quy tắc đầu phiên
Đọc theo thứ tự: AGENTS.md, STATUS.md, DECISIONS.md, phần liên quan trong PROTOCOL.md,
ROADMAP.md, rồi `git log --oneline -20`.

## Quy tắc cuối phiên
- Cập nhật STATUS.md: đã làm gì, còn dở gì, bẫy đã gặp, lưu ý cho agent kia.
- Thêm vào DECISIONS.md nếu có quyết định mới (chỉ thêm dòng mới, không xóa/sửa dòng cũ).
- Đánh dấu tiến độ trong ROADMAP.md.
- Commit nhỏ, message rõ ràng.

## Git
- Mỗi agent làm trên nhánh riêng (agent-server/<tên-việc>, agent-client/<tên-việc>),
  tốt nhất trong git worktree riêng.
- KHÔNG push/merge vào main. Chỉ con người merge.
- Một việc nhỏ một commit.

## Quy ước code Rust (để con người review được)
- Chạy `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test` trước khi báo xong.
- Không dùng unwrap()/expect() trong code xử lý mạng và input từ client; trả lỗi hoặc
  ngắt kết nối client đó, server không được panic vì dữ liệu xấu.
- Mọi chỗ kiểm tra hợp lệ input từ client phải có comment `// AUTH:` giải thích điều kiện.
- Giới hạn kích thước gói tin và tần suất gói tin mỗi client.
- Ưu tiên code đơn giản, dễ đọc hơn là tối ưu sớm. Không thêm crate mới mà không ghi vào
  DECISIONS.md.

## Quy ước code Godot
- snake_case cho file/biến, PascalCase cho class, UPPER_SNAKE_CASE cho hằng số.
- Có type hint (`var hp: int = 0`).
- Gọi `WebSocketPeer.poll()` mỗi frame.
- Ưu tiên tạo node bằng code, hạn chế sửa tay file .tscn (dễ hỏng tham chiếu).
- Client chỉ hiển thị trạng thái server gửi; có thể nội suy (interpolation) vị trí để mượt,
  nhưng không tự quyết kết quả game.

## Kiểm tra
- Chạy server: `cargo run -p server`
- Chạy bot: `cargo run -p bot` (kết nối, di chuyển, in log)
- Không báo "xong" nếu chưa chạy kiểm tra và dán kết quả ngắn gọn vào STATUS.md.

## Nguyên tắc chung
- Gặp quyết định kiến trúc chưa rõ: hỏi con người, không tự chọn.
- Không refactor ngoài phạm vi việc được giao.
- Không làm trước các bước chưa tới trong ROADMAP.md.