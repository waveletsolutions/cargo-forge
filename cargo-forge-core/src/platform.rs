//! Platform detection and tool installation.

use anyhow::{bail, Context, Result};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

/// Detected host operating system.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HostOs {
    FreeBsd,
    Linux,
    MacOs,
    Windows,
    Unknown,
}

impl HostOs {
    pub fn detect() -> Self {
        match std::env::consts::OS {
            "freebsd" => Self::FreeBsd,
            "linux"   => Self::Linux,
            "macos"   => Self::MacOs,
            "windows" => Self::Windows,
            _         => Self::Unknown,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::FreeBsd => "freebsd",
            Self::Linux   => "linux",
            Self::MacOs   => "macos",
            Self::Windows => "windows",
            Self::Unknown => "unknown",
        }
    }

    pub fn is_windows(&self) -> bool {
        matches!(self, Self::Windows)
    }
}

impl std::fmt::Display for HostOs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Check if a command exists on PATH.
pub fn cmd_exists(cmd: &str) -> bool {
    which::which(cmd).is_ok()
}

/// Run a command, inheriting stdio. Returns error if exit code != 0.
pub fn exec(program: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(program)
        .args(args)
        .status()
        .map_err(|e| anyhow::anyhow!("Failed to run '{}': {}", program, e))?;
    if !status.success() {
        bail!("'{}' exited with status: {}", program, status);
    }
    Ok(())
}

