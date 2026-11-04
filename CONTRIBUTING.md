# Contributing to Bevernote

Thank you for helping build a portable, local-first notes application.

## Development principles

- Offline behavior must not depend on a network request.
- Do not silently discard note content, ENML, metadata, or attachments.
- Keep the Rust core independent of the UI shell.
- Prefer small, typed interfaces, transactional storage changes, parameterized queries, and explicit error handling.
- Optimize using measurements; preserve correctness before micro-optimizations.
- Never commit access tokens, client secrets, private ENEX exports, personal notes, or local application databases.

## Setup and checks

```sh
npm ci
npm run format
npm test
npm run test:core
npm run lint
npm run build
```

Run `npm run tauri -- build` for desktop changes. Android changes also require an APK build and, when available, on-device interaction checks. A successful type check is not a substitute for testing the affected behavior.

Add or update tests for bug fixes. ENEX tests should include source-preserving round trips and attachment hashes. Search changes need operator and relevance coverage. Sync changes need conflict, deletion, retry, and partial-failure cases.

## Project layout

- `src/`: React interface and editor tests.
- `src-tauri/`: native IPC, OAuth, secure credential storage, and resource protocol.
- `crates/bevernote-core/`: reusable storage, ENEX, search, and sync engine.
- `docs/`: architecture and compatibility contracts.
- `tests/`: frontend/browser verification support.

Use Prettier and rustfmt rather than hand-maintaining formatting. Keep unrelated refactors out of behavior changes. Update documentation when changing the ENEX, database, or sync format.

## Security and privacy

Use only official HTTPS service endpoints and validate remote hosts. Do not log credentials or complete note contents. New remote services require an explicit user connection and must remain optional. Report vulnerabilities privately as described in `SECURITY.md`.

By contributing, you agree that your contribution is licensed under GPL-3.0-or-later.
