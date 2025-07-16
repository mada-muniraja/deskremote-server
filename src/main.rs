use mdns_sd::{ServiceDaemon, ServiceInfo};
use rdev::listen;
use std::mem::discriminant;
use std::net::Ipv4Addr;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio::task;
use tracing::{Level, info, warn};

mod protocol;
use protocol::Packet;

/// Channel payload: already-serialized Packet bytes
type PacketBytes = Vec<u8>;
static CHANNEL_CAP: usize = 128;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let (tx, _rx) = broadcast::channel::<PacketBytes>(CHANNEL_CAP);
    let tx_clone = tx.clone();
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
        None, // TXT records (none for now)
    )
    .unwrap();
    mdns.register(service).unwrap();
    info!("mDNS service _deskremote._tcp advertised on {}", my_ip);

    // ---------- Event capture ----------
    task::spawn_blocking(make_callback(tx.clone()));

    // ---------- TCP listener ----------
    let listener = TcpListener::bind((my_ip, 8888)).await?;
    info!("DeskRemote server listening on {}:8888", my_ip);

    loop {
        let (socket, addr) = listener.accept().await?;
        info!("New connection from {}", addr);
        tokio::spawn(handle_conn(socket));
    }
}

/// Convert `rdev::Key` to a byte-sized code for the protocol
#[inline]
fn key_to_u8(key: rdev::Key) -> u8 {
    // SAFETY: discriminant is a usize; we truncate to u8 for the PoC
    unsafe { std::mem::transmute_copy(&discriminant(&key)) }
}

fn make_callback(sender: broadcast::Sender<PacketBytes>) -> impl Fn(rdev::Event) {
    move |event| {
        let pkt = match event.event_type {
            rdev::EventType::MouseMove { x, y } => Packet::MouseMove { x, y },
            rdev::EventType::KeyPress(key) => Packet::KeyDown {
                code: key_to_u8(key),
            },
            rdev::EventType::KeyRelease(key) => Packet::KeyUp {
                code: key_to_u8(key),
            },
            _ => return,
        };
        let bytes = pkt.to_bytes();
        let _ = sender.send(bytes); // ignore errors (no receivers yet)
    }
}

/// Echo handler for now
async fn handle_conn(mut socket: TcpStream) -> std::io::Result<()> {
    let (mut reader, mut writer) = socket.split();
    tokio::io::copy(&mut reader, &mut writer).await?;
    Ok(())
}
