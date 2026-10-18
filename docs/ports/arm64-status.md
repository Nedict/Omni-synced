# ARM64 port status

Status as of 2026-10-18, from the `port/arm64` branch. This page records what is
built, what runs, what was not tested, and what needs a real ARM device.
The audit this port started from is in [arm64-audit.md](arm64-audit.md);
Windows specifics are in [windows-arm64.md](windows-arm64.md).

## Are the README's ARM64 downloads built by CI today?

**Yes — both ARM64 artifacts are produced by the release pipeline today**
(`.github/workflows/release.yml`). Neither is produced by the test CI
(`.github/workflows/ci.yml`), which has no ARM64 job.

| Artifact | Built how | Where |
|---|---|---|
| `solidcraft-<version>-linux-aarch64.AppImage` (+ `.deb`, `.rpm`, `.tar.gz`) | **Native build** on a GitHub-hosted ARM64 runner (`ubuntu-22.04-arm`), glibc 2.35 | `release.yml` → job `linux`, matrix `{ arch: aarch64, runner: ubuntu-22.04-arm, deb: arm64 }` (release.yml:119) |
| `solidcraft-<version>-linux-aarch64.flatpak` | Repackaged from the aarch64 tarball on `ubuntu-24.04-arm` | `release.yml` → job `flatpak`, matrix `{ arch: aarch64, runner: ubuntu-24.04-arm }` (release.yml:176) |
| `solidcraft-<version>-linux-aarch64.snap` | Repackaged from the aarch64 tarball on `ubuntu-24.04-arm` | `release.yml` → job `snap`, matrix `{ arch: aarch64, runner: ubuntu-24.04-arm }` (release.yml:214) |
| `solidcraft-<version>-windows-arm64.msi` (+ portable zip) | **Cross-compiled** on the x64 `windows-latest` runner: `packaging/windows/package.ps1 -Arch arm64` → `wix build … -arch arm64` | `release.yml` → job `windows`, matrix `{ arch: arm64, target: aarch64-pc-windows-msvc }` (release.yml:332) |

Notes:

- The Windows job comment says it outright: "Windows on ARM: cross-compiled on
  the x64 runner" (release.yml:331). The MSI's ARM64-ness comes from WiX's
  `-arch arm64` (package.ps1:97), not from anything in `solidcraft.wxs`; the
  WiX `Package` has no hardcoded `Platform` and WiX v5 derives it from `-arch`.
  The PE header check in package.ps1 (0xAA64) and `ProgramFiles6432Folder`
  are already correct for ARM64.
- The AppImage job downloads `appimagetool-aarch64.AppImage` (package.sh:126-131),
  so the AppImage tool itself runs natively on the ARM64 runner.
- The README's `releases/latest/download/<name>` links resolve because the
  release job copies every `solidcraft-<version>-*` artifact to its version-less
  name (release.yml:415-427); only the x64 copies are asserted to exist, but the
  ARM64 ones are copied too.

## Does CI test ARM64 at all?

- **Linux aarch64: smoke-tested in CI, not test-suite-tested.** The release
  `linux` job runs `APPIMAGE_EXTRACT_AND_RUN=1 <appimage> --version` and checks
  the `.zsync` (release.yml:150-155); the `snap` job installs the aarch64 snap
  and runs `solidcraft.cli --version` plus the UI smoke test under X11/Wayland
  (release.yml:234-243). No unit/engine/UI test runs on an ARM64 runner.
- **Windows arm64: built, never run.** The `windows` release job only builds and
  packages; the D3D12 shader and app-startup tests (`windows-fxc`,
  `windows-startup` in ci.yml) run on x64 `windows-latest` only. Nothing in CI
  executes the ARM64 MSI or its binaries.

## What this branch adds

1. **A compile gate for both targets in the test CI** — see
   [arm64-audit.md](arm64-audit.md) and the `.github/workflows/` changes on this
   branch — so an ARM64 build break is caught without waiting for a release.
2. **Verified native builds and test runs** on real ARM64 hardware (this
   machine is Windows ARM64; WSL provides Linux aarch64) — results below.
3. Packaging scripts were already arch-parameterized (aarch64/arm64 throughout);
   what was missing is recorded in the audit.

## What builds / what runs / what was not tested

### Builds (verified on real ARM64 hardware)

| Target | Build | Result |
|---|---|---|
| `aarch64-unknown-linux-gnu` (WSL Ubuntu 22.04, native) | `cargo build --release -p solidcraft -p solidcraft-cli` | pass (20m 59s; both binaries) |
| `aarch64-pc-windows-gnullvm` (this machine, native) | `cargo build --release -p solidcraft -p solidcraft-cli` | pass (3m 28s; both binaries, icon via llvm-mingw windres) |
| `aarch64-pc-windows-msvc` | `cargo check` (no linking) | not run locally (no MSVC ARM64 linker); CI builds it on x64 runners. Code identical to the gnullvm build. |
| `wasm32-unknown-unknown` | `cargo xtask wasm` | pass (12 crates) |

### Runs (verified on real ARM64 hardware)

- Windows ARM64: `solidcraft-cli --version` prints `solidcraft-cli 0.15.0`; `solidcraft-cli snapshot` rendered a 512x512 box PNG via the GPU renderer (`renderer: gpu`) on this machine's Adreno-class GPU.
- Linux aarch64: release binaries built; core crate tests compiled and ran.
- Clippy (`-D warnings`) on the gnullvm host: pass (7m 00s).
- Lints already passing: `fmt`, `assets` (376 files), `licences`, `lists`, `layers` (17 crates).

### Not verified / known gaps

- robustness_goldens seed 13 and seed 67 fail on Linux aarch64: topological difference (seed 13: area 2101.52 -> 2112.85, edges 103 -> 99, faces 41 -> 39, volume 2396.94 -> 2410.78; seed 67: edges 88 -> 86, faces 37 -> 36). Golden regression tests; the framework supports per-platform overrides but only by OS (`std::env::consts::OS`), not by arch. Needs investigation: likely a tolerance-sensitive boolean in the kernel whose outcome flips on aarch64 FP. Not a crash; not blocking CI (the aarch64 gate is compile-only).
- App/UI tests on Linux aarch64 hang in headless WSL (need a real display for the app test binary); core crate tests (geom/sketch/kernel/render/doc/engine) compile and run.
- Windows ARM64 MSI not installed or run (no Snapdragon X device in CI; the binary runs on this ARM64 host but the MSI packaging path is untested). The D3D12 shader tests and app-startup tests (`windows-fxc`, `windows-startup`) run on x64 only.
- 3D mouse (hidapi) not exercised on either target (no device attached).
- `cargo xtask icons` fails on this Windows host for pre-existing reasons unrelated to ARM64: `python3` not on PATH (xtask spawns `python3`), and two Python tool tests assume Linux (`/` path separators, cp1252 default encoding). Passes on CI's Linux runners. Workaround used locally: a `python3.exe` shim + `PYTHONUTF8=1`.

### What needs a real ARM device

- Linux aarch64: AppImage/deb/rpm install + run, snap smoke (X11 + Wayland), Vulkan via Mesa on a real ARM GPU (not lavapipe), wgpu GL fallback.
- Windows aarch64: install the MSI, app startup on the Adreno GPU (DX12) and WARP fallback, D3D12 shader tests, hidapi 3D-mouse detection, `solidcraft-cli` end-to-end.
