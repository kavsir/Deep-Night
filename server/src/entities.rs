use protocol::{MonsterSnapshot, NpcSnapshot, MAP_MAX_X, MAP_MAX_Y, MAP_MIN_X, MAP_MIN_Y};
use serde::Deserialize;
use std::fs;
use std::path::Path;

/// Cấu hình điểm xuất hiện của quái
#[derive(Debug, Clone, Deserialize)]
pub struct SpawnPointConfig {
    pub spawn_x: f32,
    pub spawn_y: f32,
    pub patrol_radius: f32,
}

/// Dữ liệu mẫu loại quái từ data/monsters.json
#[derive(Debug, Clone, Deserialize)]
pub struct MonsterTemplate {
    pub type_id: String,
    pub name: String,
    pub max_hp: i32,
    pub move_speed: f32,
    pub chase_speed: f32,
    pub aggro_radius: f32,
    pub attack_range: f32,
    pub leash_radius: f32,
    pub respawn_time: f32,
    pub spawns: Vec<SpawnPointConfig>,
}

/// Dữ liệu waypoint của NPC
#[derive(Debug, Clone, Deserialize)]
pub struct NpcWaypoint {
    pub x: f32,
    pub y: f32,
}

/// Dữ liệu mẫu loại NPC từ data/npcs.json
#[derive(Debug, Clone, Deserialize)]
pub struct NpcTemplate {
    pub type_id: String,
    pub name: String,
    pub spawn_x: f32,
    pub spawn_y: f32,
    pub behavior: String,
    #[serde(default)]
    pub dialogue: Option<String>,
    #[serde(default)]
    pub move_speed: Option<f32>,
    #[serde(default)]
    pub waypoints: Vec<NpcWaypoint>,
}

/// Các trạng thái State Machine của Quái vật
#[derive(Debug, Clone, PartialEq)]
pub enum MonsterFsmState {
    Idle {
        timer: f32,
    },
    Patrol {
        target_x: f32,
        target_y: f32,
    },
    Chase {
        target_id: u64,
    },
    Attack {
        target_id: u64,
        timer: f32,
    },
    Return,
    Dead {
        respawn_timer: f32,
        despawn_delay: f32,
    },
}

impl MonsterFsmState {
    pub fn as_str(&self) -> &'static str {
        match self {
            MonsterFsmState::Idle { .. } => "Idle",
            MonsterFsmState::Patrol { .. } => "Patrol",
            MonsterFsmState::Chase { .. } => "Chase",
            MonsterFsmState::Attack { .. } => "Attack",
            MonsterFsmState::Return => "Return",
            MonsterFsmState::Dead { .. } => "Dead",
        }
    }
}

/// Thực thể quái vật hoạt động trong thế giới game
#[derive(Debug, Clone)]
pub struct MonsterInstance {
    pub id: u64,
    pub type_id: String,
    pub x: f32,
    pub y: f32,
    pub spawn_x: f32,
    pub spawn_y: f32,
    pub patrol_radius: f32,
    pub max_hp: i32,
    pub hp: i32,
    pub move_speed: f32,
    pub chase_speed: f32,
    pub aggro_radius: f32,
    pub attack_range: f32,
    pub leash_radius: f32,
    pub respawn_time: f32,
    pub state: MonsterFsmState,
    patrol_step: u32,
}

impl MonsterInstance {
    pub fn new(id: u64, tmpl: &MonsterTemplate, spawn: &SpawnPointConfig) -> Self {
        Self {
            id,
            type_id: tmpl.type_id.clone(),
            x: spawn.spawn_x,
            y: spawn.spawn_y,
            spawn_x: spawn.spawn_x,
            spawn_y: spawn.spawn_y,
            patrol_radius: spawn.patrol_radius,
            max_hp: tmpl.max_hp,
            hp: tmpl.max_hp,
            move_speed: tmpl.move_speed,
            chase_speed: tmpl.chase_speed,
            aggro_radius: tmpl.aggro_radius,
            attack_range: tmpl.attack_range,
            leash_radius: tmpl.leash_radius,
            respawn_time: tmpl.respawn_time,
            state: MonsterFsmState::Idle { timer: 2.0 },
            patrol_step: 0,
        }
    }

