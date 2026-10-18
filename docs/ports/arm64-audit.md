# ARM64 port audit

Checklist for porting SolidCraft to `aarch64-unknown-linux-gnu` and
`aarch64-pc-windows-msvc`. Done 2026-10-18 on the `port/arm64` branch, from
the x86_64 Windows host baseline (this machine turned out to be Windows ARM64
itself — see [arm64-status.md](arm64-status.md)). Each item: what was looked
for, where, and the verdict.

## 1. x86 SIMD / intrinsics — clean

- `std::arch::x86_64`, `std::arch::x86`, `#[target_feature]`,
  `is_x86_feature_detected!`, `std::arch::aarch64`: **no matches anywhere** in
  the workspace (crates/, apps/, xtask/, vendor/). There is no x86 SIMD path to
  gate and no missing aarch64 fallback.
- Inline asm (`asm!`): **no matches**.
- Endianness: every `from_le_bytes`/`to_le_bytes`/`from_be_bytes`/`to_be_bytes`
  use is a **wire format** (PNG chunks, ICO DIB header, GIF screen descriptor,
  ICC profile headers, spacenav HID reports, texture pixel hashes). These are
  spec-defined byte orders, correct on both endiannesses. No host-endianness
  assumption found.

## 2. Architecture `cfg` branches — clean

- `cfg(target_arch = …)` occurs only for `wasm32`:
  `apps/solidcraft-web/Cargo.toml:22`, `apps/solidcraft-web/src/space_mouse.rs:68,71`,
  `apps/solidcraft-web/src/main.rs:9-18,580`, `xtask/src/web_offline_test.rs`,
  `crates/ui-egui/src/**` (many), `vendor/**` (dev-deps and cfg code). Every one
  has a non-wasm fallback; nothing is x86-only.
- **No `cfg(target_arch = "x86_64")` or `cfg(target_arch = "x86")` anywhere**,
  so there is no branch that silently compiles to nothing on aarch64.
- Pointer width: all `as usize` casts are entity-ID → `Vec` index conversions
  (e.g. `crates/doc/src/sheet.rs:1072`, `crates/space-mouse/src/descriptor.rs:316`).
  Both targets are 64-bit; no `target_pointer_width` assumption exists.

## 3. Vendored code — clean

- `vendor/` contains `egui-wgpu`, `matext4cgmath`, `truck-geometry`,
  `truck-meshalgo`, `truck-shapeops`, `truck-stepio` — **pure Rust, no C/C++
  sources** (searched `*.c`, `*.cc`, `*.cpp`, `*.h`: none). No build script
  compiles C with x86-only flags.

## 4. Build scripts — clean

| File | What it does | ARM64 verdict |
|---|---|---|
| `apps/solidcraft/build.rs` | Copies the .ico and writes a .rc, `embed_resource::compile_for(…, embed_resource::NONE)` | Arch-agnostic (`NONE` = no architecture). |
| `crates/ui-egui/build.rs` | Git commit/date env, compiles SVG masters to `icon_data.rs`, generates one `#[test]` per scenario | Pure Rust, arch-agnostic. |
| `crates/doc/build.rs` | Generates the design-format parts list | Pure Rust, arch-agnostic. |
| `crates/kernel/src/build.rs` | Not a build script — a geometry-builder module (`mod build`) | n/a |

## 5. Native dependencies — watch items

- `hidapi 2.6.7` with `windows-native` (`apps/solidcraft/Cargo.toml:48`) and
  `macos-shared-device` (:72). No Linux feature is enabled, so on Linux the 3D
  mouse reads `/dev/hidrawN` directly (see §6). The `windows-native` backend is
  plain Win32 `HidD_*` calls — compiles for `aarch64-pc-windows-msvc`.
- `zstd` (via `cadmpeg`, `crates/io/Cargo.toml:28`): C, but builds with the
  target's cc and uses runtime CPU detection — fine on aarch64 (both targets).
- `rustls`/`ring` (`apps/solidcraft/Cargo.toml:39`): `ring` has assembly for
  aarch64; it is a released, upstream-supported path (used on linux-aarch64 by
  the release CI already).
- `tiny-skia` with `simd` (root `Cargo.toml:79`): builds today on
  `ubuntu-22.04-arm` in the release CI (the aarch64 AppImage/snap/flatpak all
  build), so its `simd` feature is aarch64-clean.
- `embed-resource`, `toml`, `ureq`, `keyring` (`windows-native`):
  arch-agnostic or ARM64-supported upstream.

## 6. 3D mouse (spacenav / hidraw / 3DxWare) — clean

- `crates/space-mouse/src/{lib,descriptor}.rs`: report-descriptor parsing, pure
  Rust, no arch or endianness assumptions (byte orders are HID report order).
- `apps/solidcraft/src/space_mouse/hidraw.rs`: plain blocking file I/O on
  `/dev/hidrawN` + sysfs — no x86 assumption; compiles on aarch64 Linux.
- `apps/solidcraft/src/space_mouse/spacenavd.rs`: UNIX-domain socket protocol —
  arch-agnostic.
- Windows path is `hidapi` `windows-native`; macOS is `hidapi`
  `macos-shared-device` + the 3Dconnexion client framework
  (`crates/threedx-client/src/macos.rs`, macOS-only).

