//! Vietnamese typing engine. No platform APIs or global mutable state.
#![forbid(unsafe_code)]
pub mod data;
pub mod driver;
pub mod engine;
pub mod keys;
pub mod smart_switch;
mod tables;
pub mod touch;
pub mod types;
pub use driver::{Driver, Edit};
pub use engine::{Engine, character_code, rendered_char};
pub use types::*;
