use futures_util::{SinkExt, StreamExt};
use protocol::{ClientMessage, ServerMessage, MAP_MAX_X, SPAWN_X, SPAWN_Y, TICK_RATE};
use std::time::Duration;
use tokio::sync::broadcast;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

/// Helper function khởi tạo server trên cổng động (ephemeral port)
async fn spawn_test_server() -> (String, broadcast::Sender<()>) {
    let (shutdown_tx, shutdown_rx) = broadcast::channel(1);
    // Bind vào 127.0.0.1:0 để hệ điều hành cấp cổng ngẫu nhiên khả dụng
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let url = format!("ws://127.0.0.1:{}", port);
    drop(listener); // giải phóng cổng để server::start_server bind

    let server_addr = format!("127.0.0.1:{}", port);
    tokio::spawn(async move {
        let _ = server::start_server(&server_addr, Some(shutdown_rx)).await;
    });

    // Chờ server sẵn sàng
    tokio::time::sleep(Duration::from_millis(100)).await;
    (url, shutdown_tx)
}

#[tokio::test]
async fn test_join_welcome_and_snapshot() {
    let (url, _shutdown) = spawn_test_server().await;
    let (ws_stream, _) = connect_async(&url).await.unwrap();
    let (mut sink, mut stream) = ws_stream.split();

    // Gửi Join
    let join = ClientMessage::Join {
        name: "Arthur".to_string(),
    };
    sink.send(Message::Text(join.to_json().unwrap().into()))
        .await
        .unwrap();

    // Nhận Welcome
    let msg = stream.next().await.unwrap().unwrap();
    if let Message::Text(text) = msg {
        let welcome = ServerMessage::from_json(&text).unwrap();
        match welcome {
            ServerMessage::Welcome {
                player_id,
                tick_rate,
                map,
            } => {
                assert_eq!(player_id, 1);
                assert_eq!(tick_rate, TICK_RATE);
                assert_eq!(map, "starter_village");
            }
            _ => panic!("Expected Welcome message, got: {:?}", welcome),
        }
    } else {
        panic!("Expected text message");
    }

    // Nhận Snapshot
    let snapshot_msg = stream.next().await.unwrap().unwrap();
    if let Message::Text(text) = snapshot_msg {
        let snapshot = ServerMessage::from_json(&text).unwrap();
        match snapshot {
            ServerMessage::Snapshot {
                tick,
                players,
                monsters,
                npcs,
            } => {
                assert!(tick >= 1);
                assert_eq!(players.len(), 1);
                assert_eq!(players[0].id, 1);
                assert_eq!(players[0].x, SPAWN_X);
                assert_eq!(players[0].y, SPAWN_Y);
                assert!(!monsters.is_empty(), "Monsters list should not be empty");
                assert!(!npcs.is_empty(), "NPCs list should not be empty");
            }
            _ => panic!("Expected Snapshot, got: {:?}", snapshot),
        }
    }
}

#[tokio::test]
async fn test_player_movement_and_bounds() {
    let (url, _shutdown) = spawn_test_server().await;
    let (ws_stream, _) = connect_async(&url).await.unwrap();
    let (mut sink, mut stream) = ws_stream.split();

    let join = ClientMessage::Join {
        name: "Lancelot".to_string(),
    };
    sink.send(Message::Text(join.to_json().unwrap().into()))
        .await
        .unwrap();

    // Đọc Welcome
    let _ = stream.next().await.unwrap().unwrap();

    // Gửi Input di chuyển sang phải (dx=1, dy=0)
    let input = ClientMessage::Input {
        seq: 1,
        dx: 1,
        dy: 0,
    };
    sink.send(Message::Text(input.to_json().unwrap().into()))
        .await
        .unwrap();

    // Chờ và đọc tối đa 15 Snapshot để xác nhận vị trí X đã tăng
    let mut last_x = SPAWN_X;
    for _ in 0..15 {
        if let Ok(Some(Ok(Message::Text(text)))) =
            tokio::time::timeout(Duration::from_millis(200), stream.next()).await
        {
            if let Ok(ServerMessage::Snapshot { players, .. }) = ServerMessage::from_json(&text) {
                if let Some(p) = players.iter().find(|p| p.id == 1) {
                    last_x = p.x;
                    if last_x > SPAWN_X {
                        break;
                    }
                }
            }
        }
    }

    // Vị trí X phải tăng lên vì đi sang phải
    assert!(
        last_x > SPAWN_X,
        "Player position should have increased from SPAWN_X"
    );
    assert!(last_x <= MAP_MAX_X, "Player position must stay within map");
}