/// Run a command and capture stdout.
pub fn run_captured(program: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to run '{}': {}", program, e))?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Zig version to use when package managers don't have it or have an old version.
const ZIG_VERSION: &str = "0.14.0";

/// Fetch the download URL for zig from the official JSON index.
/// Uses ziglang.org/download/index.json so we always get the correct
/// filename format regardless of version (the format changed in v0.14.1).
fn fetch_zig_download_url(version: &str) -> Result<String> {
    let index_url = "https://ziglang.org/download/index.json";
    let client = reqwest::blocking::Client::builder()
        .user_agent(format!("cargo-forge/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .context("Failed to build HTTP client")?;

    let response = client.get(index_url)
        .send()
        .with_context(|| format!("Failed to fetch zig download index from {}", index_url))?;

    if !response.status().is_success() {
        bail!("Failed to fetch zig download index: HTTP {}", response.status());
    }

    let text = response.text().context("Failed to read zig download index")?;

    // Parse just enough JSON to find the right URL
    // Format: {"0.14.0": {"x86_64-linux": {"tarball": "https://..."}}}
    let arch_os = match (std::env::consts::ARCH, std::env::consts::OS) {
        ("x86_64",  "linux")   => "x86_64-linux",
        ("aarch64", "linux")   => "aarch64-linux",
        ("x86_64",  "macos")   => "x86_64-macos",
        ("aarch64", "macos")   => "aarch64-macos",
        ("x86_64",  "windows") => "x86_64-windows",
        (arch, os) => bail!("Unsupported platform {}-{} for zig download", arch, os),
    };

    // Find version block, then arch_os block, then tarball URL
    // Simple string search -- avoids pulling in a JSON crate
    let version_marker = format!("\"{}\"", version);
    let arch_marker = format!("\"{}\"", arch_os);
    let tarball_marker = "\"tarball\"";

    let version_pos = text.find(&version_marker)
        .with_context(|| format!("zig version {} not found in download index", version))?;
    let after_version = &text[version_pos..];

    let arch_pos = after_version.find(&arch_marker)
        .with_context(|| format!("Platform {} not found for zig {}", arch_os, version))?;
    let after_arch = &after_version[arch_pos..];

    let tarball_pos = after_arch.find(tarball_marker)
        .context("tarball field not found in zig download index")?;
    let after_tarball = &after_arch[tarball_pos + tarball_marker.len()..];

    // Extract URL from: "tarball": "https://..."
    let url_start = after_tarball.find("\"https://")
        .context("tarball URL not found in zig download index")?;
    let url_content = &after_tarball[url_start + 1..]; // skip opening quote
    let url_end = url_content.find('"')
        .context("tarball URL end quote not found")?;

    Ok(url_content[..url_end].to_string())
}

/// Download and install zig using pure Rust HTTP (proxy-aware via reqwest).
/// Respects HTTP_PROXY / HTTPS_PROXY / NO_PROXY environment variables.
/// Installs to ~/.local/share/cargo-forge/zig/ (no sudo required).
fn install_zig_from_tarball() -> Result<()> {
    let arch = match std::env::consts::ARCH {
        "x86_64"  => "x86_64",
        "aarch64" => "aarch64",
        other     => bail!("Unsupported architecture for zig install: {}", other),
    };

    println!("  Looking up zig {} download URL...", ZIG_VERSION);
    let url = fetch_zig_download_url(ZIG_VERSION)
        .with_context(|| format!(
            "Could not find zig {} download URL.\n               Check https://ziglang.org/download/ and install manually.",
            ZIG_VERSION
        ))?;

    // Extract filename from URL
    let filename = url.split('/').last()
        .context("Could not parse filename from zig download URL")?
        .to_string();

    // Extract directory name (strip .tar.xz or .zip)
    let dir_name = filename
        .trim_end_matches(".tar.xz")
        .trim_end_matches(".zip")
        .to_string();

    // Install to ~/.local/share/cargo-forge/zig/ -- no sudo required
    let install_base = zig_install_dir()?;
    let zig_dir = install_base.join(&dir_name);
    let zig_bin = zig_dir.join(if cfg!(windows) { "zig.exe" } else { "zig" });
    let _ = arch; // used in fetch_zig_download_url via consts

    if zig_bin.exists() {
        println!("  zig {} already downloaded", ZIG_VERSION);
        ensure_zig_on_path(&zig_bin)?;
        return Ok(());
    }

    println!("  Package manager does not have zig -- downloading v{} via Rust HTTP...", ZIG_VERSION);
    println!("  URL: {}", url);
    println!("  (respects HTTP_PROXY / HTTPS_PROXY environment variables)");

    // Download with reqwest (blocking, proxy-aware)
    let client = reqwest::blocking::Client::builder()
        .user_agent(format!("cargo-forge/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .context("Failed to build HTTP client")?;

    let mut response = client.get(&url)
        .send()
        .with_context(|| format!("Failed to download zig from {}", url))?;

    if !response.status().is_success() {
        bail!(
            "Failed to download zig: HTTP {} from {}
               Check https://ziglang.org/download/ for the correct URL.",
            response.status(),
            url
        );
    }

    // Stream download to a temp file
    let tmp_path = install_base.join(&filename);
    fs::create_dir_all(&install_base)
        .context("Failed to create zig install directory")?;

    {
        let mut tmp_file = fs::File::create(&tmp_path)
            .context("Failed to create temp file for zig download")?;
        let mut downloaded: u64 = 0;
        let mut buf = vec![0u8; 65536];
        #[allow(unused_imports)]
        use std::io::Read;
        loop {
                        let n = response.read(&mut buf)
                .context("Failed to read download stream")?;
            if n == 0 { break; }
            tmp_file.write_all(&buf[..n])
                .context("Failed to write zig download")?;
            downloaded += n as u64;
            if downloaded % (1024 * 1024 * 10) < 65536 {
                println!("  Downloaded {:.1} MB...", downloaded as f64 / 1_048_576.0);
            }
        }
        println!("  Download complete ({:.1} MB)", downloaded as f64 / 1_048_576.0);
    }

    // Extract .tar.xz using pure Rust (xz2 + tar)
    println!("  Extracting to {}...", install_base.display());
    {
        let tmp_file = fs::File::open(&tmp_path)
            .context("Failed to open downloaded zig tarball")?;
        let xz_decoder = xz2::read::XzDecoder::new(tmp_file);
        let mut archive = tar::Archive::new(xz_decoder);
        archive.unpack(&install_base)
            .context("Failed to extract zig tarball")?;
    }

    // Clean up tarball
    let _ = fs::remove_file(&tmp_path);

    if !zig_bin.exists() {
        bail!(
            "Extraction succeeded but zig binary not found at {}
               Expected directory: {}",
            zig_bin.display(),
            zig_dir.display()
        );
    }

    // Make executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&zig_bin)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&zig_bin, perms)?;
    }

    ensure_zig_on_path(&zig_bin)?;

    println!("  zig {} installed to {}", ZIG_VERSION, zig_dir.display());
    Ok(())
}

/// Returns the cargo-forge zig install directory (~/.local/share/cargo-forge/zig).
fn zig_install_dir() -> Result<PathBuf> {
    let base = dirs::data_local_dir()
        .or_else(|| std::env::var("HOME").ok().map(|h| PathBuf::from(h).join(".local").join("share")))
        .context("Could not determine local data directory")?;
    Ok(base.join("cargo-forge").join("zig"))
}

/// Returns the path to the managed zig binary if it exists.
/// Scans the install dir for any zig-* subdirectory containing a zig binary.
pub fn find_managed_zig() -> Option<PathBuf> {
    let install_base = zig_install_dir().ok()?;
    if !install_base.exists() {
        return None;
    }
    // Walk subdirectories looking for a zig binary
    std::fs::read_dir(&install_base).ok()?
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .filter_map(|e| {
            let zig_bin = e.path().join(if cfg!(windows) { "zig.exe" } else { "zig" });
            if zig_bin.exists() { Some(zig_bin) } else { None }
        })
        .next()
}

/// Add the zig binary's directory to PATH in the current process,
/// and print a note telling the user to add it to their shell config.
fn ensure_zig_on_path(zig_bin: &std::path::Path) -> Result<()> {
    let zig_dir = zig_bin.parent().unwrap();
    let current_path = std::env::var("PATH").unwrap_or_default();
    let zig_dir_str = zig_dir.to_str().unwrap_or("");

    if !current_path.contains(zig_dir_str) {
        unsafe {
            std::env::set_var("PATH", format!("{}:{}", zig_dir_str, current_path));
        }
    }

    println!();
    println!("  zig is installed but not yet on your shell PATH.");
    println!("  cargo-forge has added it to the current session.");
    println!("  To make it permanent, add this to your ~/.bashrc or ~/.zshrc:");
    println!();
    println!("    export PATH=\"{}:$PATH\"", zig_dir_str);
    println!();
    Ok(())
}

/// Install zig on the current platform.
pub fn install_zig(host: HostOs) -> Result<()> {
    match host {
        HostOs::FreeBsd => exec("sudo", &["pkg", "install", "-y", "zig"])?,
        HostOs::Linux => {
            // Try package managers first, but they're often outdated.
            // apt on Ubuntu/Debian frequently doesn't have zig at all.
            // Fall back to downloading the official tarball.
            let mut installed = false;

            if cmd_exists("apt-get") {
                let result = Command::new("sudo")
                    .args(["apt-get", "install", "-y", "zig"])
                    .status();
                if result.map(|s| s.success()).unwrap_or(false) {
                    installed = true;
                }
            } else if cmd_exists("dnf") {
                let result = Command::new("sudo")
                    .args(["dnf", "install", "-y", "zig"])
                    .status();
                if result.map(|s| s.success()).unwrap_or(false) {
                    installed = true;
                }
            }

            if !installed {
                install_zig_from_tarball()?;
            }
        }
        HostOs::MacOs => {
            if cmd_exists("brew") {
                exec("brew", &["install", "zig"])?;
            } else {
                bail!(
                    "Could not auto-install zig.\n  \
                     Install Homebrew first: https://brew.sh\n  \
                     Then: brew install zig"
                );
            }
        }
        HostOs::Windows => {
            if cmd_exists("winget") {
                exec("winget", &["install", "--id", "zig.zig", "-e", "--silent"])?;
                refresh_windows_path()?;
            } else if cmd_exists("choco") {
                exec("choco", &["install", "zig", "-y"])?;
            } else {
                bail!(
                    "Could not auto-install zig.\n  \
                     Install via winget: winget install zig.zig\n  \
                     Or download from: https://ziglang.org/download/"
                );
            }
            if !cmd_exists("zig") {
                bail!(
                    "zig was installed but is not yet on PATH.\n  \
                     Open a new terminal and run: cargo forge build\n  \
                     (Windows PATH updates require a new shell session)"
                );
            }
        }
        HostOs::Unknown => {
            bail!(
                "Unknown OS -- cannot auto-install zig.\n  \
                 Install manually: https://ziglang.org/download/"
            );
        }
    }
    Ok(())
}

/// Install zip on the current platform.
/// Not needed on Windows (uses PowerShell Compress-Archive).
pub fn install_zip(host: HostOs) -> Result<()> {
    match host {
        HostOs::FreeBsd => exec("sudo", &["pkg", "install", "-y", "zip"])?,
        HostOs::Linux => {
            if cmd_exists("apt-get") {
                exec("sudo", &["apt-get", "install", "-y", "zip"])?;
            } else if cmd_exists("dnf") {
                exec("sudo", &["dnf", "install", "-y", "zip"])?;
            } else {
                bail!("Could not auto-install zip. Install it manually.");
            }
        }
        HostOs::MacOs => {
            // zip is built into macOS
        }
        HostOs::Windows => {
            // zip not needed -- use PowerShell Compress-Archive
        }
        HostOs::Unknown => {
            bail!("Unknown OS -- cannot auto-install zip. Install it manually.");
        }
    }
    Ok(())
}

/// On Windows, reads updated PATH from registry and applies it to the
/// current process so newly installed tools are immediately available.
pub fn refresh_windows_path() -> Result<()> {
    #[cfg(windows)]
    {
        let sys = run_captured(
            "powershell",
            &["-NoProfile", "-Command",
              "[System.Environment]::GetEnvironmentVariable('PATH','Machine')"],
        ).unwrap_or_default();
        let usr = run_captured(
            "powershell",
            &["-NoProfile", "-Command",
              "[System.Environment]::GetEnvironmentVariable('PATH','User')"],
        ).unwrap_or_default();
        let merged = format!("{};{}", sys, usr);
        unsafe { std::env::set_var("PATH", &merged); }
    }
    Ok(())
}