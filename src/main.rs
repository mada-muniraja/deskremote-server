use mdns_sd::{ServiceDaemon, ServiceInfo};
use rdev::{EventType, listen};
use std::mem::discriminant;
use std::net::Ipv4Addr;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio::task;
use tracing::{Level, info, warn};

mod protocol;
use protocol::Packet;

/// Channel payload: already-serialized Packet bytes
type PacketBytes = Vec<u8>;
const CHANNEL_CAP: usize = 128;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    // Logging
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    // 1. broadcast channel for fan-out
    let (tx, _rx) = broadcast::channel::<PacketBytes>(CHANNEL_CAP);

    // 2. mDNS advertisement
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

    // 3. capture events in a blocking thread
    task::spawn_blocking({
        let tx = tx.clone();
        move || {
            if let Err(e) = listen(make_callback(tx)) {
                warn!("rdev listen error: {:?}", e);
            }
        }
    });

    // 4. TCP listener
    let listener = TcpListener::bind((my_ip, 8888)).await?;
    info!("DeskRemote server listening on {}:8888", my_ip);

    // 5. accept loop
    loop {
        let (socket, addr) = listener.accept().await?;
        info!("New connection from {}", addr);
        let rx = tx.subscribe();
        tokio::spawn(async move {
            if let Err(e) = handle_conn(socket, rx).await {
                warn!("Connection {} error: {:?}", addr, e);
            }
        });
    }
}

/// Convert rdev::Key to a compact u8 identifier (PoC)
#[inline]
fn key_to_u8(key: rdev::Key) -> u8 {
    unsafe { std::mem::transmute_copy(&discriminant(&key)) }
}

/// Build the rdev callback that forwards events into the channel
fn make_callback(sender: broadcast::Sender<PacketBytes>) -> impl Fn(rdev::Event) {
    move |event| {
        let pkt = match event.event_type {
            EventType::MouseMove { x, y } => Packet::MouseMove { x, y },
            EventType::KeyPress(key) => Packet::KeyDown {
                code: key_to_u8(key),
            },
            EventType::KeyRelease(key) => Packet::KeyUp {
                code: key_to_u8(key),
            },
            _ => return,
        };
        let bytes = pkt.to_bytes();
        let _ = sender.send(bytes); // ignore “no receivers”
    }
}

/// Stream packets to one client
async fn handle_conn(
    mut socket: TcpStream,
    mut rx: broadcast::Receiver<PacketBytes>,
) -> std::io::Result<()> {
    loop {
        let bytes = rx.recv().await.expect("channel closed");
        let len = (bytes.len() as u32).to_be_bytes();
        socket.write_all(&len).await?;
        socket.write_all(&bytes).await?;
    }
}
