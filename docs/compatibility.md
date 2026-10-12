# Compatibility shims for the SolveCraft → SolidCraft rename

The product is SolidCraft everywhere. The old name survives only in the shims listed here,
each kept so that existing installs, scripts and files keep working. Remove a shim only in a
release whose notes say so.

## Programs and packages (since 0.5.0)

| Shim | Where | Why | Test |
|---|---|---|---|
| `solvecraft-cli` alias binary: forwards every argument and stream to `solidcraft-cli`, prints a one-line deprecation note on stderr, exits with its status | `apps/solvecraft-cli/src/alias.rs`; shipped by every package (deb, rpm, tar.gz, AppImage, Flatpak, FreeBSD, MSI, portable zip, macOS CLI zip, Nix) | scripts and MCP client setups (`claude mcp add solvecraft -- solvecraft-cli mcp`) keep working for at least one release | `apps/solvecraft-cli/tests/legacy_alias.rs`; release workflow checks the deb contents |
| Linux app ID `ai.storyteller.solvecraft` (Flatpak ID, desktop entry, metainfo, MIME and icon file names, Wayland app_id) | `packaging/env.sh` (`APP_ID`), `packaging/linux/*`, the app's `with_app_id` | a new Flatpak ID would be a different app: installed Flatpaks would never update. The desktop entry, metainfo and Wayland app_id must equal the Flatpak ID | packaging-lint workflow; Flatpak smoke test |
| MSI `UpgradeCode` `6A145FD6-D285-428D-AE84-A69570CDF4DB` (a GUID, no name in it) | `packaging/windows/solidcraft.wxs` | the new MSI upgrades an existing install in place. The install folder moves to `Program Files\SolidCraft` (registry `Software\SolidCraft`); a custom folder chosen for an earlier version is not carried over this once | release workflow (MSI build) |
| deb `Replaces`/`Conflicts`/`Provides: solvecraft`, rpm `Obsoletes`/`Provides: solvecraft` | `packaging/linux/nfpm.yaml` | installing the `solidcraft` package replaces the old `solvecraft` one instead of clashing with its files | release workflow checks the deb fields |
| A copy of each AppImage's `.zsync` under the old name `solvecraft-<ver>-linux-<arch>.AppImage.zsync` | `packaging/linux/package.sh` | earlier AppImages carry update information that looks for that name; the copy points at the new AppImage | release workflow |
| `solvecraft.flatpakrepo` next to `solidcraft.flatpakrepo` | `packaging/flatpak-site.sh` | links in earlier instructions keep working (existing remotes use the repository URL, which is unchanged) | Pages workflow |
| `.solvecraft` file associations (Windows ProgId extension, macOS document type, freedesktop MIME glob and `application/x-solvecraft` alias) | `packaging/windows/solidcraft.wxs`, `packaging/macos/Info.plist.in`, `packaging/linux/ai.storyteller.solvecraft.mime.xml` | existing design files keep opening in SolidCraft from the file manager | packaging-lint workflow |
| `SOLVECRAFT_VERSION`, `SOLVECRAFT_BUILD_DATE`, `SOLVECRAFT_MAINTAINER`, `SOLVECRAFT_NO_DESKTOP_INTEGRATION` read when the `SOLIDCRAFT_*` name is unset | `packaging/env.sh`, `packaging/linux/package.sh`, `packaging/linux/AppRun` | build scripts and AppImage users' environments keep working | packaging tests |

## Not kept

- The macOS bundle identifier is now `ai.storyteller.solidcraft`: apps are installed by dragging
  from the DMG and there is no updater keyed on it. The first launch may ask again for Gatekeeper
  approval.
- The GUI binary has no alias: every package's launcher, shortcut and file association runs
  `solidcraft`.
