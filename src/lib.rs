//! A Rust port of the `sqlformat` CLI from python-sqlparse 0.6.1.

pub mod encoding;
pub mod filters;
pub mod group;
pub mod kgenerated;
pub mod lexer;
pub mod port;
pub mod splitter;
pub mod tokens;
pub mod tree;

/// Upstream version this port targets.
pub const UPSTREAM_VERSION: &str = "0.6.1";

/// Version of this crate.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
