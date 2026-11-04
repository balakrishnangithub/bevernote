//! Revisioned, conflict-preserving synchronization with no application server.
use std::{collections::HashMap, fs};

use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use uuid::Uuid;

use crate::{
    db::Library,
    error::{BeverError, Result},
    models::{LibrarySnapshot, Note, Shortcut, SyncReport},
    pcloud::{PCloudClient, PCloudCredentials},
    repository::insert_note,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Envelope<T> {
    id: String,
    revision: i64,
    updated_at: i64,
    device_id: String,
    deleted: bool,
    payload: Option<T>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MetadataEnvelope {
    revision: i64,
    updated_at: i64,
    device_id: String,
    snapshot: LibrarySnapshot,
}

#[derive(Debug, Clone, Copy, Default)]
struct SyncState {
    remote_revision: i64,
}

pub fn sync_with_pcloud(library: &Library, credentials: PCloudCredentials) -> Result<SyncReport> {
    let client = PCloudClient::new(credentials)?;
    client.initialize()?;
    let mut report = SyncReport::default();
    sync_metadata(library, &client)?;
    sync_notes(library, &client, &mut report)?;
    report.synced_at = Utc::now().timestamp_millis();
    library.set_setting("last_sync_at", &report.synced_at.to_string())?;
    library.optimize()?;
    Ok(report)
}

fn sync_notes(library: &Library, client: &PCloudClient, report: &mut SyncReport) -> Result<()> {
    let remote_files = client.list("/Bevernote/items")?;
    let mut states = sync_states(library)?;
    let mut local_notes = library
        .all_notes()?
        .into_iter()
        .map(|note| (note.id.clone(), note))
        .collect::<HashMap<_, _>>();
    let mut dirty = dirty_notes(library)?;

    for file in remote_files
        .iter()
        .filter(|file| file.name.starts_with("note-") && file.name.ends_with(".json"))
    {
        let bytes = client.download(&format!("/Bevernote/items/{}", file.name))?;
        let envelope: Envelope<Note> = serde_json::from_slice(&bytes)?;
        let state = states.get(&envelope.id).copied().unwrap_or_default();
        if envelope.deleted {
            if local_notes.contains_key(&envelope.id) {
                delete_remote_note(library, &envelope.id)?;
                report.pulled += 1;
            }
            record_state(library, "note", &envelope.id, envelope.revision, &bytes)?;
            states.insert(
                envelope.id.clone(),
                SyncState {
                    remote_revision: envelope.revision,
                },
            );
            continue;
        }
        let Some(mut remote_note) = envelope.payload else {
            continue;
        };
        remote_note.revision = envelope.revision;
        download_missing_blobs(library, client, &remote_note, report)?;
        let local = local_notes.get(&envelope.id);
        let local_is_dirty = dirty.get(&envelope.id).copied().unwrap_or(false);
        let remote_is_newer = envelope.revision > state.remote_revision;
        if remote_is_newer {
            if let Some(local_note) = local.filter(|_| local_is_dirty) {
                create_conflict_copy(library, local_note, &envelope.device_id)?;
                report.conflicts += 1;
            }
            if local_is_dirty
                || local.is_none()
                || remote_note.updated_at >= local.map_or(0, |note| note.updated_at)
            {
                apply_remote_note(library, &remote_note)?;
                report.pulled += 1;
            }
            record_state_hash(
                library,
                "note",
                &envelope.id,
                envelope.revision,
                &hash(&remote_note)?,
            )?;
            states.insert(
                envelope.id.clone(),
                SyncState {
                    remote_revision: envelope.revision,
                },
            );
        }
    }

    local_notes = library
        .all_notes()?
        .into_iter()
        .map(|note| (note.id.clone(), note))
        .collect();
    dirty = dirty_notes(library)?;
    for (id, mut note) in local_notes {
        let is_dirty = dirty.get(&id).copied().unwrap_or(false);
        let state = states.get(&id).copied().unwrap_or_default();
        let previous_hash = synced_hash(library, "note", &id)?.unwrap_or_default();
        let current_hash = hash(&note)?;
        if !is_dirty && previous_hash == current_hash {
            continue;
        }
        note.revision = note.revision.max(state.remote_revision) + 1;
        let envelope = Envelope {
            id: id.clone(),
            revision: note.revision,
            updated_at: note.updated_at,
            device_id: library.device_id().to_owned(),
            deleted: false,
            payload: Some(note.clone()),
        };
        let bytes = serde_json::to_vec(&envelope)?;
        client.upload(&format!("/Bevernote/items/note-{id}.json"), bytes)?;
        upload_missing_blobs(library, client, &note, report)?;
        mark_note_synced(library, &id, note.revision, &hash(&note)?)?;
        states.insert(
            id,
            SyncState {
                remote_revision: note.revision,
            },
        );
        report.pushed += 1;
    }

    let tombstones = {
        let connection = library.connection.lock().expect("database mutex poisoned");
        let rows = connection
            .prepare("SELECT id, deleted_at FROM sync_tombstones WHERE kind = 'note'")?
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        rows
    };
    for (id, deleted_at) in tombstones {
        let state = states.get(&id).copied().unwrap_or_default();
        let previous_hash = synced_hash(library, "note", &id)?.unwrap_or_default();
        let envelope: Envelope<Note> = Envelope {
            id: id.clone(),
            revision: state.remote_revision + 1,
            updated_at: deleted_at,
            device_id: library.device_id().to_owned(),
            deleted: true,
            payload: None,
        };
        let bytes = serde_json::to_vec(&envelope)?;
        if previous_hash == hash_bytes(&bytes) {
            continue;
        }
        client.upload(&format!("/Bevernote/items/note-{id}.json"), bytes.clone())?;
        record_state(library, "note", &id, envelope.revision, &bytes)?;
        report.pushed += 1;
    }
    Ok(())
}

fn sync_metadata(library: &Library, client: &PCloudClient) -> Result<()> {
    let root = client.list("/Bevernote")?;
    let remote = if root.iter().any(|file| file.name == "metadata.json") {
        let bytes = client.download("/Bevernote/metadata.json")?;
        Some(serde_json::from_slice::<MetadataEnvelope>(&bytes)?)
    } else {
        None
    };
    let local = library.snapshot()?;
    let merged = match &remote {
        Some(remote) => merge_snapshots(&local, &remote.snapshot),
        None => local,
    };
    apply_snapshot(library, &merged)?;
    let merged_bytes = serde_json::to_vec(&merged)?;
    let remote_bytes = remote
        .as_ref()
        .map(|value| hash(&value.snapshot))
        .transpose()?;
    if remote_bytes.as_deref() == Some(hash_bytes(&merged_bytes).as_str()) {
        return Ok(());
    }
    let envelope = MetadataEnvelope {
        revision: remote.as_ref().map_or(1, |value| value.revision + 1),
        updated_at: Utc::now().timestamp_millis(),
        device_id: library.device_id().to_owned(),
        snapshot: merged,
    };
    client.upload("/Bevernote/metadata.json", serde_json::to_vec(&envelope)?)?;
    Ok(())
}

fn merge_snapshots(local: &LibrarySnapshot, remote: &LibrarySnapshot) -> LibrarySnapshot {
    let mut stacks = local.stacks.clone();
    for item in &remote.stacks {
        match stacks.iter_mut().find(|current| current.id == item.id) {
            Some(current) if item.updated_at > current.updated_at => *current = item.clone(),
            None => stacks.push(item.clone()),
            _ => {}
        }
    }
    let mut notebooks = local.notebooks.clone();
    for item in &remote.notebooks {
        match notebooks.iter_mut().find(|current| current.id == item.id) {
            Some(current) if item.updated_at > current.updated_at => *current = item.clone(),
            None => notebooks.push(item.clone()),
            _ => {}
        }
    }
    let mut tags = local.tags.clone();
    for item in &remote.tags {
        match tags.iter_mut().find(|current| current.id == item.id) {
            Some(current) if item.updated_at > current.updated_at => *current = item.clone(),
            None => tags.push(item.clone()),
            _ => {}
        }
    }
    let mut shortcuts = local.shortcuts.clone();
    for item in &remote.shortcuts {
        if !shortcuts.iter().any(|current| {
            current.target_type == item.target_type && current.target_id == item.target_id
        }) {
            shortcuts.push(item.clone());
        }
    }
    shortcuts.sort_by_key(|shortcut| shortcut.created_at);
    LibrarySnapshot {
        stacks,
        notebooks,
        tags,
        shortcuts,
    }
}

fn apply_snapshot(library: &Library, snapshot: &LibrarySnapshot) -> Result<()> {
    let mut connection = library.connection.lock().expect("database mutex poisoned");
    let transaction = connection.transaction()?;
    for stack in &snapshot.stacks {
        let name = available_name(&transaction, "stacks", &stack.name, &stack.id)?;
        transaction.execute(
            "INSERT INTO stacks(id, name, created_at, updated_at, revision) VALUES (?1, ?2, ?3, ?4, ?5) ON CONFLICT(id) DO UPDATE SET name = excluded.name, updated_at = excluded.updated_at, revision = excluded.revision",
            params![stack.id, name, stack.created_at, stack.updated_at, stack.revision],
        )?;
    }
    for notebook in &snapshot.notebooks {
        let name = available_name(&transaction, "notebooks", &notebook.name, &notebook.id)?;
        transaction.execute(
            "INSERT INTO notebooks(id, name, stack_id, created_at, updated_at, revision) VALUES (?1, ?2, ?3, ?4, ?5, ?6) ON CONFLICT(id) DO UPDATE SET name = excluded.name, stack_id = excluded.stack_id, updated_at = excluded.updated_at, revision = excluded.revision",
            params![notebook.id, name, notebook.stack_id, notebook.created_at, notebook.updated_at, notebook.revision],
        )?;
    }
    for tag in &snapshot.tags {
        transaction.execute(
            "INSERT OR IGNORE INTO tags(id, name, parent_id, color, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![tag.id, tag.name, tag.parent_id, tag.color, tag.created_at, tag.updated_at],
        )?;
    }
    for shortcut in &snapshot.shortcuts {
        insert_shortcut(&transaction, shortcut)?;
    }
    transaction.commit()?;
    Ok(())
}

fn available_name(connection: &Connection, table: &str, name: &str, id: &str) -> Result<String> {
    debug_assert!(matches!(table, "stacks" | "notebooks"));
    let sql = format!("SELECT id FROM {table} WHERE name = ?1 COLLATE NOCASE AND id != ?2");
    let conflict: Option<String> = connection
        .query_row(&sql, params![name, id], |row| row.get(0))
        .optional()?;
    Ok(if conflict.is_some() {
        format!("{name} ({})", &id[..4.min(id.len())])
    } else {
        name.to_owned()
    })
}

fn insert_shortcut(connection: &Connection, shortcut: &Shortcut) -> Result<()> {
    connection.execute(
        "INSERT INTO shortcuts(id, target_type, target_id, label, sort_order, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6) ON CONFLICT(target_type, target_id) DO UPDATE SET label = excluded.label",
        params![shortcut.id, shortcut.target_type, shortcut.target_id, shortcut.label, shortcut.sort_order, shortcut.created_at],
    )?;
    Ok(())
}

fn download_missing_blobs(
    library: &Library,
    client: &PCloudClient,
    note: &Note,
    report: &mut SyncReport,
) -> Result<()> {
    for resource in &note.resources {
        for sha in [Some(&resource.sha256), resource.alternate_sha256.as_ref()]
            .into_iter()
            .flatten()
        {
            if library.blob_path(sha)?.exists() {
                continue;
            }
            let bytes = client.download(&format!("/Bevernote/blobs/{sha}"))?;
            let stored = library.store_blob(&bytes)?;
            if stored.sha256 != *sha {
                return Err(BeverError::Remote(format!("blob hash mismatch for {sha}")));
            }
            report.blobs_downloaded += 1;
        }
    }
    Ok(())
}

fn upload_missing_blobs(
    library: &Library,
    client: &PCloudClient,
    note: &Note,
    report: &mut SyncReport,
) -> Result<()> {
    let remote = client.list("/Bevernote/blobs")?;
    for resource in &note.resources {
        for sha in [Some(&resource.sha256), resource.alternate_sha256.as_ref()]
            .into_iter()
            .flatten()
        {
            if remote.iter().any(|file| file.name == *sha) {
                continue;
            }
            let path = library.blob_path(sha)?;
            if !path.exists() {
                return Err(BeverError::Invalid(format!("local blob {sha} is missing")));
            }
            client.upload(&format!("/Bevernote/blobs/{sha}"), fs::read(path)?)?;
            report.blobs_uploaded += 1;
        }
    }
    Ok(())
}

fn apply_remote_note(library: &Library, note: &Note) -> Result<()> {
    let mut connection = library.connection.lock().expect("database mutex poisoned");
    let transaction = connection.transaction()?;
    ensure_remote_notebook(&transaction, &note.notebook_id)?;
    transaction.execute("DELETE FROM notes_fts WHERE note_id = ?1", [&note.id])?;
    transaction.execute("DELETE FROM notes WHERE id = ?1", [&note.id])?;
    insert_note(&transaction, note)?;
    transaction.execute("UPDATE notes SET dirty = 0 WHERE id = ?1", [&note.id])?;
    transaction.commit()?;
    Ok(())
}

fn ensure_remote_notebook(connection: &Connection, id: &str) -> Result<()> {
    let exists: Option<String> = connection
        .query_row("SELECT id FROM notebooks WHERE id = ?1", [id], |row| {
            row.get(0)
        })
        .optional()?;
    if exists.is_some() {
        return Ok(());
    }
    let now = Utc::now().timestamp_millis();
    let name = format!("Synced notebook {}", &id[..4.min(id.len())]);
    connection.execute(
        "INSERT INTO notebooks(id, name, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
        params![id, name, now],
    )?;
    Ok(())
}

fn delete_remote_note(library: &Library, id: &str) -> Result<()> {
    let connection = library.connection.lock().expect("database mutex poisoned");
    connection.execute("DELETE FROM notes_fts WHERE note_id = ?1", [id])?;
    connection.execute("DELETE FROM notes WHERE id = ?1", [id])?;
    Ok(())
}

fn create_conflict_copy(library: &Library, local: &Note, device_id: &str) -> Result<()> {
    let mut copy = local.clone();
    copy.id = Uuid::new_v4().to_string();
    copy.title = format!(
        "{} (conflict from {})",
        local.title,
        &device_id[..8.min(device_id.len())]
    );
    copy.revision = 1;
    for resource in &mut copy.resources {
        resource.id = Uuid::new_v4().to_string();
        resource.note_id = copy.id.clone();
    }
    let mut connection = library.connection.lock().expect("database mutex poisoned");
    let transaction = connection.transaction()?;
    insert_note(&transaction, &copy)?;
    transaction.commit()?;
    Ok(())
}

fn dirty_notes(library: &Library) -> Result<HashMap<String, bool>> {
    let connection = library.connection.lock().expect("database mutex poisoned");
    let rows = connection
        .prepare("SELECT id, dirty FROM notes")?
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? != 0))
        })?
        .collect::<std::result::Result<HashMap<_, _>, _>>()?;
    Ok(rows)
}

