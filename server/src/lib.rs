use futures_util::{SinkExt, StreamExt};
use protocol::{
    ClientMessage, PlayerSnapshot, ServerMessage, MAP_MAX_X, MAP_MAX_Y, MAP_MIN_X, MAP_MIN_Y,
    MAX_INPUTS_PER_SECOND, MAX_MESSAGE_SIZE, MAX_TOTAL_PACKETS_PER_SECOND, PLAYER_MOVE_SPEED,
    SPAWN_X, SPAWN_Y, TICK_RATE,
};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot};
use tokio::time::Instant;
use tokio_tungstenite::tungstenite;

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
    Disconnect {
        player_id: u64,
    },
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
    out_tx: mpsc::UnboundedSender<ServerMessage>,
}

/// Chạy game loop tại tần số 20Hz, hoàn toàn tách biệt với I/O mạng
pub async fn run_game_loop(mut event_rx: mpsc::UnboundedReceiver<GameEvent>) {
    let mut tick: u64 = 0;
    let mut next_player_id: u64 = 1;
    let mut players: HashMap<u64, PlayerData> = HashMap::new();
    let tick_duration = Duration::from_millis(1000 / TICK_RATE as u64); // 50ms (20Hz)
    let mut interval = tokio::time::interval(tick_duration);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            _ = interval.tick() => {
                tick += 1;

                // 1. Gom tất cả sự kiện mạng đang chờ trong channel
                while let Ok(event) = event_rx.try_recv() {
                    handle_game_event(event, &mut next_player_id, &mut players);
                }

                // 2. Cập nhật vị trí của tất cả người chơi
                let dt = 1.0 / (TICK_RATE as f32);
                for player in players.values_mut() {
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

                // 3. Tạo Snapshot và broadcast tới tất cả client đang kết nối
                let snapshot_players: Vec<PlayerSnapshot> = players
                    .values()
                    .map(|p| PlayerSnapshot {
                        id: p.id,
                        x: p.x,
                        y: p.y,
                    })
                    .collect();

                let snapshot = ServerMessage::Snapshot {
                    tick,
                    players: snapshot_players,
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
                handle_game_event(event, &mut next_player_id, &mut players);
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

            // Lưu người chơi mới vào trạng thái server
            players.insert(
                player_id,
                PlayerData {
                    id: player_id,
                    name: name.clone(),
                    x: SPAWN_X,
                    y: SPAWN_Y,
                    last_dx: 0,
                    last_dy: 0,
                    last_seq: 0,
                    out_tx,
                },
            );

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
                if seq >= player.last_seq {
                    player.last_seq = seq;
                    player.last_dx = dx;
                    player.last_dy = dy;
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
            match msg.to_json() {
                Ok(json) => {
                    if ws_sink
                        .send(tungstenite::Message::Text(json.into()))
                        .await
                        .is_err()
                    {
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
                return;
            }
            trimmed.to_string()
        }
        ClientMessage::Input { .. } => {
            // AUTH: Client gui Input khi chua Join
            let _ = out_tx.send(ServerMessage::Error {
                message: "Phai Join truoc khi gui Input".to_string(),
            });
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

    // 2. Vòng lặp nhận gói tin sau khi đã Join thành công
    let mut window_start = Instant::now();
    let mut input_count: u32 = 0;
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
                    // Drop goi tin thua trong giay nay de tranh flood
                    continue;
                }
                // AUTH: Kiem tra huong di chuyen dx, dy phai thuoc tap {-1, 0, 1}
                if !(-1..=1).contains(&dx) || !(-1..=1).contains(&dy) {
                    // Drop input bat thuong (dx=5, dy=-2...), server khong cap nhat
                    continue;
                }
                let _ = game_tx.send(GameEvent::Input {
                    player_id,
                    seq,
                    dx,
                    dy,
                });
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
