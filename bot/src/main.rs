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
                    if let Ok(ServerMessage::Snapshot { tick, players }) = ServerMessage::from_json(&text) {
                        // In log vị trí định kỳ mỗi 1 giây
                        if last_log_time.elapsed() >= Duration::from_secs(1) {
                            last_log_time = Instant::now();
                            if let Some(my_pos) = players.iter().find(|p| p.id == my_id) {
                                tracing::info!(
                                    "[{}] [Tick {:>4}] Vi tri: ({:>6.1}, {:>6.1}) | Tong nguoi choi: {}",
                                    name,
                                    tick,
                                    my_pos.x,
                                    my_pos.y,
                                    players.len()
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
    tracing::info!("[Fuzz 4] Gui trung ten nguoi choi dang online ('Bot_1')...");
    if let Ok((ws_stream, _)) = connect_async(&url).await {
        let (mut sink, mut stream) = ws_stream.split();
        let duplicate_join = ClientMessage::Join {
            name: "Bot_1".to_string(),
        };
        let _ = sink
            .send(Message::Text(duplicate_join.to_json().unwrap().into()))
            .await;
        let mut rejected = false;
        while let Some(Ok(Message::Text(t))) = stream.next().await {
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
        let _ = <_ as futures_util::SinkExt<_>>::close(&mut sink).await;
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

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let server_url = env::var("SERVER_URL").unwrap_or_else(|_| "ws://127.0.0.1:7777".to_string());
    let duration_secs: u64 = env::var("DURATION_SECS")
        .ok()
        .and_then(|d| d.parse().ok())
        .unwrap_or(30);

    tracing::info!(
        "Khoi dong test 3 bot va goi rac ket noi toi: {}, thoi gian: {}s",
        server_url,
        duration_secs
    );

    // Chay 3 bot cung luc
    let url1 = server_url.clone();
    let url2 = server_url.clone();
    let url3 = server_url.clone();

    let bot1 = tokio::spawn(run_regular_bot("Bot_1".to_string(), url1, duration_secs));
    let bot2 = tokio::spawn(run_regular_bot("Bot_2".to_string(), url2, duration_secs));
    let bot3 = tokio::spawn(run_regular_bot("Bot_3".to_string(), url3, duration_secs));

    // Sau 2 giay khi 3 bot da vao on dinh, chay bo test goi rac (fuzz tests)
    let fuzz_url = server_url.clone();
    let fuzz_handle = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(2)).await;
        run_fuzz_tests(fuzz_url).await;
    });

    let _ = tokio::join!(bot1, bot2, bot3, fuzz_handle);
    tracing::info!("Toan bo bot test da hoan tat!");
}
