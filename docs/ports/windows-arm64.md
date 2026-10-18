# Windows ARM64 port

Notes for `aarch64-pc-windows-msvc`. Related: [arm64-status.md](arm64-status.md),
[arm64-audit.md](arm64-audit.md).

## Target triple

- `aarch64-pc-windows-msvc` — what CI cross-compiles on the x64
  `windows-latest` runner (`.github/workflows/release.yml:331-332`) and what
  `packaging/windows/package.ps1 -Arch arm64` packages
  (`solidcraft-<version>-windows-arm64.msi`, the name the README links).
- The MSI needs no `Platform` attribute in `assets/platforms/windows/solidcraft.wxs`:
  WiX v5 takes the architecture from `wix build -arch arm64`
  (`packaging/windows/package.ps1:97`). The single `UpgradeCode` is shared with
  the x64/x86 MSIs on purpose (a major upgrade replaces an older install of any
  architecture), and `ProgramFiles6432Folder` is the right folder for ARM64
  binaries. The PE header check (`0xAA64`, package.ps1:75) already matches.

## Toolchain

- CI: MSVC target, cross-compiled from an x64 Windows runner with the MSVC
  ARM64 build tools (release.yml installs only the target; the runner image has
  the ARM64 linker/libs). From Linux the documented path is
  `cargo xwin build --release --target aarch64-pc-windows-msvc -p solidcraft`.
- This machine (used for verification): **Windows ARM64** (`PROCESSOR_ARCHITECTURE=ARM64`)
  with rustup's default `stable-aarch64-pc-windows-gnullvm`. That produces
  `aarch64-pc-windows-gnullvm` binaries — a different, GNU-flavoured flavour of
  the same ARM64 Windows target. It runs the same OS and the same user code;
  only the C runtime and object format flavour differ (MSVCRT vs mingw-style).
  Known local quirk: the gnullvm sysroot ships import libraries only for the
  core DLLs, so extra system DLLs need import libs in
  `~/.cargo/syslibs-aarch64-gnullvm/` (see the global `.cargo/config.toml`).

## Known risks

- **The app is never run in CI on ARM64.** The D3D12 shader tests and the
  app-startup tests (`ci.yml`: `windows-fxc`, `windows-startup`) run on x64
  only. First real risks, in order: D3D12 adapter selection on a Qualcomm
  Adreno GPU (backend order is DX12 → Vulkan → GL, `startup.rs:178`), WARP
  fallback, and the `windows-sys` calls in `native_crash.rs`/`native_frame.rs`
  (they use only `windows-sys`, which is arch-agnostic, but they have never
  executed on ARM64).
- `ring` (TLS) has assembly for aarch64-pc-windows — upstream-supported, but it
  is exercised only at runtime here.
- `hidapi` `windows-native` (3D mouse) calls plain Win32 `HidD_*` functions —
  compiles anywhere, but no 3D mouse is attached to this machine.
- `keyring` `windows-native` (agent-chat API keys) uses the Credential Manager
  via `windows-sys` — same class of never-run-on-ARM64 risk.

## Build command

On this ARM64 host (gnullvm):

```sh
cargo build --release -p solidcraft -p solidcraft-cli        # native ARM64 Windows
```

On an x64 Windows runner / Linux (the CI path):

```sh
rustup target add aarch64-pc-windows-msvc
cargo build --release --target aarch64-pc-windows-msvc -p solidcraft -p solidcraft-cli
# or from Linux: cargo xwin build --release --target aarch64-pc-windows-msvc -p solidcraft
```

## Packaging

- `packaging/windows/package.ps1 -Arch arm64` already: builds for
  `aarch64-pc-windows-msvc`, checks the PE machine type (0xAA64) and
  subsystem, builds the MSI with `wix build … -arch arm64`, names it
  `solidcraft-<version>-windows-arm64.msi` (+ `-portable.zip`), and validates
  the shortcut icons (ICE50). Nothing to change for the arm64 flavour itself.
- On an ARM64 **host** (this machine), package.ps1's `-Arch arm64` target
  `aarch64-pc-windows-msvc` cannot link without MSVC ARM64 tools; the native
  gnullvm target builds the same binaries. The script picks the triple from
  `$Arch` alone (package.ps1:52) — see Open questions.

## Test plan

- Native build of `solidcraft` + `solidcraft-cli` on this host (done, see
  arm64-status.md).
- `cargo xtask test` on this host — the whole workspace test suite executing
  on ARM64 Windows, including the D3D12 shader tests (`solidcraft-gpu`) and the
  app startup/resize tests (`startup::tests`, `native_crash::tests`,
  `native_frame::tests`), which is the part CI never runs on ARM64.
- App smoke: start the real app headless (or with a display), open a design,
  screenshot; `solidcraft-cli eval` on an oracle recipe.
- Optional on hardware: install the MSI on a Snapdragon X machine.

## Open questions

- Should `package.ps1 -Arch arm64` detect an ARM64 host and build natively
  (gnullvm or the MSVC target when present) instead of requiring the MSVC
  ARM64 linker? CI (x64 cross) stays as it is.
- Do we want a `windows-11-arm` CI job that actually installs and runs the MSI
  (and the D3D12 tests) on ARM64? That is a paid runner decision for the owner.
- The gnullvm-vs-msvc flavour difference: is a gnullvm-built MSI acceptable as
  a local/preview artifact, or must every shipped MSI be MSVC? (CI ships MSVC.)
