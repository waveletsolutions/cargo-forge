//! `cargo forge clean` -- remove cross-compile artifacts.

use anyhow::Result;
use std::fs;
use std::path::Path;

use crate::config::ForgeConfig;
use crate::output::*;

pub fn run(workspace: &Path, config: &ForgeConfig) -> Result<()> {
    header("Cleaning artifacts");

    // Remove per-target cross-compile directories
    for t in &config.forge.target {
        if let Some(triple) = t.triple() {
            let target_dir = workspace.join("target").join(triple);
            if target_dir.exists() {
                info(&format!("Removing target/{}/...", triple));
                fs::remove_dir_all(&target_dir)?;
                ok(&format!("target/{} removed", triple));
            } else {
                ok(&format!("target/{} already clean", triple));
            }
        }
    }

    // Remove release artifacts
    let artifacts = workspace.join(&config.forge.artifacts_dir);
    if artifacts.exists() {
        info(&format!("Removing {}/...", config.forge.artifacts_dir));
        fs::remove_dir_all(&artifacts)?;
        ok(&format!("{} removed", config.forge.artifacts_dir));
    }

    println!();
    ok("Clean complete.");
    Ok(())
}