    /// Cập nhật State Machine tại mỗi tick (dt = 0.05s)
    pub fn tick(&mut self, dt: f32, player_positions: &[(u64, f32, f32)]) {
        // Tìm người chơi gần nhất trong danh sách
        let nearest_player = player_positions.iter().min_by(|(_, ax, ay), (_, bx, by)| {
            let da = (ax - self.x).hypot(ay - self.y);
            let db = (bx - self.x).hypot(by - self.y);
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        });

        let dist_from_spawn = (self.x - self.spawn_x).hypot(self.y - self.spawn_y);

        match &mut self.state {
            MonsterFsmState::Idle { timer } => {
                // Kiểm tra người chơi trong tầm phát hiện (aggro)
                if let Some(&(p_id, px, py)) = nearest_player {
                    let dist = (px - self.x).hypot(py - self.y);
                    if dist <= self.aggro_radius {
                        self.state = MonsterFsmState::Chase { target_id: p_id };
                        return;
                    }
                }

                *timer -= dt;
                if *timer <= 0.0 {
                    // Chọn điểm tuần tra tiếp theo quanh điểm spawn
                    self.patrol_step = (self.patrol_step + 1) % 4;
                    let angle = (self.patrol_step as f32) * (std::f32::consts::PI / 2.0);
                    let target_x = (self.spawn_x + angle.cos() * self.patrol_radius)
                        .clamp(MAP_MIN_X, MAP_MAX_X);
                    let target_y = (self.spawn_y + angle.sin() * self.patrol_radius)
                        .clamp(MAP_MIN_Y, MAP_MAX_Y);
                    self.state = MonsterFsmState::Patrol { target_x, target_y };
                }
            }
            MonsterFsmState::Patrol { target_x, target_y } => {
                let tx = *target_x;
                let ty = *target_y;

                // Kiểm tra người chơi trong tầm phát hiện
                if let Some(&(p_id, px, py)) = nearest_player {
                    let dist = (px - self.x).hypot(py - self.y);
                    if dist <= self.aggro_radius {
                        self.state = MonsterFsmState::Chase { target_id: p_id };
                        return;
                    }
                }

                let dx = tx - self.x;
                let dy = ty - self.y;
                let dist = dx.hypot(dy);

                if dist <= 3.0 {
                    // Đã đến điểm tuần tra, chuyển sang nghỉ ngơi
                    self.x = tx;
                    self.y = ty;
                    self.state = MonsterFsmState::Idle { timer: 2.5 };
                } else {
                    self.x += (dx / dist) * self.move_speed * dt;
                    self.y += (dy / dist) * self.move_speed * dt;
                    self.x = self.x.clamp(MAP_MIN_X, MAP_MAX_X);
                    self.y = self.y.clamp(MAP_MIN_Y, MAP_MAX_Y);
                }
            }
            MonsterFsmState::Chase { target_id } => {
                let tid = *target_id;
                let target = player_positions.iter().find(|(id, _, _)| *id == tid);

                if let Some(&(_, px, py)) = target {
                    let dist_to_target = (px - self.x).hypot(py - self.y);

                    // Nếu chạy quá xa điểm spawn ban đầu -> bỏ đuổi, quay về
                    if dist_from_spawn > self.leash_radius {
                        self.state = MonsterFsmState::Return;
                        return;
                    }

                    // Nếu áp sát mục tiêu -> chuyển sang Attack
                    if dist_to_target <= self.attack_range {
                        self.state = MonsterFsmState::Attack {
                            target_id: tid,
                            timer: 1.0,
                        };
                        return;
                    }

                    // Di chuyển áp sát người chơi
                    let dx = px - self.x;
                    let dy = py - self.y;
                    if dist_to_target > 0.001 {
                        self.x += (dx / dist_to_target) * self.chase_speed * dt;
                        self.y += (dy / dist_to_target) * self.chase_speed * dt;
                        self.x = self.x.clamp(MAP_MIN_X, MAP_MAX_X);
                        self.y = self.y.clamp(MAP_MIN_Y, MAP_MAX_Y);
                    }
                } else {
                    // Người chơi mục tiêu đã ngắt kết nối
                    self.state = MonsterFsmState::Return;
                }
            }
            MonsterFsmState::Attack { target_id, timer } => {
                let tid = *target_id;
                let target = player_positions.iter().find(|(id, _, _)| *id == tid);

                *timer -= dt;
                if *timer <= 0.0 {
                    if let Some(&(_, px, py)) = target {
                        let dist = (px - self.x).hypot(py - self.y);
                        if dist_from_spawn > self.leash_radius {
                            self.state = MonsterFsmState::Return;
                        } else if dist <= self.attack_range {
                            // Đánh tiếp sau 1 giây
                            *timer = 1.0;
                        } else {
                            // Người chơi lùi lại, tiếp tục đuổi
                            self.state = MonsterFsmState::Chase { target_id: tid };
                        }
                    } else {
                        self.state = MonsterFsmState::Return;
                    }
                }
            }
            MonsterFsmState::Return => {
                self.hp = self.max_hp; // Hồi đầy máu khi quay về

                let dx = self.spawn_x - self.x;
                let dy = self.spawn_y - self.y;
                let dist = dx.hypot(dy);

                if dist <= 3.0 {
                    self.x = self.spawn_x;
                    self.y = self.spawn_y;
                    self.state = MonsterFsmState::Idle { timer: 2.0 };
                } else {
                    self.x += (dx / dist) * self.chase_speed * dt;
                    self.y += (dy / dist) * self.chase_speed * dt;
                    self.x = self.x.clamp(MAP_MIN_X, MAP_MAX_X);
                    self.y = self.y.clamp(MAP_MIN_Y, MAP_MAX_Y);
                }
            }
            MonsterFsmState::Dead {
                respawn_timer,
                despawn_delay,
            } => {
                *despawn_delay -= dt;
                *respawn_timer -= dt;

                if *respawn_timer <= 0.0 {
                    // Hồi sinh quái
                    self.hp = self.max_hp;
                    self.x = self.spawn_x;
                    self.y = self.spawn_y;
                    self.state = MonsterFsmState::Idle { timer: 2.0 };
                }
            }
        }
    }

