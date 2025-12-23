//! Quote stream generator and event broadcasting.
//!
//! The `QuoteGenerator` runs a background thread that synthesizes `Quote` values for a
//! fixed set of `Ticker`s and broadcasts them to all subscribed clients using
//! `crossbeam_channel`. New client tasks register by sending a `Sender<QuoteEvent>` to the
//! subscription channel returned by `QuoteGenerator::start`.
//!
//! Event model:
//! - `QuoteEvent::Quote(Quote)` — a single quote tick.
//! - `QuoteEvent::Shutdown` — signal for consumers to terminate gracefully.
//!
//! Design notes:
//! - Uses a small random-walk around the last price to simulate movement.
//! - Maintains last prices in a `HashMap<Ticker, f64>` so all clients observe the same
//!   sequence of prices.
//! - Broadcast is best-effort: if sending to a client fails, that client is removed.

use crate::model::quote::Quote;
use crate::model::tickers::Ticker;
use crossbeam_channel::Sender;
use std::collections::HashMap;
use std::thread;
use std::time::Duration;

/// Message sent by the generator to its subscribers.
#[derive(Clone)]
pub enum QuoteEvent {
    /// New quote tick for a particular symbol.
    Quote(Quote),
    /// Global shutdown notification for all consumers.
    Shutdown,
}

/// Background market data generator that broadcasts to subscribers.
pub struct QuoteGenerator;

impl QuoteGenerator {
    /// Start the generator thread and return a channel for registering subscribers.
    ///
    /// The returned `Sender<Sender<QuoteEvent>>` accepts a per-subscriber channel; the
    /// generator will push every `QuoteEvent` to all registered channels. If a send fails,
    /// the corresponding subscriber is dropped from the list.
    pub fn start() -> Sender<Sender<QuoteEvent>> {
        let (subscribe_tx, subscribe_rx) = crossbeam_channel::unbounded::<Sender<QuoteEvent>>();

        thread::spawn(move || {
            let mut clients: Vec<Sender<QuoteEvent>> = Vec::new();
            let tickers = vec![Ticker::AAPL, Ticker::MSFT, Ticker::TSLA, Ticker::GOOGL];
            
            let initial_price = 100.0;
            let mut current_prices: HashMap<Ticker, f64> =
                tickers.iter().map(|t| (t.clone(), initial_price)).collect();

            println!(
                "🏭 Market Generator started (Thread ID: {:?})",
                thread::current().id()
            );

            loop {
                while let Ok(new_client_tx) = subscribe_rx.try_recv() {
                    clients.push(new_client_tx);
                    println!(
                        "Generator: New client added. Total clients: {}",
                        clients.len()
                    );
                }
                
                for ticker in &tickers {
                    let current_price = *current_prices.get(ticker).unwrap_or(&initial_price);

                    if let Ok(quote) = Quote::generate_new(ticker, current_price) {
                        current_prices.insert(ticker.clone(), quote.price);

                        let event = QuoteEvent::Quote(quote);
                        clients.retain(|client_tx| client_tx.send(event.clone()).is_ok());
                    }
                }
                
                thread::sleep(Duration::from_millis(500));
            }
        });
        subscribe_tx
    }
}
