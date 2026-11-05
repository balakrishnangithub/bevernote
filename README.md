# Bevernote

Bevernote is a free, open-source, local-first note application for people who want an Evernote-compatible library without an application server. Notes, attachments, metadata, and the full-text search index live on your device. Optional pCloud synchronization is manual and uses your own storage.

## What is implemented

- Notes, notebooks, notebook stacks, nested-tag storage, shortcuts, trash, restore, and permanent deletion.
- Fast offline title, body, tag, notebook, stack, attachment/OCR, date, and checklist search using SQLite FTS5 with BM25 relevance.
- Rich TipTap editing: normal text and H1–H4, font family/size/color, bold, italic, underline, strikethrough, highlight, lists, checklists, alignment, indentation, code blocks, subscript/superscript, tables, links, internal note links, and attachments.
- Debounced local autosave and editable note metadata, including explicit created/updated timestamp correction.
- Note-at-a-time ENEX import with early notebook creation, byte-based progress, and distinct reading, processing, and finalization phases. Original ENML, metadata, attachments, alternate resource data, recognition/OCR text, and Evernote GUIDs are retained. Unedited imported notes retain their original ENML for export.
- ENEX export and fingerprint-based duplicate prevention on reimport, including notes without GUIDs; existing local edits are never overwritten by an exact reimport.
- Single- and multi-note move, copy, and additive tagging with searchable existing-tag suggestions and new-tag entry. Copies receive new note/resource identities and current timestamps while sharing deduplicated attachment blobs. Multi-select tag filters require every selected tag to match.
- Optional on-device OCR for PNG, JPEG, and WebP images during import, with existing Evernote recognition retained and recognition failures kept nonfatal. Bundled Ocrs models primarily support Latin-alphabet text.
- Content-addressed, SHA-256-verified attachment storage with deduplication. Evernote MD5 references are also retained.
- Local PDF preview with pagination, plus Open with system application and Save As using the original filename for every attachment.
- Independent touchpad/touch scrolling for all panes, collapsible stacks, viewport-aware curated color palettes with custom colors, and contextual keyboard shortcuts including note-content-only find.
- Direct, optional pCloud upload/download with revision tracking, deletion tombstones, missing-attachment transfer, and local conflict copies.
- Responsive desktop/mobile interface, light/dark/system appearance, and virtualized note lists.
- No account, telemetry, advertising, AI service, or server-side application compute.

## Technology

- **Shell:** Tauri 2
- **Native core:** Rust, SQLite, `rusqlite`, `quick-xml`
- **UI:** React, TypeScript, TipTap, Zustand
- **Cloud adapter:** pCloud's official HTTPS API; access tokens use the operating system credential vault

The Rust core does not depend on React or Tauri and can be reused by future Linux, macOS, and iOS shells.

## Run locally

Prerequisites:

- Node.js 22 or newer and npm
- Stable Rust with Cargo
- Tauri's [platform prerequisites](https://v2.tauri.app/start/prerequisites/)
- Windows: Microsoft C++ Build Tools and WebView2

```sh
npm ci
npm run tauri -- dev
```

The ordinary web command, `npm run dev`, opens a **non-persistent browser demonstration** with sample notes. Native file access, ENEX, attachments, credential storage, and pCloud require the Tauri application.

Useful commands:

```sh
npm test                 # TipTap parsing and serialization tests
npm run test:core        # Rust tests
npm run lint             # TypeScript and Rust lint
npm run format:check     # Formatting checks
npm run build            # Production frontend
npm run tauri -- build   # Native desktop release and installers
```

### Android

Install JDK 17+, Android SDK, an NDK, and the Rust Android targets. Set `JAVA_HOME`, `ANDROID_HOME`, and `ANDROID_NDK_HOME` for your installation, then run:

```sh
rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
npm run android:init
npm run android:build
```

`android:init` is only needed once. A generated debug APK is for testing, not a signed store release. Device interaction must be verified separately from compilation.

On Windows without symbolic-link privileges, the Tauri CLI can compile the Rust library and then stop before Gradle packaging. Do not enable elevated privileges just for packaging: after the native library has been built, copy it into the generated JNI directory and package that ABI directly. From the repository root (use the equivalent paths in PowerShell):

```sh
mkdir -p src-tauri/gen/android/app/src/main/jniLibs/arm64-v8a
cp target/aarch64-linux-android/release/libbevernote_lib.so \
  src-tauri/gen/android/app/src/main/jniLibs/arm64-v8a/libbevernote_lib.so
cd src-tauri/gen/android
./gradlew.bat :app:assembleArm64Debug -x :app:rustBuildArm64Debug
```

The resulting APK is `app/build/outputs/apk/arm64/debug/app-arm64-debug.apk`. The exclusion skips only the already-completed native build/symlink step. It is not a substitute for rebuilding after Rust or frontend changes; run the normal Android build again first so the native library embeds the current frontend.

