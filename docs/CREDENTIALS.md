# Credentials native module

The host may install the `credentials` native module for an app using a **host-controlled** app identity (the remote loader should derive it from the source URL hash, never `manifest.name`). It is permission-gated as `credentials` and registers:

- `Tela.credentials.prepare()`
- `Tela.credentials.list()` → sorted array of record names only
- `Tela.credentials.load(name)`
- `Tela.credentials.saveNew(name, value)`
- `Tela.credentials.remove(name)`

All methods return Promises through `Tela.native.call`. At most four blocking file operations run across contexts; cancellation does not release admission until the actual file operation ends. Credential names are ASCII identifiers (letters/digits/`_`/`-`, 1–64 bytes); host-controlled app identities allow up to 128 bytes to accommodate source hashes. Invalid input rejects. Values are opaque JSON to the provider and are limited to 16 KiB.

State is unencrypted owner-only local storage, **not** a keychain and not non-extractable secret storage. On supported Unix platforms it lives below the local data directory at `tela-credentials/<app-id>` (separate from legacy general-purpose storage). Provider directories and credential files must be private and owned by the current user; symlinks, unsafe directories, foreign ownership, malformed data, and oversized data reject rather than being ignored.

## Bounded enumeration

`list()` takes no path, prefix, cursor, or app identity. It enumerates only this
app's existing namespace (preparing private directories if absent). It returns
names, never filesystem paths or JSON values. Names remain limited to 64 ASCII
bytes; `-` cannot be the first character. An empty store returns `[]`.

Enumeration is descriptor-relative. Each provider-owned directory component is
opened with `O_DIRECTORY | O_NOFOLLOW | O_NONBLOCK` and checked using its actual
metadata. Each entry is opened relative to that directory with
`O_NOFOLLOW | O_NONBLOCK`, then checked to be a current-user-owned, private,
regular file of at most 16 KiB. Symlinks, FIFOs/devices, subdirectories, foreign or
public files, malformed/non-UTF-8 names and oversized files reject the entire
listing. Errors contain no entry names, paths or contents. No entries are silently
repaired, deleted, followed, or returned as partial success.

The generic provider scans **at most 128 entries**, excluding `.` and `..`; the
129th rejects. Exact internal `.tmp-` plus 32 lowercase hex names are checked as
above and counted toward the bound, but omitted from the result. They can be
in-flight writes or crash leftovers; listing never cleans them up. Other hidden
names reject. File contents are **not read or validated by list**: malformed JSON
is rejected by `load`, and application schemas remain the app's responsibility.
The app must load/validate records before deriving counts or enabling actions.

This is bounded enumeration, not a transactional snapshot or an atomic capacity
reservation. Concurrent removal/replacement can reject listing; concurrent
creation may appear on a later call. `saveNew` retains its existing generic
semantics; it does not impose an app's profile cap or maintain an index. Apps must
preflight their own limits/duplicates before remote mutations and handle later
races as uncertain, not retry authorization. Storage identity is unchanged.

## Writes and removal

`saveNew` never replaces an existing credential. It writes a private temporary file, syncs it, commits with a no-replace hard link, and syncs the directory. A failure after commit but before directory sync reports that durability is uncertain. Engine shutdown can also suppress the result of already-started blocking filesystem work; callers must not automatically retry a rejected `saveNew`, because it may have committed.
