# Updates and privacy

SolidCraft tells you when a newer release is out. It never updates itself without asking, and
the check sends nothing about you or your designs.

## What you see

When a newer stable release exists, an **Update** button appears in the title bar next to
**Report a bug**. It opens a small window with the new version, its release notes and three
choices:

- **Download** opens the file that fits your installation (see the table below) in your
  browser, or the release page when there isn't one.
- **Skip this version** hides the button until an even newer release comes out.
- **Remind me later** hides it for three days.

**Preferences ▸ General ▸ Updates** has **Check for updates automatically** (on by default) and
**Check now**, which checks right away and shows the window even for a version you skipped.

## What is sent

One HTTPS request, at start and at most once a day while SolidCraft runs:

```text
GET https://api.github.com/repos/bherbruck/solidcraft/releases/latest
User-Agent: SolidCraft-update-check
Accept: application/vnd.github+json
X-GitHub-Api-Version: 2022-11-28
```

That's all: no version number, no machine or user identifier, no usage data, no cookies. GitHub
sees the request like any other visit to the API (your IP address, the time); see
[GitHub's privacy statement](https://docs.github.com/site-policy/privacy-policies/github-general-privacy-statement).
SolidCraft compares the answer with its own version on your computer.

- Offline, or GitHub doesn't answer within 15 seconds: nothing is shown, and the check is tried
  again an hour later.
- GitHub's rate limit for anonymous requests (60 an hour per address) is respected: when GitHub
  says to wait, SolidCraft waits that long, Check now included.
- Turning off **Check for updates automatically** stops the checks; only **Check now** asks.
- Setting the environment variable `SOLIDCRAFT_UPDATE_CHECK=0` turns all checks off, for
  example on managed machines.

Builds you compile yourself, the Flatpak and the browser app (which updates itself) never check.

## Which file Download gets

Releases attach the main packages under fixed names (see [releasing](https://github.com/bherbruck/solidcraft/blob/main/docs/releasing.md)),
and SolidCraft picks the one matching how it was installed:

| Installed as | Detected by | Download | How to update |
|---|---|---|---|
| AppImage | `APPIMAGE` is set | `solidcraft-linux-<arch>.AppImage` | Replace the old AppImage with the new one |
| Flatpak | `FLATPAK_ID` is set | doesn't check (see below) | `flatpak update ai.storyteller.solidcraft` |
| Snap from the Snap Store | `SNAP` is set | release page | `sudo snap refresh solidcraft` (snapd also refreshes by itself) |
| Snap from a downloaded file | `SNAP_REVISION` starts with `x` | `solidcraft-linux-<arch>.snap` | `sudo snap install --dangerous ./solidcraft-linux-<arch>.snap` |
| `.deb` | under `/usr`, dpkg knows `solidcraft` | `solidcraft-linux-<arch>.deb` | `sudo apt install ./solidcraft-linux-<arch>.deb` |
| `.rpm` | under `/usr`, an rpm database exists | `solidcraft-linux-<arch>.rpm` | `sudo dnf install ./solidcraft-linux-<arch>.rpm` |
| `.tar.gz` | anywhere else on Linux | `solidcraft-<version>-linux-<arch>.tar.gz` | Unpack over the old copy |
| Windows installer | under Program Files | `solidcraft-windows-<arch>.msi` | Run the installer |
| Windows portable | `solidcraft-cli.exe` beside it | `solidcraft-windows-<arch>-portable.zip` | Unzip over the old copy |
| Windows `.exe` | anywhere else on Windows | `SolidCraft-<version>-windows-x64.exe` | Replace the old `.exe` |
| macOS | inside `SolidCraft.app` | `solidcraft-macos-universal.dmg` | Drag the new app to Applications |

The window shows the command for Snap, `.deb` and `.rpm` installations, with a Copy button. The
Flatpak has no network permission and Flatpak itself (or your software center) offers its
updates, so it never checks. Release builds compare versions as [semver](https://semver.org): a nightly build counts
as the release it was built after, so it's offered only a newer stable release, and release
candidates are offered their final release.
