use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use mc_core::{BlockPos, SharedConstants};
use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

/// Minecraft VarInt codec (up to 5 bytes).
pub fn write_varint<W: Write>(w: &mut W, mut val: i32) -> io::Result<()> {
    let mut u = val as u32;
    loop {
        if (u & !0x7F) == 0 {
            w.write_all(&[u as u8])?;
            return Ok(());
        }
        w.write_all(&[((u & 0x7F) | 0x80) as u8])?;
        u >>= 7;
        let _ = &mut val;
    }
}

pub fn read_varint<R: Read>(r: &mut R) -> io::Result<i32> {
    let mut num_read = 0;
    let mut result = 0u32;
    loop {
        let mut buf = [0u8; 1];
        r.read_exact(&mut buf)?;
        let read = buf[0];
        let val = (read & 0x7F) as u32;
        result |= val << (7 * num_read);
        num_read += 1;
        if num_read > 5 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "VarInt too big"));
        }
        if (read & 0x80) == 0 {
            break;
        }
    }
    Ok(result as i32)
}

pub fn write_mc_string<W: Write>(w: &mut W, s: &str) -> io::Result<()> {
    let bytes = s.as_bytes();
    write_varint(w, bytes.len() as i32)?;
    w.write_all(bytes)
}

pub fn read_mc_string<R: Read>(r: &mut R) -> io::Result<String> {
    let len = read_varint(r)?.max(0) as usize;
    if len > 32767 * 4 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "String too long"));
    }
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf)?;
    String::from_utf8(buf).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Handshaking = 0,
    Status = 1,
    Login = 2,
    Configuration = 3,
    Play = 4,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerStatusJson {
    pub version: StatusVersion,
    pub players: StatusPlayers,
    pub description: StatusText,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusVersion {
    pub name: String,
    pub protocol: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusPlayers {
    pub max: u32,
    pub online: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusText {
    pub text: String,
}

/// Framed Minecraft packet (supports protocol_version = 775 uncompressed & zlib-compressed framing).
#[derive(Debug, Clone)]
pub struct RawPacket {
    pub id: i32,
    pub payload: Vec<u8>,
}

impl RawPacket {
    pub fn write_framed<W: Write>(&self, w: &mut W, compression_threshold: Option<usize>) -> io::Result<()> {
        let mut inner = Vec::new();
        write_varint(&mut inner, self.id)?;
        inner.extend_from_slice(&self.payload);

        match compression_threshold {
            None => {
                write_varint(w, inner.len() as i32)?;
                w.write_all(&inner)?;
            }
            Some(thresh) => {
                if inner.len() >= thresh {
                    let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
                    enc.write_all(&inner)?;
                    let compressed = enc.finish()?;
                    let mut body = Vec::new();
                    write_varint(&mut body, inner.len() as i32)?;
                    body.extend_from_slice(&compressed);
                    write_varint(w, body.len() as i32)?;
                    w.write_all(&body)?;
                } else {
                    let mut body = Vec::new();
                    write_varint(&mut body, 0)?;
                    body.extend_from_slice(&inner);
                    write_varint(w, body.len() as i32)?;
                    w.write_all(&body)?;
                }
            }
        }
        Ok(())
    }

    pub fn read_framed<R: Read>(r: &mut R, compressed: bool) -> io::Result<Self> {
        let frame_len = read_varint(r)?.max(0) as usize;
        let mut frame = vec![0u8; frame_len];
        r.read_exact(&mut frame)?;
        let mut cur = io::Cursor::new(&frame);

        if !compressed {
            let id = read_varint(&mut cur)?;
            let pos = cur.position() as usize;
            return Ok(Self {
                id,
                payload: frame[pos..].to_vec(),
            });
        }

        let data_len = read_varint(&mut cur)?.max(0) as usize;
        let pos = cur.position() as usize;
        let rest = &frame[pos..];
        let decompressed = if data_len == 0 {
            rest.to_vec()
        } else {
            let mut dec = ZlibDecoder::new(rest);
            let mut out = Vec::with_capacity(data_len);
            dec.read_to_end(&mut out)?;
            out
        };
        let mut dcur = io::Cursor::new(&decompressed);
        let id = read_varint(&mut dcur)?;
        let dpos = dcur.position() as usize;
        Ok(Self {
            id,
            payload: decompressed[dpos..].to_vec(),
        })
    }
}

/// High-level multiplayer sync message for LAN / dedicated server sessions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NetMessage {
    PlayerMove {
        name: String,
        x: f64,
        y: f64,
        z: f64,
        yaw: f32,
        pitch: f32,
    },
    BlockChange {
        pos: BlockPos,
        state_raw: u32,
    },
    ChatMessage {
        sender: String,
        text: String,
    },
}

pub struct LanServer {
    pub port: u16,
    pub seed: i64,
    pub peers: Arc<Mutex<Vec<TcpStream>>>,
}

impl LanServer {
    pub fn start(port: u16, seed: i64) -> io::Result<Self> {
        let listener = TcpListener::bind(("0.0.0.0", port))?;
        let actual_port = listener.local_addr()?.port();
        let peers: Arc<Mutex<Vec<TcpStream>>> = Arc::new(Mutex::new(Vec::new()));
        let peers_clone = Arc::clone(&peers);

        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut s = stream;
                let mut state = ConnectionState::Handshaking;
                loop {
                    let Ok(pkt) = RawPacket::read_framed(&mut s, false) else {
                        break;
                    };
                    match (state, pkt.id) {
                        (ConnectionState::Handshaking, 0) => {
                            let mut cur = io::Cursor::new(&pkt.payload);
                            let _proto = read_varint(&mut cur).unwrap_or(SharedConstants::PROTOCOL_VERSION);
                            let _addr = read_mc_string(&mut cur).unwrap_or_default();
                            let mut port_b = [0u8; 2];
                            let _ = cur.read_exact(&mut port_b);
                            let next = read_varint(&mut cur).unwrap_or(1);
                            state = if next == 2 {
                                ConnectionState::Login
                            } else {
                                ConnectionState::Status
                            };
                        }
                        (ConnectionState::Status, 0) => {
                            let status = ServerStatusJson {
                                version: StatusVersion {
                                    name: SharedConstants::VERSION_NAME.to_string(),
                                    protocol: SharedConstants::PROTOCOL_VERSION,
                                },
                                players: StatusPlayers {
                                    max: 20,
                                    online: peers_clone.lock().map(|p| p.len() as u32 + 1).unwrap_or(1),
                                },
                                description: StatusText {
                                    text: "Minecraft 26.1.2 Rust Edition".to_string(),
                                },
                            };
                            let mut payload = Vec::new();
                            let _ = write_mc_string(&mut payload, &serde_json::to_string(&status).unwrap_or_default());
                            let resp = RawPacket { id: 0, payload };
                            let _ = resp.write_framed(&mut s, None);
                        }
                        (ConnectionState::Status, 1) => {
                            let resp = RawPacket {
                                id: 1,
                                payload: pkt.payload,
                            };
                            let _ = resp.write_framed(&mut s, None);
                            break;
                        }
                        (ConnectionState::Login, 0) => {
                            if let Ok(mut list) = peers_clone.lock() {
                                if let Ok(cloned) = s.try_clone() {
                                    list.push(cloned);
                                }
                            }
                            let mut payload = Vec::new();
                            let _ = write_mc_string(&mut payload, "Player");
                            let _ = RawPacket { id: 2, payload }.write_framed(&mut s, None);
                            state = ConnectionState::Play;
                        }
                        (ConnectionState::Play, _) => {
                            if let Ok(mut list) = peers_clone.lock() {
                                list.retain_mut(|peer| pkt.write_framed(peer, None).is_ok());
                            }
                        }
                        _ => break,
                    }
                }
            }
        });

        Ok(Self {
            port: actual_port,
            seed,
            peers,
        })
    }
}

