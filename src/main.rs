//! Quotes UDP streaming server.
//!
//! This binary listens on a UDP socket and streams quote updates to clients that send a
//! subscription command. Internally, it wires together three main building blocks:
//!
//! - `QuoteGenerator` — produces quote events (`QuoteEvent`) and broadcasts them to all
//!   subscribed clients via `crossbeam_channel` senders.
//! - `QuoteReceiver` — listens for incoming UDP datagrams with client commands and parses
//!   them into a command structure (e.g., a subscription with requested tickers) along with the
//!   sender's `SocketAddr`.
//! - Per‑client stream task — a lightweight thread created for each client to filter quotes
//!   by the client's requested tickers and send matching quotes back to that client's address.
//!
//! Concurrency and shutdown:
//! - Crossbeam `select!` is used to multiplex incoming quotes and shutdown signals.
//! - Each client stream owns a `shutdown_rx` that is triggered either by a keep‑alive timeout
//!   (detected by `QuoteReceiver`) or by a global `QuoteEvent::Shutdown` broadcast from the
//!   generator when the application is terminating.
//! - Any I/O or channel receive error is surfaced as `ParserError` and logged; the specific
//!   client stream exits gracefully without impacting other clients.
//!
//! Network protocol (high‑level):
//! - Bind address: `0.0.0.0:8080` (see `BIND_ADDRESS`).
//! - Client sends a subscription command (header like `J_QUOTE`) with a list of tickers.
//! - Server spawns a stream thread for that client and starts sending binary‑encoded quote
//!   payloads (`Quote::to_bytes()`) to the client's `SocketAddr`.
//!
//! Note: This file only orchestrates; details such as the exact command format, `Quote`
//! serialization, and ticker parsing live under the `model` and `receiver` modules.
#![warn(missing_docs)]
use crate::error::ParserError;
use crate::model::command::Command;
use crate::model::ping_monitor::PingMonitor;
use crate::model::quote_generator::{QuoteEvent, QuoteGenerator};
use crate::model::quote::Quote;
use crate::model::tickers::Ticker;
use crate::receiver::QuoteReceiver;
use crate::udp_listener::UdpPingListener;
use bincode::Decode;
use crossbeam_channel::{select, unbounded, Receiver, Sender};
use result::Result;
use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::sync::{Arc, Mutex};
use std::thread;

mod error;
pub mod model;
mod receiver;
mod result;
mod udp_listener;

/// Default UDP bind address for the quote server.
const BIND_ADDRESS: &str = "127.0.0.1:8080";
/// Stream task for a single client.
///
/// Listens for quote events on `data_rx`, filters them by the client's `tickers`, and
/// forwards matching quotes to the client's `target_addr` via the provided UDP `socket`.
/// The task terminates when either:
/// - a shutdown signal is received on `stop_rx`, or
/// - a `QuoteEvent::Shutdown` is received from the quote generator, or
/// - a send/receive error occurs.
///
/// Errors are propagated as `ParserError` so the caller can log and recover per client.
// main.rs - исправленная функция handle_client_stream
// main.rs - исправленная функция handle_client_stream
pub fn handle_client_stream(
    socket: Arc<UdpSocket>,
    target_addr: SocketAddr,
    tickers: Vec<Ticker>,
    data_rx: Receiver<QuoteEvent>,
    stop_rx: Receiver<()>,
) -> Result<(), ParserError> {
    let tickers_str: Vec<String> = tickers.iter().map(|t| t.to_string()).collect();

    loop {
        select! {
            recv(stop_rx) -> _ => break,
            recv(data_rx) -> msg => match msg {
                Ok(QuoteEvent::Quote(quote)) => {
                    if tickers_str.contains(&quote.ticker) {
                        let data = quote.to_bytes();
                        // Используем send_to для UDP
                        if let Err(e) = socket.send_to(&data, target_addr) {
                            eprintln!("Failed to send UDP packet to {}: {}", target_addr, e);
                            break;
                        }
                    }
                },
                Ok(QuoteEvent::Shutdown) => break,
                Err(e) => {
                    println!("Ошибка при получении сообщения: {}", e);
                    break;
                },
            }
        }
    }
    Ok(())
}

