use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt::init();

    // Doc cong tu bien moi truong PORT, mac dinh 7777
    let port: u16 = env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(7777);

    let bind_addr = format!("0.0.0.0:{}", port);
    tracing::info!("Khoi dong server tai {}", bind_addr);

    server::start_server(&bind_addr, None).await?;
    Ok(())
}