fn sync_states(library: &Library) -> Result<HashMap<String, SyncState>> {
    let connection = library.connection.lock().expect("database mutex poisoned");
    let rows = connection
        .prepare("SELECT id, remote_revision FROM sync_items WHERE kind = 'note'")?
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                SyncState {
                    remote_revision: row.get::<_, i64>(1)?,
                },
            ))
        })?
        .collect::<std::result::Result<HashMap<_, _>, _>>()?;
    Ok(rows)
}

fn synced_hash(library: &Library, kind: &str, id: &str) -> Result<Option<String>> {
    let connection = library.connection.lock().expect("database mutex poisoned");
    Ok(connection
        .query_row(
            "SELECT synced_hash FROM sync_items WHERE kind = ?1 AND id = ?2",
            params![kind, id],
            |row| row.get(0),
        )
        .optional()?)
}

fn mark_note_synced(library: &Library, id: &str, revision: i64, hash: &str) -> Result<()> {
    let connection = library.connection.lock().expect("database mutex poisoned");
    connection.execute(
        "UPDATE notes SET revision = ?2, dirty = 0 WHERE id = ?1",
        params![id, revision],
    )?;
    drop(connection);
    record_state_hash(library, "note", id, revision, hash)
}

fn record_state(
    library: &Library,
    kind: &str,
    id: &str,
    revision: i64,
    bytes: &[u8],
) -> Result<()> {
    record_state_hash(library, kind, id, revision, &hash_bytes(bytes))
}

