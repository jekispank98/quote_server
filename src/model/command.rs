use crate::model::tickers::Ticker;

#[derive(Debug, Clone)]
pub struct Command {
    pub connection: String,
    pub address: String,
    pub port: String,
    pub tickers: Vec<Ticker>
}