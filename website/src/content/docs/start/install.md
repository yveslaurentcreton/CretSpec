---
title: Install CretSpec
description: Install the native cspec command on Windows, macOS or Linux.
---

CretSpec runs as a single native executable. Install **Git** and make sure `git --version` works in your terminal. You do not need a server or a runtime for a downloaded binary.

## Current availability

The first native release is being prepared. Building from this source checkout works now. Release archives and the package channels below become installable after publication; their commands are shown here for that stage.

| Channel | Package | Publication status |
| --- | --- | --- |
| GitHub Releases | Platform archive | Awaiting first native release |
| WinGet | `YvesLaurentCreton.CretSpec` | Metadata generated; catalog submission pending |
| Homebrew | `yveslaurentcreton/tap/cretspec` | Formula generated; public tap pending |
| AUR | `cretspec-bin` | PKGBUILD generated; AUR publication pending |

## Build from source

Install [Rust](https://rustup.rs/) and your platform's Rust build prerequisites. Clone the product repository, enter it, and install:

```sh
git clone https://github.com/yveslaurentcreton/CretSpec.git
cd CretSpec
cargo install --path . --locked
cspec --version
```

The checkout pins its toolchain in `rust-toolchain.toml`. Cargo installs `cspec` into its binary directory, usually `~/.cargo/bin` or `%USERPROFILE%\.cargo\bin`. Add that directory to your PATH if needed. `cargo uninstall cretspec` removes this installation.

## Release archives

After publication, download your archive and `SHA256SUMS` from the same [GitHub release](https://github.com/yveslaurentcreton/CretSpec/releases).

| Platform | Archive target | Baseline |
| --- | --- | --- |
| Windows x64 | `x86_64-pc-windows-msvc` | Windows 10/11; built and tested on Server 2022 |
| Linux x64 | `x86_64-unknown-linux-gnu` | glibc 2.35 or newer; Ubuntu 22.04 build |
| macOS Intel | `x86_64-apple-darwin` | CI on macOS 15 |
| macOS Apple silicon | `aarch64-apple-darwin` | CI on macOS 14 |

Linux needs the usual glibc and GCC runtime libraries. Alpine/musl and Linux/Windows ARM builds are outside this release matrix. macOS binaries are not notarized; respect your organization's application policy.

On Windows, calculate the ZIP hash with `Get-FileHash <archive.zip> -Algorithm SHA256` and compare it with its entry in `SHA256SUMS`. Extract it, place `cspec.exe` in a directory on your user PATH, and open a new terminal.

On Linux use `sha256sum <archive.tar.gz>`; on macOS use `shasum -a 256 <archive.tar.gz>`. Compare the complete hash with the matching entry, extract the archive, and copy `cspec` into a directory on your PATH, for example:

```sh
mkdir -p "$HOME/.local/bin"
install -m 755 cspec "$HOME/.local/bin/cspec"
```

Add `~/.local/bin` to your shell PATH if necessary. To upgrade, repeat the verified extraction with the new release. To uninstall a manual installation, remove the executable you placed on PATH. Your project repositories remain independent.

## Package managers, after publication

```powershell
winget install --exact --id YvesLaurentCreton.CretSpec
winget upgrade --exact --id YvesLaurentCreton.CretSpec
winget uninstall --exact --id YvesLaurentCreton.CretSpec
```

```sh
brew tap yveslaurentcreton/tap
brew install yveslaurentcreton/tap/cretspec
brew upgrade cretspec
brew uninstall cretspec
```

Review and trust the maintainer's Homebrew tap according to your Homebrew version's prompts. AUR users can review the PKGBUILD and install with `paru -S cretspec-bin`, upgrade with `paru -Syu`, or remove it with `paru -R cretspec-bin`. Paru is a client for AUR.

## Replace the earlier Node.js installation

If you installed the prototype globally with npm, remove it with `npm uninstall -g cretspec` when adopting the native version. Then install the native command and check `cspec --version`. Use `Get-Command cspec -All` on PowerShell or `type -a cspec` on Unix to find conflicting installations.

Existing schema-version-1 project definitions and locks remain compatible. Personal namespace configuration remains compatible; see [configuration](/CretSpec/reference/configuration/).