    /// Xuất dữ liệu snapshot (trả về None nếu quái đã chết và qua thời gian despawn 3s)
    pub fn to_snapshot(&self) -> Option<MonsterSnapshot> {
        if let MonsterFsmState::Dead { despawn_delay, .. } = self.state {
            if despawn_delay <= 0.0 {
                return None; // Đã despawn, chờ hồi sinh
            }
        }

        Some(MonsterSnapshot {
            id: self.id,
            type_id: self.type_id.clone(),
            x: self.x,
            y: self.y,
            state: self.state.as_str().to_string(),
            hp: self.hp,
            max_hp: self.max_hp,
        })
    }
}

/// Các trạng thái State Machine của NPC
#[derive(Debug, Clone, PartialEq)]
pub enum NpcFsmState {
    Idle,
    Patrol,
    Talk,
}

impl NpcFsmState {
    pub fn as_str(&self) -> &'static str {
        match self {
            NpcFsmState::Idle => "Idle",
            NpcFsmState::Patrol => "Patrol",
            NpcFsmState::Talk => "Talk",
        }
    }
}

/// Thực thể NPC trong thế giới game
#[derive(Debug, Clone)]
pub struct NpcInstance {
    pub id: u64,
    pub type_id: String,
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub spawn_x: f32,
    pub spawn_y: f32,
    pub move_speed: f32,
    pub waypoints: Vec<(f32, f32)>,
    pub current_waypoint: usize,
    pub base_behavior: String,
    pub state: NpcFsmState,
}

