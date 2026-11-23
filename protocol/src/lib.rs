use serde::{Deserialize, Serialize};

/// Số phiên bản giao thức hiện tại
pub const PROTOCOL_VERSION: &str = "v0.2";

/// Tần số tick server (Hz)
pub const TICK_RATE: u32 = 20;

/// Tốc độ di chuyển người chơi (pixels/giây)
pub const PLAYER_MOVE_SPEED: f32 = 200.0;

/// Tọa độ giới hạn bản đồ
pub const MAP_MIN_X: f32 = 0.0;
pub const MAP_MAX_X: f32 = 1000.0;
pub const MAP_MIN_Y: f32 = 0.0;
pub const MAP_MAX_Y: f32 = 1000.0;

/// Vị trí xuất hiện mặc định của người chơi
pub const SPAWN_X: f32 = 500.0;
pub const SPAWN_Y: f32 = 500.0;

/// Kích thước gói tin văn bản WebSocket tối đa từ client (bytes)
pub const MAX_MESSAGE_SIZE: usize = 4096;

/// Số lượng gói Input tối đa mỗi giây từ một client
pub const MAX_INPUTS_PER_SECOND: u32 = 30;

/// Số lượng gói Attack tối đa mỗi giây từ một client
pub const MAX_ATTACKS_PER_SECOND: u32 = 5;

/// Số lượng gói UseItem tối đa mỗi giây từ một client
pub const MAX_USE_ITEMS_PER_SECOND: u32 = 3;

/// Tổng số lượng gói tin tối đa mỗi giây từ một client
pub const MAX_TOTAL_PACKETS_PER_SECOND: u32 = 60;

/// Tầm đánh của người chơi (pixels)
pub const PLAYER_ATTACK_RANGE: f32 = 60.0;

/// Hồi chiêu đòn đánh của người chơi (giây)
pub const PLAYER_ATTACK_COOLDOWN: f32 = 0.6;

/// Hồi chiêu sử dụng vật phẩm (giây)
pub const ITEM_USE_COOLDOWN: f32 = 1.0;

/// Số ô tối đa trong túi đồ
pub const MAX_INVENTORY_SLOTS: usize = 20;

/// Thời gian chờ hồi sinh sau khi tử vong (giây)
pub const RESPAWN_DELAY: f32 = 3.0;

fn default_player_hp() -> i32 {
    100
}

fn default_player_max_hp() -> i32 {
    100
}

fn default_player_level() -> u32 {
    1
}

/// Thông tin trạng thái một người chơi trong bản tin Snapshot
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerSnapshot {
    pub id: u64,
    pub x: f32,
    pub y: f32,
    #[serde(default = "default_player_hp")]
    pub hp: i32,
    #[serde(default = "default_player_max_hp")]
    pub max_hp: i32,
    #[serde(default = "default_player_level")]
    pub level: u32,
}

/// Thông tin trạng thái một quái vật trong bản tin Snapshot (v0.1)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MonsterSnapshot {
    pub id: u64,
    pub type_id: String,
    pub x: f32,
    pub y: f32,
    pub state: String,
    pub hp: i32,
    pub max_hp: i32,
}

/// Thông tin trạng thái một NPC trong bản tin Snapshot (v0.1)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NpcSnapshot {
    pub id: u64,
    pub type_id: String,
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub state: String,
}

/// Sự kiện sát thương chiến đấu trong Snapshot (v0.2)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CombatEventSnapshot {
    pub attacker_id: u64,
    pub target_id: u64,
    pub damage: i32,
    #[serde(default)]
    pub is_crit: bool,
    pub target_hp: i32,
}

/// Ô vật phẩm trong túi đồ (v0.2)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InventorySlot {
    pub slot: u32,
    pub item_id: String,
    pub count: u32,
}

/// Các gói tin gửi từ Client -> Server
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ClientMessage {
    Join { name: String },
    Input { seq: u64, dx: i8, dy: i8 },
    Attack { seq: u64, target_id: u64 },
    UseItem { slot: u32 },
}

