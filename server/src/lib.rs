use futures_util::{SinkExt, StreamExt};
use protocol::{
    ClientMessage, CombatEventSnapshot, InventorySlot, PlayerSnapshot, ServerMessage,
    ITEM_USE_COOLDOWN, MAP_MAX_X, MAP_MAX_Y, MAP_MIN_X, MAP_MIN_Y, MAX_ATTACKS_PER_SECOND,
    MAX_INPUTS_PER_SECOND, MAX_INVENTORY_SLOTS, MAX_MESSAGE_SIZE, MAX_TOTAL_PACKETS_PER_SECOND,
    MAX_USE_ITEMS_PER_SECOND, PLAYER_ATTACK_COOLDOWN, PLAYER_ATTACK_RANGE, PLAYER_MOVE_SPEED,
    RESPAWN_DELAY, SPAWN_X, SPAWN_Y, TICK_RATE,
};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot};
use tokio::time::Instant;
use tokio_tungstenite::tungstenite;

pub mod entities;
pub mod items;
use entities::EntityManager;
use items::ItemRegistry;

pub type JoinResponse = Result<(u64, Vec<(u64, String)>), String>;

/// Các sự kiện gửi từ task mạng tới game loop
pub enum GameEvent {
    Connect {
        name: String,
        out_tx: mpsc::UnboundedSender<ServerMessage>,
        resp_tx: oneshot::Sender<JoinResponse>,
    },
    Input {
        player_id: u64,
        seq: u64,
        dx: i8,
        dy: i8,
    },
    Attack {
        player_id: u64,
        seq: u64,
        target_id: u64,
    },
    UseItem {
        player_id: u64,
        slot: u32,
    },
    Disconnect {
        player_id: u64,
    },
}

/// Một ô chứa đồ trong túi của người chơi
#[derive(Debug, Clone)]
pub struct InventorySlotEntry {
    pub item_id: String,
    pub count: u32,
}

/// Dữ liệu một người chơi được lưu trữ trong game loop
struct PlayerData {
    id: u64,
    name: String,
    x: f32,
    y: f32,
    last_dx: i8,
    last_dy: i8,
    last_seq: u64,
    // Thuộc tính chiến đấu & tiến trình (v0.2)
    hp: i32,
    max_hp: i32,
    level: u32,
    exp: u32,
    max_exp: u32,
    attack_power: i32,
    defense: i32,
    attack_cooldown: f32,
    item_cooldown: f32,
    respawn_timer: Option<f32>,
    inventory: Vec<Option<InventorySlotEntry>>,
    out_tx: mpsc::UnboundedSender<ServerMessage>,
}

impl PlayerData {
    pub fn new(id: u64, name: String, out_tx: mpsc::UnboundedSender<ServerMessage>) -> Self {
        let mut inventory = vec![None; MAX_INVENTORY_SLOTS];
        // Tặng sẵn 3 bình máu nhỏ cho người chơi mới
        inventory[0] = Some(InventorySlotEntry {
            item_id: "potion_hp".to_string(),
            count: 3,
        });

        Self {
            id,
            name,
            x: SPAWN_X,
            y: SPAWN_Y,
            last_dx: 0,
            last_dy: 0,
            last_seq: 0,
            hp: 100,
            max_hp: 100,
            level: 1,
            exp: 0,
            max_exp: 100,
            attack_power: 15,
            defense: 2,
            attack_cooldown: 0.0,
            item_cooldown: 0.0,
            respawn_timer: None,
            inventory,
            out_tx,
        }
    }

    pub fn to_snapshot(&self) -> PlayerSnapshot {
        PlayerSnapshot {
            id: self.id,
            x: self.x,
            y: self.y,
            hp: self.hp,
            max_hp: self.max_hp,
            level: self.level,
        }
    }

    pub fn to_player_stats(&self) -> ServerMessage {
        ServerMessage::PlayerStats {
            level: self.level,
            exp: self.exp,
            max_exp: self.max_exp,
            hp: self.hp,
            max_hp: self.max_hp,
            attack_power: self.attack_power,
            defense: self.defense,
        }
    }

