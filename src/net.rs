use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

use crate::sim::{PlayerInput, RallySim};

pub const DISCOVERY_PORT: u16 = 24680;
pub const GAME_PORT: u16 = 24681;

#[derive(Clone, Debug)]
pub struct DiscoveredSession {
    pub addr: SocketAddr,
    pub name: String,
    pub last_seen: Instant,
}

pub struct NetHost {
    discovery_sock: UdpSocket,
    game_sock: UdpSocket,
    game_name: String,
    pub peer: Option<SocketAddr>,
    pub peer_name: String,
    pub peer_color: usize,
    pub remote_input: PlayerInput,
}

impl NetHost {
    pub fn start(game_name: String) -> io::Result<Self> {
        Self::bind(game_name, DISCOVERY_PORT, GAME_PORT)
    }

    fn bind(game_name: String, discovery_port: u16, game_port: u16) -> io::Result<Self> {
        let discovery_sock = UdpSocket::bind(("0.0.0.0", discovery_port))?;
        discovery_sock.set_nonblocking(true)?;
        let game_sock = UdpSocket::bind(("0.0.0.0", game_port))?;
        game_sock.set_nonblocking(true)?;
        Ok(Self {
            discovery_sock,
            game_sock,
            game_name,
            peer: None,
            peer_name: "Player 2".to_string(),
            peer_color: 3,
            remote_input: PlayerInput::default(),
        })
    }

    /// Polls for discovery pings and client join requests. Returns `true` when a client has joined.
    pub fn poll_lobby(&mut self, host_name: &str, host_color: usize) -> bool {
        let mut buf = [0u8; 256];
        while let Ok((len, src)) = self.discovery_sock.recv_from(&mut buf) {
            if len >= 4 && &buf[0..4] == b"BVQ?" {
                let mut reply = b"BVR!".to_vec();
                reply.extend_from_slice(self.game_name.as_bytes());
                let _ = self.discovery_sock.send_to(&reply, src);
            }
        }
        while let Ok((len, src)) = self.game_sock.recv_from(&mut buf) {
            if len >= 6 && &buf[0..4] == b"BVJN" {
                self.peer = Some(src);
                self.peer_color = (buf[4] as usize) % 8;
                let nlen = (buf[5] as usize).min(len - 6).min(12);
                self.peer_name = String::from_utf8_lossy(&buf[6..6 + nlen]).to_string();
                let mut ack = vec![b'B', b'V', b'O', b'K', host_color as u8, host_name.len().min(12) as u8];
                ack.extend_from_slice(&host_name.as_bytes()[..host_name.len().min(12)]);
                let _ = self.game_sock.send_to(&ack, src);
                return true;
            }
        }
        false
    }

    pub fn poll_input(&mut self) -> Option<bool> {
        let mut buf = [0u8; 64];
        while let Ok((len, src)) = self.game_sock.recv_from(&mut buf) {
            if Some(src) == self.peer && len >= 5 {
                if &buf[0..4] == b"BVIN" {
                    let bits = buf[4];
                    self.remote_input.left = (bits & 1) != 0;
                    self.remote_input.right = (bits & 2) != 0;
                    self.remote_input.jump = (bits & 4) != 0;
                } else if &buf[0..4] == b"BVQT" {
                    return Some(true);
                }
            }
        }
        None
    }

    pub fn send_state(&self, rally: &RallySim, sound_mask: u8) {
        let Some(peer) = self.peer else { return };
        let mut pkt = [0u8; 27];
        pkt[0..4].copy_from_slice(b"BVST");
        pkt[4..8].copy_from_slice(&rally.ball.x.to_le_bytes());
        pkt[8..12].copy_from_slice(&rally.ball.y.to_le_bytes());
        pkt[12..14].copy_from_slice(&(rally.players[0].x as i16).to_le_bytes());
        pkt[14..16].copy_from_slice(&(rally.players[0].y as i16).to_le_bytes());
        pkt[16] = rally.players[0].anim_frame as u8;
        pkt[17..19].copy_from_slice(&(rally.players[1].x as i16).to_le_bytes());
        pkt[19..21].copy_from_slice(&(rally.players[1].y as i16).to_le_bytes());
        pkt[21] = rally.players[1].anim_frame as u8;
        pkt[22] = (rally.ball.spin_phase >> 3) as u8;
        pkt[23] = rally.players[0].score as u8;
        pkt[24] = rally.players[1].score as u8;
        pkt[25] = if rally.is_first_rally { 0xFF } else { rally.server as u8 };
        pkt[26] = sound_mask;
        let _ = self.game_sock.send_to(&pkt, peer);
    }