impl NpcInstance {
    pub fn new(id: u64, tmpl: &NpcTemplate) -> Self {
        let waypoints: Vec<(f32, f32)> = tmpl.waypoints.iter().map(|w| (w.x, w.y)).collect();
        let base_state = if tmpl.behavior == "patrol" && !waypoints.is_empty() {
            NpcFsmState::Patrol
        } else {
            NpcFsmState::Idle
        };

        Self {
            id,
            type_id: tmpl.type_id.clone(),
            name: tmpl.name.clone(),
            x: tmpl.spawn_x,
            y: tmpl.spawn_y,
            spawn_x: tmpl.spawn_x,
            spawn_y: tmpl.spawn_y,
            move_speed: tmpl.move_speed.unwrap_or(60.0),
            waypoints,
            current_waypoint: 0,
            base_behavior: tmpl.behavior.clone(),
            state: base_state,
        }
    }

    /// Cập nhật State Machine tại mỗi tick (dt = 0.05s)
    pub fn tick(&mut self, dt: f32, player_positions: &[(u64, f32, f32)]) {
        // Khoảng cách tới người chơi gần nhất
        let nearest_player_dist = player_positions
            .iter()
            .map(|(_, px, py)| (px - self.x).hypot(py - self.y))
            .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let is_player_near = nearest_player_dist.is_some_and(|d| d <= 50.0);

        if is_player_near {
            self.state = NpcFsmState::Talk;
            return;
        }

        // Nếu người chơi rời xa > 60px thì quay lại hành vi mặc định
        if self.state == NpcFsmState::Talk && nearest_player_dist.is_none_or(|d| d > 60.0) {
            self.state = if self.base_behavior == "patrol" && !self.waypoints.is_empty() {
                NpcFsmState::Patrol
            } else {
                NpcFsmState::Idle
            };
        }

        if self.state == NpcFsmState::Patrol && !self.waypoints.is_empty() {
            let (tx, ty) = self.waypoints[self.current_waypoint];
            let dx = tx - self.x;
            let dy = ty - self.y;
            let dist = dx.hypot(dy);

            if dist <= 3.0 {
                self.x = tx;
                self.y = ty;
                self.current_waypoint = (self.current_waypoint + 1) % self.waypoints.len();
            } else {
                self.x += (dx / dist) * self.move_speed * dt;
                self.y += (dy / dist) * self.move_speed * dt;
            }
        }
    }

    pub fn to_snapshot(&self) -> NpcSnapshot {
        NpcSnapshot {
            id: self.id,
            type_id: self.type_id.clone(),
            name: self.name.clone(),
            x: self.x,
            y: self.y,
            state: self.state.as_str().to_string(),
        }
    }
}

/// Bộ quản lý toàn bộ quái và NPC trên Server
pub struct EntityManager {
    pub monsters: Vec<MonsterInstance>,
    pub npcs: Vec<NpcInstance>,
}

