//! Quote data model and binary encoding helpers.
//!
//! A `Quote` is the payload sent to clients. It contains the ticker symbol, the last
//! traded price, a synthetic volume, and a millisecond UTC timestamp. This module also
//! provides helper methods for generating synthetic prices and for encoding/decoding
//! quotes with `bincode`.

use crate::error::ParserError;
use crate::model::tickers::Ticker;
use bincode::{Decode, Encode};
use chrono::Utc;
use rand::Rng;

/// Market quote for a single ticker symbol.
#[derive(Debug, Clone, Encode, Decode)]
pub struct Quote {
    /// Symbol identifier (string form of `Ticker`).
    pub ticker: String,
    /// Last traded price.
    pub price: f64,
    /// Synthetic trade volume associated with this tick.
    pub volume: u32,
    /// UTC timestamp in milliseconds since Unix epoch.
    pub timestamp: u64,
}

impl Quote {
    pub fn next_price(current_price: f64) -> f64 {
        let mut rng = rand::rng();
        let change: f64 = rng.gen_range(-0.01..0.01);
        let new_price = current_price * (1.0 + change);
        new_price.max(0.01)
    }

    pub fn generate_new(ticker: &Ticker, current_price: f64) -> Result<Quote, ParserError> {
        let mut rng = rand::rng();
        let volume = match ticker {
            Ticker::AAPL | Ticker::MSFT | Ticker::TSLA => {
                1000 + rng.random_range(0..5000) as u32
            },
            _ => 100 + rng.random_range(0..1000) as u32,
        };

        Ok(Quote {
            ticker: ticker.to_string(),
            price: Self::next_price(current_price),
            volume,
            timestamp: Utc::now().timestamp_millis() as u64,
        })
    }
    pub fn to_bytes(&self) -> Vec<u8> {
        bincode::encode_to_vec(self, bincode::config::standard())
            .expect("Failed to encode quote")
    }

}