    pub fn send_quit(&self) {
        if let Some(peer) = self.peer {
            let _ = self.game_sock.send_to(b"BVQT", peer);
        }
    }
}

pub struct NetClient {
    sock: UdpSocket,
    pub sessions: Vec<DiscoveredSession>,
    pub host_addr: Option<SocketAddr>,
    pub host_name: String,
    pub host_color: usize,
}

impl NetClient {
    pub fn new() -> io::Result<Self> {
        let sock = UdpSocket::bind(("0.0.0.0", 0))?;
        sock.set_nonblocking(true)?;
        sock.set_broadcast(true)?;
        Ok(Self {
            sock,
            sessions: Vec::new(),
            host_addr: None,
            host_name: "Player 1".to_string(),
            host_color: 0,
        })
    }

    pub fn send_discovery(&mut self, target_ip: &str) {
        self.sessions.clear();
        let addr_str = if target_ip.trim().is_empty() {
            format!("255.255.255.255:{DISCOVERY_PORT}")
        } else {
            format!("{}:{DISCOVERY_PORT}", target_ip.trim())
        };
        let _ = self.sock.send_to(b"BVQ?", addr_str);
        if target_ip.trim().is_empty() {
            let _ = self.sock.send_to(b"BVQ?", format!("127.0.0.1:{DISCOVERY_PORT}"));
        }
    }

    pub fn poll_discovery(&mut self) {
        let mut buf = [0u8; 256];
        while let Ok((len, src)) = self.sock.recv_from(&mut buf) {
            if len >= 4 && &buf[0..4] == b"BVR!" {
                let name = String::from_utf8_lossy(&buf[4..len]).to_string();
                let game_addr = SocketAddr::new(src.ip(), GAME_PORT);
                if let Some(existing) = self.sessions.iter_mut().find(|s| s.addr == game_addr) {
                    existing.name = name;
                    existing.last_seen = Instant::now();
                } else {
                    self.sessions.push(DiscoveredSession {
                        addr: game_addr,
                        name,
                        last_seen: Instant::now(),
                    });
                }
            }
        }
        self.sessions.retain(|s| s.last_seen.elapsed() < Duration::from_secs(10));
    }

    pub fn join(&mut self, addr: SocketAddr, my_name: &str, my_color: usize) -> bool {
        let mut pkt = vec![b'B', b'V', b'J', b'N', my_color as u8, my_name.len().min(12) as u8];
        pkt.extend_from_slice(&my_name.as_bytes()[..my_name.len().min(12)]);
        for _ in 0..5 {
            let _ = self.sock.send_to(&pkt, addr);
            std::thread::sleep(Duration::from_millis(30));
            let mut buf = [0u8; 64];
            if let Ok((len, src)) = self.sock.recv_from(&mut buf) {
                if src == addr && len >= 6 && &buf[0..4] == b"BVOK" {
                    self.host_addr = Some(addr);
                    self.host_color = (buf[4] as usize) % 8;
                    let nlen = (buf[5] as usize).min(len - 6).min(12);
                    self.host_name = String::from_utf8_lossy(&buf[6..6 + nlen]).to_string();
                    return true;
                }
            }
        }
        false
    }

    pub fn send_input(&self, input: PlayerInput) {
        let Some(addr) = self.host_addr else { return };
        let bits = (u8::from(input.left)) | (u8::from(input.right) << 1) | (u8::from(input.jump) << 2);
        let pkt = [b'B', b'V', b'I', b'N', bits];
        let _ = self.sock.send_to(&pkt, addr);
    }

