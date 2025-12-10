use crate::model::command::Command;
use crate::model::ping_monitor::PingMonitor;
use std::net::{SocketAddr, UdpSocket};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;
use crossbeam_channel::Sender;

pub struct QuoteReceiver {
    pub(crate) socket: UdpSocket,
    ping_monitor: Arc<Mutex<PingMonitor>>
}

pub trait Receiver: Send + Sync {
    fn start_with_channel(
        self: Box<Self>,
        stop_tx: Sender<SocketAddr>
    ) -> (JoinHandle<()>, mpsc::Receiver<(Command, SocketAddr)>);
}

impl Receiver for QuoteReceiver {
    fn start_with_channel(
        self: Box<Self>,
        stop_tx: Sender<SocketAddr>
    ) -> (JoinHandle<()>, mpsc::Receiver<(Command, SocketAddr)>) {
        QuoteReceiver::start_with_channel(*self, stop_tx)
    }
}
impl QuoteReceiver {
    pub fn new(bind_addr: &str) -> Result<Self, std::io::Error> {
        let socket = UdpSocket::bind(bind_addr)?;
        let ping_monitor = Arc::new(Mutex::new(PingMonitor::new(5)));
        println!("Ресивер запущен на {}", bind_addr);
        Ok(Self { socket, ping_monitor })
    }
    /*pub fn start_with_channel(self) -> (JoinHandle<()>, mpsc::Receiver<(Command, SocketAddr)>) {
        let (tx, rx) = mpsc::channel();

        let handle = thread::spawn(move || {
            if let Err(e) = self.receive_loop_with_channel(tx) {
                eprintln!("Ошибка в receive_loop_with_channel: {}", e);
            }
        });
        (handle, rx)
    }*/

    pub fn start_with_channel(
        self,
        stop_tx: Sender<SocketAddr>,
    ) -> (JoinHandle<()>, mpsc::Receiver<(Command, SocketAddr)>) {
        let (tx, rx) = mpsc::channel();
        let ping_monitor = self.ping_monitor.clone();
        let monitor_handle = Self::start_ping_monitor(ping_monitor.clone(), stop_tx);
        let handle = thread::spawn(move || {
            if let Err(e) = self.receive_loop_with_channel(tx, ping_monitor) {
                eprintln!("Ошибка в receive_loop_with_channel: {}", e);
            }
        });
        (handle, rx)
    }

    fn start_ping_monitor(
        ping_monitor: Arc<Mutex<PingMonitor>>,
        stop_tx: Sender<SocketAddr>,
    ) -> JoinHandle<()> {
        thread::spawn(move || {
            let check_interval = Duration::from_secs(1);

            loop {
                thread::sleep(check_interval);

                let timed_out_clients = {
                    let mut monitor = ping_monitor.lock().unwrap();
                    monitor.check_timeouts()
                };

                // Уведомляем о таймаутах
                for client_addr in timed_out_clients {
                    if let Err(e) = stop_tx.send(client_addr) {
                        eprintln!("Ошибка отправки уведомления о таймауте: {}", e);
                    }
                }
            }
        })
    }



    /*fn receive_loop_with_channel(
        self,
        tx: mpsc::Sender<(Command, SocketAddr)>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut buf = [0u8; 1024];

        println!("Канал приёма данных активирован");

        loop {
            match self.socket.recv_from(&mut buf) {
                Ok((size, src_addr)) => {
                    match bincode::decode_from_slice::<Command, _>(
                        &buf[..size],
                        bincode::config::standard(),
                    ) {
                        Ok(quotes) => {
                            if tx.send((quotes.0, src_addr)).is_err() {
                                println!("Канал закрыт, завершение потока приёма");
                                break;
                            }
                        }
                        Err(e) => {
                            eprintln!("Ошибка десериализации: {}", e);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Ошибка получения данных: {}", e);
                }
            }
        }

        Ok(())
    }*/

    fn receive_loop_with_channel(
        self,
        tx: mpsc::Sender<(Command, SocketAddr)>,
        ping_monitor: Arc<Mutex<PingMonitor>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut buf = [0u8; 1024];
        println!("Канал приёма данных активирован");

        loop {
            match self.socket.recv_from(&mut buf) {
                Ok((size, src_addr)) => {
                    match bincode::decode_from_slice::<Command, _>(
                        &buf[..size],
                        bincode::config::standard(),
                    ) {
                        Ok((command, _)) => {
                            match command.header.as_str() {
                                "PING" => {
                                    let mut monitor = ping_monitor.lock().unwrap();
                                    monitor.update_ping(src_addr);
                                    println!("Ping получен от {}", src_addr);
                                }
                                _ => {
                                    let is_active = {
                                        let monitor = ping_monitor.lock().unwrap();
                                        monitor.is_client_active(&src_addr)
                                    };

                                    if is_active {
                                        if tx.send((command, src_addr)).is_err() {
                                            println!("Канал закрыт, завершение потока приёма");
                                            break;
                                        }
                                    } else {
                                        println!("Клиент {} неактивен (таймаут ping), игнорируем команду", src_addr);
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Ошибка десериализации: {}", e);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Ошибка получения данных: {}", e);
                }
            }
        }

        Ok(())
    }
}
