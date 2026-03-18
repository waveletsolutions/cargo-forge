//! `cargo forge check` -- verify all dependencies are installed.

use anyhow::Result;
use std::path::Path;

use crate::config::ForgeConfig;
use crate::output::*;
use crate::platform::{cmd_exists, find_managed_zig, run_captured, HostOs};

/// Run check with output. Returns true if all deps satisfied.
pub fn run(_workspace: &Path, config: &ForgeConfig) -> Result<bool> {
    header("Checking dependencies");
    let ok_flag = check_all(config, true)?;
    println!();
    if ok_flag {
        ok("All dependencies satisfied -- ready to build!");
    } else {
        warn("Some dependencies are missing. Run: cargo forge fix");
    }
    Ok(ok_flag)
}

/// Run check silently (no output). Returns true if all deps satisfied.
pub fn run_silent(_workspace: &Path, config: &ForgeConfig) -> Result<bool> {
    check_all(config, false)
}

fn check_all(config: &ForgeConfig, verbose: bool) -> Result<bool> {
    let host = HostOs::detect();
    let mut missing = 0;

    // zig -- check system PATH first, then cargo-forge managed install
    if config.forge.deps.zig {
        let zig_ver = if cmd_exists("zig") {
            run_captured("zig", &["version"]).ok()
        } else if let Some(managed) = find_managed_zig() {
            run_captured(managed.to_str().unwrap_or("zig"), &["version"]).ok()
        } else {
            None
        };
        if let Some(ver) = zig_ver {
            if verbose { ok(&format!("zig {}", ver)); }
        } else {
            if verbose {
                warn("zig -- not found");
                println!("       run: cargo forge fix  (auto-downloads zig)");
                println!("       or:  {}", zig_install_hint(host));
            }
            missing += 1;
        }
    }

    // zip -- not needed on Windows
    if config.forge.deps.zip && !host.is_windows() {
        if cmd_exists("zip") {
            if verbose { ok("zip"); }
        } else {
            if verbose {
                warn("zip -- not found");
                println!("       {}", zip_install_hint(host));
            }
            missing += 1;
        }
    } else if host.is_windows() {
        if verbose { ok("zip (PowerShell Compress-Archive -- built in)"); }
    }

    // cargo-zigbuild
    if cmd_exists("cargo-zigbuild") {
        let ver = run_captured("cargo-zigbuild", &["--version"])
            .unwrap_or_default()
            .replace("cargo-zigbuild ", "");
        if verbose { ok(&format!("cargo-zigbuild {}", ver)); }
    } else {
        if verbose {
            warn("cargo-zigbuild -- not found");
            println!("       cargo install cargo-zigbuild");
        }
        missing += 1;
    }

    // rustup targets
    if cmd_exists("rustup") {
        let installed = run_captured("rustup", &["target", "list", "--installed"])
            .unwrap_or_default();
        for t in &config.forge.target {
            // Skip native target -- built with plain cargo
            if t.is_native(host.as_str()) {
                continue;
            }
            if let Some(triple) = t.triple() {
                if installed.contains(triple) {
                    if verbose { ok(&format!("rustup target: {}", triple)); }
                } else {
                    if verbose {
                        warn(&format!("rustup target: {} -- not installed", triple));
                        println!("       rustup target add {}", triple);
                    }
                    missing += 1;
                }
            }
        }
    } else {
        if verbose {
            warn("rustup -- not found");
            println!("       curl https://sh.rustup.rs -sSf | sh");
        }
        missing += 1;
    }

    // min_version check
    if let Some(ref min) = config.forge.min_version {
        let current = env!("CARGO_PKG_VERSION");
        if verbose {
            // Simple string compare -- good enough for semver x.y.z
            if current < min.as_str() {
                warn(&format!(
                    "cargo-forge {} is installed but this project requires >= {}",
                    current, min
                ));
                println!("       cargo install forge-release");
                missing += 1;
            } else {
                ok(&format!("cargo-forge {} (>= {} required)", current, min));
            }
        }
    }

    // Host platform note
    if verbose {
        println!();
        match host {
            HostOs::Windows => ok("Windows host -- produces FreeBSD, Linux, and Windows binaries"),
            HostOs::FreeBsd => ok("FreeBSD host -- produces FreeBSD (native), Linux, and Windows binaries"),
            HostOs::Linux   => {
                ok("Linux host -- produces Linux (native) and Windows binaries");
                warn("FreeBSD cross-compilation from Linux is not supported.");
                println!("       Build the FreeBSD binary on a FreeBSD machine or from Windows.");
            }
            _ => {}
        }
    }

    Ok(missing == 0)
}

fn zig_install_hint(host: HostOs) -> &'static str {
    match host {
        HostOs::FreeBsd  => "sudo pkg install zig",
        HostOs::Linux    => "sudo apt install zig  (or: sudo dnf install zig)",
        HostOs::MacOs    => "brew install zig",
        HostOs::Windows  => "winget install zig.zig  (or: choco install zig)",
        HostOs::Unknown  => "see https://ziglang.org/download/",
    }
}

fn zip_install_hint(host: HostOs) -> &'static str {
    match host {
        HostOs::FreeBsd => "sudo pkg install zip",
        HostOs::Linux   => "sudo apt install zip  (or: sudo dnf install zip)",
        HostOs::MacOs   => "zip is built into macOS",
        _               => "install zip",
    }
}