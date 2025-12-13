//! Domain models and utilities for the quote server.
//!
//! This module groups all core data types and helpers used by the UDP receiver,
//! streaming tasks, and the background quote generator:
//! - `command` — client request structure parsed from incoming datagrams.
//! - `quote` — market `Quote` type and (de)serialization helpers.
//! - `tickers` — supported ticker symbols used across the system.
//! - `ping_monitor` — in-memory keep-alive tracker for client timeouts.
//! - `quote_generator` — background data generator and `QuoteEvent` broadcasting.

pub mod command;
pub mod quote;
pub mod tickers;
pub mod ping_monitor;
pub mod quote_generator;
