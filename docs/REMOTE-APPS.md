# Remote JavaScript apps

Stock Tela can run one prebuilt JavaScript bundle from an HTTPS manifest:

```sh
tela run --url https://example.invalid/apps/demo/manifest.json
```

The URL is illustrative, not a hosted app. Local apps remain supported with
`tela run <directory>`. A local path and `--url` are mutually exclusive; one is
required. Local `run`, `dev`, and `init` retain their existing behavior.

## Manifest contract

Remote manifests are strict UTF-8 JSON objects with exactly these fields:

```json
{
  "name": "demo",
  "version": "1.0.0",
  "entry": "dist/bundle.js",
  "sha256": "<64 hexadecimal characters: SHA-256 of the exact bundle bytes>",
  "permissions": ["network", "storage"]
}
```

Replace the placeholder with a real digest before use. All fields are required.
`name` and `version` are informational strings, **not** application identities or
trust decisions. Unknown fields, invalid types, missing digests, and unsupported
permissions are rejected. Digests may use upper- or lowercase hexadecimal.

`entry` must be a canonical relative `.js` path. Each nonempty segment may contain
only ASCII letters, digits, `.`, `_`, and `-`, and must not be `.` or `..`.
Absolute paths, schemes, percent escapes, backslashes, query strings, fragments,
and empty segments are forbidden. Entries resolve relative to the **submitted
canonical manifest URL**, even if that request follows same-origin redirects.

Build the bundle before hosting it. The remote loader does not install npm
packages, invoke build commands, load native libraries, or download dependencies.
It passes the verified UTF-8 bundle directly to Tela's existing engine.

### Permissions

Only these typed permission names are accepted:

| Permission | Engine capability requested |
| --- | --- |
| `network` | Fetch and WebSocket APIs; not restricted to the manifest origin |
| `storage` | Persistent app storage in the source-derived namespace |
| `crypto` | Stock engine cryptographic APIs |
| `credentials` | Stock engine private app credentials in the source-derived namespace |
| `clipboard` | Clipboard access |

An empty array requests none of these capabilities. Ordering and duplicates do
not affect approval. Unknown names, `env`, `filesystem`, and filesystem scope
strings are rejected rather than granting ambient environment or file access.
The loader passes only validated permissions to `Engine::new_with_manifest` and
does not register embedding-specific native modules. The installed stock engine
determines the implementation of each API; a manifest does not install APIs.

## Download and consent policy

- Manifest and bundle URLs must use HTTPS, without userinfo or fragments.
  Normal certificate/hostname validation is always enabled; there is no insecure
  CLI switch.
- Bundle requests and all redirects must stay on the submitted manifest's
  origin (scheme, host, effective port). Up to five redirects per resource are
  allowed. A cross-origin redirect is rejected before its target is requested.
- Manifests are limited to 64 KiB and bundles to 4 MiB. Both must be valid UTF-8.
  Limits apply while streaming, including when Content-Length is absent or wrong.
- Each resource download has a 30-second total deadline including redirects and
  body streaming, and a 10-second connection timeout.
- The bundle's SHA-256 must match the manifest before any approval or execution.
  Cached bytes are checked too. A mismatch fails closed; it is not repaired
  silently from the network.
- On first use, or after a change to the digest or permission set, Tela displays
  the escaped canonical source URL, full digest, and requested permissions.
  All three standard streams must be genuine terminals to grant approval.
  Enter exactly `y` or `yes`, followed by Enter. Denial, EOF (even after an
  unterminated `yes`), redirected streams, or noninteractive approval fails
  closed. There is no auto-approve flag or environment variable.
- An unchanged source/digest/permission grant can be reused without prompting.
  This does not bypass the engine's own terminal requirements. The manifest is
  fetched every run: this is not an offline launcher. Cached identical bundle
  bytes avoid another bundle download; missing bytes are fetched and reverified.
- Version and name changes never suppress required code/permission reapproval.
  Removing a permission also changes the approved set. A denied update leaves
  the previous grant intact, but cannot execute the denied bundle.

**This is HTTPS transport plus user-approved content hashes, not a signature or
verified-publisher system.** A digest supplied by the same server does not prove
who wrote the code. Read the permissions and trust the source before consenting.
Network permission allows the app to contact other origins; the loader's
same-origin rule constrains downloads, not application network activity. A
remote app can display arbitrary terminal UI once approved. Tela's ordinary
engine capabilities and resource limits still apply; this loader is not a new
process sandbox or a publisher identity system.

## Source identity and private cache

The engine namespace is `remote-` followed by the lowercase SHA-256 of the
canonical submitted manifest URL. URL canonicalization includes hostname casing
and default-port normalization. The full canonical URL, including any query,
is the identity; changing its path or query produces a different namespace.
Redirects do not rebind identity. Never put secrets in manifest URLs: the source
is displayed for consent and stored in its local grant.

The declared name cannot select another application's storage or credentials.
Updates at the same source retain their namespace, while approval binds the
source, bundle digest, and normalized permission set. Different sources need
separate approval even when they share identical bytes.

The cache lives in `tela-remote-apps` inside the platform user cache directory.
The loader creates a private owner-only directory and 0600 files. It uses
Unix descriptor-relative operations, rejects symlinks anywhere in the cache
path, rejects non-owned or group/other-accessible cache roots/files, and rejects
nonregular or hard-linked files. Cache directories configured through symlinks
must be replaced with real directories before using this loader. File writes
use exclusive temporary files and atomic replacement, followed by synchronization.
Bundle keys are content hashes; grant keys are source hashes. No manifest field
is used as a filesystem path in the cache.

Grants are saved only after explicit approval and successful validation. Deleting
the cache revokes saved approvals without intentionally removing engine app
storage/credentials. An invalid grant prompts again; unsafe paths or tampered
bundle bytes fail closed. These protections do not defend against another
process already running as the same OS user and able to edit that user's files.

## Verification scope

CLI tests inject download fixtures, temporary private cache roots, and approval
providers; they never use real endpoints or user cache/storage. Coverage includes
argument exclusivity, strict manifest parsing, wrong hashes, UTF-8 and byte
limits, path/origin policy, terminal/EOF denial, code/permission reapproval,
source identity, cache tampering/permissions/symlinks/hardlinks, atomic writes,
and loading approved fixture JavaScript into the stock engine. Loopback transport
tests reject plaintext on an HTTPS connection and verify a stalled handshake
hits the download deadline. Redirect and streaming-limit policy are unit-tested;
a successful local TLS download/redirect chain and interactive PTY launch are
not covered by this initial suite.

Run from the Tela workspace with the project's configured SDK/toolchain:

```sh
cargo test --offline --locked -p tela-cli
```
