//! Ping/keep-alive state tracker for UDP clients.
//!
//! This module provides a lightweight, in-memory monitor that tracks the last time a
//! client (identified by `SocketAddr`) sent a keep-alive/ping. It exposes three core
//! operations:
//!
//! - `PingMonitor::update_ping(addr)` — record a fresh ping for a client and mark it active.
//! - `PingMonitor::check_timeouts()` — scan all clients and return the addresses that have
//!   exceeded the configured timeout; those clients are marked inactive internally.
//! - `PingMonitor::is_client_active(addr)` — read-only check whether a client is currently
//!   considered active.
//!
//! Design notes:
//! - Time is measured using `std::time::Instant`, which is monotonic and immune to system
//!   clock changes.
//! - The monitor is not synchronized; if it is shared across threads, wrap it with a
//!   synchronization primitive (e.g., `Mutex` or `RwLock`).
//! - `check_timeouts` is idempotent between pings: once a client times out, it stays
//!   inactive until the next `update_ping` marks it active again.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

/// Internal bookkeeping for a client connection.
///
/// This is intentionally minimal: last observed ping time and a cached `is_active` flag
/// to avoid re-emitting the same timeout multiple times between pings.
struct ClientConnection {
    last_ping: Instant,
    is_active: bool,
}

/// Tracks client keep-alive pings and determines inactivity based on a timeout.
pub struct PingMonitor {
    /// All known clients with their last ping time and active flag.
    clients: HashMap<SocketAddr, ClientConnection>,
    /// Threshold after which a client is considered timed out.
    timeout: Duration,
}

impl PingMonitor {
    pub fn new(timeout_secs: u64) -> Self {
        Self {
            clients: HashMap::new(),
            timeout: Duration::from_secs(timeout_secs),
        }
    }

    pub fn update_ping(&mut self, addr: SocketAddr) {
        let now = Instant::now();
        self.clients.entry(addr)
            .and_modify(|conn| {
                conn.last_ping = now;
                conn.is_active = true;
            })
            .or_insert(ClientConnection {
                last_ping: now,
                is_active: true,
            });
    }

    pub fn check_timeouts(&mut self) -> Vec<SocketAddr> {
        let now = Instant::now();
        let mut timed_out = Vec::new();

        for (addr, conn) in &mut self.clients {
            if conn.is_active && now.duration_since(conn.last_ping) > self.timeout {
                conn.is_active = false;
                timed_out.push(*addr);
            }
        }

        timed_out
    }

    pub fn is_client_active(&self, addr: &SocketAddr) -> bool {
        self.clients.get(addr)
            .map(|conn| conn.is_active)
            .unwrap_or(false)
    }
}