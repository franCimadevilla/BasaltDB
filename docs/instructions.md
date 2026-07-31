# BasaltDB — Setup & Tooling for Collaborators

This document describes the tools a contributor needs to install to build and
test the BasaltDB repository, based on the environment set up during Phase 1
development.

---

## Prerequisites

- A Windows machine (the instructions below are Windows-specific; see the
  [Alternatives](#alternatives) section for other platforms).
- Git (to clone the repository).

---

## 1. Rust (via rustup)

Install Rust through [rustup](https://rustup.rs/), the standard Rust toolchain
manager:

```powershell
winget install --id Rustlang.Rustup
```

Then verify it works:

```powershell
rustup --version
cargo --version
```

The repository targets Rust `edition 2024` (see `Cargo.toml`), which requires
Rust 1.85 or newer.

---

## 2. GNU toolchain (required for linking)

The repository cannot be built with the default MSVC toolchain because the
Visual Studio MSVC Build Tools (the `link.exe` linker) are **not installed** on
this machine. To get a working linker without a full Visual Studio install,
use the GNU (MinGW-w64) Rust toolchain.

Install the GNU toolchain and its `clippy` component:

```powershell
rustup toolchain install stable-x86_64-pc-windows-gnu --profile minimal
rustup component add --toolchain stable-x86_64-pc-windows-gnu clippy
```

The project already pins this toolchain via `rust-toolchain.toml`, so plain
`cargo` commands select it automatically — no `+toolchain` flag needed.

---

## 3. MinGW-w64 linker

The Rust GNU toolchain bundles `rustc`/`cargo`, but the actual C linker
(`gcc`, `as`, `ld`, `dlltool`) comes from a MinGW-w64 distribution. This
session installed the self-contained **WinLibs** build:

```powershell
winget install --id BrechtSanders.WinLibs.MCF.UCRT
```

WinLibs installs into `AppData\Local\Microsoft\WinGet\Packages`, for example:

```
C:\Users\<you>\AppData\Local\Microsoft\WinGet\Packages\BrechtSanders.WinLibs.MCF.UCRT_<hash>\mingw64\bin
```

That `mingw64\bin` directory must be on `PATH` so `cargo` can find `gcc`,
`as`, and `dlltool.exe`. winget adds it automatically; if linking still fails
with a "program not found" error, add the directory manually:

```powershell
# Replace <...> with the actual installed path
[Environment]::SetEnvironmentVariable("Path", "C:\...\mingw64\bin;$env:Path", "User")
```

---

## 4. Verify the setup

From the repository root, run:

```powershell
cargo build
cargo test
cargo clippy --all-targets
```

Expected results (as of this session):

- `cargo build` completes without errors.
- `cargo test` runs **40 tests**: 13 unit tests, 26 integration tests
  (`tests/storage_tests.rs`), and 1 doc-test — all passing.
- `cargo clippy --all-targets` reports no warnings.

---

## Dependencies

- **`tempfile`** (dev-dependency) is declared in `Cargo.toml`. Cargo downloads
  it automatically on the first build; no manual step required.

---

## Alternatives

### macOS / Linux

These platforms ship a system linker (or use `cc`), so the plain
`stable` toolchain works. Delete or ignore `rust-toolchain.toml` (or replace
its contents) and use:

```bash
rustup toolchain install stable
cargo test
```

### Windows with Visual Studio Build Tools

If you prefer the standard MSVC toolchain, install the "Desktop development
with C++" workload from the [Visual Studio Installer](https://visualstudio.microsoft.com/downloads/)
(Build Tools are sufficient). Then remove the `[toolchain]` section from
`rust-toolchain.toml` and run:

```powershell
rustup toolchain install stable-x86_64-pc-windows-msvc
cargo test
```

---

## Troubleshooting

| Symptom | Fix |
|---------|-----|
| `error: failed to parse the 'edition' key` | Your Cargo is older than 1.85; update rustup/rust. |
| `linker 'link.exe' not found` | You are on the MSVC toolchain without Build Tools. Use the GNU toolchain (section 2) or install Build Tools (Alternatives). |
| `error calling dlltool 'dlltool.exe': program not found` | `mingw64\bin` is not on `PATH`; add it (section 3). |
| `error: 'cargo-clippy.exe' is not installed for the toolchain` | Run the `rustup component add ... clippy` command from section 2. |
