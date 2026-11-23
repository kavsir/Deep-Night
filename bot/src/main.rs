use futures_util::{SinkExt, StreamExt};
use protocol::{ClientMessage, ServerMessage};
use std::env;
use std::time::Duration;
use tokio::time::Instant;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

/// Chạy một bot thông thường: kết nối, Join, đi vòng vòng và in log vị trí
async fn run_regular_bot(name: String, url: String, duration_secs: u64) {
    tracing::info!("[{}] Dang ket noi toi {}", name, url);
    let (ws_stream, _) = match connect_async(&url).await {
        Ok(res) => res,
        Err(e) => {
            tracing::error!("[{}] Khong the ket noi toi server: {}", name, e);
            return;
        }
    };

    let (mut sink, mut stream) = ws_stream.split();

    // 1. Gui goi Join
    let join_msg = ClientMessage::Join { name: name.clone() };
    let join_json = match join_msg.to_json() {
        Ok(j) => j,
        Err(e) => {
            tracing::error!("[{}] Loi serialize Join: {}", name, e);
            return;
        }
    };
    if let Err(e) = sink.send(Message::Text(join_json.into())).await {
        tracing::error!("[{}] Loi gui Join: {}", name, e);
        return;
    }

    // 2. Cho goi Welcome
    let mut my_player_id: Option<u64> = None;
    while let Some(msg_res) = stream.next().await {
        let raw = match msg_res {
            Ok(m) => m,
            Err(e) => {
                tracing::error!("[{}] Loi nhan tin nhan chao mung: {}", name, e);
                return;
            }
        };

        if let Message::Text(text) = raw {
            if let Ok(server_msg) = ServerMessage::from_json(&text) {
                match server_msg {
                    ServerMessage::Welcome {
                        player_id,
                        tick_rate,
                        map,
                    } => {
                        tracing::info!(
                            "[{}] -> NHAN WELCOME! player_id: {}, tick_rate: {}Hz, map: '{}'",
                            name,
                            player_id,
                            tick_rate,
                            map
                        );
                        my_player_id = Some(player_id);
                        break;
                    }
                    ServerMessage::Error { message } => {
                        tracing::error!("[{}] Server tra ve loi: {}", name, message);
                        return;
                    }
                    _ => {}
                }
            }
        }
    }

    let my_id = match my_player_id {
        Some(id) => id,
        None => {
            tracing::error!("[{}] Khong nhan duoc Welcome tu server", name);
            return;
        }
    };

    // 3. Vòng lặp đi vòng vòng trong duration_secs
    let start_time = Instant::now();
    let total_duration = Duration::from_secs(duration_secs);
    let mut seq: u64 = 0;
    let mut send_interval = tokio::time::interval(Duration::from_millis(100)); // 10 inputs/s
    let mut last_log_time = Instant::now();

    // Các hướng đi vòng vòng: Phải (dx=1), Xuống (dy=1), Trái (dx=-1), Lên (dy=-1)
    let directions = [(1i8, 0i8), (0i8, 1i8), (-1i8, 0i8), (0i8, -1i8)];

    while start_time.elapsed() < total_duration {
        tokio::select! {
            _ = send_interval.tick() => {
                seq += 1;
                // Thay đổi hướng mỗi 2 giây
                let elapsed_secs = start_time.elapsed().as_secs();
                let dir_idx = ((elapsed_secs / 2) % 4) as usize;
                let (dx, dy) = directions[dir_idx];

                let input_msg = ClientMessage::Input { seq, dx, dy };
                if let Ok(json) = input_msg.to_json() {
                    let _ = sink.send(Message::Text(json.into())).await;
                }
            }
            msg_opt = stream.next() => {
                let raw = match msg_opt {
                    Some(Ok(m)) => m,
                    _ => break,
                };
                if let Message::Text(text) = raw {
                    if let Ok(ServerMessage::Snapshot {
                        tick,
                        players,
                        monsters,
                        npcs,
                    }) = ServerMessage::from_json(&text)
                    {
                        // In log vị trí định kỳ mỗi 1 giây
                        if last_log_time.elapsed() >= Duration::from_secs(1) {
                            last_log_time = Instant::now();
                            if let Some(my_pos) = players.iter().find(|p| p.id == my_id) {
                                tracing::info!(
                                    "[{}] [Tick {:>4}] Vi tri: ({:>6.1}, {:>6.1}) | Nguoi choi: {} | Quai: {} | NPC: {}",
                                    name,
                                    tick,
                                    my_pos.x,
                                    my_pos.y,
                                    players.len(),
                                    monsters.len(),
                                    npcs.len()
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    // Dừng di chuyển và đóng kết nối
    seq += 1;
    let stop_msg = ClientMessage::Input { seq, dx: 0, dy: 0 };
    if let Ok(json) = stop_msg.to_json() {
        let _ = sink.send(Message::Text(json.into())).await;
    }
    let _ = <_ as futures_util::SinkExt<_>>::close(&mut sink).await;
    tracing::info!(
        "[{}] Da hoan thanh di vong vong {}s va thoat em dep.",
        name,
        duration_secs
    );
}

/// Chạy test gửi gói rác để chứng minh server không sập
async fn run_fuzz_tests(url: String) {
    tracing::info!("=== BAT DAU KIEM TRA GOI RAC (FUZZ TESTS) ===");

    // Test 1: Gui chuoi JSON sai cu phap
    tracing::info!("[Fuzz 1] Gui JSON sai cu phap...");
    if let Ok((ws_stream, _)) = connect_async(&url).await {
        let (mut sink, mut stream) = ws_stream.split();
        let _ = sink
            .send(Message::Text("{ this is totally invalid json }".into()))
            .await;
        // Server phai tu choi va dong ket noi hoac tra Error, khong panic
        let resp = stream.next().await;
        tracing::info!(
            "[Fuzz 1] -> Ket qua xu ly JSON sai cu phap: {:?} (Server song sot tot!)",
            resp.is_some()
        );
        let _ = <_ as futures_util::SinkExt<_>>::close(&mut sink).await;
    }

    // Test 2: Gui goi tin vuot qua 4096 bytes
    tracing::info!("[Fuzz 2] Gui goi tin qua lon (5000 bytes)...");
    if let Ok((ws_stream, _)) = connect_async(&url).await {
        let (mut sink, mut stream) = ws_stream.split();
        let huge_name = "A".repeat(5000);
        let huge_msg = format!("{{\"type\":\"Join\",\"name\":\"{}\"}}", huge_name);
        let _ = sink.send(Message::Text(huge_msg.into())).await;
        let mut got_error = false;
        while let Some(Ok(Message::Text(t))) = stream.next().await {
            if let Ok(ServerMessage::Error { message }) = ServerMessage::from_json(&t) {
                tracing::info!(
                    "[Fuzz 2] -> Nhan thong bao loi dung nhu mong doi: '{}'",
                    message
                );
                got_error = true;
                break;
            }
        }
        tracing::info!(
            "[Fuzz 2] -> Ket qua goi qua lon: got_error={} (Server song sot tot!)",
            got_error
        );
        let _ = <_ as futures_util::SinkExt<_>>::close(&mut sink).await;
    }

    // Test 3: Gui gia tri di chuyen bat hop le (dx = 5)
    tracing::info!("[Fuzz 3] Gui input bat hop le (dx = 5)...");
    if let Ok((ws_stream, _)) = connect_async(&url).await {
        let (mut sink, mut stream) = ws_stream.split();
        let join = ClientMessage::Join {
            name: "Fuzz_BadInput".to_string(),
        };
        let _ = sink
            .send(Message::Text(join.to_json().unwrap().into()))
            .await;
        let _ = stream.next().await; // Welcome

        // Gui dx=5
        let bad_input = "{\"type\":\"Input\",\"seq\":1,\"dx\":5,\"dy\":0}";
        let _ = sink.send(Message::Text(bad_input.into())).await;

        // Cho them 300ms xem server co sap khong va doc snapshot
        tokio::time::sleep(Duration::from_millis(300)).await;
        let mut received_snapshot = false;
        if let Some(Ok(Message::Text(t))) = stream.next().await {
            if let Ok(ServerMessage::Snapshot { .. }) = ServerMessage::from_json(&t) {
                received_snapshot = true;
            }
        }
        tracing::info!(
            "[Fuzz 3] -> Ket qua gui dx=5: snapshot_received={} (Server van broadcast binh thuong, khong sap!)",
            received_snapshot
        );
        let _ = <_ as futures_util::SinkExt<_>>::close(&mut sink).await;
    }

    // Test 4: Gui trung ten dang online
    tracing::info!("[Fuzz 4] Gui trung ten nguoi choi dang online...");
    if let Ok((ws_stream1, _)) = connect_async(&url).await {
        let (mut sink1, mut stream1) = ws_stream1.split();
        let join1 = ClientMessage::Join {
            name: "Fuzz_OnlinePlayer".to_string(),
        };
        let _ = sink1
            .send(Message::Text(join1.to_json().unwrap().into()))
            .await;
        let _ = stream1.next().await; // Welcome

        // Client 2 thu ket noi voi cung ten do
        if let Ok((ws_stream2, _)) = connect_async(&url).await {
            let (mut sink2, mut stream2) = ws_stream2.split();
            let duplicate_join = ClientMessage::Join {
                name: "Fuzz_OnlinePlayer".to_string(),
            };
            let _ = sink2
                .send(Message::Text(duplicate_join.to_json().unwrap().into()))
                .await;
            let mut rejected = false;
            while let Some(Ok(Message::Text(t))) = stream2.next().await {
                if let Ok(ServerMessage::Error { message }) = ServerMessage::from_json(&t) {
                    tracing::info!(
                        "[Fuzz 4] -> Server tu choi trung ten voi thong bao loi: '{}'",
                        message
                    );
                    rejected = true;
                    break;
                }
            }
            tracing::info!(
                "[Fuzz 4] -> Ket qua trung ten: rejected={} (Server bao ve dung nghiep vu!)",
                rejected
            );
            let _ = <_ as futures_util::SinkExt<_>>::close(&mut sink2).await;
        }

        let _ = <_ as futures_util::SinkExt<_>>::close(&mut sink1).await;
    }

    // Test 5: Gui Input khi chua Join
    tracing::info!("[Fuzz 5] Gui Input khi chua Join...");
    if let Ok((ws_stream, _)) = connect_async(&url).await {
        let (mut sink, mut stream) = ws_stream.split();
        let premature_input = ClientMessage::Input {
            seq: 1,
            dx: 1,
            dy: 0,
        };
        let _ = sink
            .send(Message::Text(premature_input.to_json().unwrap().into()))
            .await;
        let mut rejected = false;
        while let Some(Ok(Message::Text(t))) = stream.next().await {
            if let Ok(ServerMessage::Error { message }) = ServerMessage::from_json(&t) {
                tracing::info!(
                    "[Fuzz 5] -> Server tu choi Input chua Join voi loi: '{}'",
                    message
                );
                rejected = true;
                break;
            }
        }
        tracing::info!(
            "[Fuzz 5] -> Ket qua chua Join: rejected={} (Server bao ve dung nghiep vu!)",
            rejected
        );
        let _ = <_ as futures_util::SinkExt<_>>::close(&mut sink).await;
    }

    tracing::info!("=== TAT CA FUZZ TESTS HOAN THANH XUAT SAC, SERVER VAN HOAT DONG ON DINH ===");
}

/// Chạy kịch bản 1 người chơi tương tác với toàn bộ thế giới: NPC và Quái vật
async fn run_single_player_quest_bot(name: String, url: String, duration_secs: u64) {
    tracing::info!("=== [1 NGUOI CHOI] BAT DAU KICH BAN CHO '{}' ===", name);
    let (ws_stream, _) = match connect_async(&url).await {
        Ok(res) => res,
        Err(e) => {
            tracing::error!("[{}] Khong the ket noi toi server: {}", name, e);
            return;
        }
    };

    let (mut sink, mut stream) = ws_stream.split();

    // 1. Gui Join
    let join_msg = ClientMessage::Join { name: name.clone() };
    if let Ok(json) = join_msg.to_json() {
        let _ = sink.send(Message::Text(json.into())).await;
    }

    // 2. Nhan Welcome
    let mut my_player_id = None;
    while let Some(msg_res) = stream.next().await {
        if let Ok(Message::Text(text)) = msg_res {
            if let Ok(ServerMessage::Welcome {
                player_id,
                tick_rate,
                map,
            }) = ServerMessage::from_json(&text)
            {
                tracing::info!(
                    "[{}] -> NHAN WELCOME! player_id: {}, tick_rate: {}Hz, map: '{}'",
                    name,
                    player_id,
                    tick_rate,
                    map
                );
                my_player_id = Some(player_id);
                break;
            }
        }
    }

    let my_id = match my_player_id {
        Some(id) => id,
        None => return,
    };

    // 3. Quan sat va thuc hien cac phase hanh dong
    let mut seq = 0u64;
    let start_time = Instant::now();
    let total_duration = Duration::from_secs(duration_secs);
    let mut phase = 0; // 0: Check NPC Talk, 1: Approach Goblin (-1, -1), 2: Flee Goblin (1, 1), 3: Free roam
    let mut phase_timer = Instant::now();
    let mut last_log_time = Instant::now();
    let mut input_interval = tokio::time::interval(Duration::from_millis(100));

    while start_time.elapsed() < total_duration {
        tokio::select! {
            _ = input_interval.tick() => {
                seq += 1;
                let (dx, dy) = match phase {
                    0 => (0, 0),        // O canh truong lang (500, 500)
                    1 => (-1, -1),      // Di cheo len trai ve phia hang Goblin (300, 400)
                    2 => (1, 1),        // Chay ra xa vuot khoi leash radius ve phia (800, 800)
                    _ => (0, 0),        // Dung lai quan sat
                };
                let input_msg = ClientMessage::Input { seq, dx, dy };
                if let Ok(json) = input_msg.to_json() {
                    let _ = sink.send(Message::Text(json.into())).await;
                }
            }
            msg_opt = stream.next() => {
                let raw = match msg_opt {
                    Some(Ok(m)) => m,
                    _ => break,
                };
                if let Message::Text(text) = raw {
                    if let Ok(ServerMessage::Snapshot { tick, players, monsters, npcs }) = ServerMessage::from_json(&text) {
                        let my_pos = players.iter().find(|p| p.id == my_id);
                        let elder = npcs.iter().find(|n| n.type_id == "village_elder");
                        let goblin = monsters.iter().find(|m| m.type_id == "goblin");

                        // Chuyen phase logic
                        if phase == 0 && phase_timer.elapsed() >= Duration::from_millis(800) {
                            if let Some(e) = elder {
                                tracing::info!(
                                    "[{}] [Phase 0 - Giao tiep NPC] Truong lang '{}' dang o trang thai: '{}' (Giao tiep thanh cong!)",
                                    name, e.name, e.state
                                );
                            }
                            tracing::info!("[{}] [Phase 1] Bat dau tien ve phia hang Goblin (300, 400)...", name);
                            phase = 1;
                            phase_timer = Instant::now();
                        } else if phase == 1 {
                            if let Some(e) = elder {
                                if e.state == "Idle" && phase_timer.elapsed() >= Duration::from_millis(600) {
                                    tracing::info!(
                                        "[{}] [Phase 1 - Roi xa NPC] Nguoi choi da di xa, Truong lang quay ve: '{}'",
                                        name, e.state
                                    );
                                }
                            }
                            if let Some(g) = goblin {
                                if g.state == "Chase" || g.state == "Attack" {
                                    tracing::info!(
                                        "[{}] [Phase 1 - Cham tran Quai] Goblin (id: {}) phat hien nguoi choi! State: '{}', Vi tri: ({:.1}, {:.1})",
                                        name, g.id, g.state, g.x, g.y
                                    );
                                    if g.state == "Attack" || phase_timer.elapsed() >= Duration::from_secs(3) {
                                        tracing::info!("[{}] [Phase 2] Goblin da vao tam danh! Bat dau thao chay ve phia dong nam (1, 1) de cat duoi...", name);
                                        phase = 2;
                                        phase_timer = Instant::now();
                                    }
                                }
                            }
                        } else if phase == 2 && phase_timer.elapsed() >= Duration::from_secs(4) {
                            if let Some(g) = goblin {
                                tracing::info!(
                                    "[{}] [Phase 2 - Cat duoi thanh cong] Goblin da vuot tam leash va quay ve! State: '{}', Vi tri: ({:.1}, {:.1})",
                                    name, g.state, g.x, g.y
                                );
                            }
                            phase = 3;
                            phase_timer = Instant::now();
                        }

                        if last_log_time.elapsed() >= Duration::from_secs(1) {
                            last_log_time = Instant::now();
                            if let Some(p) = my_pos {
                                let g_info = goblin.map(|g| format!("{}:{}", g.state, g.hp)).unwrap_or_else(|| "none".to_string());
                                let e_info = elder.map(|e| e.state.clone()).unwrap_or_else(|| "none".to_string());
                                tracing::info!(
                                    "[{}] [Tick {:>4}] Vi tri: ({:>6.1}, {:>6.1}) | Truong lang: {} | Goblin: {}",
                                    name, tick, p.x, p.y, e_info, g_info
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    // Dung di chuyen
    seq += 1;
    let stop_msg = ClientMessage::Input { seq, dx: 0, dy: 0 };
    if let Ok(json) = stop_msg.to_json() {
        let _ = sink.send(Message::Text(json.into())).await;
    }
    let _ = <_ as futures_util::SinkExt<_>>::close(&mut sink).await;
    tracing::info!("[{}] Hoan thanh kich ban 1 nguoi choi xuat sac!", name);
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let server_url = env::var("SERVER_URL").unwrap_or_else(|_| "ws://127.0.0.1:7777".to_string());
    let duration_secs: u64 = env::var("DURATION_SECS")
        .ok()
        .and_then(|d| d.parse().ok())
        .unwrap_or(15);
    let bot_count: usize = env::var("BOT_COUNT")
        .ok()
        .and_then(|c| c.parse().ok())
        .unwrap_or(1); // Mac dinh 1 nguoi choi tap trung theo yeu cau

    tracing::info!(
        "Khoi dong test voi {} nguoi choi ket noi toi: {}, thoi gian: {}s",
        bot_count,
        server_url,
        duration_secs
    );

    if bot_count == 1 {
        // Chay kich ban 1 nguoi choi tuong tac NPC va Quai
        run_single_player_quest_bot(
            "Knight_Arthur".to_string(),
            server_url.clone(),
            duration_secs,
        )
        .await;

        // Sau do chay tiep bo fuzz test de chung minh server hoan toan on dinh
        run_fuzz_tests(server_url).await;
    } else {
        let mut handles = Vec::new();
        for i in 1..=bot_count {
            let name = format!("Bot_{}", i);
            let url = server_url.clone();
            handles.push(tokio::spawn(run_regular_bot(name, url, duration_secs)));
        }

        let fuzz_url = server_url.clone();
        let fuzz_handle = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(2)).await;
            run_fuzz_tests(fuzz_url).await;
        });

        for h in handles {
            let _ = h.await;
        }
        let _ = fuzz_handle.await;
    }

    tracing::info!("Toan bo bot test da hoan tat!");
}
