# pCloud synchronization

Bevernote uses pCloud as file storage. It does not deploy compute, a database, or an API service to pCloud.

## Remote layout

```text
/Bevernote/
  metadata.json
  items/
    note-<uuid>.json
  blobs/
    <sha256>
```

Each note envelope contains an ID, monotonically increasing revision, update timestamp, device ID, deletion flag, and an optional serialized note. Attachment bytes are separate content-addressed objects. Metadata envelopes contain stacks, notebooks, tags, and shortcuts.

## Connection

Desktop OAuth uses pCloud's implicit token grant with `http://127.0.0.1:53682/` as its redirect. The native listener is bound before the browser opens, preventing a fast authorization from racing listener startup. A loopback web page converts the URL fragment into a local callback and the native process validates the OAuth state before storing the token. The returned API hostname selects the account's region, with `locationid` as a fallback. Once the token is securely stored, the native app publishes the connected state immediately; an optional profile request is not required to enable synchronization. The pCloud console must enable implicit grant for this flow. Manual token entry is available for environments where loopback OAuth is unsuitable and validates the token with `userinfo` before saving it.

Only `api.pcloud.com` and `eapi.pcloud.com` are accepted as API hosts. Download hosts must be pCloud subdomains. Tokens are stored through the operating system credential vault and are not written to logs or the notes database.

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