    pub fn to_inventory_message(&self) -> ServerMessage {
        let mut slots = Vec::new();
        for (i, entry_opt) in self.inventory.iter().enumerate() {
            if let Some(entry) = entry_opt {
                slots.push(InventorySlot {
                    slot: i as u32,
                    item_id: entry.item_id.clone(),
                    count: entry.count,
                });
            }
        }
        ServerMessage::Inventory { slots }
    }

    /// Thêm kinh nghiệm và kiểm tra thăng cấp
    pub fn add_exp(&mut self, gained: u32) -> bool {
        self.exp += gained;
        let mut leveled_up = false;
        while self.exp >= self.max_exp {
            self.level += 1;
            self.exp -= self.max_exp;
            self.max_exp = self.level * 100;
            self.max_hp += 20;
            self.attack_power += 3;
            self.hp = self.max_hp; // Hồi đầy máu khi lên cấp
            leveled_up = true;
        }
        leveled_up
    }

    /// Thêm vật phẩm vào túi đồ (ưu tiên stack vào ô có sẵn, sau đó đưa vào ô trống đầu tiên)
    pub fn add_item(&mut self, item_id: &str, mut count: u32, registry: &ItemRegistry) -> bool {
        let max_stack = registry.get(item_id).map(|t| t.max_stack).unwrap_or(99);

        // 1. Tìm các ô đã có cùng loại vật phẩm và chưa đầy stack
        for slot in self.inventory.iter_mut().flatten() {
            if slot.item_id == item_id && slot.count < max_stack {
                let space = max_stack - slot.count;
                let add_now = space.min(count);
                slot.count += add_now;
                count -= add_now;
                if count == 0 {
                    return true;
                }
            }
        }

        // 2. Tìm ô trống đầu tiên
        for slot in self.inventory.iter_mut() {
            if slot.is_none() {
                let add_now = max_stack.min(count);
                *slot = Some(InventorySlotEntry {
                    item_id: item_id.to_string(),
                    count: add_now,
                });
                count -= add_now;
                if count == 0 {
                    return true;
                }
            }
        }

        count == 0
    }
}

