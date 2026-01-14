//! The `logger` module is a simple utility that requires manual verification.
//! See `bin/logger_demo.rs` for a test binary demonstrating its usage.

mod logger;
pub use logger::*;

pub use tracing::{debug, error, info, trace, warn};
