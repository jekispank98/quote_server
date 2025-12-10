use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};
use std::net::{SocketAddr, UdpSocket};
// pub type ClientMap = Arc<RwLock<HashMap<SocketAddr, Instant>>>;

pub struct QuoteReceiver {
    socket: UdpSocket,
    ping_monitor: Arc<Mutex<PingMonitor>>,
}

struct ClientConnection {
    last_ping: Instant,
    is_active: bool,
}

pub struct PingMonitor {
    clients: HashMap<SocketAddr, ClientConnection>,
    timeout: Duration,
}

impl PingMonitor {
    pub fn new(timeout_secs: u64) -> Self {
        Self {
            clients: HashMap::new(),
            timeout: Duration::from_secs(timeout_secs),
        }
    }

    pub fn update_ping(&mut self, addr: SocketAddr) {
        let now = Instant::now();
        self.clients.entry(addr)
            .and_modify(|conn| {
                conn.last_ping = now;
                conn.is_active = true;
            })
            .or_insert(ClientConnection {
                last_ping: now,
                is_active: true,
            });
    }

    pub fn check_timeouts(&mut self) -> Vec<SocketAddr> {
        let now = Instant::now();
        let mut timed_out = Vec::new();

        for (addr, conn) in &mut self.clients {
            if conn.is_active && now.duration_since(conn.last_ping) > self.timeout {
                conn.is_active = false;
                timed_out.push(*addr);
            }
        }

        timed_out
    }

    pub fn remove_client(&mut self, addr: &SocketAddr) {
        self.clients.remove(addr);
    }

    pub fn is_client_active(&self, addr: &SocketAddr) -> bool {
        self.clients.get(addr)
            .map(|conn| conn.is_active)
            .unwrap_or(false)
    }
}