/// Chạy game loop tại tần số 20Hz, hoàn toàn tách biệt với I/O mạng
pub async fn run_game_loop(mut event_rx: mpsc::UnboundedReceiver<GameEvent>) {
    let mut tick: u64 = 0;
    let mut next_player_id: u64 = 1;
    let mut players: HashMap<u64, PlayerData> = HashMap::new();
    let mut entity_manager = EntityManager::load_from_data_or_defaults();
    let item_registry = ItemRegistry::load_from_data_or_defaults();
    let mut pending_combat_events: Vec<CombatEventSnapshot> = Vec::new();

    let tick_duration = Duration::from_millis(1000 / TICK_RATE as u64); // 50ms (20Hz)
    let mut interval = tokio::time::interval(tick_duration);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            _ = interval.tick() => {
                tick += 1;
                let dt = 1.0 / (TICK_RATE as f32);

                // 1. Gom tất cả sự kiện mạng đang chờ trong channel
                while let Ok(event) = event_rx.try_recv() {
                    handle_game_event(
                        event,
                        &mut next_player_id,
                        &mut players,
                        &mut entity_manager,
                        &item_registry,
                        &mut pending_combat_events,
                    );
                }

                // 2. Cập nhật vị trí & cooldown của tất cả người chơi
                for player in players.values_mut() {
                    player.attack_cooldown = (player.attack_cooldown - dt).max(0.0);
                    player.item_cooldown = (player.item_cooldown - dt).max(0.0);

                    // Xử lý tử vong và hồi sinh
                    if let Some(ref mut timer) = player.respawn_timer {
                        *timer -= dt;
                        if *timer <= 0.0 {
                            player.respawn_timer = None;
                            player.hp = player.max_hp;
                            player.x = SPAWN_X;
                            player.y = SPAWN_Y;
                            player.last_dx = 0;
                            player.last_dy = 0;
                            let _ = player.out_tx.send(player.to_player_stats());
                        }
                        continue; // Người chơi đang chết không thể di chuyển
                    }

                    let dx = player.last_dx;
                    let dy = player.last_dy;
                    if dx != 0 || dy != 0 {
                        let (norm_dx, norm_dy) = if dx != 0 && dy != 0 {
                            let factor = 1.0 / std::f32::consts::SQRT_2;
                            (dx as f32 * factor, dy as f32 * factor)
                        } else {
                            (dx as f32, dy as f32)
                        };

                        let new_x = player.x + norm_dx * PLAYER_MOVE_SPEED * dt;
                        let new_y = player.y + norm_dy * PLAYER_MOVE_SPEED * dt;

                        // AUTH: Kẹp vị trí người chơi nghiêm ngặt trong biên map
                        player.x = new_x.clamp(MAP_MIN_X, MAP_MAX_X);
                        player.y = new_y.clamp(MAP_MIN_Y, MAP_MAX_Y);
                    }
                }

                // 3. Cập nhật State Machine cho quái vật và NPC
                // Chỉ truyền tọa độ của những người chơi còn sống
                let player_positions: Vec<(u64, f32, f32)> = players
                    .values()
                    .filter(|p| p.hp > 0 && p.respawn_timer.is_none())
                    .map(|p| (p.id, p.x, p.y))
                    .collect();

                let monster_attacks = entity_manager.tick(dt, &player_positions);

                // Xử lý quái vật tấn công người chơi
                for atk in monster_attacks {
                    if let Some(player) = players.get_mut(&atk.target_player_id) {
                        if player.hp > 0 && player.respawn_timer.is_none() {
                            let damage = (atk.attack_power - player.defense).max(1);
                            player.hp = (player.hp - damage).max(0);

                            pending_combat_events.push(CombatEventSnapshot {
                                attacker_id: atk.monster_id,
                                target_id: player.id,
                                damage,
                                is_crit: false,
                                target_hp: player.hp,
                            });

                            let _ = player.out_tx.send(player.to_player_stats());

                            if player.hp == 0 {
                                player.respawn_timer = Some(RESPAWN_DELAY);
                                player.last_dx = 0;
                                player.last_dy = 0;
                            }
                        }
                    }
                }

                // 4. Tạo Snapshot và broadcast tới tất cả client đang kết nối
                let snapshot_players: Vec<PlayerSnapshot> = players
                    .values()
                    .map(|p| p.to_snapshot())
                    .collect();

                let (snapshot_monsters, snapshot_npcs) = entity_manager.get_snapshots();

                let combat_events_this_tick = std::mem::take(&mut pending_combat_events);

                let snapshot = ServerMessage::Snapshot {
                    tick,
                    players: snapshot_players,
                    monsters: snapshot_monsters,
                    npcs: snapshot_npcs,
                    combat_events: combat_events_this_tick,
                };

                let mut disconnected_ids = Vec::new();
                for (id, player) in &players {
                    if player.out_tx.send(snapshot.clone()).is_err() {
                        disconnected_ids.push(*id);
                    }
                }

                // Dọn dẹp các kết nối bị đóng đột ngột
                for id in disconnected_ids {
                    if let Some(removed) = players.remove(&id) {
                        tracing::info!("Nguoi choi '{}' (id: {}) mat ket noi dot ngot", removed.name, id);
                        let left_msg = ServerMessage::PlayerLeft { id };
                        for p in players.values() {
                            let _ = p.out_tx.send(left_msg.clone());
                        }
                    }
                }
            }
            Some(event) = event_rx.recv() => {
                handle_game_event(
                    event,
                    &mut next_player_id,
                    &mut players,
                    &mut entity_manager,
                    &item_registry,
                    &mut pending_combat_events,
                );
            }
            else => {
                tracing::info!("Game event channel da dong, dung game loop");
                break;
            }
        }
    }
}

