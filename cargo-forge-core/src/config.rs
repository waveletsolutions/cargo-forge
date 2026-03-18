//! forge.toml configuration -- deserialization and defaults.

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::fs;
use std::path::Path;

// ---------------------------------------------------------------------------
// Top-level config
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ForgeConfig {
    pub forge: ForgeSection,
}

#[derive(Debug, Deserialize)]
pub struct ForgeSection {
    /// Binary name to package (e.g. "anvil", "myapp")
    pub binary: String,

    /// Minimum required cargo-forge version (optional)
    pub min_version: Option<String>,

    /// Where to read the project version from.
    /// Defaults to "Cargo.toml". Can be "forge.toml" to hardcode.
    #[serde(default = "default_version_from")]
    pub version_from: String,

    /// Hardcoded version (used when version_from = "forge.toml")
    pub version: Option<String>,

    /// Output directory for release artifacts. Defaults to "release-artifacts".
    #[serde(default = "default_artifacts_dir")]
    pub artifacts_dir: String,

    /// Cross-compile targets
    #[serde(default)]
    pub target: Vec<TargetConfig>,

    /// Build dependencies
    #[serde(default)]
    pub deps: DepsConfig,
}

fn default_version_from() -> String {
    "Cargo.toml".to_string()
}

fn default_artifacts_dir() -> String {
    "release-artifacts".to_string()
}

// ---------------------------------------------------------------------------
// Target config
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Clone)]
pub struct TargetConfig {
    /// Target platform. Supported values:
    ///   freebsd-x86_64
    ///   linux-x86_64
    ///   windows-x86_64
    ///   macos-x86_64
    ///   macos-aarch64
    pub platform: String,

    /// Archive format: "tar.gz" or "zip". Auto-detected from platform if omitted.
    pub archive: Option<String>,
}

impl TargetConfig {
    /// Rustup/cargo target triple for this platform.
    pub fn triple(&self) -> Option<&'static str> {
        match self.platform.as_str() {
            "freebsd-x86_64"  => Some("x86_64-unknown-freebsd"),
            "linux-x86_64"    => Some("x86_64-unknown-linux-gnu"),
            "linux-aarch64"   => Some("aarch64-unknown-linux-gnu"),
            "windows-x86_64"  => Some("x86_64-pc-windows-gnu"),
            "macos-x86_64"    => Some("x86_64-apple-darwin"),
            "macos-aarch64"   => Some("aarch64-apple-darwin"),
            _ => None,
        }
    }

    /// Binary filename for this platform (with .exe on Windows).
    pub fn binary_name(&self, base: &str) -> String {
        if self.platform.starts_with("windows") {
            format!("{}.exe", base)
        } else {
            base.to_string()
        }
    }

    /// Human-readable display name for this platform.
    pub fn display_name(&self) -> &str {
        match self.platform.as_str() {
            "freebsd-x86_64"  => "FreeBSD x86_64",
            "linux-x86_64"    => "Linux x86_64",
            "linux-aarch64"   => "Linux ARM64 (aarch64)",
            "windows-x86_64"  => "Windows x86_64",
            "macos-x86_64"    => "macOS x86_64",
            "macos-aarch64"   => "macOS aarch64",
            other             => other,
        }
    }

    /// Archive extension -- from config or auto-detected.
    pub fn archive_ext(&self) -> &str {
        if let Some(ref a) = self.archive {
            return a.as_str();
        }
        if self.platform.starts_with("windows") {
            "zip"
        } else {
            "tar.gz"
        }
    }

    /// Whether this platform is the native host.
    pub fn is_native(&self, host: &str) -> bool {
        self.platform.starts_with(host)
    }
}

// ---------------------------------------------------------------------------
// Deps config
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Default)]
pub struct DepsConfig {
    /// Require zig (needed for cargo-zigbuild cross-compilation). Default true.
    #[serde(default = "default_true")]
    pub zig: bool,

    /// Require zip (needed for .zip archives on non-Windows). Default true.
    #[serde(default = "default_true")]
    pub zip: bool,
}

fn default_true() -> bool {
    true
}

// ---------------------------------------------------------------------------
// Load / validate
// ---------------------------------------------------------------------------

pub fn load(workspace: &Path) -> Result<ForgeConfig> {
    let path = workspace.join("forge.toml");
    if !path.exists() {
        bail!(
            "forge.toml not found in {}\n  Run: cargo forge init",
            workspace.display()
        );
    }
    let text = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    let config: ForgeConfig = toml::from_str(&text)
        .with_context(|| format!("Failed to parse {}", path.display()))?;

    validate(&config)?;
    Ok(config)
}

fn validate(config: &ForgeConfig) -> Result<()> {
    if config.forge.binary.is_empty() {
        bail!("forge.toml: [forge] binary cannot be empty");
    }
    if config.forge.target.is_empty() {
        bail!("forge.toml: no [[forge.target]] entries -- add at least one target");
    }
    for t in &config.forge.target {
        if t.triple().is_none() {
            bail!(
                "forge.toml: unknown platform '{}'\n  \
                 Supported: freebsd-x86_64, linux-x86_64, linux-aarch64, \
                 windows-x86_64, macos-x86_64, macos-aarch64",
                t.platform
            );
        }
    }
    Ok(())
}