impl ClientMessage {
    /// Chuyển đổi sang chuỗi JSON
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Phân tích cú pháp chuỗi JSON thành ClientMessage
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Kiểm tra tính hợp lệ nghiệp vụ của gói tin Client
    pub fn is_valid(&self) -> bool {
        match self {
            ClientMessage::Join { name } => {
                let trimmed = name.trim();
                let count = trimmed.chars().count();
                // AUTH: Tên người chơi từ 1-20 ký tự, không chứa ký tự điều khiển
                (1..=20).contains(&count) && !name.chars().any(|c| c.is_control())
            }
            ClientMessage::Input { dx, dy, .. } => {
                // AUTH: Hướng di chuyển dx, dy bắt buộc chỉ trong {-1, 0, 1}
                (-1..=1).contains(dx) && (-1..=1).contains(dy)
            }
            ClientMessage::Attack { target_id, .. } => {
                // AUTH: target_id phải hợp lệ (> 0)
                *target_id > 0
            }
            ClientMessage::UseItem { slot } => {
                // AUTH: slot nằm trong khoảng 0..MAX_INVENTORY_SLOTS
                (*slot as usize) < MAX_INVENTORY_SLOTS
            }
        }
    }
}

/// Các gói tin gửi từ Server -> Client
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ServerMessage {
    Welcome {
        player_id: u64,
        tick_rate: u32,
        map: String,
    },
    PlayerJoined {
        id: u64,
        name: String,
    },
    PlayerLeft {
        id: u64,
    },
    Snapshot {
        tick: u64,
        players: Vec<PlayerSnapshot>,
        #[serde(default)]
        monsters: Vec<MonsterSnapshot>,
        #[serde(default)]
        npcs: Vec<NpcSnapshot>,
        #[serde(default)]
        combat_events: Vec<CombatEventSnapshot>,
    },
    PlayerStats {
        level: u32,
        exp: u32,
        max_exp: u32,
        hp: i32,
        max_hp: i32,
        attack_power: i32,
        defense: i32,
    },
    Inventory {
        slots: Vec<InventorySlot>,
    },
    Error {
        message: String,
    },
}

