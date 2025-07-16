use std::io::Read;
use std::net::TcpStream;

#[test]
fn server_streams_mouse_moves() {
    let mut conn = TcpStream::connect("192.168.x.x:8888").unwrap();
    let mut buf = vec![0; 1024];
    let n = conn.read(&mut buf).unwrap();
    assert!(n >= 4); // at least the length header
}
