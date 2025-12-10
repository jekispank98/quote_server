use crate::error::ParserError;
use bincode::{Decode, Encode};
use chrono::Utc;
use rand::Rng;
use crate::model::tickers::Ticker;

#[derive(Debug, Clone, Encode, Decode)]
pub struct StockQuote {
    pub ticker: String,
    pub price: f64,
    pub volume: u32,
    pub timestamp: u64,
}

impl StockQuote {
    pub fn generate_new(ticker: &Ticker) -> Result<StockQuote, ParserError> {
        let mut rng = rand::rng();
        let volume = match ticker {
            Ticker::AAPL | Ticker::MSFT| Ticker::TSLA => 1000 + (rand::random::<f64>() * 5000.0) as u32,
            _ => 100 + (rand::random::<f64>() * 1000.0) as u32,
        };
        let new_quote: StockQuote = StockQuote {
            ticker: ticker.to_string(),
            price: rng.random_range(0f64..f64::MAX),
            volume,
            timestamp: Utc::now().timestamp_millis() as u64,
        };
        Ok(new_quote)
    }

    pub fn to_string(&self) -> String {
        format!(
            "{}|{}|{}|{}",
            self.ticker, self.price, self.volume, self.timestamp
        )
    }

    pub fn from_string(s: &str) -> Option<Self> {
        println!("Parsing input {}", s);
        let parts: Vec<&str> = s.split('|').collect();
        if parts.len() == 4 {
            Some(StockQuote {
                ticker: parts[0].to_string(),
                price: parts[1].parse().ok()?,
                volume: parts[2].parse().ok()?,
                timestamp: parts[3].parse().ok()?,
            })
        } else {
            None
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(self.ticker.as_bytes());
        bytes.push(b'|');
        bytes.extend_from_slice(self.price.to_string().as_bytes());
        bytes.push(b'|');
        bytes.extend_from_slice(self.volume.to_string().as_bytes());
        bytes.push(b'|');
        bytes.extend_from_slice(self.timestamp.to_string().as_bytes());
        bytes
    }
}
