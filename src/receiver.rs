use crate::model::command::Command;
use std::net::{SocketAddr, UdpSocket};
use std::sync::mpsc;
use std::thread;
use std::thread::JoinHandle;

pub struct QuoteReceiver {
    socket: UdpSocket,
}

pub trait Receiver: Send + Sync {
    fn start_with_channel(
        self: Box<Self>,
    ) -> (JoinHandle<()>, mpsc::Receiver<(Command, SocketAddr)>);
}

impl Receiver for QuoteReceiver {
    fn start_with_channel(
        self: Box<Self>,
    ) -> (JoinHandle<()>, mpsc::Receiver<(Command, SocketAddr)>) {
        QuoteReceiver::start_with_channel(*self)
    }
}
impl QuoteReceiver {
    pub fn new(bind_addr: &str) -> Result<Self, std::io::Error> {
        let socket = UdpSocket::bind(bind_addr)?;
        println!("Ресивер запущен на {}", bind_addr);
        Ok(Self { socket })
    }
    pub fn start_with_channel(self) -> (JoinHandle<()>, mpsc::Receiver<(Command, SocketAddr)>) {
        let (tx, rx) = mpsc::channel();

        let handle = thread::spawn(move || {
            if let Err(e) = self.receive_loop_with_channel(tx) {
                eprintln!("Ошибка в receive_loop_with_channel: {}", e);
            }
        });
        (handle, rx)
    }
    fn receive_loop_with_channel(
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
                            // Отправляем данные в основной поток
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
    }
}