/// Xử lý từng sự kiện gửi tới game loop
fn handle_game_event(
    event: GameEvent,
    next_player_id: &mut u64,
    players: &mut HashMap<u64, PlayerData>,
    entity_manager: &mut EntityManager,
    item_registry: &ItemRegistry,
    pending_combat_events: &mut Vec<CombatEventSnapshot>,
) {
    match event {
        GameEvent::Connect {
            name,
            out_tx,
            resp_tx,
        } => {
            // AUTH: Kiem tra trung lap ten nguoi choi dang online
            let name_exists = players.values().any(|p| p.name.eq_ignore_ascii_case(&name));
            if name_exists {
                let _ = resp_tx.send(Err("Ten nguoi choi da ton tai".to_string()));
                return;
            }

            let player_id = *next_player_id;
            *next_player_id += 1;

            // Danh sách người chơi hiện có để gửi cho người mới
            let online_list: Vec<(u64, String)> =
                players.values().map(|p| (p.id, p.name.clone())).collect();

            // Thông báo PlayerJoined cho tất cả người chơi khác
            let joined_msg = ServerMessage::PlayerJoined {
                id: player_id,
                name: name.clone(),
            };
            for p in players.values() {
                let _ = p.out_tx.send(joined_msg.clone());
            }

            // Tạo người chơi mới
            let new_player = PlayerData::new(player_id, name.clone(), out_tx.clone());
            players.insert(player_id, new_player);

            tracing::info!("Nguoi choi '{}' da tham gia voi id: {}", name, player_id);
            let _ = resp_tx.send(Ok((player_id, online_list)));
        }
        GameEvent::Input {
            player_id,
            seq,
            dx,
            dy,
        } => {
            if let Some(player) = players.get_mut(&player_id) {
                // AUTH: Kiem tra seq moi nhat, bo qua input cu hon da xu ly
                // Khong cho di chuyen khi dang cho hoi sinh
                if seq >= player.last_seq && player.respawn_timer.is_none() {
                    player.last_seq = seq;
                    player.last_dx = dx;
                    player.last_dy = dy;
                }
            }
        }
        GameEvent::Attack {
            player_id,
            target_id,
            ..
        } => {
            if let Some(player) = players.get_mut(&player_id) {
                // AUTH: Nguoi choi phai con song va het thoi gian hoi chieu danh
                if player.hp <= 0 || player.respawn_timer.is_some() || player.attack_cooldown > 0.0
                {
                    return;
                }

                // AUTH: Tim muc tieu quai vat
                let monster_opt = entity_manager
                    .monsters
                    .iter_mut()
                    .find(|m| m.id == target_id);

                if let Some(monster) = monster_opt {
                    // AUTH: Quai vat phai con song
                    if monster.hp <= 0 {
                        return;
                    }

                    // AUTH: Khoang cach tan cong phai <= PLAYER_ATTACK_RANGE
                    let dist = (monster.x - player.x).hypot(monster.y - player.y);
                    if dist > PLAYER_ATTACK_RANGE {
                        return;
                    }

                    // Thoa man dieu kien tan cong: dat cooldown
                    player.attack_cooldown = PLAYER_ATTACK_COOLDOWN;

                    // Tinh toan sat thuong: damage = max(1, player_atk - monster_def)
                    let damage = (player.attack_power - monster.defense).max(1);
                    let killed = monster.apply_damage(player.id, damage);

                    pending_combat_events.push(CombatEventSnapshot {
                        attacker_id: player.id,
                        target_id: monster.id,
                        damage,
                        is_crit: false,
                        target_hp: monster.hp,
                    });

                    if killed {
                        // Nhan kinh nghiem va kiem tra thang cap
                        let leveled_up = player.add_exp(monster.exp_reward);

                        // Roi vat pham theo ty le
                        let drops = monster.roll_drops();
                        let mut inventory_changed = false;
                        for (item_id, count) in drops {
                            if player.add_item(&item_id, count, item_registry) {
                                inventory_changed = true;
                            }
                        }

                        if inventory_changed {
                            let _ = player.out_tx.send(player.to_inventory_message());
                        }

                        if leveled_up || monster.exp_reward > 0 {
                            let _ = player.out_tx.send(player.to_player_stats());
                        }
                    }
                }
            }
        }
        GameEvent::UseItem { player_id, slot } => {
            if let Some(player) = players.get_mut(&player_id) {
                // AUTH: Nguoi choi phai con song va het cooldown dung vat pham
                if player.hp <= 0 || player.respawn_timer.is_some() || player.item_cooldown > 0.0 {
                    return;
                }

                let slot_idx = slot as usize;
                // AUTH: Slot phai hop le trong pham vi [0, MAX_INVENTORY_SLOTS - 1]
                if slot_idx >= MAX_INVENTORY_SLOTS {
                    return;
                }

                if let Some(ref mut entry) = player.inventory[slot_idx] {
                    if let Some(tmpl) = item_registry.get(&entry.item_id) {
                        if tmpl.item_type == "consumable" && tmpl.heal_amount > 0 {
                            player.item_cooldown = ITEM_USE_COOLDOWN;
                            player.hp = (player.hp + tmpl.heal_amount).min(player.max_hp);

                            entry.count -= 1;
                            let empty = entry.count == 0;
                            if empty {
                                player.inventory[slot_idx] = None;
                            }

                            let _ = player.out_tx.send(player.to_inventory_message());
                            let _ = player.out_tx.send(player.to_player_stats());
                        }
                    }
                }
            }
        }
        GameEvent::Disconnect { player_id } => {
            if let Some(removed) = players.remove(&player_id) {
                tracing::info!(
                    "Nguoi choi '{}' (id: {}) da ngat ket noi",
                    removed.name,
                    player_id
                );
                let left_msg = ServerMessage::PlayerLeft { id: player_id };
                for p in players.values() {
                    let _ = p.out_tx.send(left_msg.clone());
                }
            }
        }
    }
}