#[tokio::test]
async fn test_duplicate_name_rejected() {
    let (url, _shutdown) = spawn_test_server().await;

    // Client 1 join với tên "Galahad"
    let (ws_stream1, _) = connect_async(&url).await.unwrap();
    let (mut sink1, mut stream1) = ws_stream1.split();
    let join1 = ClientMessage::Join {
        name: "Galahad".to_string(),
    };
    sink1
        .send(Message::Text(join1.to_json().unwrap().into()))
        .await
        .unwrap();
    let _ = stream1.next().await.unwrap().unwrap(); // Welcome

    // Client 2 cố gắng join cùng tên "Galahad"
    let (ws_stream2, _) = connect_async(&url).await.unwrap();
    let (mut sink2, mut stream2) = ws_stream2.split();
    let join2 = ClientMessage::Join {
        name: "Galahad".to_string(),
    };
    sink2
        .send(Message::Text(join2.to_json().unwrap().into()))
        .await
        .unwrap();

    // Client 2 phải nhận được gói Error
    let msg2 = stream2.next().await.unwrap().unwrap();
    if let Message::Text(text) = msg2 {
        let err = ServerMessage::from_json(&text).unwrap();
        match err {
            ServerMessage::Error { message } => {
                assert_eq!(message, "Ten nguoi choi da ton tai");
            }
            _ => panic!("Expected Error message for duplicate name, got: {:?}", err),
        }
    } else {
        panic!("Expected text error");
    }

    // Client 2 cũng nhận được WebSocket Close frame với lý do tương ứng
    if let Some(Ok(Message::Close(Some(frame)))) = stream2.next().await {
        assert_eq!(frame.reason, "Ten nguoi choi da ton tai");
    }
}

#[tokio::test]
async fn test_fuzz_bad_inputs_do_not_crash_server() {
    let (url, _shutdown) = spawn_test_server().await;

    // 1. JSON sai cú pháp
    {
        let (ws, _) = connect_async(&url).await.unwrap();
        let (mut sink, mut stream) = ws.split();
        sink.send(Message::Text("{ this is invalid json".into()))
            .await
            .unwrap();
        // Server gửi Error hoặc đóng kết nối, không crash
        let _ = stream.next().await;
    }

    // 2. Gói tin quá lớn (> 4096 bytes)
    {
        let (ws, _) = connect_async(&url).await.unwrap();
        let (mut sink, mut stream) = ws.split();
        let huge = "A".repeat(5000);
        let msg = format!("{{\"type\":\"Join\",\"name\":\"{}\"}}", huge);
        sink.send(Message::Text(msg.into())).await.unwrap();
        let resp = stream.next().await.unwrap().unwrap();
        if let Message::Text(t) = resp {
            let err = ServerMessage::from_json(&t).unwrap();
            match err {
                ServerMessage::Error { message } => {
                    assert_eq!(message, "Goi tin vuot qua kich thuoc cho phep");
                }
                _ => panic!("Expected size error, got: {:?}", err),
            }
        }
    }

    // 3. Input khi chưa Join
    {
        let (ws, _) = connect_async(&url).await.unwrap();
        let (mut sink, mut stream) = ws.split();
        let input = ClientMessage::Input {
            seq: 1,
            dx: 1,
            dy: 0,
        };
        sink.send(Message::Text(input.to_json().unwrap().into()))
            .await
            .unwrap();
        let resp = stream.next().await.unwrap().unwrap();
        if let Message::Text(t) = resp {
            let err = ServerMessage::from_json(&t).unwrap();
            match err {
                ServerMessage::Error { message } => {
                    assert_eq!(message, "Phai Join truoc khi gui Input");
                }
                _ => panic!("Expected Must Join error, got: {:?}", err),
            }
        }
    }

    // 4. Client hợp lệ tiếp tục kết nối bình thường để chứng minh server hoàn toàn không sập
    {
        let (ws, _) = connect_async(&url).await.unwrap();
        let (mut sink, mut stream) = ws.split();
        let join = ClientMessage::Join {
            name: "Survivor".to_string(),
        };
        sink.send(Message::Text(join.to_json().unwrap().into()))
            .await
            .unwrap();
        let resp = stream.next().await.unwrap().unwrap();
        if let Message::Text(t) = resp {
            let welcome = ServerMessage::from_json(&t).unwrap();
            match welcome {
                ServerMessage::Welcome { player_id, .. } => {
                    assert_eq!(player_id, 1);
                }
                _ => panic!("Expected welcome, got: {:?}", welcome),
            }
        }
    }
}

