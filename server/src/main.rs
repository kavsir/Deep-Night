use protocol::PROTOCOL_VERSION;
use tokio_tungstenite as _;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    tracing::info!(
        "Mini-MMORPG Server skeleton ready. Protocol version: {}",
        PROTOCOL_VERSION
    );
}