## 7. wgpu backend selection — clean

- `apps/solidcraft/src/startup.rs:133-158` (`choose_backends`) and
  `WINDOWS_ORDER` (:178): on Windows try DX12 → Vulkan → GL with a
  hardware-adapter probe, else DX12 (WARP); elsewhere wgpu's default. Arch-
  agnostic by construction:
  - Linux aarch64: Mesa Vulkan (or GL) via wgpu default; lavapipe for software.
  - Windows aarch64 (Snapdragon X): DX12 first — the API its drivers are tested
    with; WARP is the software fallback; `SOLIDCRAFT_GPU=cpu|software|hardware`
    overrides (`docs/book/src/guide/getting-started.md`).
- `crates/ui-egui/src/gpu.rs:113-151` (`offscreen_backends`): same logic for
  offscreen tests. No change needed.

## 8. Packaging — already arch-parameterized (gaps: none found)

| File | aarch64/arm64 handling |
|---|---|
| `packaging/linux/package.sh` | `ARCH=aarch64; DEB_ARCH=arm64` (:35-40); `BASENAME=…-linux-aarch64` (:42); downloads `appimagetool-aarch64.AppImage` (:126-131). |
| `packaging/linux/nfpm.yaml` | `arch: ${NFPM_ARCH}` (:13); package.sh exports `NFPM_ARCH=arm64` for aarch64. |
| `packaging/linux/snap-pack.sh` + `snap/snapcraft.yaml` | `SNAP_ARCH=arm64` (snap-pack.sh:35); `platforms: [amd64, arm64]` (snapcraft.yaml:37-39). |
| `packaging/linux/flatpak-bundle.sh` + `flatpak/*.yml` | aarch64 matrix entry in `release.yml:176`. |
| `packaging/windows/package.ps1` | `-Arch arm64` → target `aarch64-pc-windows-msvc` (:52), PE machine 0xAA64 check (:75), `wix build … -arch arm64` (:97), MSI name `solidcraft-$Version-windows-arm64.msi` (:95) — matches the README link. |
| `assets/platforms/windows/solidcraft.wxs` | No hardcoded `Platform`; WiX v5 derives it from `-arch`. `ProgramFiles6432Folder` (:41) is correct for ARM64. |
| `packaging/docker/Dockerfile` | wasm site + nginx — multi-arch by nature (`docker.yml` builds amd64+arm64). |
| `packaging/freebsd/`, `packaging/macos/` | x86_64 / universal only; not part of this port. |

## 9. CI — gaps found (what this branch adds)

- `.github/workflows/ci.yml`: **no ARM64 job at all** (only manual `wasm` and
  `riscv64-gates`). An ARM64 compile break lands unnoticed between releases.
- `.github/workflows/release.yml`: builds Linux aarch64 **natively**
  (`ubuntu-22.04-arm`, :119) and Windows arm64 **cross-compiled on x64**
  (:331-332). Linux aarch64 gets a smoke test (`--version`, snap install);
  **Windows arm64 gets none** — the MSI is never installed or run in CI.
- `.github/workflows/packaging-lint.yml`: `actionlint` over workflows and
  `shellcheck` over packaging scripts — anything added here must pass both.
- Release `windows` job runs `packaging/windows/package.ps1 -Arch arm64`
  on an x64 runner: needs the MSVC ARM64 linker, which `cargo xwin` provides.
  On an ARM64 Windows host the native target (`aarch64-pc-windows-gnullvm`,
  or `-msvc` where the MSVC tools exist) can build the same binaries.

## 10. Baseline on the host (`cargo xtask ci`, 2026-10-18)

This machine is **Windows ARM64** (`aarch64-pc-windows-gnullvm`, PROCESSOR_ARCHITECTURE=ARM64).

| Gate | Result |
|---|---|
| `cargo fmt --check` | pass |
| `cargo xtask assets` | pass (376 files, all attributed) |
| `cargo xtask icons` | **fail — pre-existing Windows-host issue, not ARM64**: `python3` not on PATH (xtask spawns `python3`, `xtask/src/icons.rs:11`) and two Python tool tests assume Linux (`tests/tools/test_icon_export.py:38` expects `/` path separators; `tests/tools/test_icon_review.py` review server). CI's Linux runners pass. Workaround used locally: a `python3.exe` shim + `PYTHONUTF8=1`. |
| `cargo xtask licences --check` | pass |
| `cargo xtask lists` | pass |
| `cargo xtask layers` | pass (17 crates, no violations) |
| `cargo clippy -D warnings` | see final run |
| `cargo xtask test` | see final run |
| wasm32 check | see final run |

## 11. What a real ARM device must confirm

- **Linux aarch64**: AppImage `--version`, `.deb`/`.rpm` install, snap smoke
  (X11 + Wayland), `solidcraft-cli` eval/run on a recipe, Vulkan via Mesa on a
  real ARM GPU (not lavapipe), wgpu GL fallback.
- **Windows aarch64**: install the MSI, app startup (D3D12 on the Adreno GPU
  and WARP fallback), the D3D12 shader tests, hidapi 3D-mouse detection, and
  `solidcraft-cli` end-to-end.
