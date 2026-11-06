//! Shared desktop policies. OS event delivery, native controls and preferences remain in adapters.
#![forbid(unsafe_code)]
pub mod emoji;
pub mod prefs;
pub mod session;
pub mod typing_log;
pub mod ui;
pub use session::{DesktopSession, Effect, EventResult, Modifiers, Platform};
