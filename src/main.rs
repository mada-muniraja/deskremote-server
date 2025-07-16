use mdns_sd::{ServiceDaemon, ServiceInfo};
use rdev::{Event, listen};
use std::net::Ipv4Addr;
use tokio::net::{TcpListener, TcpStream};
use tokio::task;
use tracing::{Level, info, warn};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    // ---------- mDNS ----------
    let my_ip = match local_ip_address::local_ip()
        .unwrap_or(std::net::IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)))
    {
        std::net::IpAddr::V4(ip) => ip,
        std::net::IpAddr::V6(_) => Ipv4Addr::new(127, 0, 0, 1),
    };
    let mdns = ServiceDaemon::new().expect("mDNS daemon");
    let service = ServiceInfo::new(
        "_deskremote._tcp.local.",
        "DeskRemote Server",
        "deskremote.local.",
        my_ip,
        8888,
        None,
    )
    .unwrap();
    mdns.register(service).unwrap();
    info!("mDNS service _deskremote._tcp advertised on {}", my_ip);

    // ---------- Event capture ----------
    task::spawn_blocking(move || {
        if let Err(e) = listen(callback) {
            warn!("rdev listen error: {:?}", e);
        }
    });

    // ---------- TCP listener ----------
    let listener = TcpListener::bind((my_ip, 8888)).await?;
    info!("DeskRemote server listening on {}:8888", my_ip);

    loop {
        let (socket, addr) = listener.accept().await?;
        info!("New connection from {}", addr);
        tokio::spawn(handle_conn(socket));
    }
}

fn callback(event: Event) {
    info!("{:?}", event);
}

async fn handle_conn(mut socket: TcpStream) -> std::io::Result<()> {
    let (mut reader, mut writer) = socket.split();
    tokio::io::copy(&mut reader, &mut writer).await?;
    Ok(())
}
