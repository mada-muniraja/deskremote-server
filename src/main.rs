use tracing::{Level, info};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("DeskRemote server starting 🚀");

    // TODO: event capture, TCP listener, mDNS advertise
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }
}