impl ServerMessage {
    /// Chuyển đổi sang chuỗi JSON
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Phân tích cú pháp chuỗi JSON thành ServerMessage
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_join_serialization() {
        let msg = ClientMessage::Join {
            name: "Arthur".to_string(),
        };
        let json = msg.to_json().unwrap();
        assert_eq!(json, r#"{"type":"Join","name":"Arthur"}"#);

        let parsed = ClientMessage::from_json(&json).unwrap();
        assert_eq!(parsed, msg);
        assert!(parsed.is_valid());
    }

    #[test]
    fn test_client_join_validation() {
        let valid = ClientMessage::Join {
            name: "Lancelot".to_string(),
        };
        assert!(valid.is_valid());

        let empty = ClientMessage::Join {
            name: "   ".to_string(),
        };
        assert!(!empty.is_valid());

        let too_long = ClientMessage::Join {
            name: "A".repeat(21),
        };
        assert!(!too_long.is_valid());

        let control_char = ClientMessage::Join {
            name: "Arthur\n".to_string(),
        };
        assert!(!control_char.is_valid());
    }

    #[test]
    fn test_client_input_serialization() {
        let msg = ClientMessage::Input {
            seq: 10,
            dx: 1,
            dy: -1,
        };
        let json = msg.to_json().unwrap();
        assert_eq!(json, r#"{"type":"Input","seq":10,"dx":1,"dy":-1}"#);

        let parsed = ClientMessage::from_json(&json).unwrap();
        assert_eq!(parsed, msg);
        assert!(parsed.is_valid());
    }

    #[test]
    fn test_client_input_validation() {
        let valid = ClientMessage::Input {
            seq: 1,
            dx: 0,
            dy: 1,
        };
        assert!(valid.is_valid());

        let invalid_dx = ClientMessage::Input {
            seq: 2,
            dx: 2,
            dy: 0,
        };
        assert!(!invalid_dx.is_valid());

        let invalid_dy = ClientMessage::Input {
            seq: 3,
            dx: 0,
            dy: -2,
        };
        assert!(!invalid_dy.is_valid());
    }

    #[test]
    fn test_client_attack_and_use_item() {
        let attack = ClientMessage::Attack {
            seq: 5,
            target_id: 10001,
        };
        let json_attack = attack.to_json().unwrap();
        assert_eq!(
            json_attack,
            r#"{"type":"Attack","seq":5,"target_id":10001}"#
        );
        let parsed_attack = ClientMessage::from_json(&json_attack).unwrap();
        assert_eq!(parsed_attack, attack);
        assert!(parsed_attack.is_valid());

        let invalid_attack = ClientMessage::Attack {
            seq: 6,
            target_id: 0,
        };
        assert!(!invalid_attack.is_valid());

        let use_item = ClientMessage::UseItem { slot: 0 };
        let json_use = use_item.to_json().unwrap();
        assert_eq!(json_use, r#"{"type":"UseItem","slot":0}"#);
        let parsed_use = ClientMessage::from_json(&json_use).unwrap();
        assert_eq!(parsed_use, use_item);
        assert!(parsed_use.is_valid());

        let invalid_use = ClientMessage::UseItem { slot: 20 };
        assert!(!invalid_use.is_valid());
    }

    #[test]
    fn test_server_messages_serialization() {
        let welcome = ServerMessage::Welcome {
            player_id: 1,
            tick_rate: 20,
            map: "starter_village".to_string(),
        };
        let json_welcome = welcome.to_json().unwrap();
        assert_eq!(
            json_welcome,
            r#"{"type":"Welcome","player_id":1,"tick_rate":20,"map":"starter_village"}"#
        );
        let parsed_welcome = ServerMessage::from_json(&json_welcome).unwrap();
        assert_eq!(parsed_welcome, welcome);

        let stats = ServerMessage::PlayerStats {
            level: 2,
            exp: 50,
            max_exp: 200,
            hp: 120,
            max_hp: 120,
            attack_power: 18,
            defense: 5,
        };
        let json_stats = stats.to_json().unwrap();
        let parsed_stats = ServerMessage::from_json(&json_stats).unwrap();
        assert_eq!(parsed_stats, stats);

        let inv = ServerMessage::Inventory {
            slots: vec![
                InventorySlot {
                    slot: 0,
                    item_id: "potion_hp".to_string(),
                    count: 3,
                },
                InventorySlot {
                    slot: 1,
                    item_id: "goblin_ear".to_string(),
                    count: 5,
                },
            ],
        };
        let json_inv = inv.to_json().unwrap();
        let parsed_inv = ServerMessage::from_json(&json_inv).unwrap();
        assert_eq!(parsed_inv, inv);

        let snapshot = ServerMessage::Snapshot {
            tick: 100,
            players: vec![
                PlayerSnapshot {
                    id: 1,
                    x: 10.0,
                    y: 20.5,
                    hp: 100,
                    max_hp: 100,
                    level: 1,
                },
                PlayerSnapshot {
                    id: 2,
                    x: 30.5,
                    y: 40.0,
                    hp: 80,
                    max_hp: 100,
                    level: 1,
                },
            ],
            monsters: vec![MonsterSnapshot {
                id: 10001,
                type_id: "goblin".to_string(),
                x: 100.0,
                y: 100.0,
                state: "Patrol".to_string(),
                hp: 50,
                max_hp: 50,
            }],
            npcs: vec![NpcSnapshot {
                id: 20001,
                type_id: "village_elder".to_string(),
                name: "Trưởng làng".to_string(),
                x: 520.0,
                y: 510.0,
                state: "Idle".to_string(),
            }],
            combat_events: vec![CombatEventSnapshot {
                attacker_id: 1,
                target_id: 10001,
                damage: 15,
                is_crit: false,
                target_hp: 35,
            }],
        };
        let json_snapshot = snapshot.to_json().unwrap();
        let parsed_snapshot = ServerMessage::from_json(&json_snapshot).unwrap();
        assert_eq!(parsed_snapshot, snapshot);

        // Kiểm tra khả năng tương thích ngược (deserialize snapshot v0 không có monsters, npcs, combat_events, hp, max_hp, level)
        let v0_json = r#"{"type":"Snapshot","tick":50,"players":[{"id":1,"x":10.0,"y":20.0}]}"#;
        let v0_parsed: ServerMessage = ServerMessage::from_json(v0_json).unwrap();
        match v0_parsed {
            ServerMessage::Snapshot {
                tick,
                players,
                monsters,
                npcs,
                combat_events,
            } => {
                assert_eq!(tick, 50);
                assert_eq!(players.len(), 1);
                assert_eq!(players[0].hp, 100);
                assert_eq!(players[0].level, 1);
                assert!(monsters.is_empty());
                assert!(npcs.is_empty());
                assert!(combat_events.is_empty());
            }
            _ => panic!("Expected Snapshot"),
        }
    }
}