/// Xử lý một kết nối WebSocket từ client
pub async fn handle_connection(
    stream: TcpStream,
    peer_addr: SocketAddr,
    game_tx: mpsc::UnboundedSender<GameEvent>,
) {
    let ws_stream = match tokio_tungstenite::accept_async(stream).await {
        Ok(ws) => ws,
        Err(e) => {
            tracing::debug!("WebSocket handshake that bai tu {}: {}", peer_addr, e);
            return;
        }
    };

    let (mut ws_sink, mut ws_reader) = ws_stream.split();
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<ServerMessage>();

    // Task phụ trợ ghi gói tin tới WebSocket client
    let writer_task = tokio::spawn(async move {
        while let Some(msg) = out_rx.recv().await {
            let close_reason = if let ServerMessage::Error { ref message } = msg {
                Some(message.clone())
            } else {
                None
            };

            match msg.to_json() {
                Ok(json) => {
                    if ws_sink
                        .send(tungstenite::Message::Text(json.into()))
                        .await
                        .is_err()
                    {
                        break;
                    }

                    if let Some(reason) = close_reason {
                        let close_frame = tungstenite::protocol::CloseFrame {
                            code: tungstenite::protocol::frame::coding::CloseCode::Policy,
                            reason: reason.into(),
                        };
                        let _ = ws_sink
                            .send(tungstenite::Message::Close(Some(close_frame)))
                            .await;
                        break;
                    }
                }
                Err(e) => {
                    tracing::error!("Khong the serialize ServerMessage: {}", e);
                }
            }
        }
        let _ = ws_sink.close().await;
    });

    // 1. Chờ gói Join đầu tiên với timeout 5 giây
    let join_timeout = Duration::from_secs(5);
    let first_msg_result = tokio::time::timeout(join_timeout, ws_reader.next()).await;

    let raw_msg = match first_msg_result {
        Ok(Some(Ok(m))) => m,
        Ok(Some(Err(e))) => {
            tracing::debug!("Loi doc goi tin dau tien tu {}: {}", peer_addr, e);
            return;
        }
        Ok(None) => return,
        Err(_) => {
            // AUTH: Timeout 5 giay khong gui goi Join -> ngat ket noi
            tracing::debug!("Timeout 5 giay cho Join tu {}", peer_addr);
            let _ = out_tx.send(ServerMessage::Error {
                message: "Timeout cho goi Join".to_string(),
            });
            drop(out_tx);
            let _ = writer_task.await;
            return;
        }
    };

    // AUTH: Kiem tra kich thuoc goi tin khong vuot qua MAX_MESSAGE_SIZE (4096 bytes)
    let text = match raw_msg {
        tungstenite::Message::Text(t) => {
            if t.len() > MAX_MESSAGE_SIZE {
                // AUTH: Kich thuoc vuot qua gioi han, tu choi va ngat ket noi
                let _ = out_tx.send(ServerMessage::Error {
                    message: "Goi tin vuot qua kich thuoc cho phep".to_string(),
                });
                drop(out_tx);
                let _ = writer_task.await;
                return;
            }
            t.to_string()
        }
        _ => return,
    };

    // AUTH: Kiem tra cu phap JSON hop le
    let client_msg = match ClientMessage::from_json(&text) {
        Ok(m) => m,
        Err(_) => {
            // AUTH: Du lieu khong phai JSON hop le
            let _ = out_tx.send(ServerMessage::Error {
                message: "Du lieu khong phai JSON hop le".to_string(),
            });
            drop(out_tx);
            let _ = writer_task.await;
            return;
        }
    };

    let player_name = match client_msg {
        ClientMessage::Join { name } => {
            let trimmed = name.trim();
            let count = trimmed.chars().count();
            // AUTH: Kiem tra do dai ten tu 1-20 va khong chua ky tu dieu khien
            if !(1..=20).contains(&count) || name.chars().any(|c| c.is_control()) {
                let _ = out_tx.send(ServerMessage::Error {
                    message: "Ten nguoi choi khong hop le".to_string(),
                });
                drop(out_tx);
                let _ = writer_task.await;
                return;
            }
            trimmed.to_string()
        }
        _ => {
            // AUTH: Client gui goi khac khi chua Join
            let _ = out_tx.send(ServerMessage::Error {
                message: "Phai Join truoc khi gui goi tin khac".to_string(),
            });
            drop(out_tx);
            let _ = writer_task.await;
            return;
        }
    };

    // Gửi yêu cầu đăng ký người chơi tới game loop
    let (resp_tx, resp_rx) = oneshot::channel();
    if game_tx
        .send(GameEvent::Connect {
            name: player_name.clone(),
            out_tx: out_tx.clone(),
            resp_tx,
        })
        .is_err()
    {
        return;
    }

    let (player_id, online_players) = match resp_rx.await {
        Ok(Ok(info)) => info,
        Ok(Err(err_msg)) => {
            // AUTH: Ten nguoi choi da ton tai, tu choi bang Error
            let _ = out_tx.send(ServerMessage::Error { message: err_msg });
            drop(out_tx);
            let _ = writer_task.await;
            return;
        }
        Err(_) => return,
    };

    // Gửi Welcome cho người chơi mới
    let _ = out_tx.send(ServerMessage::Welcome {
        player_id,
        tick_rate: TICK_RATE,
        map: "starter_village".to_string(),
    });

    // Gửi danh sách người chơi hiện có cho người chơi mới
    for (existing_id, existing_name) in online_players {
        let _ = out_tx.send(ServerMessage::PlayerJoined {
            id: existing_id,
            name: existing_name,
        });
    }

    // Gửi PlayerStats và Inventory ban đầu
    let _ = out_tx.send(ServerMessage::PlayerStats {
        level: 1,
        exp: 0,
        max_exp: 100,
        hp: 100,
        max_hp: 100,
        attack_power: 15,
        defense: 2,
    });
    let _ = out_tx.send(ServerMessage::Inventory {
        slots: vec![InventorySlot {
            slot: 0,
            item_id: "potion_hp".to_string(),
            count: 3,
        }],
    });

    // 2. Vòng lặp nhận gói tin sau khi đã Join thành công
    let mut window_start = Instant::now();
    let mut input_count: u32 = 0;
    let mut attack_count: u32 = 0;
    let mut use_item_count: u32 = 0;
    let mut total_count: u32 = 0;

    while let Some(msg_result) = ws_reader.next().await {
        let msg = match msg_result {
            Ok(m) => m,
            Err(_) => break, // Lỗi mạng hoặc client đóng kết nối
        };

        let text = match msg {
            tungstenite::Message::Text(t) => {
                // AUTH: Kiem tra kich thuoc frame khong vuot qua MAX_MESSAGE_SIZE
                if t.len() > MAX_MESSAGE_SIZE {
                    let _ = out_tx.send(ServerMessage::Error {
                        message: "Goi tin vuot qua kich thuoc cho phep".to_string(),
                    });
                    break;
                }
                t.to_string()
            }
            tungstenite::Message::Close(_) => break,
            tungstenite::Message::Ping(_) | tungstenite::Message::Pong(_) => continue,
            _ => continue,
        };

        // Quản lý rate limit theo cửa sổ 1 giây
        let now = Instant::now();
        if now.duration_since(window_start) >= Duration::from_secs(1) {
            window_start = now;
            input_count = 0;
            attack_count = 0;
            use_item_count = 0;
            total_count = 0;
        }
        total_count += 1;

        // AUTH: Kiem tra tong so goi tin khong vuot qua 60 packets/s chong spam DoS
        if total_count > MAX_TOTAL_PACKETS_PER_SECOND {
            let _ = out_tx.send(ServerMessage::Error {
                message: "Vuot qua tan suat goi tin cho phep".to_string(),
            });
            break;
        }

        // AUTH: Kiem tra parse JSON hop le
        let parsed = match ClientMessage::from_json(&text) {
            Ok(p) => p,
            Err(_) => {
                // AUTH: Du lieu khong parse duoc JSON, bo qua khong lam sap server
                continue;
            }
        };

        match parsed {
            ClientMessage::Input { seq, dx, dy } => {
                input_count += 1;
                // AUTH: Kiem tra rate limit toi da 30 Input/giay
                if input_count > MAX_INPUTS_PER_SECOND {
                    continue;
                }
                // AUTH: Kiem tra huong di chuyen dx, dy phai thuoc tap {-1, 0, 1}
                if !(-1..=1).contains(&dx) || !(-1..=1).contains(&dy) {
                    continue;
                }
                let _ = game_tx.send(GameEvent::Input {
                    player_id,
                    seq,
                    dx,
                    dy,
                });
            }
            ClientMessage::Attack { seq, target_id } => {
                attack_count += 1;
                // AUTH: Kiem tra rate limit toi da 5 Attack/giay
                if attack_count > MAX_ATTACKS_PER_SECOND {
                    continue;
                }
                // AUTH: target_id phai hop le > 0
                if target_id == 0 {
                    continue;
                }
                let _ = game_tx.send(GameEvent::Attack {
                    player_id,
                    seq,
                    target_id,
                });
            }
            ClientMessage::UseItem { slot } => {
                use_item_count += 1;
                // AUTH: Kiem tra rate limit toi da 3 UseItem/giay
                if use_item_count > MAX_USE_ITEMS_PER_SECOND {
                    continue;
                }
                // AUTH: slot phai nam trong pham vi [0, MAX_INVENTORY_SLOTS - 1]
                if (slot as usize) >= MAX_INVENTORY_SLOTS {
                    continue;
                }
                let _ = game_tx.send(GameEvent::UseItem { player_id, slot });
            }
            ClientMessage::Join { .. } => {
                // AUTH: Client da Join thanh cong khong duoc phep Join lai, bo qua
                continue;
            }
        }
    }

    // Thông báo ngắt kết nối cho game loop
    let _ = game_tx.send(GameEvent::Disconnect { player_id });
    let _ = writer_task.await;
}

