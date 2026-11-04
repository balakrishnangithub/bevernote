# Architecture

## Design constraints

Bevernote is local-first. The application must create, edit, import, render, and search a library with networking disabled. There is no Bevernote application server and no remote search or database service.

The main boundaries are:

```text
React / TipTap UI
       │ typed Tauri commands
       ▼
Native application adapter (src-tauri)
       │
       ▼
Reusable Rust core (crates/bevernote-core)
       ├── SQLite + FTS5: structured data and offline search
       ├── Blob store: content-addressed attachment bytes
       ├── ENEX/ENML: import, export, editable representation
       └── pCloud adapter: explicit client-to-cloud synchronization
```

The browser adapter in `src/lib/browserApi.ts` exists only to exercise the interface during frontend development. It is not the production persistence layer.

## Native core modules

- `models.rs`: serialized domain types shared across the IPC boundary.
- `db.rs`: schema, SQLite pragmas, settings, device identity, and sync bookkeeping.
- `repository.rs`: transactional note/metadata/attachment operations and FTS maintenance.
- `search.rs`: parsed query operators and parameterized SQL.
- `blob.rs`: SHA-256 and Evernote MD5 calculation, deduplication, and verified reads.
- `enml.rs`: editable HTML conversion, source-preserving export helpers, text extraction, and internal-link rewriting.
- `enex.rs`: note-at-a-time XML reader, resource decoder, importer, and exporter.
- `pcloud.rs`: small official-API client with host allowlisting.
- `sync.rs`: envelopes, revisions, merging, conflict copies, and attachment transfer.

## Persistence invariants

1. A note mutation and its search-index update happen in one SQLite transaction.
2. User search text never becomes SQL; operators become parameterized clauses.
3. Attachments are named by SHA-256. MD5 is retained only for Evernote compatibility, not security.
4. Imported notes retain `original_enml` alongside editable HTML. Export uses original ENML only while the editable representation still corresponds to it.
5. Remote blobs are accepted only when their downloaded bytes hash to the requested SHA-256.
6. Permanently deleted notes leave a sync tombstone so deletion can propagate.
7. Conflicting local edits are copied before a remote version replaces them.

SQLite uses WAL mode, `synchronous=NORMAL`, foreign keys, a busy timeout, and targeted indexes. FTS5 uses Unicode tokenization and BM25 ranking. The current tokenizer is not a specialized CJK trigram tokenizer; adding one would require migration and benchmark coverage.

## Editor compatibility

TipTap stores an HTML-like editable document. Custom nodes preserve `en-media`, classic `en-todo`, and `en-crypt`. Modern Evernote checklist list items are normalized into editable task items. This is preferable to flattening notes into Markdown, which would lose important ENML structure and attachment references.

Attachment bytes are served to the WebView through the restricted `bever:` resource protocol. The protocol resolves only a known resource MD5/SHA-256 and responds with its stored MIME type.

## Performance boundaries

- Note lists are virtualized in React.
- Search executes locally in SQLite rather than scanning note HTML in JavaScript.
- ENEX parsing yields one note at a time, but base64 text and decoded data for the current resource remain in memory.
- Sync lists remote metadata and transfers missing blobs on demand. It is intentionally explicit and synchronous within a native background task.
- A content hash and dirty flag prevent unchanged notes from uploading on every sync.

## Future shells

Linux and macOS can use the same Tauri/React shell. iOS will need platform-specific review of file picking, background execution, OAuth callback handling, and secure storage. These are not claimed as working targets until built and tested.
