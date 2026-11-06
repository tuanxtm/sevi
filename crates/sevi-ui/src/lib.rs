//! Sevi's shared, disposable desktop interface.
mod app;
mod chrome;
mod controls;
mod emoji;
mod native;
mod theme;
pub use app::{preview, run_client};
pub use sevi_ui_protocol as protocol;

pub fn is_client() -> bool {
    std::env::args().any(|a| a == "--sevi-ui-client")
}
