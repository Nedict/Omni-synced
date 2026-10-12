SolidCraft is open-source parametric 3D CAD in Rust: sketch, constrain, extrude, and change a
parameter to watch the timeline rebuild. [Documentation](https://bherbruck.github.io/solidcraft/)
· [try it in the browser](https://bherbruck.github.io/solidcraft/app/) ·
[contributing](https://github.com/bherbruck/solidcraft/blob/main/CONTRIBUTING.md) ·
[issues](https://github.com/bherbruck/solidcraft/issues). AI agents can model with it over MCP:
`solidcraft-cli mcp` ([how](https://bherbruck.github.io/solidcraft/guide/mcp.html)).

**Renamed to SolidCraft.** Downloads, installers and programs now carry the SolidCraft name:
the desktop app is `solidcraft`, the command line `solidcraft-cli`. `solvecraft-cli` is still
installed as a deprecated alias that forwards to `solidcraft-cli` (it prints a one-line note on
stderr), so scripts and MCP setups such as `claude mcp add solvecraft -- solvecraft-cli mcp`
keep working; new setups use `claude mcp add solidcraft -- solidcraft-cli mcp`. Existing designs
open as before. Upgrades: the `.msi` upgrades an earlier install in place (it now installs to
`Program Files\SolidCraft`); the `solidcraft` `.deb`/`.rpm` replaces the old `solvecraft`
package; the Flatpak app ID `ai.storyteller.solvecraft` is unchanged, so Flatpak updates
continue; earlier AppImages still find updates.

**Which file?**
- **Windows:** `SolidCraft-{version}-windows-x64.exe` runs as is. Or install the `.msi`, or unzip the
  `-portable.zip` (x64, x86 and arm64).
- **macOS 11+** (Apple silicon and Intel): open the `.dmg` and drag `SolidCraft.app` onto Applications.
  The CLI is in `solidcraft-cli-{version}-macos-universal.zip`.
- **Linux** (x86_64 and aarch64, glibc 2.35+): the `.AppImage` (`chmod +x`, then run it), the
  `.deb` or `.rpm`, the `.flatpak` (`flatpak install --user <file>`), or the `.tar.gz`.
- **FreeBSD:** `tar -xzf solidcraft-{version}-freebsd-x86_64.tar.gz --strip-components 1 -C /usr/local`.
- **Web:** `solidcraft-web-{version}.zip` is a static site to host yourself.
- `SHA256SUMS.txt`: `sha256sum -c SHA256SUMS.txt --ignore-missing`.

**Updating macOS from SolveCraft.app:** quit the app, rename the existing app in
Applications to `SolidCraft.app`, then drag the new app from the DMG onto Applications
and choose **Replace**. If both names are already installed, move the old `SolveCraft.app`
to the Trash. Remove any Dock shortcut with the old name and pin the new app. Designs
and settings are unchanged. The DMG includes these instructions.

For Flatpak feed installation and updates, add Flathub for the required runtime before
installing SolidCraft; keep both remotes in the same user installation:

```sh
flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak remote-add --user --if-not-exists solidcraft https://bherbruck.github.io/solidcraft/flatpak/solidcraft.flatpakrepo
flatpak install --user solidcraft ai.storyteller.solvecraft
flatpak run ai.storyteller.solvecraft
```

Single-file `.flatpak` bundles include the runtime repository address; missing runtimes
still need an internet connection. [Linux installation details](https://bherbruck.github.io/solidcraft/guide/getting-started.html#linux-flatpak).

The builds aren't signed yet. On macOS, right-click `SolidCraft.app` in Applications and choose
**Open** the first time (or run `xattr -dr com.apple.quarantine /Applications/SolidCraft.app`).
On Windows, SmartScreen may say "Windows protected your PC": **More info**, then **Run anyway**.
Licences are built in (Help ▸ About ▸ Licences, `solidcraft-cli licences`); the packages also
install `THIRD-PARTY-LICENSES.txt`. Known limitations:
[ROADMAP.md](https://github.com/bherbruck/solidcraft/blob/main/ROADMAP.md#known-limitations).

