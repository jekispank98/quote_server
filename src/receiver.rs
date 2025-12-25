use std::ffi::c_ushort;
use crate::error::ParserError;
use crate::model::command::Command;
use crate::model::ping_monitor::PingMonitor;
use crate::model::tickers::Ticker;
use crossbeam_channel::{unbounded, Receiver, Sender};
use std::io::Read;
use std::net::{SocketAddr, TcpListener};
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;

pub struct QuoteReceiver {
    pub(crate) socket: TcpListener,
    // УДАЛИТЕ ping_monitor отсюда, он не нужен в этой структуре
}

impl QuoteReceiver {
    pub fn new(bind_addr: &str) -> Result<Self, ParserError> {
        let socket = TcpListener::bind(bind_addr)?;
        println!("TCP сервер команд запущен на {}", socket.local_addr()?);
        Ok(Self { socket }) // Только socket, без ping_monitor
    }

    pub fn start_with_channel(
        self,
        ping_monitor: Arc<Mutex<PingMonitor>>, // Добавляем параметром
        stop_tx: Sender<SocketAddr>,
    ) -> (JoinHandle<()>, Receiver<(Command, SocketAddr)>) {
        let (tx, rx) = unbounded();

        // ping_monitor уже передан как параметр
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
            println!("stream: {:?}", &stream);
            match stream {
                Ok(mut stream) => {
                    let client_tcp_addr = stream.peer_addr()?;
                    println!("client_tcp_addr: {:?}", &client_tcp_addr);
                    let mut buf = [0u8; 1024];

                    match stream.read(&mut buf) {
                        Ok(size) => {

                            if let Ok(command) = bincode::decode_from_slice::<Command, _>(
                                &buf[..size],
                                bincode::config::standard(),
                            ) {
                                println!("Received command {:?}", command);
                                let target_udp_addr = command.0.get_udp_addr()?;
                                tx.send((command.0, target_udp_addr))?;
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

// receiver.rs - функция для парсинга текстовых команд
fn parse_text_command(cmd: &str, client_tcp_addr: SocketAddr) -> Option<Command> {
    // Пример команды: STREAM udp://127.0.0.1:34254 AAPL,TSLA
    let parts: Vec<&str> = cmd.trim().split_whitespace().collect();

    if parts.len() >= 3 && parts[0] == "STREAM" {
        // Парсим UDP адрес из формата udp://127.0.0.1:34254
        let udp_url = parts[1];
        let udp_parts: Vec<&str> = udp_url.split("://").collect();

        if udp_parts.len() != 2 || udp_parts[0] != "udp" {
            eprintln!("Invalid UDP URL format: {}", udp_url);
            return None;
        }

        let udp_addr_parts: Vec<&str> = udp_parts[1].split(':').collect();
        if udp_addr_parts.len() != 2 {
            eprintln!("Invalid UDP address format: {}", udp_parts[1]);
            return None;
        }

        let udp_address = udp_addr_parts[0];
        let udp_port = udp_addr_parts[1];

        // Парсим тикеры
        let tickers: Vec<Ticker> = parts[2]
            .split(',')
            .filter_map(|t| t.parse::<Ticker>().ok())
            .collect();

        if !tickers.is_empty() {
            return Some(Command {
                header: "STREAM".to_string(),
                connection: "UDP".to_string(),
                address: client_tcp_addr.ip().to_string(),
                port: client_tcp_addr.port().to_string(),
                tickers,
            });
        }
    }

    None
}
