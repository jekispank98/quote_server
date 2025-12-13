//! Client command model used by the UDP receiver and server logic.
//!
//! A `Command` represents a deserialized client request received over UDP. The typical
//! request is a subscription for quote streaming (e.g., header `J_QUOTE`) together with
//! connection metadata and a list of requested `Ticker`s.
//!
//! Serialization:
//! - Binary: `bincode` via `Decode`/`Encode` derives — used for compact wire format.
//! - JSON: `serde` via `Serialize`/`Deserialize` derives — handy for debugging/tools.
//!
//! Fields have intentionally simple string types to mirror what is sent over the wire; the
//! business logic validates and interprets them at higher layers.

use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use crate::model::tickers::Ticker;

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
    pub tickers: Vec<Ticker>,
}
