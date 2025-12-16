use std::net::UdpSocket;
use std::sync::{Arc, Mutex};
use std::thread;
use crate::model::ping_monitor::PingMonitor;

pub struct UdpPingListener {
    socket: Arc<UdpSocket>,
    ping_monitor: Arc<Mutex<PingMonitor>>,
}

impl UdpPingListener {
    pub fn start(socket: Arc<UdpSocket>, ping_monitor: Arc<Mutex<PingMonitor>>) {
        thread::spawn(move || {
            let mut buf = [0u8; 128];
            loop {
                if let Ok((_, src_addr)) = socket.recv_from(&mut buf) {
                    // Если пришел пакет, считаем его пингом от клиента
                    let mut monitor = ping_monitor.lock().unwrap();
                    monitor.update_ping(src_addr);
                }
            }
        });
    }
}