#[tokio::test]
async fn test_disconnect_broadcasts_player_left() {
    let (url, _shutdown) = spawn_test_server().await;

    // Client 1 (Watcher)
    let (ws1, _) = connect_async(&url).await.unwrap();
    let (mut sink1, mut stream1) = ws1.split();
    let join1 = ClientMessage::Join {
        name: "Watcher".to_string(),
    };
    sink1
        .send(Message::Text(join1.to_json().unwrap().into()))
        .await
        .unwrap();
    let _ = stream1.next().await.unwrap().unwrap(); // Welcome

    // Client 2 (Leaver)
    let (ws2, _) = connect_async(&url).await.unwrap();
    let (mut sink2, mut stream2) = ws2.split();
    let join2 = ClientMessage::Join {
        name: "Leaver".to_string(),
    };
    sink2
        .send(Message::Text(join2.to_json().unwrap().into()))
        .await
        .unwrap();
    let _ = stream2.next().await.unwrap().unwrap(); // Welcome

    // Client 1 nhận được PlayerJoined của Client 2
    let mut got_joined = false;
    while let Ok(Some(Ok(Message::Text(t)))) =
        tokio::time::timeout(Duration::from_millis(200), stream1.next()).await
    {
        if let Ok(ServerMessage::PlayerJoined { id, name }) = ServerMessage::from_json(&t) {
            if id == 2 && name == "Leaver" {
                got_joined = true;
                break;
            }
        }
    }
    assert!(got_joined, "Watcher should receive PlayerJoined for Leaver");

    // Client 2 đóng kết nối
    drop(sink2);
    drop(stream2);

    // Client 1 phải nhận được PlayerLeft của Client 2
    let mut got_left = false;
    while let Ok(Some(Ok(Message::Text(t)))) =
        tokio::time::timeout(Duration::from_millis(500), stream1.next()).await
    {
        if let Ok(ServerMessage::PlayerLeft { id }) = ServerMessage::from_json(&t) {
            if id == 2 {
                got_left = true;
                break;
            }
        }
    }
    assert!(
        got_left,
        "Watcher should receive PlayerLeft when Leaver disconnects"
    );
}

