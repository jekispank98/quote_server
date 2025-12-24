use std::net::SocketAddr;
// command.rs
use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use crate::model::tickers::Ticker;

/// Константы для команд
pub const HEADER: &str = "J_QUOTE";
pub const PING: &str = "PING";
pub const CONNECTION: &str = "UDP";

/// Describes a client request received by the server.
#[derive(Debug, Clone, Decode, Encode, Serialize, Deserialize)]
pub struct Command {
    /// Operation identifier, e.g. `"J_QUOTE"` for quote stream subscription.
    pub header: String,
    /// Transport/protocol hint as provided by the client (informational).
    pub connection: String,
    /// Client-reported address (string form). Server uses the actual `SocketAddr` it sees.
    pub address: String,
    /// Client-reported port (string form).
    pub port: String,
    /// List of tickers the client is interested in receiving.
    pub tickers: Vec<Ticker>
}

impl Command {
    /// Creates a new subscription (`J_QUOTE`) command.
    pub fn new(
        address: &str,      // TCP адрес клиента
        port: &str,     // UDP порт для данных
        tickers: Vec<Ticker>
    ) -> Self {
        Command {
            header: String::from(HEADER),
            connection: String::from(CONNECTION),
            address: String::from(address),
            port: String::from(port),
            tickers
        }
    }

    /// Creates a new keep-alive `PING` command.
    pub fn new_ping(address: &str, port: &str) -> Self {
        Command {
            header: String::from(PING),
            connection: String::from(CONNECTION),
            address: String::from(address),
            port: String::from(port),
            tickers: Vec::new(),
        }
    }

    /// Получить UDP адрес клиента в формате SocketAddr
    pub fn get_udp_addr(&self) -> Result<SocketAddr, std::net::AddrParseError> {
        format!("{}:{}", self.address, self.port).parse()
    }

    /// Получить TCP адрес клиента в формате SocketAddr
    pub fn get_tcp_addr(&self) -> Result<SocketAddr, std::net::AddrParseError> {
        format!("{}:{}", self.address, self.port).parse()
    }
}