//! Revisioned, conflict-preserving synchronization with no application server.
use std::{
    collections::{HashMap, HashSet},
    fs,
    time::Instant,
};

use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use uuid::Uuid;

use crate::{
    db::Library,
    error::{BeverError, Result},
    models::{LibrarySnapshot, Note, Shortcut, SyncProgress, SyncReport},
    pcloud::{PCloudClient, PCloudCredentials, RemoteFile},
    repository::insert_note,
};

const REMOTE_INDEX_PATH: &str = "/Bevernote/items/index.json";

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

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct IndexedRemoteNote {
    revision: i64,
    size: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
struct RemoteNoteIndex {
    #[serde(default)]
    notes: HashMap<String, IndexedRemoteNote>,
}

#[derive(Debug, Clone, Default)]
struct SyncState {
    remote_revision: i64,
    synced_hash: String,
}

struct Progress<'a> {
    callback: &'a mut dyn FnMut(SyncProgress),
    started: Instant,
}

impl Progress<'_> {
    fn emit(
        &mut self,
        client: &PCloudClient,
        phase: &str,
        message: impl Into<String>,
        current: u64,
        total: u64,
    ) {
        let metrics = client.metrics();
        (self.callback)(SyncProgress {
            phase: phase.to_owned(),
            message: message.into(),
            current,
            total,
            elapsed_ms: elapsed_ms(self.started),
            network_ms: metrics.network_ms,
            remote_requests: metrics.remote_requests,
            bytes_uploaded: metrics.bytes_uploaded,
            bytes_downloaded: metrics.bytes_downloaded,
        });
    }
}

pub fn sync_with_pcloud(library: &Library, credentials: PCloudCredentials) -> Result<SyncReport> {
    sync_with_pcloud_progress(library, credentials, |_| {})
}

pub fn sync_with_pcloud_progress(
    library: &Library,
    credentials: PCloudCredentials,
    progress: impl FnMut(SyncProgress),
) -> Result<SyncReport> {
    let client = PCloudClient::new(credentials)?;
    let mut callback = progress;
    let mut progress = Progress {
        callback: &mut callback,
        started: Instant::now(),
    };
    progress.emit(
        &client,
        "preparing",
        "Connecting to pCloud and preparing sync folders",
        0,
        0,
    );
    client.initialize()?;
    let mut report = SyncReport::default();
    sync_metadata(library, &client, &mut progress)?;
    sync_notes(library, &client, &mut report, &mut progress)?;
    progress.emit(
        &client,
        "finalizing",
        "Optimizing the local library and finishing sync",
        0,
        0,
    );
    library.optimize()?;
    report.synced_at = Utc::now().timestamp_millis();
    library.set_setting("last_sync_at", &report.synced_at.to_string())?;
    let metrics = client.metrics();
    report.duration_ms = elapsed_ms(progress.started);
    report.network_ms = metrics.network_ms;
    report.remote_requests = metrics.remote_requests;
    report.bytes_uploaded = metrics.bytes_uploaded;
    report.bytes_downloaded = metrics.bytes_downloaded;
    progress.emit(&client, "complete", "Synchronization complete", 0, 0);
    Ok(report)
}