fn main() -> Result<(), ParserError> {
    // 1. Общий UDP сокет для работы с данными и пингами
    let udp_socket = Arc::new(UdpSocket::bind("0.0.0.0:8081")?);
    println!("UDP сокет создан на порту: {}", udp_socket.local_addr()?);

    // 2. Клонируем сокет для пинг-листенера
    let ping_socket = Arc::clone(&udp_socket);

    // 3. Мониторинг пингов
    let ping_monitor = Arc::new(Mutex::new(PingMonitor::new(5)));
    let (stop_tx, stop_rx) = unbounded::<SocketAddr>();

    // 4. Запускаем поток, который слушает пинги по UDP
    let ping_monitor_clone = Arc::clone(&ping_monitor);
    thread::spawn(move || {
        UdpPingListener::start(ping_socket, ping_monitor_clone);
    });

    // 5. Запускаем поток, который проверяет таймауты
    let stop_tx_clone = stop_tx.clone();
    let ping_monitor_for_checker = Arc::clone(&ping_monitor);
    thread::spawn(move || {
        // Если у вас есть статический метод start_ping_monitor
        // Или создайте отдельную функцию
        start_ping_monitor(ping_monitor_for_checker, stop_tx_clone);
    });

    // 6. TCP Ресивер для команд (Пункт 2 и 3)
    let (cmd_tx, cmd_rx) = unbounded::<(Command, SocketAddr)>();
    let tcp_receiver = QuoteReceiver::new(BIND_ADDRESS)?;
    thread::spawn(move || {
        if let Err(e) = tcp_receiver.receive_loop_with_channel(cmd_tx) {
            eprintln!("Receiver loop failed: {:?}", e);
        };
    });

    let subscription_tx = QuoteGenerator::start();
    let mut active_streams: HashMap<SocketAddr, (Sender<()>, Sender<QuoteEvent>)> = HashMap::new();

    loop {
        select! {
            recv(cmd_rx) -> msg => if let Ok((cmd, target_udp_addr)) = msg {
                let (shutdown_tx, shutdown_rx) = unbounded::<()>();
                let (client_data_tx, client_data_rx) = unbounded::<QuoteEvent>();

                // Подписываем клиента на генератор
                if let Err(e) = subscription_tx.send(client_data_tx.clone()) {
                    eprintln!("Failed to subscribe client: {}", e);
                    continue;
                }

                // Сохраняем оба канала
                active_streams.insert(target_udp_addr, (shutdown_tx, client_data_tx));

                let socket_clone = Arc::clone(&udp_socket);
                let tickers = cmd.tickers;

                // Запускаем поток для клиента с указанным UDP адресом
                thread::spawn(move || {
                    if let Err(e) = handle_client_stream(
                        socket_clone,
                        target_udp_addr,
                        tickers,
                        client_data_rx,
                        shutdown_rx,
                    ) {
                        eprintln!("Client stream error: {:?}", e);
                    }
                });
                println!("Создан стрим для клиента на UDP адресе: {}", target_udp_addr);
            },

            recv(stop_rx) -> addr => if let Ok(client_addr) = addr {
                if let Some((shutdown_tx, _)) = active_streams.remove(&client_addr) {
                    let _ = shutdown_tx.send(());
                    println!("Стрим для {} закрыт: таймаут пинга", client_addr);
                }
            }
        }
    }
}

// Добавьте эту функцию если нет QuoteReceiver::start_ping_monitor
fn start_ping_monitor(
    ping_monitor: Arc<Mutex<PingMonitor>>,
    stop_tx: Sender<SocketAddr>,
) {
    thread::spawn(move || {
        let check_interval = std::time::Duration::from_secs(1);

        loop {
            std::thread::sleep(check_interval);

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
    });
}