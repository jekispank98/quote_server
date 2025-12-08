use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use std::net::SocketAddr;
pub type ClientMap = Arc<RwLock<HashMap<SocketAddr, Instant>>>;