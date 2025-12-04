use std::time::{SystemTime, UNIX_EPOCH};
use crate::model::stock_quote::StockQuote;

#[derive(Debug, Clone)]
pub struct QuoteGenerator {

}

impl QuoteGenerator {
    pub fn generate_quote(&mut self, ticker: &str) -> Option<StockQuote> {
        // ... логика изменения цены ...

        let volume = match ticker {
            // Популярные акции имеют больший объём
            "AAPL" | "MSFT" | "TSLA" => 1000 + (rand::random::<f64>() * 5000.0) as u32,
            // Обычные акции - средний объём
            _ => 100 + (rand::random::<f64>() * 1000.0) as u32,
        };

        Some(StockQuote {
            ticker: ticker.to_string(),
            price: *last_price,
            volume,
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64,
        })
    }
}