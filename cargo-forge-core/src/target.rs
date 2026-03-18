//! `cargo forge target` -- manage build targets in forge.toml.

use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;

use crate::output::*;
use crate::platform::HostOs;

// ---------------------------------------------------------------------------
// Known platforms
// ---------------------------------------------------------------------------

struct Platform {
    id: &'static str,
    description: &'static str,
    notes: &'static str,
}

const PLATFORMS: &[Platform] = &[
    Platform {
        id: "freebsd-x86_64",
        description: "FreeBSD x86_64",
        notes: "native on FreeBSD; zigbuild from Windows",
    },
    Platform {
        id: "linux-x86_64",
        description: "Linux x86_64",
        notes: "native on Linux; zigbuild from FreeBSD/Windows",
    },
    Platform {
        id: "linux-aarch64",
        description: "Linux ARM64 (aarch64)",
        notes: "zigbuild from any host -- covers Pi 4/5, AWS Graviton, Apple Silicon",
    },
    Platform {
        id: "windows-x86_64",
        description: "Windows x86_64",
        notes: "native on Windows; zigbuild from FreeBSD/Linux",
    },
    Platform {
        id: "macos-x86_64",
        description: "macOS x86_64",
        notes: "native on Intel Mac only -- cross-compile not supported",
    },
    Platform {
        id: "macos-aarch64",
        description: "macOS ARM64 (Apple Silicon)",
        notes: "native on Apple Silicon only -- cross-compile not supported",
    },
];

fn find_platform(id: &str) -> Option<&'static Platform> {
    PLATFORMS.iter().find(|p| p.id == id)
}

// ---------------------------------------------------------------------------
// list
// ---------------------------------------------------------------------------

pub fn list(workspace: &Path) -> Result<()> {
    let forge_toml = workspace.join("forge.toml");
    let host = HostOs::detect();

    // Read currently configured targets
    let configured: Vec<String> = if forge_toml.exists() {
        let text = fs::read_to_string(&forge_toml)?;
        parse_platforms(&text)
    } else {
        vec![]
    };

    println!();
    println!("  Configured targets:");
    if configured.is_empty() {
        println!("    (none -- run: cargo forge target add <platform>)");
    } else {
        for p in &configured {
            if let Some(info) = find_platform(p) {
                println!("    {}  {}", format_check(), info.description);
            } else {
                println!("    {}  {} (unknown platform)", format_check(), p);
            }
        }
    }

    println!();
    println!("  Available platforms:");
    for p in PLATFORMS {
        let is_configured = configured.iter().any(|c| c == p.id);
        let host_arch = match std::env::consts::ARCH {
            "x86_64"  => "x86_64",
            "aarch64" => "aarch64",
            _         => "",
        };
        let native_platform = format!("{}-{}", host.as_str(), host_arch);
        let is_native = p.id == native_platform;
        let marker = if is_configured { format_check() } else { "  ".to_string() };
        let native_tag = if is_native { " (this machine)" } else { "" };
        println!(
            "    {}  {:<22} -- {}{}",
            marker, p.id, p.notes, native_tag
        );
    }

    println!();
    println!("  Usage:");
    println!("    cargo forge target add <platform>");
    println!("    cargo forge target remove <platform>");
    println!();

    Ok(())
}

fn format_check() -> String {
    use colored::Colorize;
    "ok".green().to_string()
}

// ---------------------------------------------------------------------------
// add
// ---------------------------------------------------------------------------

pub fn add(workspace: &Path, platform: &str) -> Result<()> {
    // Validate platform
    if find_platform(platform).is_none() {
        bail!(
            "Unknown platform: '{}'\n  \
             Run 'cargo forge target list' to see available platforms.",
            platform
        );
    }

    let forge_toml = workspace.join("forge.toml");
    if !forge_toml.exists() {
        bail!(
            "forge.toml not found.\n  Run: cargo forge init"
        );
    }

    let text = fs::read_to_string(&forge_toml)
        .context("Failed to read forge.toml")?;

    let configured = parse_platforms(&text);

    if configured.iter().any(|p| p == platform) {
        warn(&format!("{} is already in forge.toml", platform));
        return Ok(());
    }

    // Build the new target block
    let info = find_platform(platform).unwrap();
    let archive = if platform.starts_with("windows") {
        "\narchive = \"zip\""
    } else {
        ""
    };

    let new_block = format!(
        "\n[[forge.target]]\nplatform = \"{}\"{}\n",
        platform, archive
    );

    // Append before [forge.deps] if present, otherwise at end
    let new_text = if text.contains("[forge.deps]") {
        text.replace("[forge.deps]", &format!("{}\n[forge.deps]", new_block.trim_start()))
    } else {
        format!("{}{}", text.trim_end(), new_block)
    };

    fs::write(&forge_toml, new_text)
        .context("Failed to write forge.toml")?;

    ok(&format!("Added {} ({}) to forge.toml", platform, info.description));
    println!("  Run: cargo forge fix    to install the rustup target");
    println!("  Run: cargo forge build  to build");
    Ok(())
}

// ---------------------------------------------------------------------------
// remove
// ---------------------------------------------------------------------------

pub fn remove(workspace: &Path, platform: &str) -> Result<()> {
    let forge_toml = workspace.join("forge.toml");
    if !forge_toml.exists() {
        bail!("forge.toml not found.\n  Run: cargo forge init");
    }

    let text = fs::read_to_string(&forge_toml)
        .context("Failed to read forge.toml")?;

    let configured = parse_platforms(&text);
    if !configured.iter().any(|p| p == platform) {
        warn(&format!("{} is not in forge.toml", platform));
        return Ok(());
    }

    // Remove the [[forge.target]] block for this platform
    let new_text = remove_target_block(&text, platform);

    fs::write(&forge_toml, new_text)
        .context("Failed to write forge.toml")?;

    ok(&format!("Removed {} from forge.toml", platform));
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract configured platform IDs from forge.toml text.
fn parse_platforms(text: &str) -> Vec<String> {
    let mut platforms = Vec::new();
    let mut in_target = false;

    for line in text.lines() {
        let line = line.trim();
        if line == "[[forge.target]]" {
            in_target = true;
            continue;
        }
        if line.starts_with('[') && line != "[[forge.target]]" {
            in_target = false;
        }
        if in_target && line.starts_with("platform") && line.contains('=') {
            if let Some(val) = line.split('=').nth(1) {
                let val = val.trim().trim_matches('"').to_string();
                platforms.push(val);
            }
        }
    }
    platforms
}

/// Remove a [[forge.target]] block matching the given platform from toml text.
fn remove_target_block(text: &str, platform: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut result = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i].trim();

        // Check if this is the start of a [[forge.target]] block
        if line == "[[forge.target]]" {
            // Look ahead to see if this block contains our platform
            let mut j = i + 1;
            let mut found = false;
            while j < lines.len() {
                let next = lines[j].trim();
                if next.starts_with('[') { break; }
                if next.starts_with("platform") && next.contains('=') {
                    if let Some(val) = next.split('=').nth(1) {
                        if val.trim().trim_matches('"') == platform {
                            found = true;
                            break;
                        }
                    }
                }
                j += 1;
            }

            if found {
                // Skip this entire block (up to next section or end)
                i += 1;
                while i < lines.len() {
                    let next = lines[i].trim();
                    if next.starts_with('[') { break; }
                    i += 1;
                }
                // Skip trailing blank line if present
                if i < lines.len() && lines[i].trim().is_empty() {
                    i += 1;
                }
                continue;
            }
        }

        result.push(lines[i]);
        i += 1;
    }

    result.join("\n")
}