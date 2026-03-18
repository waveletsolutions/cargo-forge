# cargo-forge

**Turn-key cross-platform release builds for Rust projects.**

Build release binaries for Linux, Windows, FreeBSD, and macOS from a single
machine. No VMs, no containers, no Docker. Uses
[zig](https://ziglang.org) as a universal cross-linker via
[cargo-zigbuild](https://github.com/rust-cross/cargo-zigbuild).

---

## Install

```sh
cargo install cargo-forge
```

## Quick start

```sh
cd my-rust-project
cargo forge init        # creates forge.toml
cargo forge fix         # installs zig, zip, rustup targets
cargo forge build       # builds all platform binaries
```

That's it. Release artifacts land in `release-artifacts/` with a `SHA256SUMS`
file ready to upload.

---

## forge.toml

`cargo forge init` generates this for you. Edit to taste:

```toml
[forge]
binary       = "myapp"
version_from = "Cargo.toml"

# Minimum cargo-forge version required (optional)
# min_version = "1.0.0"

[[forge.target]]
platform = "freebsd-x86_64"

[[forge.target]]
platform = "linux-x86_64"

[[forge.target]]
platform = "windows-x86_64"
archive = "zip"

[forge.deps]
zig = true
zip = true
```

### Supported platforms

| Platform | Triple |
|---|---|
| `freebsd-x86_64` | x86_64-unknown-freebsd |
| `linux-x86_64` | x86_64-unknown-linux-gnu |
| `linux-aarch64` | aarch64-unknown-linux-gnu |
| `windows-x86_64` | x86_64-pc-windows-gnu |
| `macos-x86_64` | x86_64-apple-darwin |
| `macos-aarch64` | aarch64-apple-darwin |

---

## Commands

```sh
cargo forge init              # create forge.toml in current project
cargo forge fix               # install missing dependencies
cargo forge check             # verify all dependencies are installed
cargo forge build             # build all configured platforms
cargo forge build --suffix rc1  # pre-release (e.g. 1.0.0-rc1)
cargo forge clean             # remove cross-compile artifacts
```

---

## How it works

- **Native target** is built with plain `cargo build --release`
- **Cross targets** are built with `cargo zigbuild --release --target <triple>`
- **Zig** acts as a universal linker -- no system cross-toolchains needed
- **Windows archives** use PowerShell `Compress-Archive` (no zip required)
- **SHA256SUMS** is generated automatically

### Host support matrix

| Host | FreeBSD | Linux | Windows | macOS |
|---|---|---|---|---|
| FreeBSD | native | zigbuild | zigbuild | - |
| Linux | zigbuild | native | zigbuild | - |
| Windows | zigbuild | zigbuild | native | - |
| macOS | - | zigbuild | zigbuild | native |

---

## Using forge-core as a library (optional)

If you want version-pinned builds via `Cargo.lock`, add `forge-core` as
a dependency in your project's `xtask`:

```toml
# xtask/Cargo.toml
[dependencies]
forge-core = "0.1"
```

```rust
// xtask/src/main.rs
fn main() {
    forge_core::run();
}
```

Add `.cargo/config.toml` to your project root:

```toml
[alias]
xtask = "run --package xtask --"
```

Then `cargo xtask build` and `cargo forge build` both work, reading the same
`forge.toml`. The xtask version is pinned in `Cargo.lock`; the global
`cargo-forge` binary uses whatever version you have installed.

---

## License

Licensed under either of

- [MIT License](LICENSE-MIT)
- [Apache License, Version 2.0](LICENSE-APACHE)

at your option.

Copyright 2026 [Wavelet Solutions LLC](https://waveletsolutions.com) /
Artisan Technologies R&D division.