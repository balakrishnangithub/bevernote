# pCloud synchronization

Bevernote uses pCloud as file storage. It does not deploy compute, a database, or an API service to pCloud.

## Remote layout

```text
/Bevernote/
  metadata.json
  items/
    index.json
    note-<uuid>.json
  blobs/
    <sha256>
```

Each note envelope contains an ID, monotonically increasing revision, update timestamp, device ID, deletion flag, and an optional serialized note. Attachment bytes are separate content-addressed objects. Metadata envelopes contain stacks, notebooks, tags, and shortcuts. The revision index stores each note's revision and byte size; it is an optimization, not the source of note content.

## Connection

Desktop OAuth uses pCloud's implicit token grant with `http://127.0.0.1:53682/` as its redirect. The native listener is bound before the browser opens, preventing a fast authorization from racing listener startup. A loopback web page converts the URL fragment into a local callback and the native process validates the OAuth state before storing the token. The returned API hostname selects the account's region, with `locationid` as a fallback. Once the token is securely stored, the native app publishes the connected state immediately; an optional profile request is not required to enable synchronization. The pCloud console must enable implicit grant for this flow. Manual token entry is available for environments where loopback OAuth is unsuitable and validates the token with `userinfo` before saving it.

Android uses the native `bevernote://oauth/pcloud` deep link instead of a loopback server. This URI must be added to the pCloud application's Redirect URIs alongside the desktop URI, with implicit grant enabled. Warm and cold callbacks are handled natively, query and fragment parameters are decoded, and the destination and OAuth state are validated. Already-consumed callbacks are ignored. Android access tokens use Keystore-backed encrypted SharedPreferences; callback URLs and tokens are never logged or emitted to the frontend.

Only `api.pcloud.com` and `eapi.pcloud.com` are accepted as API hosts. Download hosts must be pCloud subdomains. Tokens are stored through the operating system credential vault and are not written to logs or the notes database.

## Efficient incremental transfers

- The first sync without a remote revision index downloads remote note envelopes once and builds the index. Later syncs skip a download only when the indexed revision exactly matches the locally observed revision and its byte size matches the current remote file.
- Sync-state hashes are loaded in one local query rather than one query per note.
- The remote attachment directory is listed at most once per upload pass, and missing blobs are uploaded before their note envelopes.
- Attachments for remote notes are downloaded only when that remote note will actually be applied.
- The native app reports the current operation, elapsed time, successful network time, request count, and uploaded/downloaded bytes. These measurements describe activity; they are not a benchmark or a guarantee of a particular speedup.

## Change handling

- Dirty notes and notes whose local content hash differs from their last-synced hash are uploaded with a new revision.
- Remote revisions newer than the locally observed revision are downloaded.
- If both sides changed, Bevernote creates a local conflict copy before applying the remote note.
- Permanent deletion uploads a tombstone envelope.
- Missing attachment blobs are transferred and verified by SHA-256.
- Notebook, stack, and tag updates merge by last update time. Shortcuts merge additively.

## Current limitations

This is an early synchronization format, not a complete CRDT or an end-to-end encrypted vault:

- Sync runs only when requested; there is no background scheduler or push notification.
- Metadata deletion/reordering does not fully propagate, and shortcut removal is not propagated.
- Two devices should not synchronize the same library concurrently; operations are not a multi-device transaction.
- Remote JSON and blobs are not application-level encrypted. HTTPS protects transport, not access by the storage provider.
- Sync is not a substitute for independent backups or ENEX exports.

A live account test requires the user's explicit pCloud authentication. Unit tests cover merge helpers and host validation; a successful compilation alone is not evidence of live cloud synchronization.
