use std::net::{SocketAddr, UdpSocket};
use std::sync::mpsc;
use std::thread;
use std::thread::JoinHandle;
use crate::model::stock_quote::StockQuote;

pub struct QuoteReceiver {
    socket: UdpSocket
}

pub trait Receiver: Send + Sync {
    fn start_with_channel(
        self: Box<Self>,
    ) -> (
        thread::JoinHandle<()>,
        mpsc::Receiver<(StockQuote, SocketAddr)>,
    );
    fn receive_loop_with_channel(
        self: Box<Self>,
        tx: mpsc::Sender<(StockQuote, SocketAddr)>,
    ) -> Result<(), Box<dyn std::error::Error>>;
}

impl Receiver for QuoteReceiver {
    fn start_with_channel(
        self,
    ) -> (JoinHandle<()>, mpsc::Receiver<(StockQuote, SocketAddr)>) {
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
        tx: mpsc::Sender<(StockQuote, std::net::SocketAddr)>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut buf = [0u8; 1024];

        println!("Канал приёма данных активирован");

        loop {
            match self.socket.recv_from(&mut buf) {
                Ok((size, src_addr)) => {
                    match bincode::decode_from_slice::<StockQuote, _>(&buf[..size], bincode::config::standard()) {
                        Ok(metrics) => {
                            // Отправляем данные в основной поток
                            if tx.send((metrics.0, src_addr)).is_err() {
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