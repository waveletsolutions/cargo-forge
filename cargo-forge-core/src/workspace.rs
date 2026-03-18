//! Workspace root detection and version reading.

use anyhow::{bail, Context, Result};
use std::env;
use std::fs;
use std::path::PathBuf;

/// Find the workspace root by walking up from the current directory,
/// looking for a Cargo.toml containing [workspace].
/// Also checks CARGO_MANIFEST_DIR (set by cargo when running as xtask).
pub fn find_root() -> Result<PathBuf> {
    // When run as `cargo xtask`, CARGO_MANIFEST_DIR points to the xtask
    // package directory. The workspace root is one level up.
    if let Ok(manifest_dir) = env::var("CARGO_MANIFEST_DIR") {
        let xtask_dir = PathBuf::from(manifest_dir);
        if let Some(parent) = xtask_dir.parent() {
            if parent.join("Cargo.toml").exists() {
                let contents = fs::read_to_string(parent.join("Cargo.toml"))
                    .unwrap_or_default();
                if contents.contains("[workspace]") {
                    return Ok(parent.to_path_buf());
                }
            }
        }
    }

    // Fallback: walk up from cwd looking for Cargo.toml
    // Accept either a workspace Cargo.toml OR any Cargo.toml alongside a forge.toml
    let mut dir = env::current_dir().context("Failed to get current directory")?;
    loop {
        let cargo_toml = dir.join("Cargo.toml");
        if cargo_toml.exists() {
            let contents = fs::read_to_string(&cargo_toml).unwrap_or_default();
            // Workspace project
            if contents.contains("[workspace]") {
                return Ok(dir);
            }
            // Single-crate project with forge.toml
            if dir.join("forge.toml").exists() {
                return Ok(dir);
            }
            // Single-crate project without forge.toml yet (e.g. during init)
            if contents.contains("[package]") {
                return Ok(dir);
            }
        }
        if !dir.pop() {
            bail!(
                "Could not find workspace root.\n  \
                 Run this command from within your Rust project directory."
            );
        }
    }
}

/// Read the project version from Cargo.toml.
pub fn read_version(workspace: &std::path::Path) -> Result<String> {
    let cargo_toml = fs::read_to_string(workspace.join("Cargo.toml"))
        .context("Failed to read Cargo.toml")?;
    for line in cargo_toml.lines() {
        let line = line.trim();
        if line.starts_with("version") && line.contains('=') {
            if let Some(v) = line.split('=').nth(1) {
                let v = v.trim().trim_matches('"');
                if !v.is_empty() {
                    return Ok(v.to_string());
                }
            }
        }
    }
    bail!("Could not read version from Cargo.toml")
}

/// Read the binary name from Cargo.toml [[bin]] section or [package] name.
pub fn read_binary_name(workspace: &std::path::Path) -> Result<String> {
    let cargo_toml = fs::read_to_string(workspace.join("Cargo.toml"))
        .context("Failed to read Cargo.toml")?;
    // Look for [[bin]] name = "..."
    let mut in_bin = false;
    for line in cargo_toml.lines() {
        let line = line.trim();
        if line == "[[bin]]" {
            in_bin = true;
            continue;
        }
        if in_bin && line.starts_with("name") && line.contains('=') {
            if let Some(v) = line.split('=').nth(1) {
                return Ok(v.trim().trim_matches('"').to_string());
            }
        }
        if in_bin && line.starts_with('[') {
            in_bin = false;
        }
    }
    // Fall back to [package] name
    for line in cargo_toml.lines() {
        let line = line.trim();
        if line.starts_with("name") && line.contains('=') {
            if let Some(v) = line.split('=').nth(1) {
                return Ok(v.trim().trim_matches('"').to_string());
            }
        }
    }
    bail!("Could not read binary name from Cargo.toml")
}