/// Khởi động server lắng nghe tại địa chỉ chỉ định
pub async fn start_server(
    addr: &str,
    mut shutdown_rx: Option<tokio::sync::broadcast::Receiver<()>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let listener = TcpListener::bind(addr).await?;
    let local_addr = listener.local_addr()?;
    tracing::info!("Server Mini-MMORPG dang lang nghe tai: {}", local_addr);

    let (game_tx, game_rx) = mpsc::unbounded_channel::<GameEvent>();

    // Khởi chạy game loop 20Hz trên task riêng biệt
    let game_loop_handle = tokio::spawn(async move {
        run_game_loop(game_rx).await;
    });

    loop {
        if let Some(ref mut rx) = shutdown_rx {
            tokio::select! {
                accept_res = listener.accept() => {
                    match accept_res {
                        Ok((stream, peer_addr)) => {
                            let tx = game_tx.clone();
                            tokio::spawn(async move {
                                handle_connection(stream, peer_addr, tx).await;
                            });
                        }
                        Err(e) => {
                            tracing::warn!("Loi chap nhan ket noi: {}", e);
                        }
                    }
                }
                _ = rx.recv() => {
                    tracing::info!("Nhan tin hieu dung server");
                    break;
                }
            }
        } else {
            match listener.accept().await {
                Ok((stream, peer_addr)) => {
                    let tx = game_tx.clone();
                    tokio::spawn(async move {
                        handle_connection(stream, peer_addr, tx).await;
                    });
                }
                Err(e) => {
                    tracing::warn!("Loi chap nhan ket noi: {}", e);
                }
            }
        }
    }

    drop(game_tx);
    let _ = game_loop_handle.await;
    Ok(())
}
