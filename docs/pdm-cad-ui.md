# Experimental local PDM in the CAD desktop

Follow-up #3002 / #3003, stacked on the authenticated file workspace #2996 and
original service/RFC #2870. This is a bounded local prototype for public-safe toy
designs, not a production PDM system. It adds a desktop connection/version panel
through engine commands. It does not add remote transport or an automatic CAD link
resolver.

## Connect and browse

Build the desktop application and `solidcraft-pdm`. Follow the private owner-token
bootstrap in [Local file workspace](pdm-local-workspace.md), start the service on
loopback, then start this desktop build from the **same credential-bearing launch
environment**. No external account is needed. The panel has no token input, and
ordinary preferences and designs never store the credential.

For an isolated desktop test profile, set `SOLIDCRAFT_SETTINGS_DIR` to an absolute
private directory before launch. It moves settings, startup markers, graphics
preferences and this panel's working-copy root together, and skips legacy-settings
adoption. An empty/relative override fails before normal-profile startup. Also set
`SOLIDCRAFT_RECOVERY_DIR`, `SOLIDCRAFT_CRASH_DIR` and `SOLIDCRAFT_CONTROL_DIR` to
separate private test folders. On Windows, changing `APPDATA` alone does not move
eframe's settings, which otherwise use the OS roaming-folder API.

Open **File → Local PDM** or choose **Local PDM** in the command palette. Enter the
local port and select **Connect Local PDM**. Only `127.0.0.1` is supported; there is
no arbitrary URL/host field. The connection is authenticated by listing versions.
Select **Refresh PDM Versions** to refresh the list; selecting a row shows the exact
dependency references declared for that version. The list includes history, with
document identity and version number kept separate.

A failed authentication/network connection clears the connected state. To rotate
the credential, stop/restart the service with a new token and restart the desktop
with the updated launch environment; the old capability is not retained in a
settings file. Reconnect and refresh after an uncertain network result before
retrying a write. A failed acknowledgement does not prove the server rejected a
check-in. Version conflicts remain explicit and do not silently advance the base.

## Check in the saved active design

1. Finish or cancel interactive tools and save the active native design (`.solidcraft`
   or supported native JSON). Unsaved/dirty designs, active sketch edits, pending
   occurrence moves and interactive feature edits cannot be checked in.
2. In the PDM panel choose a lowercase document ID. Leave **Expected parent** empty
   to create that document, or select an existing version as the explicit base.
3. Optionally mark existing version rows **Use as dependency**. Selecting a version
   also loads its declared dependency list. These are version references, not
   automatically discovered links from the CAD document.
4. Select **Check In Saved Design**. The native worker verifies that the saved file
   still represents the captured active design and saved visibility; an externally
   changed file is refused. The same verified bytes are uploaded, with no second
   path read that could upload a different file after validation.

The server checks the expected parent atomically as before. Success records a new
immutable version and updates the panel's base to that version. A stale base returns
a conflict; refresh and decide explicitly how to proceed. The active design, local
file and undo history are not rewritten by check-in. PDM operations are not CAD undo
steps. The existing 1 MiB file and bounded prototype graph/store limits still apply.

## Retrieve and open a new working copy

Select an exact version, then **Retrieve and open NEW working copy**. The native
worker retrieves and verifies its pinned closure into a new random directory under
the application's private `pdm-workspaces` folder, preserving the snapshot/manifest
layout from #2996. It creates a separate `working.solidcraft` using create-new
semantics. It never writes over the source design or an existing destination.

The copy is parsed/evaluated in a **separate Session** off the UI thread. Only a
successful load is offered to the document manager, and it always opens a **new tab**.
The current tab remains available with its dirty state and undo history intact.
At the tab limit the copy remains on disk and the active design is not replaced.
Other file operations and active modelling dialogs/tools finish before a completed
retrieval is opened; opening the PDM panel does not cancel those interactions. If loading or
writing fails, no active design is replaced; a downloaded snapshot or partial new
working file may remain in the private cache for inspection. No partial result is
opened as a successful CAD design.

The panel shows the retrieved identity/version and working-copy filename; hover
reveals its local path. Selecting an older version never substitutes the latest.
**Declared PDM dependencies are metadata in this increment.** Saved CAD link paths
are neither rewritten nor resolved through the service. Existing cached assembly
geometry can display without its original source, but that is not proof of PDM
link resolution.

## Commands and host boundary

`pdm.panel` and `pdm.status` return host-only state programmatically; only interactive
start opens the panel. `pdm.connect`, `pdm.disconnect`, `pdm.refresh`, `pdm.check_in`
and `pdm.retrieve` enqueue one host operation and return its request ID. Poll
`pdm.status` for completion. `pdm.retrieve` defaults to opening a new tab; `open:false`
creates the separate local copy without opening it. Commands never accept a token
parameter. Unsupported/credential-bearing parameter fields are refused before the
engine's command journal hook, with values omitted from errors.

The engine holds only a host-only queue/status and metadata. It has no socket,
credential or GUI dependency, and scratch evaluation Sessions do not inherit the
host capability. The UI reads that state and acts through commands. The desktop
adapter owns credentials, authenticated clients, filesystem work and background
threads. A host without the adapter, including the current web application, reports
PDM unavailable. No network/OS dependency is added to the engine or layered libraries.

Only one operation runs at a time; additional operations and Disconnect are refused
while it is busy. Closing the panel does not cancel a check-in. The UI remains
responsive while network/disk/evaluation work runs. Socket budgets remain separate
per phase, not a whole-operation/disk deadline. The feature does not implement
transaction receipts, cancellation of an in-flight remote write or general DoS
resistance.

## Security and remaining work

The single owner token grants access to the whole local store. The process
environment is visible to the same OS user/admin, local transport is plaintext,
working files inherit the selected private parent permissions, and host compromise
is outside this prototype's protection. No credential is intentionally shown in
the GUI, status, design, ordinary preferences or normal errors. Keep the host and
private application data free from hostile local mutation.

This increment adds no identity-provider account, roles, checkout leases, release
approval, audit, hardened ACL management, power-loss guarantee, tested backup/restore,
TLS, pairing, relay or cloud deployment. The intended local/cloud domain contract
remains the RFC's; easy remote access without user router setup still requires the
future authenticated encrypted outbound transport and permitted relay infrastructure.
No network-policy bypass is proposed. Production use and real-design migration are
not claimed.
