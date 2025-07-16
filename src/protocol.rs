use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum Packet {
    MouseMove { x: f64, y: f64 },
    KeyDown { code: u8 },
    KeyUp { code: u8 },
}

impl Packet {
    pub fn to_bytes(&self) -> Vec<u8> {
        bincode::serialize(self).unwrap()
    }
    pub fn from_bytes(buf: &[u8]) -> Result<Self, bincode::Error> {
        bincode::deserialize(buf)
    }
}
