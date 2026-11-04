use std::{collections::HashMap, path::Path};

use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    db::Library,
    enex::EnexNote,
    enml::{enml_to_html, plain_text, recognition_text, rewrite_internal_links},
    error::{BeverError, Result},
    models::{
        LibrarySnapshot, Note, NoteFilter, NoteSummary, Notebook, Resource, SaveNote, Shortcut,
        Stack, Tag,
    },
};

const NOTE_COLUMNS: &str = "id, notebook_id, title, content_html, original_enml, plain_text, created_at, updated_at, deleted_at, attributes_json, evernote_guid, revision";
const RESOURCE_COLUMNS: &str = "id, note_id, sha256, md5, mime, file_name, size, width, height, duration, recognition_xml, recognition_text, attributes_json, alternate_sha256, position";

#[derive(Debug)]
pub enum ImportNoteResult {
    Imported(Box<Note>),
    Duplicate(String),
}

impl Library {
    pub fn snapshot(&self) -> Result<LibrarySnapshot> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        let mut stacks = connection.prepare("SELECT id, name, created_at, updated_at, revision FROM stacks ORDER BY name COLLATE NOCASE")?;
        let stacks = stacks
            .query_map([], |row| {
                Ok(Stack {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    created_at: row.get(2)?,
                    updated_at: row.get(3)?,
                    revision: row.get(4)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut notebooks = connection.prepare("SELECT id, name, stack_id, created_at, updated_at, revision FROM notebooks ORDER BY name COLLATE NOCASE")?;
        let notebooks = notebooks
            .query_map([], |row| {
                Ok(Notebook {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    stack_id: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                    revision: row.get(5)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut tags = connection.prepare("SELECT id, name, parent_id, color, created_at, updated_at FROM tags ORDER BY name COLLATE NOCASE")?;
        let tags = tags
            .query_map([], |row| {
                Ok(Tag {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    parent_id: row.get(2)?,
                    color: row.get(3)?,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut shortcuts = connection.prepare("SELECT id, target_type, target_id, label, sort_order, created_at FROM shortcuts ORDER BY sort_order, created_at")?;
        let shortcuts = shortcuts
            .query_map([], |row| {
                Ok(Shortcut {
                    id: row.get(0)?,
                    target_type: row.get(1)?,
                    target_id: row.get(2)?,
                    label: row.get(3)?,
                    sort_order: row.get(4)?,
                    created_at: row.get(5)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(LibrarySnapshot {
            stacks,
            notebooks,
            tags,
            shortcuts,
        })
    }

    pub fn ensure_notebook(&self, name: &str, stack_id: Option<&str>) -> Result<String> {
        let name = normalized_name(name)?;
        let connection = self.connection.lock().expect("database mutex poisoned");
        ensure_notebook_with(&connection, name, stack_id)
    }

    pub fn set_notebook_stack(
        &self,
        notebook_id: &str,
        stack_id: Option<&str>,
    ) -> Result<Notebook> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        let changed = connection.execute(
            "UPDATE notebooks SET stack_id = ?2, updated_at = ?3, revision = revision + 1 WHERE id = ?1",
            params![notebook_id, stack_id, Utc::now().timestamp_millis()],
        )?;
        if changed == 0 {
            return Err(BeverError::NotFound(format!("notebook {notebook_id}")));
        }
        let notebook = connection.query_row(
            "SELECT id, name, stack_id, created_at, updated_at, revision FROM notebooks WHERE id = ?1",
            [notebook_id],
            |row| {
                Ok(Notebook {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    stack_id: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                    revision: row.get(5)?,
                })
            },
        )?;
        Ok(notebook)
    }

    pub fn create_stack(&self, name: &str) -> Result<Stack> {
        let name = normalized_name(name)?;
        let now = Utc::now().timestamp_millis();
        let stack = Stack {
            id: Uuid::new_v4().to_string(),
            name: name.to_owned(),
            created_at: now,
            updated_at: now,
            revision: 1,
        };
        let connection = self.connection.lock().expect("database mutex poisoned");
        connection.execute(
            "INSERT INTO stacks(id, name, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
            params![stack.id, stack.name, now],
        )?;
        Ok(stack)
    }

    pub fn create_tag(&self, name: &str, parent_id: Option<&str>) -> Result<Tag> {
        let name = normalized_name(name)?;
        let connection = self.connection.lock().expect("database mutex poisoned");
        let id = ensure_tag_with(&connection, name, parent_id)?;
        drop(connection);
        self.snapshot()?
            .tags
            .into_iter()
            .find(|tag| tag.id == id)
            .ok_or_else(|| BeverError::NotFound(format!("tag {id}")))
    }

    pub fn create_note(&self, notebook_id: Option<&str>) -> Result<Note> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        let notebook_id = match notebook_id {
            Some(id) => id.to_owned(),
            None => default_notebook(&connection)?,
        };
        let now = Utc::now().timestamp_millis();
        let id = Uuid::new_v4().to_string();
        connection.execute(
            "INSERT INTO notes(id, notebook_id, title, content_html, plain_text, created_at, updated_at, attributes_json) VALUES (?1, ?2, '', '<p></p>', '', ?3, ?3, '{}')",
            params![id, notebook_id, now],
        )?;
        reindex_note(&connection, &id)?;
        drop(connection);
        self.get_note(&id)
    }

    pub fn save_note(&self, input: &SaveNote) -> Result<Note> {
        let mut connection = self.connection.lock().expect("database mutex poisoned");
        let transaction = connection.transaction()?;
        let plain_text = plain_text(&input.content_html);
        let changed = transaction.execute(
            "UPDATE notes SET notebook_id = ?2, title = ?3, content_html = ?4, plain_text = ?5, attributes_json = ?6, updated_at = ?7, revision = revision + 1, dirty = 1 WHERE id = ?1",
            params![
                input.id,
                input.notebook_id,
                input.title.trim(),
                input.content_html,
                plain_text,
                serde_json::to_string(&input.attributes)?,
                Utc::now().timestamp_millis(),
            ],
        )?;
        if changed == 0 {
            return Err(BeverError::NotFound(format!("note {}", input.id)));
        }
        transaction.execute("DELETE FROM note_tags WHERE note_id = ?1", [&input.id])?;
        for name in &input.tags {
            let tag_id = ensure_tag_with(&transaction, name, None)?;
            transaction.execute(
                "INSERT OR IGNORE INTO note_tags(note_id, tag_id) VALUES (?1, ?2)",
                params![input.id, tag_id],
            )?;
        }
        reindex_note(&transaction, &input.id)?;
        transaction.commit()?;
        drop(connection);
        self.get_note(&input.id)
    }

    pub fn move_notes(&self, ids: &[String], notebook_id: &str) -> Result<u64> {
        if ids.is_empty() {
            return Ok(0);
        }
        let mut connection = self.connection.lock().expect("database mutex poisoned");
        let transaction = connection.transaction()?;
        let notebook_exists = transaction
            .query_row(
                "SELECT 1 FROM notebooks WHERE id = ?1",
                [notebook_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .is_some();
        if !notebook_exists {
            return Err(BeverError::NotFound(format!("notebook {notebook_id}")));
        }
        let now = Utc::now().timestamp_millis();
        let mut moved = 0;
        for id in ids {
            let changed = transaction.execute(
                "UPDATE notes SET notebook_id = ?2, updated_at = ?3, revision = revision + 1, dirty = 1 WHERE id = ?1",
                params![id, notebook_id, now],
            )?;
            if changed == 0 {
                return Err(BeverError::NotFound(format!("note {id}")));
            }
            moved += 1;
        }
        transaction.commit()?;
        Ok(moved)
    }

    pub fn copy_notes(&self, ids: &[String], notebook_id: &str) -> Result<u64> {
        if ids.is_empty() {
            return Ok(0);
        }
        let notes = ids
            .iter()
            .map(|id| self.get_note(id))
            .collect::<Result<Vec<_>>>()?;
        let mut connection = self.connection.lock().expect("database mutex poisoned");
        let transaction = connection.transaction()?;
        let notebook_exists = transaction
            .query_row(
                "SELECT 1 FROM notebooks WHERE id = ?1",
                [notebook_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .is_some();
        if !notebook_exists {
            return Err(BeverError::NotFound(format!("notebook {notebook_id}")));
        }
        for original in notes {
            let now = Utc::now().timestamp_millis();
            let mut copy = original.clone();
            copy.id = Uuid::new_v4().to_string();
            copy.notebook_id = notebook_id.to_owned();
            let title = original.title.trim();
            copy.title = format!(
                "{} (copy)",
                if title.is_empty() { "Untitled" } else { title }
            );
            copy.created_at = now;
            copy.updated_at = now;
            copy.evernote_guid = None;
            copy.revision = 1;
            for resource in &mut copy.resources {
                resource.id = Uuid::new_v4().to_string();
                resource.note_id = copy.id.clone();
            }
            insert_note(&transaction, &copy)?;
        }
        transaction.commit()?;
        Ok(ids.len() as u64)
    }

    pub fn add_tags_to_notes(&self, ids: &[String], tags: &[String]) -> Result<u64> {
        if ids.is_empty() || tags.is_empty() {
            return Ok(0);
        }
        let names = tags
            .iter()
            .map(|name| normalized_name(name))
            .collect::<Result<Vec<_>>>()?;
        let mut connection = self.connection.lock().expect("database mutex poisoned");
        let transaction = connection.transaction()?;
        let tag_ids = names
            .into_iter()
            .map(|name| ensure_tag_with(&transaction, name, None))
            .collect::<Result<Vec<_>>>()?;
        let now = Utc::now().timestamp_millis();
        let mut changed_notes = 0;
        for id in ids {
            let exists = transaction
                .query_row("SELECT 1 FROM notes WHERE id = ?1", [id], |row| {
                    row.get::<_, i64>(0)
                })
                .optional()?
                .is_some();
            if !exists {
                return Err(BeverError::NotFound(format!("note {id}")));
            }
            let mut changed = false;
            for tag_id in &tag_ids {
                changed |= transaction.execute(
                    "INSERT OR IGNORE INTO note_tags(note_id, tag_id) VALUES (?1, ?2)",
                    params![id, tag_id],
                )? > 0;
            }
            if changed {
                transaction.execute(
                    "UPDATE notes SET updated_at = ?2, revision = revision + 1, dirty = 1 WHERE id = ?1",
                    params![id, now],
                )?;
                reindex_note(&transaction, id)?;
                changed_notes += 1;
            }
        }
        transaction.commit()?;
        Ok(changed_notes)
    }

    pub fn update_note_timestamps(
        &self,
        id: &str,
        created_at: i64,
        updated_at: i64,
    ) -> Result<Note> {
        if created_at < 0 || updated_at < created_at {
            return Err(BeverError::Invalid(
                "updated time must be at or after created time".into(),
            ));
        }
        let connection = self.connection.lock().expect("database mutex poisoned");
        let changed = connection.execute(
            "UPDATE notes SET created_at = ?2, updated_at = ?3, revision = revision + 1, dirty = 1 WHERE id = ?1",
            params![id, created_at, updated_at],
        )?;
        if changed == 0 {
            return Err(BeverError::NotFound(format!("note {id}")));
        }
        drop(connection);
        self.get_note(id)
    }

    pub fn set_note_trashed(&self, id: &str, trashed: bool) -> Result<()> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        let deleted_at = trashed.then(|| Utc::now().timestamp_millis());
        let changed = connection.execute(
            "UPDATE notes SET deleted_at = ?2, revision = revision + 1, dirty = 1 WHERE id = ?1",
            params![id, deleted_at],
        )?;
        if changed == 0 {
            return Err(BeverError::NotFound(format!("note {id}")));
        }
        Ok(())
    }

    pub fn delete_note_permanently(&self, id: &str) -> Result<()> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        connection.execute("DELETE FROM notes_fts WHERE note_id = ?1", [id])?;
        connection.execute("DELETE FROM notes WHERE id = ?1", [id])?;
        connection.execute(
            "INSERT INTO sync_tombstones(kind, id, deleted_at) VALUES ('note', ?1, ?2) ON CONFLICT(kind, id) DO UPDATE SET deleted_at = excluded.deleted_at",
            params![id, Utc::now().timestamp_millis()],
        )?;
        Ok(())
    }

    pub fn get_note(&self, id: &str) -> Result<Note> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        note_with(&connection, id)
    }

    pub fn all_notes(&self) -> Result<Vec<Note>> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        let ids = connection
            .prepare("SELECT id FROM notes ORDER BY created_at")?
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        ids.iter().map(|id| note_with(&connection, id)).collect()
    }

    pub fn list_notes(&self, filter: &NoteFilter) -> Result<Vec<NoteSummary>> {
        crate::search::search_notes(self, filter)
    }

    pub fn toggle_shortcut(&self, target_type: &str, target_id: &str, label: &str) -> Result<bool> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        let existing: Option<String> = connection
            .query_row(
                "SELECT id FROM shortcuts WHERE target_type = ?1 AND target_id = ?2",
                params![target_type, target_id],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(id) = existing {
            connection.execute("DELETE FROM shortcuts WHERE id = ?1", [id])?;
            return Ok(false);
        }
        connection.execute(
            "INSERT INTO shortcuts(id, target_type, target_id, label, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![Uuid::new_v4().to_string(), target_type, target_id, label, Utc::now().timestamp_millis()],
        )?;
        Ok(true)
    }

    pub fn attach_file(&self, note_id: &str, path: impl AsRef<Path>) -> Result<Resource> {
        let path = path.as_ref();
        let stored = self.store_blob_file(path)?;
        let file_name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("attachment")
            .to_owned();
        let mime = mime_guess::from_path(path)
            .first_or_octet_stream()
            .essence_str()
            .to_owned();
        let mut connection = self.connection.lock().expect("database mutex poisoned");
        let transaction = connection.transaction()?;
        let position: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(position) + 1, 0) FROM resources WHERE note_id = ?1",
            [note_id],
            |row| row.get(0),
        )?;
        let resource = Resource {
            id: Uuid::new_v4().to_string(),
            note_id: note_id.to_owned(),
            sha256: stored.sha256,
            md5: stored.md5,
            mime,
            file_name: Some(file_name.clone()),
            size: stored.size,
            width: None,
            height: None,
            duration: None,
            recognition_xml: None,
            recognition_text: None,
            attributes: json!({"file-name": file_name, "attachment": false}),
            alternate_sha256: None,
            position,
        };
        insert_resource(&transaction, &resource)?;
        transaction.execute(
            "UPDATE notes SET content_html = content_html || ?2, updated_at = ?3, revision = revision + 1, dirty = 1 WHERE id = ?1",
            params![
                note_id,
                format!("<p><en-media hash=\"{}\" type=\"{}\"></en-media></p>", resource.md5, resource.mime),
                Utc::now().timestamp_millis()
            ],
        )?;
        reindex_note(&transaction, note_id)?;
        transaction.commit()?;
        Ok(resource)
    }

    pub fn resource(&self, id: &str) -> Result<Resource> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        let note_id = connection
            .query_row("SELECT note_id FROM resources WHERE id = ?1", [id], |row| {
                row.get::<_, String>(0)
            })
            .optional()?
            .ok_or_else(|| BeverError::NotFound(format!("resource {id}")))?;
        resources_for_note(&connection, &note_id)?
            .into_iter()
            .find(|resource| resource.id == id)
            .ok_or_else(|| BeverError::NotFound(format!("resource {id}")))
    }

    pub fn resource_location(&self, md5_or_sha: &str) -> Result<Resource> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        let note_id = connection
            .query_row(
                "SELECT note_id FROM resources WHERE md5 = ?1 OR sha256 = ?1 ORDER BY position LIMIT 1",
                [md5_or_sha],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or_else(|| BeverError::NotFound(format!("resource {md5_or_sha}")))?;
        let resources = resources_for_note(&connection, &note_id)?;
        resources
            .into_iter()
            .find(|resource| resource.md5 == md5_or_sha || resource.sha256 == md5_or_sha)
            .ok_or_else(|| BeverError::NotFound(format!("resource {md5_or_sha}")))
    }

    pub fn import_enex_note(
        &self,
        imported: EnexNote,
        notebook_id: &str,
    ) -> Result<ImportNoteResult> {
        let note_id = Uuid::new_v4().to_string();
        let mut resources = Vec::with_capacity(imported.resources.len());
        for (position, imported_resource) in imported.resources.into_iter().enumerate() {
            let stored = self.store_blob(&imported_resource.data)?;
            let alternate_sha256 = imported_resource
                .alternate_data
                .as_deref()
                .map(|data| self.store_blob(data))
                .transpose()?
                .map(|blob| blob.sha256);
            let file_name = imported_resource
                .attributes
                .get("file-name")
                .and_then(Value::as_str)
                .map(str::to_owned);
            let ocr = imported_resource
                .recognition_xml
                .as_deref()
                .map(recognition_text)
                .or(imported_resource.ocr_text);
            resources.push(Resource {
                id: Uuid::new_v4().to_string(),
                note_id: note_id.clone(),
                sha256: stored.sha256,
                md5: stored.md5,
                mime: imported_resource.mime,
                file_name,
                size: stored.size,
                width: imported_resource.width,
                height: imported_resource.height,
                duration: imported_resource.duration,
                recognition_xml: imported_resource.recognition_xml,
                recognition_text: ocr,
                attributes: imported_resource.attributes,
                alternate_sha256,
                position: position as i64,
            });
        }
        let html = enml_to_html(&imported.content);
        let note = Note {
            id: note_id,
            notebook_id: notebook_id.to_owned(),
            title: imported.title,
            plain_text: plain_text(&html),
            content_html: html,
            original_enml: Some(imported.content),
            created_at: imported.created_at,
            updated_at: imported.updated_at,
            deleted_at: None,
            attributes: imported.attributes,
            evernote_guid: imported.evernote_guid,
            revision: 1,
            tags: imported.tags,
            resources,
        };
        let import_key = note_import_key(&note)?;
        let mut connection = self.connection.lock().expect("database mutex poisoned");
        let duplicate = find_import_duplicate(&connection, &note, &import_key)?;
        let transaction = connection.transaction()?;
        let result = if let Some(existing_id) = duplicate {
            ImportNoteResult::Duplicate(existing_id)
        } else {
            insert_note(&transaction, &note)?;
            ImportNoteResult::Imported(Box::new(note))
        };
        let imported_id = match &result {
            ImportNoteResult::Imported(note) => &note.id,
            ImportNoteResult::Duplicate(id) => id,
        };
        transaction.execute(
            "INSERT INTO import_sources(import_key, note_id, imported_at) VALUES (?1, ?2, ?3) ON CONFLICT(import_key) DO NOTHING",
            params![import_key, imported_id, Utc::now().timestamp_millis()],
        )?;
        transaction.commit()?;
        drop(connection);
        match result {
            ImportNoteResult::Imported(note) => Ok(ImportNoteResult::Imported(Box::new(
                self.get_note(&note.id)?,
            ))),
            duplicate => Ok(duplicate),
        }
    }

    pub fn rewrite_import_links(&self) -> Result<()> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        let mapping = connection
            .prepare("SELECT evernote_guid, id FROM notes WHERE evernote_guid IS NOT NULL")?
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?.to_lowercase(),
                    row.get::<_, String>(1)?,
                ))
            })?
            .collect::<std::result::Result<HashMap<_, _>, _>>()?;
        if mapping.is_empty() {
            return Ok(());
        }
        let notes = connection
            .prepare(
                "SELECT id, content_html FROM notes WHERE content_html LIKE '%evernote:///view/%'",
            )?
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        for (id, html) in notes {
            let rewritten = rewrite_internal_links(&html, &mapping);
            if rewritten != html {
                connection.execute(
                    "UPDATE notes SET content_html = ?2 WHERE id = ?1",
                    params![id, rewritten],
                )?;
            }
        }
        Ok(())
    }

    pub fn setting(&self, key: &str) -> Result<Option<String>> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        Ok(connection
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
                row.get(0)
            })
            .optional()?)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        connection.execute(
            "INSERT INTO settings(key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }
}

fn normalized_name(name: &str) -> Result<&str> {
    let name = name.trim();
    if name.is_empty() {
        Err(BeverError::Invalid("name cannot be empty".into()))
    } else if name.len() > 255 {
        Err(BeverError::Invalid(
            "name cannot exceed 255 characters".into(),
        ))
    } else {
        Ok(name)
    }
}

fn default_notebook(connection: &Connection) -> Result<String> {
    connection
        .query_row(
            "SELECT id FROM notebooks ORDER BY name = 'Inbox' DESC, name LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| BeverError::NotFound("default notebook".into()))
}

fn ensure_notebook_with(
    connection: &Connection,
    name: &str,
    stack_id: Option<&str>,
) -> Result<String> {
    if let Some(id) = connection
        .query_row(
            "SELECT id FROM notebooks WHERE name = ?1 COLLATE NOCASE",
            [name],
            |row| row.get::<_, String>(0),
        )
        .optional()?
    {
        return Ok(id);
    }
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().timestamp_millis();
    connection.execute(
        "INSERT INTO notebooks(id, name, stack_id, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)",
        params![id, name, stack_id, now],
    )?;
    Ok(id)
}

fn ensure_tag_with(connection: &Connection, name: &str, parent_id: Option<&str>) -> Result<String> {
    let name = normalized_name(name)?;
    if let Some(id) = connection
        .query_row(
            "SELECT id FROM tags WHERE name = ?1 COLLATE NOCASE AND parent_id IS ?2",
            params![name, parent_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?
    {
        return Ok(id);
    }
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().timestamp_millis();
    connection.execute(
        "INSERT INTO tags(id, name, parent_id, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)",
        params![id, name, parent_id, now],
    )?;
    Ok(id)
}

pub(crate) fn insert_note(connection: &Connection, note: &Note) -> Result<()> {
    connection.execute(
        "INSERT INTO notes(id, notebook_id, title, content_html, original_enml, plain_text, created_at, updated_at, deleted_at, attributes_json, evernote_guid, revision) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            note.id,
            note.notebook_id,
            note.title,
            note.content_html,
            note.original_enml,
            note.plain_text,
            note.created_at,
            note.updated_at,
            note.deleted_at,
            serde_json::to_string(&note.attributes)?,
            note.evernote_guid,
            note.revision,
        ],
    )?;
    for tag in &note.tags {
        let tag_id = ensure_tag_with(connection, tag, None)?;
        connection.execute(
            "INSERT OR IGNORE INTO note_tags(note_id, tag_id) VALUES (?1, ?2)",
            params![note.id, tag_id],
        )?;
    }
    for resource in &note.resources {
        insert_resource(connection, resource)?;
    }
    reindex_note(connection, &note.id)?;
    Ok(())
}

fn insert_resource(connection: &Connection, resource: &Resource) -> Result<()> {
    connection.execute(
        "INSERT INTO resources(id, note_id, sha256, md5, mime, file_name, size, width, height, duration, recognition_xml, recognition_text, attributes_json, alternate_sha256, position) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
        params![
            resource.id,
            resource.note_id,
            resource.sha256,
            resource.md5,
            resource.mime,
            resource.file_name,
            resource.size,
            resource.width,
            resource.height,
            resource.duration,
            resource.recognition_xml,
            resource.recognition_text,
            serde_json::to_string(&resource.attributes)?,
            resource.alternate_sha256,
            resource.position,
        ],
    )?;
    Ok(())
}

fn note_with(connection: &Connection, id: &str) -> Result<Note> {
    let sql = format!("SELECT {NOTE_COLUMNS} FROM notes WHERE id = ?1");
    let mut note = connection
        .query_row(&sql, [id], |row| {
            let attributes: String = row.get(9)?;
            Ok(Note {
                id: row.get(0)?,
                notebook_id: row.get(1)?,
                title: row.get(2)?,
                content_html: row.get(3)?,
                original_enml: row.get(4)?,
                plain_text: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
                deleted_at: row.get(8)?,
                attributes: serde_json::from_str(&attributes).unwrap_or_else(|_| json!({})),
                evernote_guid: row.get(10)?,
                revision: row.get(11)?,
                tags: Vec::new(),
                resources: Vec::new(),
            })
        })
        .optional()?
        .ok_or_else(|| BeverError::NotFound(format!("note {id}")))?;
    note.tags = tags_for_note(connection, id)?;
    note.resources = resources_for_note(connection, id)?;
    Ok(note)
}

fn note_import_key(note: &Note) -> Result<String> {
    let mut tags = note.tags.clone();
    tags.sort_by_key(|tag| tag.to_lowercase());
    let resources = note
        .resources
        .iter()
        .map(|resource| {
            json!({
                "sha256": resource.sha256,
                "md5": resource.md5,
                "mime": resource.mime,
                "fileName": resource.file_name,
                "size": resource.size,
                "width": resource.width,
                "height": resource.height,
                "duration": resource.duration,
                "recognitionXml": resource.recognition_xml,
                "attributes": resource.attributes,
                "alternateSha256": resource.alternate_sha256,
                "position": resource.position,
            })
        })
        .collect::<Vec<_>>();
    let fingerprint = json!({
        "evernoteGuid": note.evernote_guid.as_deref().map(str::to_lowercase),
        "originalEnml": note.original_enml,
        "title": note.title,
        "createdAt": note.created_at,
        "updatedAt": note.updated_at,
        "attributes": note.attributes,
        "tags": tags,
        "resources": resources,
    });
    let mut hasher = Sha256::new();
    hasher.update(serde_json::to_vec(&fingerprint)?);
    Ok(format!("{:x}", hasher.finalize()))
}

fn find_import_duplicate(
    connection: &Connection,
    note: &Note,
    import_key: &str,
) -> Result<Option<String>> {
    if let Some(id) = connection
        .query_row(
            "SELECT note_id FROM import_sources WHERE import_key = ?1",
            [import_key],
            |row| row.get::<_, String>(0),
        )
        .optional()?
    {
        return Ok(Some(id));
    }
    // Recover identity for libraries imported before import_sources was introduced.
    let candidates = connection
        .prepare(
            "SELECT id FROM notes WHERE original_enml = ?1 OR (?2 IS NOT NULL AND lower(evernote_guid) = lower(?2))",
        )?
        .query_map(
            params![note.original_enml, note.evernote_guid],
            |row| row.get::<_, String>(0),
        )?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    for id in candidates {
        if note_import_key(&note_with(connection, &id)?)? == import_key {
            return Ok(Some(id));
        }
    }
    Ok(None)
}

fn tags_for_note(connection: &Connection, note_id: &str) -> Result<Vec<String>> {
    Ok(connection
        .prepare("SELECT t.name FROM tags t JOIN note_tags nt ON nt.tag_id = t.id WHERE nt.note_id = ?1 ORDER BY t.name COLLATE NOCASE")?
        .query_map([note_id], |row| row.get(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?)
}

fn resources_for_note(connection: &Connection, note_id: &str) -> Result<Vec<Resource>> {
    let sql =
        format!("SELECT {RESOURCE_COLUMNS} FROM resources WHERE note_id = ?1 ORDER BY position");
    Ok(connection
        .prepare(&sql)?
        .query_map([note_id], |row| {
            let attributes: String = row.get(12)?;
            Ok(Resource {
                id: row.get(0)?,
                note_id: row.get(1)?,
                sha256: row.get(2)?,
                md5: row.get(3)?,
                mime: row.get(4)?,
                file_name: row.get(5)?,
                size: row.get(6)?,
                width: row.get(7)?,
                height: row.get(8)?,
                duration: row.get(9)?,
                recognition_xml: row.get(10)?,
                recognition_text: row.get(11)?,
                attributes: serde_json::from_str(&attributes).unwrap_or_else(|_| json!({})),
                alternate_sha256: row.get(13)?,
                position: row.get(14)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?)
}

pub(crate) fn reindex_note(connection: &Connection, note_id: &str) -> Result<()> {
    let (title, body): (String, String) = connection.query_row(
        "SELECT title, plain_text FROM notes WHERE id = ?1",
        [note_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let tags = tags_for_note(connection, note_id)?.join(" ");
    let resources = connection
        .prepare("SELECT COALESCE(file_name, ''), COALESCE(recognition_text, '') FROM resources WHERE note_id = ?1")?
        .query_map([note_id], |row| {
            Ok(format!("{} {}", row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?
        .join(" ");
    connection.execute("DELETE FROM notes_fts WHERE note_id = ?1", [note_id])?;
    connection.execute(
        "INSERT INTO notes_fts(note_id, title, body, tags, resources) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![note_id, title, body, tags, resources],
    )?;
    Ok(())
}
