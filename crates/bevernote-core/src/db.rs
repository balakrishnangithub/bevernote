use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

use rusqlite::{Connection, OptionalExtension};
use uuid::Uuid;

use crate::error::Result;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS stacks (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL COLLATE NOCASE UNIQUE,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    revision INTEGER NOT NULL DEFAULT 1
);
CREATE TABLE IF NOT EXISTS notebooks (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL COLLATE NOCASE UNIQUE,
    stack_id TEXT REFERENCES stacks(id) ON DELETE SET NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    revision INTEGER NOT NULL DEFAULT 1
);
CREATE TABLE IF NOT EXISTS notes (
    id TEXT PRIMARY KEY,
    notebook_id TEXT NOT NULL REFERENCES notebooks(id),
    title TEXT NOT NULL DEFAULT '',
    content_html TEXT NOT NULL DEFAULT '',
    original_enml TEXT,
    plain_text TEXT NOT NULL DEFAULT '',
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    deleted_at INTEGER,
    attributes_json TEXT NOT NULL DEFAULT '{}',
    evernote_guid TEXT,
    revision INTEGER NOT NULL DEFAULT 1,
    dirty INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX IF NOT EXISTS notes_evernote_guid
    ON notes(evernote_guid) WHERE evernote_guid IS NOT NULL;
CREATE INDEX IF NOT EXISTS notes_notebook_updated ON notes(notebook_id, updated_at DESC);
CREATE INDEX IF NOT EXISTS notes_updated ON notes(updated_at DESC);
CREATE INDEX IF NOT EXISTS notes_deleted ON notes(deleted_at);
CREATE TABLE IF NOT EXISTS tags (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    parent_id TEXT REFERENCES tags(id) ON DELETE CASCADE,
    color TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS tags_parent_name
    ON tags(COALESCE(parent_id, ''), lower(name));
CREATE TABLE IF NOT EXISTS note_tags (
    note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    tag_id TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY(note_id, tag_id)
);
CREATE INDEX IF NOT EXISTS note_tags_tag ON note_tags(tag_id, note_id);
CREATE TABLE IF NOT EXISTS resources (
    id TEXT PRIMARY KEY,
    note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    sha256 TEXT NOT NULL,
    md5 TEXT NOT NULL,
    mime TEXT NOT NULL,
    file_name TEXT,
    size INTEGER NOT NULL,
    width INTEGER,
    height INTEGER,
    duration INTEGER,
    recognition_xml TEXT,
    recognition_text TEXT,
    attributes_json TEXT NOT NULL DEFAULT '{}',
    alternate_sha256 TEXT,
    position INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS resources_note ON resources(note_id, position);
CREATE INDEX IF NOT EXISTS resources_md5 ON resources(md5);
CREATE INDEX IF NOT EXISTS resources_sha ON resources(sha256);
CREATE TABLE IF NOT EXISTS import_sources (
    import_key TEXT PRIMARY KEY,
    note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    imported_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS import_sources_note ON import_sources(note_id);
CREATE TABLE IF NOT EXISTS shortcuts (
    id TEXT PRIMARY KEY,
    target_type TEXT NOT NULL,
    target_id TEXT NOT NULL,
    label TEXT NOT NULL,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    UNIQUE(target_type, target_id)
);
CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS sync_items (
    kind TEXT NOT NULL,
    id TEXT NOT NULL,
    remote_revision INTEGER NOT NULL DEFAULT 0,
    synced_hash TEXT NOT NULL DEFAULT '',
    synced_at INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY(kind, id)
);
CREATE TABLE IF NOT EXISTS sync_tombstones (
    kind TEXT NOT NULL,
    id TEXT NOT NULL,
    deleted_at INTEGER NOT NULL,
    PRIMARY KEY(kind, id)
);
CREATE VIRTUAL TABLE IF NOT EXISTS notes_fts USING fts5(
    note_id UNINDEXED,
    title,
    body,
    tags,
    resources,
    tokenize = 'unicode61 remove_diacritics 2'
);
"#;

#[derive(Debug)]
pub struct Library {
    pub(crate) connection: Mutex<Connection>,
    pub(crate) blob_dir: PathBuf,
    device_id: String,
}

impl Library {
    pub fn open(data_dir: impl AsRef<Path>) -> Result<Self> {
        let data_dir = data_dir.as_ref();
        fs::create_dir_all(data_dir)?;
        Self::open_at(data_dir.join("bevernote.db"), data_dir.join("blobs"))
    }

    pub fn open_at(db_path: impl AsRef<Path>, blob_dir: impl AsRef<Path>) -> Result<Self> {
        let blob_dir = blob_dir.as_ref().to_path_buf();
        fs::create_dir_all(&blob_dir)?;
        let connection = Connection::open(db_path)?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        connection.execute_batch(
            "PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;",
        )?;
        connection.execute_batch(SCHEMA)?;
        ensure_default_notebook(&connection)?;
        let device_id = device_id(&connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
            blob_dir,
            device_id,
        })
    }

    pub fn device_id(&self) -> &str {
        &self.device_id
    }

    pub fn blob_dir(&self) -> &Path {
        &self.blob_dir
    }

    pub fn optimize(&self) -> Result<()> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        connection.execute_batch("PRAGMA optimize;")?;
        Ok(())
    }
}

fn ensure_default_notebook(connection: &Connection) -> Result<()> {
    let exists: Option<String> = connection
        .query_row("SELECT id FROM notebooks LIMIT 1", [], |row| row.get(0))
        .optional()?;
    if exists.is_none() {
        let now = chrono::Utc::now().timestamp_millis();
        connection.execute(
            "INSERT INTO notebooks(id, name, created_at, updated_at) VALUES (?1, 'Inbox', ?2, ?2)",
            [Uuid::new_v4().to_string(), now.to_string()],
        )?;
    }
    Ok(())
}

fn device_id(connection: &Connection) -> Result<String> {
    if let Some(id) = connection
        .query_row(
            "SELECT value FROM settings WHERE key = 'device_id'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?
    {
        return Ok(id);
    }
    let id = Uuid::new_v4().to_string();
    connection.execute(
        "INSERT INTO settings(key, value) VALUES ('device_id', ?1)",
        [&id],
    )?;
    Ok(id)
}