    pub fn poll_state(&self) -> Option<ClientStatePacket> {
        let mut latest = None;
        let mut buf = [0u8; 64];
        while let Ok((len, src)) = self.sock.recv_from(&mut buf) {
            if Some(src) == self.host_addr && len >= 4 {
                if &buf[0..4] == b"BVQT" {
                    return Some(ClientStatePacket::Quit);
                }
                if &buf[0..4] == b"BVST" && len >= 27 {
                    latest = Some(ClientStatePacket::State {
                        ball_x: i32::from_le_bytes(buf[4..8].try_into().unwrap()),
                        ball_y: i32::from_le_bytes(buf[8..12].try_into().unwrap()),
                        p0_x: i16::from_le_bytes(buf[12..14].try_into().unwrap()) as i32,
                        p0_y: i16::from_le_bytes(buf[14..16].try_into().unwrap()) as i32,
                        p0_frame: buf[16] as i32,
                        p1_x: i16::from_le_bytes(buf[17..19].try_into().unwrap()) as i32,
                        p1_y: i16::from_le_bytes(buf[19..21].try_into().unwrap()) as i32,
                        p1_frame: buf[21] as i32,
                        spin_frame: buf[22],
                        scores: [buf[23] as i32, buf[24] as i32],
                        server_raw: buf[25],
                        sound_mask: buf[26],
                    });
                }
            }
        }
        latest
    }

    pub fn send_quit(&self) {
        if let Some(addr) = self.host_addr {
            let _ = self.sock.send_to(b"BVQT", addr);
        }
    }
}

pub enum ClientStatePacket {
    State {
        ball_x: i32,
        ball_y: i32,
        p0_x: i32,
        p0_y: i32,
        p0_frame: i32,
        p1_x: i32,
        p1_y: i32,
        p1_frame: i32,
        spin_frame: u8,
        scores: [i32; 2],
        server_raw: u8,
        sound_mask: u8,
    },
    Quit,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::PlayerControl;

    #[test]
    fn host_and_client_handshake_roundtrip() {
        let mut host = NetHost::bind("TestGame".to_string(), 0, 0).unwrap();
        let game_addr = SocketAddr::new(
            std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
            host.game_sock.local_addr().unwrap().port(),
        );
        let handle = std::thread::spawn(move || {
            let mut client = NetClient::new().unwrap();
            let ok = client.join(game_addr, "Guest", 5);
            (ok, client.host_name, client.host_color)
        });

        let mut joined = false;
        for _ in 0..20 {
            if host.poll_lobby("Host", 2) {
                joined = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(joined);
        assert_eq!(host.peer_name, "Guest");
        assert_eq!(host.peer_color, 5);

        let (ok, host_name, host_color) = handle.join().unwrap();
        assert!(ok);
        assert_eq!(host_name, "Host");
        assert_eq!(host_color, 2);
    }

    #[test]
    fn state_packet_serialization_roundtrip() {
        let mut host = NetHost::bind("TestGame".to_string(), 0, 0).unwrap();
        let mut client = NetClient::new().unwrap();
        let client_addr = SocketAddr::new(
            std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
            client.sock.local_addr().unwrap().port(),
        );
        let host_addr = SocketAddr::new(
            std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
            host.game_sock.local_addr().unwrap().port(),
        );
        host.peer = Some(client_addr);
        client.host_addr = Some(host_addr);

        let mut rally = RallySim::new(
            [PlayerControl::Keyboard, PlayerControl::Network],
            [7, 4],
            1,
            false,
            1,
        );
        rally.ball.x = 12345;
        rally.ball.y = 6789;
        rally.ball.spin_phase = 5 << 3;
        host.send_state(&rally, 3);

        std::thread::sleep(Duration::from_millis(5));
        match client.poll_state() {
            Some(ClientStatePacket::State {
                ball_x,
                ball_y,
                scores,
                spin_frame,
                server_raw,
                sound_mask,
                ..
            }) => {
                assert_eq!(ball_x, 12345);
                assert_eq!(ball_y, 6789);
                assert_eq!(scores, [7, 4]);
                assert_eq!(spin_frame, 5);
                assert_eq!(server_raw, 1);
                assert_eq!(sound_mask, 3);
            }
            _ => panic!("Expected State packet"),
        }

        client.send_input(PlayerInput {
            left: true,
            right: false,
            jump: true,
        });
        std::thread::sleep(Duration::from_millis(5));
        assert_eq!(host.poll_input(), None);
        assert!(host.remote_input.left);
        assert!(!host.remote_input.right);
        assert!(host.remote_input.jump);
    }
}
