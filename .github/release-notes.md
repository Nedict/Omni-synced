SolidCraft is open-source parametric 3D CAD in Rust: sketch, constrain, extrude, and change a
parameter to watch the timeline rebuild. [Documentation](https://bherbruck.github.io/solidcraft/)
· [try it in the browser](https://bherbruck.github.io/solidcraft/app/) ·
[contributing](https://github.com/bherbruck/solidcraft/blob/main/CONTRIBUTING.md) ·
[issues](https://github.com/bherbruck/solidcraft/issues). AI agents can model with it over MCP:
`solidcraft-cli mcp` ([how](https://bherbruck.github.io/solidcraft/guide/mcp.html)).

**Why 0.6:** this is a minor bump because two things change the files you work with: the
rename to SolidCraft and the new design file format. Files from earlier builds open fine, but
builds 0.5 and older can't open designs saved by 0.6.

**What's new**
- **New design file format.** Saves are a zip container by default; plain JSON is still one
  click away (details below).
- **Sheet metal:** flange corners close at any angle, and cut-outs fold across bends at any
  angle, including curved bends.
- **Crash reports:** after a crash, Help ▸ Crash Reports… shows what happened with a ready-to-file
  issue draft. Reports stay on your computer; nothing is sent. `solidcraft-cli crashes` lists and
  replays them.
- **Snap package** for x86_64 and aarch64 on every release, alongside the AppImage, `.deb`, `.rpm`
  and Flatpak.
- **Trackpad navigation** (two-finger pan, pinch zoom), a fillet option that shrinks a radius too
  big for its edges, cleaner dialogs and toolbar panels, and the unfinished Generative Design
  workspace is gone.
- Many modelling fixes: extrude/revolve in moved components, To Object picks, very flat arcs,
  touching lines and arcs in booleans, and `solidcraft-cli run` now fails when a feature fails.

**New file format.** Save and Save As now write `.solidcraft` designs as a zip container
that stores imported STEP, mesh and image data as binary, once each, at full precision, so
designs with imports save and open faster. What that means for you:
- Your existing `.solidcraft` and `.solvecraft` files (plain JSON) still open as before; the
  format is detected from the contents.
- Want plain JSON (for diffs or scripts)? Use **File ▸ Save As JSON…**, or pick a
  `.json` name in Save As; later saves keep JSON. **File ▸ Export Design as JSON…** writes a
  separate JSON copy. From the CLI, `solidcraft-cli run … --save design.json`; from MCP or
  scripts, pass `"format": "json"` to `save` / `file.save`.
- SolidCraft 0.5 and older can't open designs saved by 0.6, as a container or as JSON (the
  JSON format tag is now `solidcraft/14`). Keep a copy of a file you may still open in 0.5.

**Renamed to SolidCraft.** Everything now uses the new name: the `solidcraft` and
`solidcraft-cli` programs, `.solidcraft` designs, `SOLIDCRAFT_*` environment variables,
settings folders, package names and the `solidcraft` MCP server. Upgrading from SolveCraft:
- `.solvecraft` designs still open (double-click, Open, drag-drop, the CLI and MCP); new saves
  are `.solidcraft`.
- Settings, preferences and recovery files are copied to the new folders on first launch; the
  old folders are left in place. `SOLVECRAFT_*` environment variables are still read.
- `solvecraft-cli` remains as a deprecated alias for one release; it forwards to
  `solidcraft-cli` and prints a note. Agents: `claude mcp add solidcraft -- solidcraft-cli mcp`;
  an existing `claude mcp add solvecraft -- solvecraft-cli mcp` setup keeps working.
- Windows: the `.msi` upgrades the existing installation in place (now in
  `Program Files\SolidCraft`). Linux: the `solidcraft` `.deb`/`.rpm` replaces the `solvecraft`
  package. The Flatpak app ID stays `ai.storyteller.solvecraft`, so `flatpak update` just works,
  and existing AppImages find the update.
- macOS: quit the app, move the old `SolveCraft.app` to the Trash (your designs and settings
  stay), then drag `SolidCraft.app` from the DMG onto Applications and pin it in the Dock.
- Details: [compatibility notes](https://github.com/bherbruck/solidcraft/blob/main/docs/compatibility.md).

Documentation, the browser app and the Flatpak feed use the SolidCraft project Pages URLs;
old-name Pages addresses are not permanent aliases.

**Which file?**
- **Windows:** `SolidCraft-{version}-windows-x64.exe` runs as is. Or install the `.msi`, or unzip the
  `-portable.zip` (x64, x86 and arm64).
- **macOS 11+** (Apple silicon and Intel): open the `.dmg` and drag `SolidCraft.app` onto Applications.
  The CLI is in `solidcraft-cli-{version}-macos-universal.zip`.
- **Linux** (x86_64 and aarch64, glibc 2.35+): the `.AppImage` (`chmod +x`, then run it), the
  `.deb` or `.rpm`, the `.flatpak` (`flatpak install --user <file>`), the `.snap`
  (`sudo snap install --dangerous <file>`, or `sudo snap install solidcraft` from the Snap Store), or the `.tar.gz`.
- **FreeBSD:** `tar -xzf solidcraft-{version}-freebsd-x86_64.tar.gz --strip-components 1 -C /usr/local`.
- **Web:** `solidcraft-web-{version}.zip` is a static site to host yourself.
- `SHA256SUMS.txt`: `sha256sum -c SHA256SUMS.txt --ignore-missing`.
- Files without a version in the name (`solidcraft-windows-x64.msi`, `solidcraft-linux-x86_64.AppImage`, …)
  are the same packages under fixed names, for download links that always fetch the latest release.

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

