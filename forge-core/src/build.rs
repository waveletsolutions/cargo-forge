//! `cargo forge build` -- build release binaries for all configured platforms.

use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::config::{ForgeConfig, TargetConfig};
use crate::output::*;
use colored::Colorize;
use crate::platform::{exec, run_captured, HostOs};
use crate::workspace;

pub fn run(workspace: &Path, config: &ForgeConfig, suffix: Option<&str>) -> Result<()> {
    let host = HostOs::detect();

    // Resolve version
    let base_version = match config.forge.version_from.as_str() {
        "forge.toml" => config.forge.version.clone()
            .context("forge.toml: version_from = \"forge.toml\" but no version field set")?,
        _ => workspace::read_version(workspace)?,
    };
    let version = match suffix {
        Some(s) => format!("{}-{}", base_version, s.trim_start_matches('v')),
        None    => base_version.clone(),
    };

    // Print header
    println!();
    println!("{}", format!("Building {} v{} -- all platforms", config.forge.binary, version).bold());
    println!("  Version:  {} (from {})", base_version, config.forge.version_from);
    if let Some(s) = suffix {
        println!("  Suffix:   {}", s);
    }
    println!("  Host:     {}", host);

    // Clean and recreate artifacts dir
    let artifacts = workspace.join(&config.forge.artifacts_dir);
    if artifacts.exists() {
        fs::remove_dir_all(&artifacts)?;
    }
    fs::create_dir_all(&artifacts)?;

    // Build each target
    for t in &config.forge.target {
        if t.is_native(host.as_str()) {
            build_native(workspace, config, &version, t, &artifacts)?;
        } else {
            build_cross(workspace, config, &version, t, &artifacts)?;
        }
    }

    // Checksums
    generate_checksums(&artifacts)?;

    // Summary
    println!();
    println!("{}", format!("=== {} v{} ready ===", config.forge.binary, version).bold().green());
    println!();
    for entry in fs::read_dir(&artifacts)?.flatten() {
        let meta = entry.metadata()?;
        println!(
            "  {} ({:.1} MB)",
            entry.file_name().to_string_lossy(),
            meta.len() as f64 / 1_048_576.0
        );
    }
    println!();
    println!("Upload to your release page:");
    println!("  Tag: v{}", version);
    println!("  Upload everything from {}/", config.forge.artifacts_dir);
    println!();

    Ok(())
}

fn build_native(
    workspace: &Path,
    config: &ForgeConfig,
    version: &str,
    target: &TargetConfig,
    artifacts: &Path,
) -> Result<()> {
    header(&format!("{} (native)", target.platform));
    info("Building...");

    let status = Command::new("cargo")
        .args(["build", "--release"])
        .current_dir(workspace)
        .status()
        .context("Failed to run cargo build")?;
    if !status.success() {
        bail!("cargo build --release failed");
    }

    let binary_name = target.binary_name(&config.forge.binary);
    let binary_path = workspace.join("target").join("release").join(&binary_name);
    let archive_name = format!("{}-{}-{}", config.forge.binary, version, target.platform);

    package(artifacts, &binary_path, &binary_name, &archive_name, target.archive_ext())?;
    ok(&format!("{}.{}", archive_name, target.archive_ext()));
    Ok(())
}

fn build_cross(
    workspace: &Path,
    config: &ForgeConfig,
    version: &str,
    target: &TargetConfig,
    artifacts: &Path,
) -> Result<()> {
    let triple = target.triple().unwrap();
    header(&format!("{} (via cargo-zigbuild)", target.platform));
    info("Building...");

    let status = Command::new("cargo")
        .args(["zigbuild", "--release", "--target", triple])
        .current_dir(workspace)
        .status()
        .context("Failed to run cargo zigbuild")?;
    if !status.success() {
        bail!("cargo zigbuild --release --target {} failed", triple);
    }

    let binary_name = target.binary_name(&config.forge.binary);
    let binary_path = workspace
        .join("target")
        .join(triple)
        .join("release")
        .join(&binary_name);
    let archive_name = format!("{}-{}-{}", config.forge.binary, version, target.platform);

    package(artifacts, &binary_path, &binary_name, &archive_name, target.archive_ext())?;
    ok(&format!("{}.{}", archive_name, target.archive_ext()));
    Ok(())
}

fn package(
    artifacts: &Path,
    binary_path: &Path,
    binary_name: &str,
    archive_name: &str,
    archive_ext: &str,
) -> Result<()> {
    if !binary_path.exists() {
        let dir = binary_path.parent().unwrap();
        let entries: Vec<String> = fs::read_dir(dir)
            .map(|rd| rd.flatten()
                .map(|e| e.file_name().to_string_lossy().to_string())
                .collect())
            .unwrap_or_default();
        bail!(
            "Binary not found: {}\n  Directory contents: {}",
            binary_path.display(),
            if entries.is_empty() { "(empty)".to_string() } else { entries.join(", ") }
        );
    }

    match archive_ext {
        "tar.gz" => {
            let archive = artifacts.join(format!("{}.tar.gz", archive_name));
            exec(
                "tar",
                &[
                    "-czf",
                    archive.to_str().unwrap(),
                    "-C",
                    binary_path.parent().unwrap().to_str().unwrap(),
                    binary_name,
                ],
            )?;
        }
        "zip" => {
            let archive = artifacts.join(format!("{}.zip", archive_name));
            if cfg!(windows) {
                let ps_cmd = format!(
                    "Compress-Archive -Path '{}' -DestinationPath '{}' -Force",
                    binary_path.display(),
                    archive.display()
                );
                exec("powershell", &["-NoProfile", "-Command", &ps_cmd])?;
            } else {
                exec(
                    "zip",
                    &["-q", "-j", archive.to_str().unwrap(), binary_path.to_str().unwrap()],
                )?;
            }
        }
        _ => bail!("Unknown archive format: {}", archive_ext),
    }

    Ok(())
}

fn generate_checksums(artifacts: &Path) -> Result<()> {
    header("Checksums");

    let mut files: Vec<String> = fs::read_dir(artifacts)?
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            if name != "SHA256SUMS" { Some(name) } else { None }
        })
        .collect();
    files.sort();

    let prev = std::env::current_dir()?;
    std::env::set_current_dir(artifacts)?;

    let output = if run_captured("sha256sum", &["--version"]).is_ok() {
        let mut args = vec!["--"];
        for f in &files { args.push(f.as_str()); }
        Command::new("sha256sum").args(&args).output()?
    } else {
        let mut args = vec!["-a", "256", "--"];
        for f in &files { args.push(f.as_str()); }
        Command::new("shasum").args(&args).output()?
    };

    fs::write("SHA256SUMS", &output.stdout)?;
    print!("{}", String::from_utf8_lossy(&output.stdout));

    std::env::set_current_dir(prev)?;
    Ok(())
}