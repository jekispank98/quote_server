use crate::model::ping_monitor::PingMonitor;
use std::net::UdpSocket;
use std::sync::{Arc, Mutex};
use std::thread;

pub struct UdpPingListener;

impl UdpPingListener {
    pub fn start(socket: Arc<UdpSocket>, ping_monitor: Arc<Mutex<PingMonitor>>) {
        thread::spawn(move || {
            let mut buf = [0u8; 128];
            loop {
                if let Ok((size, addr)) = socket.recv_from(&mut buf) {
                    if size >= 4 && &buf[..4] == b"PING" {
                        println!("Received ping from {}", addr);
                        let mut monitor = ping_monitor.lock().unwrap();
                        monitor.update_ping(addr);
                    }
                }
            }
        });
    }
}