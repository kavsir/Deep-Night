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
            ServerMessage::Snapshot { tick, players } => {
                assert!(tick >= 1);
                assert_eq!(players.len(), 1);
                assert_eq!(players[0].id, 1);
                assert_eq!(players[0].x, SPAWN_X);
                assert_eq!(players[0].y, SPAWN_Y);
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
