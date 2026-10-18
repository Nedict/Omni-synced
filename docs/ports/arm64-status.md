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

Filled in as the port progresses; see the final section of this file at handoff.
