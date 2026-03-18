//! cargo-forge -- turn-key cross-platform release builds for Rust projects.
//!
//! Installed as a cargo subcommand:
//!   cargo install cargo-forge
//!   cargo forge init
//!   cargo forge fix
//!   cargo forge build
//!
//! All logic lives in forge-core.

fn main() {
    forge_core::run();
}
