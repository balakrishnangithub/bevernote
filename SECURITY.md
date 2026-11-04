# Security policy

Bevernote handles private notes, attachments, and cloud access tokens. Please do not disclose a suspected vulnerability in a public issue or attach private note exports to a report.

When the project is published on GitHub, use GitHub's private vulnerability reporting for that repository. Include the affected version, reproduction steps, impact, and a minimal synthetic example. Remove all personal note content and credentials.

## Credential safety

- pCloud access tokens belong in the operating system credential vault.
- Client secrets are not needed for the implicit-grant client flow and must not be embedded in the app.
- Never commit `.env` files, ENEX files, application databases, token dumps, or the ignored `reference/` and `test-data/` directories.
- Security logs and bug reports must not contain access tokens or full note contents.

## Data-protection expectations

Local notes are stored in the current user's application-data directory, not in an application-managed encrypted vault. Device disk encryption and OS account security remain important. pCloud transport uses HTTPS, but the remote Bevernote copy is not end-to-end encrypted. Evernote encrypted blocks are retained but not decrypted by Bevernote.
