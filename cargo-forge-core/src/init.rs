//! `cargo forge init` -- write forge.toml into the current project.

use anyhow::{bail, Result};
use std::fs;
use std::path::Path;

use crate::output::*;
use crate::workspace;

pub fn run(workspace: &Path) -> Result<()> {
    let forge_toml = workspace.join("forge.toml");

    if forge_toml.exists() {
        bail!(
            "forge.toml already exists at {}\n  \
             Edit it directly to change targets.",
            forge_toml.display()
        );
    }

    // Read binary name from Cargo.toml if possible
    let binary = workspace::read_binary_name(workspace)
        .unwrap_or_else(|_| "myapp".to_string());

    let contents = format!(
        r#"[forge]
binary       = "{}"
version_from = "Cargo.toml"

# Minimum cargo-forge version required to build this project (optional).
# min_version = "0.1.0"

# Output directory for release artifacts.
# artifacts_dir = "release-artifacts"

# Add one [[forge.target]] section per platform you want to build for.
# Supported platforms:
#   freebsd-x86_64, linux-x86_64, linux-aarch64,
#   windows-x86_64, macos-x86_64, macos-aarch64
#
# archive defaults to "zip" for Windows, "tar.gz" for everything else.

[[forge.target]]
platform = "freebsd-x86_64"

[[forge.target]]
platform = "linux-x86_64"

[[forge.target]]
platform = "windows-x86_64"
archive = "zip"

[forge.deps]
zig = true   # required for cross-compilation via cargo-zigbuild
zip = true   # required for .zip archives (auto-skipped on Windows)
"#,
        binary
    );

    fs::write(&forge_toml, contents)?;
    ok(&format!("forge.toml written to {}", forge_toml.display()));
    println!();
    println!("  Next steps:");
    println!("    cargo forge fix     install dependencies");
    println!("    cargo forge build   build all platforms");
    println!();

    Ok(())
}