## ENEX compatibility and search

Import an `.enex` file from the sidebar. Each import creates or reuses a notebook named after the file. The importer reads one note at a time; memory is bounded by the current note and its largest decoded attachment rather than the entire export. Attachment data is streamed into neither memory nor disk in tiny chunks yet, so unusually large single attachments still require corresponding free memory.

Search examples:

```text
project roadmap
"exact phrase"
intitle:meeting
tag:receipt
notebook:Personal
created:2026-01-01..2026-12-31
resource:image
contains:checklist
-todo:false any:alpha beta
```

Supported operators include `intitle:`, `tag:`, `notebook:`, `stack:`, `created:`, `updated:`, `resource:`, `contains:`, `source:`, `todo:`, negation, `any:`, quoted phrases, and prefix terms. Search runs entirely against the local SQLite index.

Encrypted Evernote `en-crypt` blocks are preserved and displayed as encrypted blocks, but Bevernote does **not** decrypt them. OCR already embedded in ENEX is searchable. Optional on-device OCR can also index PNG, JPEG, and WebP images during import; it uses bundled Ocrs/RTen models, performs no network requests, and is best suited to Latin-alphabet text. Disable it under **Settings & sync → Offline image OCR**. See [ENEX compatibility](docs/enex.md) for details.

## Optional pCloud synchronization

1. Create or open a pCloud OAuth application. The bundled public client identifier is for the application details supplied during development; no client secret is included or needed for implicit token flow.
2. Configure `http://127.0.0.1:53682/` for Windows/desktop, then **enable implicit grant (token flow)** and save the pCloud application settings. Do not add the Android `bevernote://oauth/pcloud` callback to the console; the native deep link is handled automatically by the working Android flow and never connects to a desktop loopback server.
3. Open **Settings & sync → Connect with browser**, or use the manual access-token option with the correct Global/Europe API region.
4. Choose **Sync now**. Bevernote creates `/Bevernote/items`, `/Bevernote/blobs`, and `/Bevernote/metadata.json` in your pCloud account. The current operation, elapsed/network time, request count, and transferred bytes are visible while sync runs.

Tokens are kept in the OS credential vault, not the notes database; Android uses Keystore-backed encrypted preferences. Bevernote communicates only with approved pCloud HTTPS hosts. The remote copy is **not end-to-end encrypted**; pCloud can access the uploaded content. Sync is explicitly initiated, never scheduled. One native worker runs app-wide; its progress remains visible outside Settings. Closing the Windows window during sync hides Bevernote to the tray until the worker finishes, and Android uses a data-sync foreground service with notification progress when permission allows. Background completion is best-effort: force-stop, reboot, OS process termination, Android 14+ data-sync time limits, and OEM battery policies can interrupt it.

A remote revision index lets subsequent syncs skip unchanged-note downloads. The first sync after upgrading from 0.1.2 rebuilds this index and still downloads the remote note envelopes once.

Concurrent edits produce a local conflict copy rather than silently discarding the local version. Metadata merging is currently additive for shortcuts and last-updated-wins for notebooks, stacks, and tags; metadata deletion propagation and unattended/background synchronization are not yet implemented. Do not use sync as your only backup.

## Data and portability

The native app stores its SQLite database and attachment blob store in the operating system's application-data directory for `app.bevernote.desktop`. Keep ENEX exports or a separate backup of that directory. Content hashes are verified during attachment import and download.

## Verification

The automated suite covers ENEX parsing and unchanged round trips, duplicate GUIDs, repeated tags within a note, attribute values, checklist conversion, internal links, staged import progress, OCR indexing, blob deduplication, copied-note identity and attachment reuse, bulk move/tag operations, timestamp validation, offline search, sync merge helpers, and editor serialization. Browser checks cover bulk organization, multi-tag filtering, timestamp editing, disconnected sync-action visibility, independent pane scrolling, attachment actions, and mobile navigation. Three private real-world ENEX samples were also tested locally without being added to source control:

- 2 notes / 2 attachments: import and export/reimport verification passed.
- 3 notes / 7 attachments, approximately 251 MB of ENEX: import and export/reimport verification passed, including original ENML, metadata, and attachment hashes.
- 705 notes / 1,606 attachments, approximately 922 MiB of ENEX: ordered streaming progress and import completed without warnings in a bounded check with OCR disabled.

See `test-results/` for local-only test artifacts. That directory and `test-data/` are ignored by Git.

## Project documentation

- [Architecture](docs/architecture.md)
- [ENEX and ENML compatibility](docs/enex.md)
- [Synchronization format and limitations](docs/sync.md)
- [Contributing](CONTRIBUTING.md)
- [Security policy](SECURITY.md)

## License

GPL-3.0-or-later. Evernote and pCloud are trademarks of their respective owners; Bevernote is not affiliated with either service.
