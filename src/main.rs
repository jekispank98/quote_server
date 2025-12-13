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
use std::thread;

mod error;
mod model;
mod receiver;
mod result;

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
    socket: UdpSocket,
    target_addr: SocketAddr,
    tickers: Vec<Ticker>,
    data_rx: Receiver<QuoteEvent>,
    stop_rx: Receiver<()>,
) -> Result<(), ParserError> {
    println!("▶️ Запущен поток стриминга для клиента: {}", target_addr);

    loop {
        select! {
            recv(stop_rx) -> msg => match msg {
                Ok(_) => {
                    println!("Stream shutdown {} according the signal.", target_addr);
                    break;
                },
                Err(e) => {
                    return Err(ParserError::ChannelRecv(e.to_string()));
                },
            },

            recv(data_rx) -> msg => match msg {
                Ok(QuoteEvent::Quote(quote)) => {
                    let is_interested = tickers.iter().any(|t| t.to_string() == quote.ticker);

                    if is_interested {
                        let data = quote.to_bytes();
                        if let Err(e) = socket.send_to(&data, target_addr) {
                            return Err(ParserError::Format(format!("Sending error {}: {}. Stop stream", target_addr, e)))
                        }
                    }
                },
                Ok(QuoteEvent::Shutdown) => {
                    println!("Stop stream for {} according the quote_generator (Shutdown signal).", target_addr);
                    break;
                },
                Err(e) => {
                    return Err(ParserError::ChannelRecv(e.to_string()));
                },
            },
        }
    }
    println!("Stream for {} is completed.", target_addr);
    Ok(())
}

fn main() -> Result<(), ParserError> {
    println!("--- Start quotes up server ---");

    let subscription_tx = QuoteGenerator::start();

    let (stop_tx, stop_rx) = unbounded::<SocketAddr>();
    let receiver = QuoteReceiver::new(BIND_ADDRESS)?;
    let server_socket_clone = receiver.socket.try_clone()?;
    let (_receiver_thread_handle, cmd_rx) = receiver.start_with_channel(stop_tx);
    let mut active_streams: HashMap<SocketAddr, Sender<()>> = HashMap::new();

    println!(
        "The server is running and waiting for events {}",
        BIND_ADDRESS
    );

    loop {
        select! {
            recv(cmd_rx) -> msg => match msg {
                Ok((cmd, src_addr)) => {
                    match cmd.header.as_str() {
                        "J_QUOTE" => {
                            println!("Quote's request from {}", src_addr);
                            if active_streams.contains_key(&src_addr) {
                                println!("The client {} is active already", src_addr);
                                continue;
                            }

                            let (shutdown_tx, shutdown_rx) = unbounded::<()>();
                            active_streams.insert(src_addr, shutdown_tx);

                            let (client_data_tx, client_data_rx) = unbounded::<QuoteEvent>();

                            if subscription_tx.send(client_data_tx).is_err() {
                                eprintln!("Error registering client in the quote_generator. Termination.");
                                active_streams.remove(&src_addr);
                                continue;
                            }

                            let tickers = cmd.tickers.clone();
                            let socket_clone_for_thread = server_socket_clone.try_clone()?;

                            thread::spawn(move || {
                                if let Err(e) = handle_client_stream(
                                    socket_clone_for_thread,
                                    src_addr,
                                    tickers,
                                    client_data_rx,
                                    shutdown_rx
                                ) {
                                    eprintln!("Critical error for {}: {:?}", src_addr, e);
                                }
                            });
                        },

                        _ => println!("Unknown command from {}: {}", src_addr, cmd.header),
                    }
                },
                Err(e) => {
                    return Err(ParserError::ChannelRecv(e.to_string()));
                },
            },
            recv(stop_rx) -> msg => match msg {
                Ok(timeout_addr) => {
                    println!("Client time-out {}. Stop stream", timeout_addr);
                    if let Some(shutdown_tx) = active_streams.remove(&timeout_addr) {
                        let _ = shutdown_tx.send(());
                        println!("Stream for {} is stopped by Keep-Alive.", timeout_addr);
                    } else {
                        println!("Client {} was not found in any active streams, but received a timeout.", timeout_addr);
                    }
                },
                Err(e) => return Err(ParserError::ChannelRecv(e.to_string()))
            },
        }
    }
}
