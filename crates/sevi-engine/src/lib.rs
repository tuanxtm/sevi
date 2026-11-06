//! Vietnamese typing engine. No platform APIs or global mutable state.
#![cfg_attr(not(feature = "ffi"), forbid(unsafe_code))]
pub mod data;
pub mod driver;
pub mod engine;
#[cfg(feature = "ffi")]
pub mod ffi;
pub mod keys;
pub mod smart_switch;
mod tables;
pub mod touch;
pub mod types;
pub use driver::{Driver, Edit};
pub use engine::{Engine, character_code, rendered_char};
pub use types::*;