pub fn ping_server(addr: &str) -> io::Result<ServerStatusJson> {
    let mut stream = TcpStream::connect(addr)?;
    let mut hs = Vec::new();
    write_varint(&mut hs, SharedConstants::PROTOCOL_VERSION)?;
    write_mc_string(&mut hs, addr)?;
    stream.set_read_timeout(Some(std::time::Duration::from_secs(3)))?;
    hs.extend_from_slice(&25565u16.to_be_bytes());
    write_varint(&mut hs, 1)?;
    RawPacket { id: 0, payload: hs }.write_framed(&mut stream, None)?;
    RawPacket { id: 0, payload: vec![] }.write_framed(&mut stream, None)?;

    let resp = RawPacket::read_framed(&mut stream, false)?;
    let mut cur = io::Cursor::new(&resp.payload);
    let json_str = read_mc_string(&mut cur)?;
    serde_json::from_str(&json_str).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varint_and_slp_server_roundtrip() {
        let mut buf = Vec::new();
        for v in [0, 1, 127, 128, 775, 4790, -1] {
            buf.clear();
            write_varint(&mut buf, v).unwrap();
            let mut cur = io::Cursor::new(&buf);
            assert_eq!(read_varint(&mut cur).unwrap(), v);
        }

        let srv = LanServer::start(0, 42).unwrap();
        let status = ping_server(&format!("127.0.0.1:{}", srv.port)).unwrap();
        assert_eq!(status.version.protocol, SharedConstants::PROTOCOL_VERSION);
        assert_eq!(status.version.name, "26.1.2");
    }
}
