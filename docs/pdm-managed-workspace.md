# Managed native workspace implementation

This draft extends the local service and native client in #3003. It remains a
loopback prototype for disposable designs. Existing stores, running applications,
and previously supplied trial binaries are not migrated or modified.

## Contracts

- The server owns stable document identities, logical paths, authenticated roles,
  exclusive persistent checkout reservations, and immutable published versions.
  User names supplied by clients never establish authority. Reservation secrets
  remain in the native adapter, outside commands, designs, status and logs.
- One normal workspace maps each document identity to one relative file path.
  Its private manifest records server/workspace identity, base version and hash.
  Unchecked files are read-only; only a successful server reservation enables
  managed saves. A separate Save As is an unmanaged copy.
- Local Save does not publish. Check In validates the saved native document,
  reservation, expected base and actual dependency versions; durable publication
  precedes reservation release. A repeated operation returns its original result.
  Lost responses and failed operations preserve local work and recoverable state.
- Refresh Status never replaces local files. Get Latest validates the entire
  requested update and refuses dirty overwrite before installing and reloading.
  Cancel Checkout explicitly restores the archived bytes and preserves a local
  recovery copy before discarding edits. Reservations do not expire silently.
- Working assemblies reference component identity/current workspace copies.
  Local component changes propagate through the CAD update operation without
  requiring publication. Remote publication requires controlled retrieval and
  reload. Each archived assembly records the versions actually consumed, even
  when a component is behind the server head. Open As Archived uses a separate
  exact-version snapshot and never changes normal workspace files.
- Restore as New Version requires an explicit confirmation and a clean local
  copy at the current head. It publishes the selected file's historical bytes
  and dependency references as a new head with the current head as parent,
  preserving earlier versions and the normal workspace path. It does not roll
  back other component files. Receipt reconciliation prevents an acknowledged
  restore from creating another version when retried.

## Incremental slices and validation

The authorized delivery includes three deployment modes, using the same service
protocol, authorization and CAD workflow:

| Mode | Service and metadata | Blob archive | Access | Current state |
| --- | --- | --- | --- | --- |
| Local / LAN | Local host | Local filesystem | Local client; authenticated TLS for LAN | Loopback implementation under verification; LAN pending |
| Local with remote access | Local host | Filesystem or configured object store | Outbound connector and public HTTPS relay | Pending implementation and isolated relay tests |
| Fully hosted | Hosted independently of local workstations | Filesystem or S3-compatible storage; separate optional Azure adapter | Authenticated HTTPS | Pending implementation and isolated hosted-mode tests |

Setup must guide service installation, first administrator creation, archive
selection and backup policy, workspace selection and client login. Remote mode
adds guided connector pairing. User login, connector identity and storage
credentials are separate capabilities and must never be interchanged. Secure
credential storage, TLS and server-side permissions precede enabling any
non-loopback listener. Installation must not silently expose the current service.

Follow-up implementation slices after the verified native local workflow are:
transport/storage boundaries and bootstrap; secure LAN/hosted endpoint and login;
outbound connector plus local relay integration; optional object-store adapters;
guided setup and mode-specific backup/recovery validation. Reuse existing claims
where cohesive and check ownership before creating focused dependent work.
All development uses disposable local test services and credentials. No public
provisioning, paid service or real-user migration is authorized by this plan.
CAD responsiveness remains the priority; scoped builds run at low priority.

1. Server catalog, authenticated reservations, version metadata and retry receipts;
   test competing callers, stale bases, restart recovery and bypass rejection.
2. Workspace manifest, safe path mapping and same-path file transitions; test
   hostile paths, dirty copies, interrupted updates and read-only state.
3. Shared engine commands and asynchronous native adapter; derive dependencies
   from native links and workspace metadata, never manual document/version input.
   Test saved-document validation, managed save guards and reconnect recovery.
