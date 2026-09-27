pub mod cli;
pub mod detect;
pub mod engine;
pub mod error;
pub mod masker;
pub mod output;
pub mod server;
pub mod stream;
pub mod types;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
