use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use crate::model::tickers::Ticker;

#[derive(Debug, Clone, Decode, Encode, Serialize, Deserialize)]
pub struct Command {
    pub connection: String,
    pub address: String,
    pub port: String,
    pub tickers: Vec<Ticker>,
}