fn record_state_hash(
    library: &Library,
    kind: &str,
    id: &str,
    revision: i64,
    hash: &str,
) -> Result<()> {
    let connection = library.connection.lock().expect("database mutex poisoned");
    connection.execute(
        "INSERT INTO sync_items(kind, id, remote_revision, synced_hash, synced_at) VALUES (?1, ?2, ?3, ?4, ?5) ON CONFLICT(kind, id) DO UPDATE SET remote_revision = excluded.remote_revision, synced_hash = excluded.synced_hash, synced_at = excluded.synced_at",
        params![kind, id, revision, hash, Utc::now().timestamp_millis()],
    )?;
    Ok(())
}

fn hash(value: &impl Serialize) -> Result<String> {
    Ok(hash_bytes(&serde_json::to_vec(value)?))
}

fn hash_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_merge_keeps_both_devices_items() {
        let directory = tempfile::tempdir().unwrap();
        let library =
            Library::open_at(directory.path().join("db"), directory.path().join("blobs")).unwrap();
        library.ensure_notebook("Local", None).unwrap();
        let local = library.snapshot().unwrap();
        let mut remote = local.clone();
        remote.notebooks[0].name = "Remote".into();
        remote.notebooks[0].updated_at += 100;
        let merged = merge_snapshots(&local, &remote);
        assert_eq!(merged.notebooks[0].name, "Remote");
        apply_snapshot(&library, &merged).unwrap();
        assert!(library
            .snapshot()
            .unwrap()
            .notebooks
            .iter()
            .any(|notebook| notebook.name == "Remote"));
    }

    #[test]
    fn conflict_copy_preserves_local_edits() {
        let directory = tempfile::tempdir().unwrap();
        let library =
            Library::open_at(directory.path().join("db"), directory.path().join("blobs")).unwrap();
        let local = library.create_note(None).unwrap();
        create_conflict_copy(&library, &local, "12345678-device").unwrap();
        let notes = library.all_notes().unwrap();
        assert_eq!(notes.len(), 2);
        assert!(notes
            .iter()
            .any(|note| note.title.contains("conflict from 12345678")));
    }
}
