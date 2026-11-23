use serde::{Deserialize, Serialize};

/// Số phiên bản giao thức hiện tại
pub const PROTOCOL_VERSION: &str = "v0.1";

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

/// Tổng số lượng gói tin tối đa mỗi giây từ một client
pub const MAX_TOTAL_PACKETS_PER_SECOND: u32 = 60;

/// Thông tin trạng thái một người chơi trong bản tin Snapshot
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerSnapshot {
    pub id: u64,
    pub x: f32,
    pub y: f32,
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

/// Các gói tin gửi từ Client -> Server
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ClientMessage {
    Join { name: String },
    Input { seq: u64, dx: i8, dy: i8 },
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

        let joined = ServerMessage::PlayerJoined {
            id: 2,
            name: "Lancelot".to_string(),
        };
        let json_joined = joined.to_json().unwrap();
        assert_eq!(
            json_joined,
            r#"{"type":"PlayerJoined","id":2,"name":"Lancelot"}"#
        );

        let left = ServerMessage::PlayerLeft { id: 2 };
        let json_left = left.to_json().unwrap();
        assert_eq!(json_left, r#"{"type":"PlayerLeft","id":2}"#);

        let snapshot = ServerMessage::Snapshot {
            tick: 100,
            players: vec![
                PlayerSnapshot {
                    id: 1,
                    x: 10.0,
                    y: 20.5,
                },
                PlayerSnapshot {
                    id: 2,
                    x: 30.5,
                    y: 40.0,
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
        };
        let json_snapshot = snapshot.to_json().unwrap();
        let parsed_snapshot = ServerMessage::from_json(&json_snapshot).unwrap();
        assert_eq!(parsed_snapshot, snapshot);

        // Kiểm tra khả năng tương thích ngược (deserialize snapshot v0 không có monsters, npcs)
        let v0_json = r#"{"type":"Snapshot","tick":50,"players":[{"id":1,"x":10.0,"y":20.0}]}"#;
        let v0_parsed: ServerMessage = ServerMessage::from_json(v0_json).unwrap();
        match v0_parsed {
            ServerMessage::Snapshot {
                tick,
                players,
                monsters,
                npcs,
            } => {
                assert_eq!(tick, 50);
                assert_eq!(players.len(), 1);
                assert!(monsters.is_empty());
                assert!(npcs.is_empty());
            }
            _ => panic!("Expected Snapshot"),
        }

        let err = ServerMessage::Error {
            message: "Loi ket noi".to_string(),
        };
        let json_err = err.to_json().unwrap();
        assert_eq!(json_err, r#"{"type":"Error","message":"Loi ket noi"}"#);
    }
}