#[tokio::test]
async fn test_single_player_monster_aggro_and_leash() {
    let (url, _shutdown) = spawn_test_server().await;
    let (ws_stream, _) = connect_async(&url).await.unwrap();
    let (mut sink, mut stream) = ws_stream.split();

    let join = ClientMessage::Join {
        name: "Knight_Gareth".to_string(),
    };
    sink.send(Message::Text(join.to_json().unwrap().into()))
        .await
        .unwrap();

    // Doc Welcome
    let _ = stream.next().await.unwrap().unwrap();

    // 1. Cho xem Goblin o trang thai Idle hoac Patrol
    let mut goblin_id = None;
    for _ in 0..15 {
        if let Ok(Some(Ok(Message::Text(text)))) =
            tokio::time::timeout(Duration::from_millis(200), stream.next()).await
        {
            if let Ok(ServerMessage::Snapshot { monsters, .. }) = ServerMessage::from_json(&text) {
                if let Some(m) = monsters.iter().find(|m| m.type_id == "goblin") {
                    goblin_id = Some(m.id);
                    assert!(m.state == "Idle" || m.state == "Patrol");
                    break;
                }
            }
        }
    }
    let target_goblin_id = goblin_id.expect("Khong tim thay goblin trong snapshot");

    // 2. Nguoi choi di chuyen toi gan Goblin (dx = -1, dy = -1 huong ve 300, 400)
    let mut seq = 1;
    let mut saw_chase = false;
    for _ in 0..40 {
        let input = ClientMessage::Input {
            seq,
            dx: -1,
            dy: -1,
        };
        seq += 1;
        let _ = sink
            .send(Message::Text(input.to_json().unwrap().into()))
            .await;

        if let Ok(Some(Ok(Message::Text(text)))) =
            tokio::time::timeout(Duration::from_millis(100), stream.next()).await
        {
            if let Ok(ServerMessage::Snapshot { monsters, .. }) = ServerMessage::from_json(&text) {
                if let Some(m) = monsters.iter().find(|m| m.id == target_goblin_id) {
                    if m.state == "Chase" || m.state == "Attack" {
                        saw_chase = true;
                        break;
                    }
                }
            }
        }
    }
    assert!(
        saw_chase,
        "Goblin phai chuyen sang Chase/Attack khi nguoi choi den gan"
    );

    // 3. Nguoi choi chay xa ra ngoai tam leash (dx = 1, dy = 1 ve phia 800, 800)
    let mut saw_return = false;
    for _ in 0..60 {
        let input = ClientMessage::Input { seq, dx: 1, dy: 1 };
        seq += 1;
        let _ = sink
            .send(Message::Text(input.to_json().unwrap().into()))
            .await;

        if let Ok(Some(Ok(Message::Text(text)))) =
            tokio::time::timeout(Duration::from_millis(100), stream.next()).await
        {
            if let Ok(ServerMessage::Snapshot { monsters, .. }) = ServerMessage::from_json(&text) {
                if let Some(m) = monsters.iter().find(|m| m.id == target_goblin_id) {
                    if m.state == "Return" || m.state == "Idle" {
                        saw_return = true;
                        break;
                    }
                }
            }
        }
    }
    assert!(
        saw_return,
        "Goblin phai chuyen sang Return/Idle khi nguoi choi chay ra ngoai leash radius"
    );
}

#[tokio::test]
async fn test_single_player_npc_interaction_proximity() {
    let (url, _shutdown) = spawn_test_server().await;
    let (ws_stream, _) = connect_async(&url).await.unwrap();
    let (mut sink, mut stream) = ws_stream.split();

    let join = ClientMessage::Join {
        name: "Knight_Kay".to_string(),
    };
    sink.send(Message::Text(join.to_json().unwrap().into()))
        .await
        .unwrap();

    // Doc Welcome
    let _ = stream.next().await.unwrap().unwrap();

    // 1. Nguoi choi xuat hien tai (500, 500). Truong lang o (520, 510), khoang cach ~22px <= 50px.
    // NPC phai chuyen sang trang thai "Talk".
    let mut saw_talk = false;
    for _ in 0..15 {
        if let Ok(Some(Ok(Message::Text(text)))) =
            tokio::time::timeout(Duration::from_millis(200), stream.next()).await
        {
            if let Ok(ServerMessage::Snapshot { npcs, .. }) = ServerMessage::from_json(&text) {
                if let Some(npc) = npcs.iter().find(|n| n.type_id == "village_elder") {
                    if npc.state == "Talk" {
                        saw_talk = true;
                        break;
                    }
                }
            }
        }
    }
    assert!(
        saw_talk,
        "Truong lang phai o trang thai Talk khi nguoi choi o gan <= 50px"
    );

    // 2. Nguoi choi di chuyen ra xa (dx = 0, dy = 1 xuong phia duoi) de khoang cach > 60px
    let mut saw_idle = false;
    for seq in 1..=30 {
        let input = ClientMessage::Input { seq, dx: 0, dy: 1 };
        let _ = sink
            .send(Message::Text(input.to_json().unwrap().into()))
            .await;

        if let Ok(Some(Ok(Message::Text(text)))) =
            tokio::time::timeout(Duration::from_millis(100), stream.next()).await
        {
            if let Ok(ServerMessage::Snapshot { npcs, .. }) = ServerMessage::from_json(&text) {
                if let Some(npc) = npcs.iter().find(|n| n.type_id == "village_elder") {
                    if npc.state == "Idle" {
                        saw_idle = true;
                        break;
                    }
                }
            }
        }
    }
    assert!(
        saw_idle,
        "Truong lang phai quay lai Idle khi nguoi choi di xa > 60px"
    );
}