fn sync_metadata(
    library: &Library,
    client: &PCloudClient,
    progress: &mut Progress<'_>,
) -> Result<()> {
    progress.emit(
        client,
        "metadata",
        "Reading notebook and tag metadata",
        0,
        0,
    );
    let root = client.list("/Bevernote")?;
    let remote = if root.iter().any(|file| file.name == "metadata.json") {
        progress.emit(client, "metadata", "Downloading remote metadata", 0, 0);
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
    let remote_hash = remote
        .as_ref()
        .map(|value| hash(&value.snapshot))
        .transpose()?;
    if remote_hash.as_deref() == Some(hash_bytes(&merged_bytes).as_str()) {
        return Ok(());
    }
    progress.emit(client, "metadata", "Uploading merged metadata", 0, 0);
    let envelope = MetadataEnvelope {
        revision: remote.as_ref().map_or(1, |value| value.revision + 1),
        updated_at: Utc::now().timestamp_millis(),
        device_id: library.device_id().to_owned(),
        snapshot: merged,
    };
    client.upload("/Bevernote/metadata.json", serde_json::to_vec(&envelope)?)?;
    Ok(())
}

fn sync_notes(
    library: &Library,
    client: &PCloudClient,
    report: &mut SyncReport,
    progress: &mut Progress<'_>,
) -> Result<()> {
    progress.emit(client, "discovering", "Listing remote notes", 0, 0);
    let remote_files = client.list("/Bevernote/items")?;
    let (mut remote_index, had_remote_index) = load_remote_index(client, &remote_files, progress)?;
    let original_index = remote_index.clone();
    let mut states = sync_states(library)?;
    let mut local_notes = library
        .all_notes()?
        .into_iter()
        .map(|note| (note.id.clone(), note))
        .collect::<HashMap<_, _>>();
    let mut dirty = dirty_notes(library)?;
    let remote_notes = remote_files
        .iter()
        .filter_map(|file| note_id_from_filename(&file.name).map(|id| (id, file)))
        .collect::<Vec<_>>();
    let remote_total = remote_notes.len() as u64;

    for (index, (id, file)) in remote_notes.into_iter().enumerate() {
        let state = states.get(id).cloned().unwrap_or_default();
        if remote_note_is_unchanged(id, file, &remote_index, &state) {
            if index % 100 == 0 {
                progress.emit(
                    client,
                    "checking",
                    "Remote note revisions are already synchronized",
                    index as u64 + 1,
                    remote_total,
                );
            }
            continue;
        }
        progress.emit(
            client,
            "pulling",
            format!("Downloading remote note {} of {}", index + 1, remote_total),
            index as u64 + 1,
            remote_total,
        );
        let bytes = client.download(&format!("/Bevernote/items/{}", file.name))?;
        let envelope: Envelope<Note> = serde_json::from_slice(&bytes)?;
        if envelope.id != id {
            return Err(BeverError::Remote(format!(
                "remote note identity mismatch in {}",
                file.name
            )));
        }
        remote_index.notes.insert(
            envelope.id.clone(),
            IndexedRemoteNote {
                revision: envelope.revision,
                size: bytes.len() as u64,
            },
        );
        if envelope.revision <= state.remote_revision {
            continue;
        }
        if envelope.deleted {
            if local_notes.contains_key(&envelope.id) {
                delete_remote_note(library, &envelope.id)?;
                report.pulled += 1;
            }
            let synced_hash = hash_bytes(&bytes);
            record_state(library, "note", &envelope.id, envelope.revision, &bytes)?;
            states.insert(
                envelope.id.clone(),
                SyncState {
                    remote_revision: envelope.revision,
                    synced_hash,
                },
            );
            continue;
        }
        let Some(mut remote_note) = envelope.payload else {
            return Err(BeverError::Remote(format!(
                "remote note {} has no content",
                envelope.id
            )));
        };
        remote_note.revision = envelope.revision;
        let local = local_notes.get(&envelope.id);
        let local_is_dirty = dirty.get(&envelope.id).copied().unwrap_or(false);
        if let Some(local_note) = local.filter(|_| local_is_dirty) {
            create_conflict_copy(library, local_note, &envelope.device_id)?;
            report.conflicts += 1;
        }
        let should_apply = local_is_dirty
            || local.is_none()
            || remote_note.updated_at >= local.map_or(0, |note| note.updated_at);
        if should_apply {
            progress.emit(
                client,
                "pulling",
                format!("Applying note: {}", remote_note.title),
                index as u64 + 1,
                remote_total,
            );
            download_missing_blobs(library, client, &remote_note, report, progress)?;
            apply_remote_note(library, &remote_note)?;
            report.pulled += 1;
        }
        let synced_hash = hash(&remote_note)?;
        record_state_hash(
            library,
            "note",
            &envelope.id,
            envelope.revision,
            &synced_hash,
        )?;
        states.insert(
            envelope.id.clone(),
            SyncState {
                remote_revision: envelope.revision,
                synced_hash,
            },
        );
    }

    local_notes = library
        .all_notes()?
        .into_iter()
        .map(|note| (note.id.clone(), note))
        .collect();
    dirty = dirty_notes(library)?;
    let mut pending_notes = Vec::new();
    for (id, note) in local_notes {
        let is_dirty = dirty.get(&id).copied().unwrap_or(false);
        let state = states.get(&id).cloned().unwrap_or_default();
        if is_dirty || state.synced_hash != hash(&note)? {
            pending_notes.push((id, note));
        }
    }
    pending_notes.sort_by(|(left_id, left), (right_id, right)| {
        left.title
            .cmp(&right.title)
            .then_with(|| left_id.cmp(right_id))
    });
    let pending_total = pending_notes.len() as u64;
    let mut remote_blobs = if pending_notes.is_empty() {
        HashSet::new()
    } else {
        progress.emit(
            client,
            "discovering",
            "Listing remote attachments once",
            0,
            0,
        );
        client
            .list("/Bevernote/blobs")?
            .into_iter()
            .map(|file| file.name)
            .collect()
    };
    let mut missing_uploads = HashSet::new();
    for (_, note) in &pending_notes {
        for sha in note_shas(note) {
            if !remote_blobs.contains(sha) {
                missing_uploads.insert(sha.to_owned());
            }
        }
    }
    let upload_total = missing_uploads.len() as u64;
    let mut upload_index = 0;

    for (index, (id, mut note)) in pending_notes.into_iter().enumerate() {
        progress.emit(
            client,
            "pushing",
            format!("Uploading note: {}", note.title),
            index as u64 + 1,
            pending_total,
        );
        let state = states.get(&id).cloned().unwrap_or_default();
        note.revision = note.revision.max(state.remote_revision) + 1;
        upload_missing_blobs(
            library,
            client,
            &note,
            report,
            &mut remote_blobs,
            &mut upload_index,
            upload_total,
            progress,
        )?;
        let envelope = Envelope {
            id: id.clone(),
            revision: note.revision,
            updated_at: note.updated_at,
            device_id: library.device_id().to_owned(),
            deleted: false,
            payload: Some(note.clone()),
        };
        let bytes = serde_json::to_vec(&envelope)?;
        client.upload(&format!("/Bevernote/items/note-{id}.json"), bytes.clone())?;
        remote_index.notes.insert(
            id.clone(),
            IndexedRemoteNote {
                revision: note.revision,
                size: bytes.len() as u64,
            },
        );
        let synced_hash = hash(&note)?;
        mark_note_synced(library, &id, note.revision, &synced_hash)?;
        states.insert(
            id,
            SyncState {
                remote_revision: note.revision,
                synced_hash,
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
        let state = states.get(&id).cloned().unwrap_or_default();
        let envelope: Envelope<Note> = Envelope {
            id: id.clone(),
            revision: state.remote_revision + 1,
            updated_at: deleted_at,
            device_id: library.device_id().to_owned(),
            deleted: true,
            payload: None,
        };
        let bytes = serde_json::to_vec(&envelope)?;
        if state.synced_hash == hash_bytes(&bytes) {
            continue;
        }
        progress.emit(client, "pushing", "Uploading a note deletion", 0, 0);
        client.upload(&format!("/Bevernote/items/note-{id}.json"), bytes.clone())?;
        remote_index.notes.insert(
            id.clone(),
            IndexedRemoteNote {
                revision: envelope.revision,
                size: bytes.len() as u64,
            },
        );
        record_state(library, "note", &id, envelope.revision, &bytes)?;
        report.pushed += 1;
    }

    if remote_index != original_index || (!had_remote_index && !remote_index.notes.is_empty()) {
        progress.emit(
            client,
            "finalizing",
            "Updating the remote revision index",
            0,
            0,
        );
        client.upload(REMOTE_INDEX_PATH, serde_json::to_vec(&remote_index)?)?;
    }
    Ok(())
}

fn load_remote_index(
    client: &PCloudClient,
    files: &[RemoteFile],
    progress: &mut Progress<'_>,
) -> Result<(RemoteNoteIndex, bool)> {
    if !files.iter().any(|file| file.name == "index.json") {
        return Ok((RemoteNoteIndex::default(), false));
    }
    progress.emit(
        client,
        "discovering",
        "Reading the remote revision index",
        0,
        0,
    );
    let bytes = client.download(REMOTE_INDEX_PATH)?;
    let index = serde_json::from_slice(&bytes).map_err(|error| {
        BeverError::Remote(format!(
            "remote revision index is invalid and will need to be rebuilt: {error}"
        ))
    })?;
    Ok((index, true))
}

fn note_id_from_filename(filename: &str) -> Option<&str> {
    filename
        .strip_prefix("note-")?
        .strip_suffix(".json")
        .filter(|id| !id.is_empty())
}

fn remote_note_is_unchanged(
    id: &str,
    file: &RemoteFile,
    index: &RemoteNoteIndex,
    state: &SyncState,
) -> bool {
    index
        .notes
        .get(id)
        .is_some_and(|remote| remote.revision == state.remote_revision && remote.size == file.size)
}

fn note_shas(note: &Note) -> impl Iterator<Item = &str> {
    note.resources.iter().flat_map(|resource| {
        [
            Some(resource.sha256.as_str()),
            resource.alternate_sha256.as_deref(),
        ]
        .into_iter()
        .flatten()
    })
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
    progress: &mut Progress<'_>,
) -> Result<()> {
    let mut missing = Vec::new();
    let mut seen = HashSet::new();
    for resource in &note.resources {
        for sha in [Some(&resource.sha256), resource.alternate_sha256.as_ref()]
            .into_iter()
            .flatten()
        {
            if seen.insert(sha) && !library.blob_path(sha)?.exists() {
                missing.push((
                    sha.to_owned(),
                    resource
                        .file_name
                        .clone()
                        .unwrap_or_else(|| "attachment".into()),
                ));
            }
        }
    }
    let total = missing.len() as u64;
    for (index, (sha, filename)) in missing.into_iter().enumerate() {
        progress.emit(
            client,
            "attachments",
            format!("Downloading attachment: {filename}"),
            index as u64 + 1,
            total,
        );
        let bytes = client.download(&format!("/Bevernote/blobs/{sha}"))?;
        let stored = library.store_blob(&bytes)?;
        if stored.sha256 != sha {
            return Err(BeverError::Remote(format!("blob hash mismatch for {sha}")));
        }
        report.blobs_downloaded += 1;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn upload_missing_blobs(
    library: &Library,
    client: &PCloudClient,
    note: &Note,
    report: &mut SyncReport,
    remote: &mut HashSet<String>,
    current: &mut u64,
    total: u64,
    progress: &mut Progress<'_>,
) -> Result<()> {
    for resource in &note.resources {
        for sha in [Some(&resource.sha256), resource.alternate_sha256.as_ref()]
            .into_iter()
            .flatten()
        {
            if remote.contains(sha) {
                continue;
            }
            let path = library.blob_path(sha)?;
            if !path.exists() {
                return Err(BeverError::Invalid(format!("local blob {sha} is missing")));
            }
            *current += 1;
            let filename = resource.file_name.as_deref().unwrap_or("attachment");
            progress.emit(
                client,
                "attachments",
                format!("Uploading attachment: {filename}"),
                *current,
                total,
            );
            client.upload(&format!("/Bevernote/blobs/{sha}"), fs::read(path)?)?;
            remote.insert(sha.to_owned());
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
        .prepare("SELECT id, remote_revision, synced_hash FROM sync_items WHERE kind = 'note'")?
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                SyncState {
                    remote_revision: row.get::<_, i64>(1)?,
                    synced_hash: row.get::<_, String>(2)?,
                },
            ))
        })?
        .collect::<std::result::Result<HashMap<_, _>, _>>()?;
    Ok(rows)
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

fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
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

    #[test]
    fn remote_index_skips_only_matching_revision_and_size() {
        let file = RemoteFile {
            name: "note-note-1.json".into(),
            size: 200,
        };
        let state = SyncState {
            remote_revision: 4,
            synced_hash: "hash".into(),
        };
        let mut index = RemoteNoteIndex::default();
        index.notes.insert(
            "note-1".into(),
            IndexedRemoteNote {
                revision: 4,
                size: 200,
            },
        );
        assert!(remote_note_is_unchanged("note-1", &file, &index, &state));
        index.notes.get_mut("note-1").unwrap().size = 201;
        assert!(!remote_note_is_unchanged("note-1", &file, &index, &state));
        index.notes.get_mut("note-1").unwrap().size = 200;
        index.notes.get_mut("note-1").unwrap().revision = 5;
        assert!(!remote_note_is_unchanged("note-1", &file, &index, &state));
        index.notes.get_mut("note-1").unwrap().revision = 3;
        assert!(!remote_note_is_unchanged("note-1", &file, &index, &state));
        assert!(!remote_note_is_unchanged(
            "note-1",
            &file,
            &RemoteNoteIndex::default(),
            &state
        ));
    }

    #[test]
    fn remote_note_ids_accept_only_note_envelopes() {
        assert_eq!(note_id_from_filename("note-abc.json"), Some("abc"));
        assert_eq!(note_id_from_filename("index.json"), None);
        assert_eq!(note_id_from_filename("note-.json"), None);
    }
}
