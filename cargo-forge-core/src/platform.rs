//! Platform detection and tool installation.

use anyhow::{bail, Result};
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

/// Install zig on the current platform.
pub fn install_zig(host: HostOs) -> Result<()> {
    match host {
        HostOs::FreeBsd => exec("sudo", &["pkg", "install", "-y", "zig"])?,
        HostOs::Linux => {
            if cmd_exists("apt-get") {
                exec("sudo", &["apt-get", "install", "-y", "zig"])?;
            } else if cmd_exists("dnf") {
                exec("sudo", &["dnf", "install", "-y", "zig"])?;
            } else {
                bail!(
                    "Could not auto-install zig.\n  \
                     Install manually: https://ziglang.org/download/"
                );
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