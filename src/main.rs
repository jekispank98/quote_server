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
use crate::model::quote_generator::{QuoteEvent, QuoteGenerator};
use crate::model::tickers::Ticker;
use crate::receiver::QuoteReceiver;
use crossbeam_channel::{select, unbounded, Receiver, Sender};
use result::Result;
use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::sync::{Arc, Mutex};
use std::thread;
use crate::model::command::Command;
use crate::model::ping_monitor::PingMonitor;
use crate::udp_listener::UdpPingListener;

mod error;
mod model;
mod receiver;
mod result;
mod udp_listener;

/// Default UDP bind address for the quote server.
const BIND_ADDRESS: &str = "0.0.0.0:8080";
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
pub fn handle_client_stream(
    socket: Arc<UdpSocket>,
    target_addr: SocketAddr,
    tickers: Vec<Ticker>,
    data_rx: Receiver<QuoteEvent>,
    stop_rx: Receiver<()>,
) -> Result<(), ParserError> {
    loop {
        select! {
            recv(stop_rx) -> _ => break,
            recv(data_rx) -> msg => match msg {
                Ok(QuoteEvent::Quote(quote)) => {
                    if tickers.iter().any(|t| t.to_string() == quote.ticker) {
                        let data = quote.to_bytes();
                        let _ = socket.send_to(&data, target_addr);
                    }
                },
                Ok(QuoteEvent::Shutdown) => break,
                _ => break,
            }
        }
    }
    Ok(())
}

fn main() -> Result<(), ParserError> {
    // 1. Общий UDP сокет для работы с данными и пингами
    let udp_socket = Arc::new(UdpSocket::bind("0.0.0.0:8081")?);

    // 2. Мониторинг пингов (Пункт 5)
    let ping_monitor = Arc::new(Mutex::new(PingMonitor::new(5)));
    let (stop_tx, stop_rx) = unbounded::<SocketAddr>();

    // Запускаем поток, который слушает пинги по UDP
    UdpPingListener::start(Arc::clone(&udp_socket), Arc::clone(&ping_monitor));

    // Запускаем поток, который проверяет таймауты
    QuoteReceiver::start_ping_monitor(Arc::clone(&ping_monitor), stop_tx);

    // 3. TCP Ресивер для команд (Пункт 2 и 3)
    let (cmd_tx, cmd_rx) = unbounded::<(Command, SocketAddr)>();
    let tcp_receiver = QuoteReceiver::new(BIND_ADDRESS)?; // Внутри TCP socket
    thread::spawn(move || {
        tcp_receiver.receive_loop_with_channel(cmd_tx);
    });

    let subscription_tx = QuoteGenerator::start();
    let mut active_streams: HashMap<SocketAddr, Sender<()>> = HashMap::new();

    loop {
        select! {
            // Получена команда STREAM по TCP
            recv(cmd_rx) -> msg => if let Ok((cmd, target_udp_addr)) = msg {
                let (shutdown_tx, shutdown_rx) = unbounded::<()>();
                active_streams.insert(target_udp_addr, shutdown_tx);

                let (client_data_tx, client_data_rx) = unbounded::<QuoteEvent>();
                subscription_tx.send(client_data_tx).ok();

                let socket_clone = Arc::clone(&udp_socket);
                let tickers = cmd.tickers;

                // 4. Поток для клиента (Пункт 4 и 6)
                thread::spawn(move || {
                    handle_client_stream(socket_clone, target_udp_addr, tickers, client_data_rx, shutdown_rx)
                });
            },

            // Остановка потока по таймауту пинга (Пункт 5)
            recv(stop_rx) -> addr => if let Ok(target_addr) = addr {
                if let Some(tx) = active_streams.remove(&target_addr) {
                    let _ = tx.send(());
                    println!("Стрим для {} закрыт: таймаут пинга", target_addr);
                }
            }
        }
    }
}
