//! forge-core -- cross-platform Rust release build library.
//!
//! Used by both the `cargo-forge` binary and any project's `xtask`.
//!
//! # Quick start
//!
//! ```toml
//! # forge.toml at workspace root
//! [forge]
//! binary = "myapp"
//!
//! [[forge.target]]
//! platform = "freebsd-x86_64"
//!
//! [[forge.target]]
//! platform = "linux-x86_64"
//!
//! [[forge.target]]
//! platform = "windows-x86_64"
//! archive = "zip"
//! ```
//!
//! Then from your project:
//! ```sh
//! cargo forge fix    # install deps
//! cargo forge build  # build all platforms
//! ```

pub mod config;
pub mod output;
pub mod platform;
pub mod workspace;

mod check;
mod fix;
mod build;
mod clean;
mod init;

use anyhow::Result;
use clap::{Parser, Subcommand};

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

#[derive(Parser, Debug)]
#[command(
    name = "forge",
    bin_name = "cargo forge",
    about = "Turn-key cross-platform release builds for Rust projects",
    long_about = "Builds release binaries for all configured platforms using\n\
                  cargo-zigbuild + zig as a universal cross-linker.\n\
                  No VMs, no containers, no root required.\n\n\
                  First time: cargo forge init && cargo forge fix\n\
                  Every release: cargo forge build"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Version suffix (e.g. rc1 -> 1.0.0-rc1)
    #[arg(long, value_name = "SUFFIX", global = true)]
    suffix: Option<String>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Initialize forge.toml in the current project
    Init,
    /// Check that all dependencies are installed
    Check,
    /// Install missing dependencies (zig, zip, cargo-zigbuild, rustup targets)
    Fix,
    /// Build release binaries for all configured platforms
    Build,
    /// Remove cross-compile artifacts and release-artifacts/
    Clean,
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Main entry point. Call this from `cargo-forge` binary or an xtask.
pub fn run() {
    // When invoked as `cargo forge <cmd>`, cargo passes argv as:
    //   cargo-forge forge <cmd>
    // The extra "forge" needs to be stripped so clap sees just <cmd>.
    // When invoked directly as `cargo-forge <cmd>` it works as-is.
    let args: Vec<String> = std::env::args().collect();
    let args: Vec<String> = if args.get(1).map(|s| s.as_str()) == Some("forge") {
        // Strip the extra "forge" argument
        std::iter::once(args[0].clone())
            .chain(args.into_iter().skip(2))
            .collect()
    } else {
        args
    };

    let cli = Cli::parse_from(args);

    let result = match cli.command.unwrap_or(Commands::Build) {
        Commands::Init  => run_init(),
        Commands::Check => run_check(),
        Commands::Fix   => run_fix(),
        Commands::Build => run_build(cli.suffix.as_deref()),
        Commands::Clean => run_clean(),
    };

    if let Err(e) = result {
        output::fail(&format!("{:#}", e));
        std::process::exit(1);
    }
}

fn run_init() -> Result<()> {
    let workspace = workspace::find_root()?;
    init::run(&workspace)
}

fn run_check() -> Result<()> {
    let workspace = workspace::find_root()?;
    let config = config::load(&workspace)?;
    let ok = check::run(&workspace, &config)?;
    if !ok { std::process::exit(1); }
    Ok(())
}

fn run_fix() -> Result<()> {
    let workspace = workspace::find_root()?;
    let config = config::load(&workspace)?;
    fix::run(&workspace, &config)
}

fn run_build(suffix: Option<&str>) -> Result<()> {
    let workspace = workspace::find_root()?;
    let config = config::load(&workspace)?;

    // Check deps first
    let ok = check::run_silent(&workspace, &config)?;
    if !ok {
        output::fail("Dependencies missing.");
        output::fail("Run: cargo forge fix");
        std::process::exit(1);
    }

    build::run(&workspace, &config, suffix)
}

fn run_clean() -> Result<()> {
    let workspace = workspace::find_root()?;
    let config = config::load(&workspace)?;
    clean::run(&workspace, &config)
}