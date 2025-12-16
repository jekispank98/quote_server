use crate::error::ParserError;
use crate::model::command::Command;
use crate::model::ping_monitor::PingMonitor;
use crossbeam_channel::{unbounded, Receiver, Sender};
use std::io::Read;
use std::net::{IpAddr, SocketAddr, TcpListener};
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;

pub struct QuoteReceiver {
    pub(crate) socket: TcpListener,
    ping_monitor: Arc<Mutex<PingMonitor>>,
}
impl QuoteReceiver {
    pub fn new(bind_addr: &str) -> Result<Self, ParserError> {
        let socket = TcpListener::bind(bind_addr)?;
        let ping_monitor = Arc::new(Mutex::new(PingMonitor::new(5)));
        println!("Ресивер запущен на {}", bind_addr);
        Ok(Self {
            socket,
            ping_monitor,
        })
    }

    pub fn start_with_channel(
        self,
        stop_tx: Sender<SocketAddr>,
    ) -> (JoinHandle<()>, Receiver<(Command, SocketAddr)>) {
        let (tx, rx) = unbounded();
        let ping_monitor = self.ping_monitor.clone();
        Self::start_ping_monitor(ping_monitor.clone(), stop_tx);
        let handle = thread::spawn(move || {
            if let Err(e) = self.receive_loop_with_channel(tx) {
                eprintln!("Ошибка в receive_loop_with_channel: {}", e);
            }
        });
        (handle, rx)
    }

    pub(crate) fn start_ping_monitor(
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

                for client_addr in timed_out_clients {
                    if let Err(e) = stop_tx.send(client_addr) {
                        eprintln!("Ошибка отправки уведомления о таймауте: {}", e);
                    }
                }
            }
        })
    }
    pub(crate) fn receive_loop_with_channel(
        self,
        tx: Sender<(Command, SocketAddr)>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        println!("TCP сервер команд запущен на {}", self.socket.local_addr()?);

        for stream in self.socket.incoming() {
            match stream {
                Ok(mut stream) => {
                    let mut buf = [0u8; 1024];
                    match stream.read(&mut buf) {
                        Ok(size) => {
                            // Парсим команду (например, бинарную через bincode)
                            if let Ok((command, _)) = bincode::decode_from_slice::<Command, _>(&buf[..size], bincode::config::standard()) {
                                // В команде теперь должен быть адрес UDP, куда стримить
                                let target_udp_addr = format!("{}:{}", &command.address, &command.port)
                                    .parse()?;
                                tx.send((command, target_udp_addr))?;
                            }
                        }
                        Err(e) => eprintln!("Ошибка чтения TCP: {}", e),
                    }
                }
                Err(e) => eprintln!("Ошибка TCP соединения: {}", e),
            }
        }
        Ok(())
    }
}
