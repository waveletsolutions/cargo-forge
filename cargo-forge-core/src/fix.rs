//! `cargo forge fix` -- install missing dependencies.

use anyhow::Result;
use std::path::Path;

use crate::config::ForgeConfig;
use crate::output::*;
use crate::platform::{cmd_exists, install_zig, install_zip, exec, run_captured, HostOs};

pub fn run(_workspace: &Path, config: &ForgeConfig) -> Result<()> {
    header("Installing dependencies");
    let host = HostOs::detect();

    // zig
    if config.forge.deps.zig {
        if cmd_exists("zig") {
            ok("zig already installed");
        } else {
            info("Installing zig...");
            install_zig(host)?;
            ok("zig installed");
        }
    }

    // zip (not needed on Windows)
    if config.forge.deps.zip {
        if host.is_windows() {
            ok("zip not needed on Windows (using PowerShell Compress-Archive)");
        } else if cmd_exists("zip") {
            ok("zip already installed");
        } else {
            info("Installing zip...");
            install_zip(host)?;
            ok("zip installed");
        }
    }

    // cargo-zigbuild
    if cmd_exists("cargo-zigbuild") {
        ok("cargo-zigbuild already installed");
    } else {
        info("Installing cargo-zigbuild...");
        exec("cargo", &["install", "cargo-zigbuild"])?;
        ok("cargo-zigbuild installed");
    }

    // rustup targets
    if cmd_exists("rustup") {
        let installed = run_captured("rustup", &["target", "list", "--installed"])
            .unwrap_or_default();
        for t in &config.forge.target {
            if t.is_native(host.as_str()) {
                continue;
            }
            if let Some(triple) = t.triple() {
                if installed.contains(triple) {
                    ok(&format!("rustup target {} already installed", triple));
                } else {
                    info(&format!("Adding rustup target {}...", triple));
                    exec("rustup", &["target", "add", triple])?;
                    ok(&format!("rustup target {} added", triple));
                }
            }
        }
    } else {
        anyhow::bail!(
            "rustup not found.\n  \
             Install from: https://rustup.rs"
        );
    }

    println!();
    ok("All dependencies installed. Run: cargo forge build");
    Ok(())
}