impl EntityManager {
    pub fn load_from_data_or_defaults() -> Self {
        let mut monsters = Vec::new();
        let mut npcs = Vec::new();
        let mut next_monster_id = 10001u64;
        let mut next_npc_id = 20001u64;

        // Đọc data/monsters.json
        let monsters_path = Path::new("data/monsters.json");
        let monster_templates: Vec<MonsterTemplate> = if monsters_path.exists() {
            fs::read_to_string(monsters_path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_else(Self::default_monster_templates)
        } else {
            Self::default_monster_templates()
        };

        for tmpl in &monster_templates {
            for spawn in &tmpl.spawns {
                monsters.push(MonsterInstance::new(next_monster_id, tmpl, spawn));
                next_monster_id += 1;
            }
        }

        // Đọc data/npcs.json
        let npcs_path = Path::new("data/npcs.json");
        let npc_templates: Vec<NpcTemplate> = if npcs_path.exists() {
            fs::read_to_string(npcs_path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_else(Self::default_npc_templates)
        } else {
            Self::default_npc_templates()
        };

        for tmpl in &npc_templates {
            npcs.push(NpcInstance::new(next_npc_id, tmpl));
            next_npc_id += 1;
        }

        tracing::info!(
            "Da khoi tao {} quai vat va {} NPC trong the gioi",
            monsters.len(),
            npcs.len()
        );

        Self { monsters, npcs }
    }

    /// Cập nhật logic FSM cho tất cả quái và NPC
    pub fn tick(&mut self, dt: f32, player_positions: &[(u64, f32, f32)]) {
        for m in &mut self.monsters {
            m.tick(dt, player_positions);
        }
        for n in &mut self.npcs {
            n.tick(dt, player_positions);
        }
    }

    /// Thu thập snapshot quái và NPC
    pub fn get_snapshots(&self) -> (Vec<MonsterSnapshot>, Vec<NpcSnapshot>) {
        let monster_snaps = self
            .monsters
            .iter()
            .filter_map(|m| m.to_snapshot())
            .collect();
        let npc_snaps = self.npcs.iter().map(|n| n.to_snapshot()).collect();
        (monster_snaps, npc_snaps)
    }

    fn default_monster_templates() -> Vec<MonsterTemplate> {
        vec![
            MonsterTemplate {
                type_id: "goblin".to_string(),
                name: "Yêu tinh".to_string(),
                max_hp: 50,
                move_speed: 100.0,
                chase_speed: 150.0,
                aggro_radius: 180.0,
                attack_range: 35.0,
                leash_radius: 300.0,
                respawn_time: 10.0,
                spawns: vec![
                    SpawnPointConfig {
                        spawn_x: 300.0,
                        spawn_y: 400.0,
                        patrol_radius: 80.0,
                    },
                    SpawnPointConfig {
                        spawn_x: 700.0,
                        spawn_y: 300.0,
                        patrol_radius: 100.0,
                    },
                ],
            },
            MonsterTemplate {
                type_id: "wolf".to_string(),
                name: "Sói xám".to_string(),
                max_hp: 80,
                move_speed: 120.0,
                chase_speed: 180.0,
                aggro_radius: 220.0,
                attack_range: 40.0,
                leash_radius: 350.0,
                respawn_time: 15.0,
                spawns: vec![
                    SpawnPointConfig {
                        spawn_x: 200.0,
                        spawn_y: 700.0,
                        patrol_radius: 120.0,
                    },
                    SpawnPointConfig {
                        spawn_x: 800.0,
                        spawn_y: 750.0,
                        patrol_radius: 100.0,
                    },
                ],
            },
        ]
    }

    fn default_npc_templates() -> Vec<NpcTemplate> {
        vec![
            NpcTemplate {
                type_id: "village_elder".to_string(),
                name: "Trưởng làng Galahad".to_string(),
                spawn_x: 520.0,
                spawn_y: 510.0,
                behavior: "idle".to_string(),
                dialogue: Some("Chào mừng hiệp sĩ đến với ngôi làng!".to_string()),
                move_speed: None,
                waypoints: vec![],
            },
            NpcTemplate {
                type_id: "village_guard".to_string(),
                name: "Lính canh".to_string(),
                spawn_x: 450.0,
                spawn_y: 500.0,
                behavior: "patrol".to_string(),
                dialogue: None,
                move_speed: Some(60.0),
                waypoints: vec![
                    NpcWaypoint { x: 450.0, y: 500.0 },
                    NpcWaypoint { x: 550.0, y: 500.0 },
                ],
            },
        ]
    }
}
