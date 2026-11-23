# STATUS

## Đang làm
- Bước 3 trong ROADMAP (Chiến đấu, kinh nghiệm, túi đồ) đã hoàn tất 100% cả phía Server và Client.
- Chờ chỉ đạo của con người để chuyển sang Bước 4 (SQLite: đăng nhập, lưu nhân vật).

## Vừa xong

### Bước 3: Chiến đấu, Kinh nghiệm, Túi đồ (Server & Client Hoàn Tất)
1. **Giao thức PROTOCOL v0.2**:
   - Gói Client -> Server: `Attack { seq, target_id }`, `UseItem { slot }`.
   - Gói Server -> Client: `PlayerStats { level, exp, max_exp, hp, max_hp, attack_power, defense }`, `Inventory { slots: [{ slot, item_id, count }] }`.
   - Mở rộng `Snapshot`: Bổ sung `hp`, `max_hp`, `level` cho người chơi; bổ sung mảng `combat_events: [{ attacker_id, target_id, damage, is_crit, target_hp }]`.
2. **Server & Game Data**:
   - Thêm `data/items.json`: định nghĩa `potion_hp` (hồi 40 HP, max stack 20), `goblin_ear` (max stack 99), `wolf_pelt` (max stack 99).
   - Nâng cấp `data/monsters.json` và `server/src/entities.rs`: thêm `attack_power`, `defense`, `exp_reward`, và bảng drop table ngẫu nhiên (`roll_drops`).
   - Xử lý chiến đấu Server Authoritative: Người chơi tấn công quái trong tầm 60px (`PLAYER_ATTACK_RANGE`), cooldown 0.6s; quái vật tấn công người chơi trong tầm 35-40px chu kỳ 1.0s. Quái chết thưởng EXP và rơi đồ trực tiếp vào túi đồ 20 ô của người chơi.
   - Thăng cấp tự động: Khi $\text{exp} \ge \text{level} \times 100$, tăng cấp, $+20\text{ HP}$, $+3\text{ Công}$ và hồi đầy máu.
   - Sử dụng vật phẩm: `UseItem` hồi 40 HP, cooldown 1.0s, trừ số lượng vật phẩm trong ô đồ.
   - Hồi sinh sau tử vong: Bất động 3s khi hết máu, hồi sinh tại Spawn (500, 500) với 100% HP.
3. **Bot Test & Fuzz Testing**:
   - Kịch bản 1 người chơi (`Knight_Arthur`):
     - Gặp Trưởng làng Galahad -> NPC `Talk`.
     - Tiến về hang Goblin -> Goblin chuyển `Chase` rồi `Attack`.
     - Người chơi tấn công Goblin, Goblin phản kích (cả 2 bên đều nhận sát thương và sinh `CombatEvent`).
     - Người chơi hạ gục Goblin -> nhận 25 EXP, nhặt chiến lợi phẩm vào túi đồ (`potion_hp` và `goblin_ear`).
     - Người chơi dùng bình máu (`slot = 0`) -> HP hồi phục đầy 100%, túi đồ trừ 1 bình máu.
   - 7 bài kiểm tra gói rác (Fuzz Tests): JSON sai cú pháp, gói > 4096 bytes, `dx=5`, trùng tên online, Input khi chưa Join, Attack quái không tồn tại, UseItem slot > 19. Tất cả đều xử lý êm đẹp, server hoạt động ổn định 100%.
4. **Client Godot 4**:
   - [client/server_packet.gd](file:///d:/Deep-Night/client/server_packet.gd): Giải mã an toàn `PlayerStats`, `Inventory`, và `combat_events` trong `Snapshot`.
   - [client/world.gd](file:///d:/Deep-Night/client/world.gd):
     - Thanh máu HP và cấp độ trên đầu người chơi.
     - Số nhảy sát thương bay lên và mờ dần trong 0.8s (`Floating Combat Text`).
     - Cơ chế nhắm mục tiêu quái vật tự động hoặc theo cự ly gần (`get_nearest_monster`).
   - [client/main.gd](file:///d:/Deep-Night/client/main.gd):
     - HUD hiển thị thanh máu HP (đỏ/xanh), thanh EXP, cấp độ, chỉ số Công/Thủ.
     - Quickbar 6 ô hiển thị tên và số lượng vật phẩm trong túi đồ.
     - Bấm phím `Space` hoặc chuột để tấn công quái trong tầm 60px.
     - Bấm phím số `1`-`6` hoặc nhấp chuột vào ô để sử dụng vật phẩm tức thì.
5. **Kết quả kiểm thử tự động**:
   - Rust Workspace: **14/14 tests PASSED** (`cargo test --workspace`).
   - Godot Client Headless: **3/3 tests PASSED** (`Godot_v4.4.1-stable_win64_console.exe --headless`).
   - Clippy & Fmt: **CLEAN** (`cargo clippy --workspace --tests -- -D warnings`, `cargo fmt --all -- --check`).

## Yêu cầu cho agent kia
- Không có. Cả hai phía Server và Client đã hoàn thành đồng bộ Bước 3.

## Bẫy đã gặp
- Gói `Welcome` phải luôn là gói tin đầu tiên trả về sau khi nhận `Join` trước khi gửi `PlayerStats` và `Inventory`, tránh làm lệch thứ tự phân tích gói tin của bot và client.
- Trên luồng Snapshot liên tục 20Hz (50ms/lần), không dùng `while let Ok(Some(...)) = timeout(100ms, ...)` vì dữ liệu liên tục đến trước timeout dẫn đến vòng lặp vô tận; cần giới hạn số lần lặp cụ thể `for _ in 0..N`.
- Tiếp cận mục tiêu quái vật cần tính toán vector di chuyển theo từng trục để tránh người chơi đi quá đà (overshooting) ngoài tầm đánh cận chiến 60px.