4. Native PDM ribbon and dedicated archive workspace: connection, folder browser,
   file status, owner/time, history, relationships and version preview status.
   All actions reach the same command queue; filesystem/network work stays off
   the UI thread. Show unsupported/unavailable preview state honestly.
5. Isolated native trial of creation, checkout, edit/save/check-in at the same
   path, cancellation, competing clients, local/remote component updates and
   historical reconstruction. Preserve existing trial processes and data.

Run focused domain/native/UI checks, strict Clippy, layering/assets/lists, the
Wasm gate and the repository CI entry point. Record any independently reproduced
baseline failure accurately. Keep the PR draft until the user trial is complete.

## Explicit limits

### Storage and remote access contracts

The authoritative PDM service owns identities, authorization, reservations,
version manifests and relationships. Blob storage is a separate replaceable
boundary: put immutable content by digest, read verified bytes, test existence,
and report durable completion. Filesystem storage is implemented in this slice.
An S3-compatible implementation and a separate Azure Blob adapter are future
options, not current capabilities. Metadata publication must never reference
an unavailable blob; provider retries need idempotency and integrity checks.
No provider account or cloud dependency is required by the local deployment.

Remote access to an on-premises service should use an outbound connector to an
HTTPS relay, guided pairing and authenticated user login. Clients should not
require a VPN or incoming router ports. The connector authenticates the service;
it does not grant a remote caller document access. User authentication through
OIDC authorization code with PKCE and optional MFA remains a separate boundary
from server-side authorization. The relay must not bypass checkout fencing or
hold an unrestricted owner capability on behalf of all users.

An object store alone does not make a local service reachable or keep it running
when its host is off. A fully hosted deployment needs the authoritative service
and metadata storage hosted as well. Open S3-compatible storage and self-hosted
relay implementations require license, protocol and security review before any
integration; no compatibility is assumed from their advertised protocol names.
Provider provisioning, public endpoints and real credentials are outside this
increment. These contracts must not delay verification of the local workflow.

No public listener, remote deployment, encrypted transport, external identity
provider, approvals or revision lifecycle is implied by this increment. Backup
must include the complete service store and metadata; version history alone is
not a backup. Any incomplete behavior remains disabled or explicitly reported.

## Verified offline backup and restore foundation

Stop the local service before using these commands. The backup command acquires
the same exclusive store lock and refuses a running service. Both output paths
must be new directories whose parents already exist:

```sh
solidcraft-pdm backup /private/toy-store /private/toy-backup
solidcraft-pdm verify-backup /private/toy-backup
solidcraft-pdm restore /private/toy-backup /private/restored-toy-store
```

The snapshot includes legacy content, managed version metadata and blobs, catalog,
server identity, checkout reservations, retry receipts and configured principal
hashes. It excludes process locks and unpublished staging data. The backup copies
only recognized store files, validates the copied domain and dependency references,
and writes its completion manifest last. Verification checks every listed file's
size and SHA-256 and rejects links, extra files and bounded-input violations.
Limits are 10,000 files, 8 MiB per file and 256 MiB total; metadata is also subject
to the service's smaller limits. Output JSON is an integrity receipt, not a design
export, credential dump or cryptographic attestation.

Restore verifies the manifest before creating its destination, holds the writer
lock while copying, and validates domain metadata and referenced content before
removing `restore.pending`. An interrupted or invalid restore remains marked and
cannot start as a service; preserve it for diagnosis and retry into another new
directory. Existing destinations are never replaced. Test the restored service
in isolation: it retains the same identity and reservations, so the original and
restored copies must not operate simultaneously as authoritative servers.

Backups contain private design data and checkout capabilities. Unix output roots
use mode 0700; Windows inherits its destination-parent ACL. Choose a private parent
directory. The owner bearer credential comes from the service environment and is
not included; preserve it separately or explicitly configure replacement ownership.
This foundation does not encrypt/sign backups, schedule retention, compact metadata,
provide remote/object storage, or establish recovery objectives. File sync and
interrupted-process tests do not establish power-loss durability on every filesystem.
