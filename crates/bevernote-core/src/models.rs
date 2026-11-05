use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Stack {
    pub id: String,
    pub name: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub revision: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Notebook {
    pub id: String,
    pub name: String,
    pub stack_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub revision: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub color: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Shortcut {
    pub id: String,
    pub target_type: String,
    pub target_id: String,
    pub label: String,
    pub sort_order: i64,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Resource {
    pub id: String,
    pub note_id: String,
    pub sha256: String,
    pub md5: String,
    pub mime: String,
    pub file_name: Option<String>,
    pub size: u64,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub duration: Option<u64>,
    pub recognition_xml: Option<String>,
    pub recognition_text: Option<String>,
    pub attributes: Value,
    pub alternate_sha256: Option<String>,
    pub position: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub id: String,
    pub notebook_id: String,
    pub title: String,
    pub content_html: String,
    pub original_enml: Option<String>,
    pub plain_text: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
    pub attributes: Value,
    pub evernote_guid: Option<String>,
    pub revision: i64,
    pub tags: Vec<String>,
    pub resources: Vec<Resource>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NoteSummary {
    pub id: String,
    pub notebook_id: String,
    pub notebook_name: String,
    pub title: String,
    pub snippet: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
    pub tags: Vec<String>,
    pub has_attachments: bool,
    pub is_shortcut: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveNote {
    pub id: String,
    pub notebook_id: String,
    pub title: String,
    pub content_html: String,
    pub attributes: Value,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum NoteSort {
    Relevance,
    #[default]
    UpdatedDesc,
    UpdatedAsc,
    CreatedDesc,
    CreatedAsc,
    TitleAsc,
    TitleDesc,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteFilter {
    #[serde(default)]
    pub query: String,
    pub notebook_id: Option<String>,
    pub stack_id: Option<String>,
    #[serde(default)]
    pub tag_ids: Vec<String>,
    #[serde(default)]
    pub include_trash: bool,
    #[serde(default)]
    pub trash_only: bool,
    pub created_from: Option<i64>,
    pub created_to: Option<i64>,
    pub updated_from: Option<i64>,
    pub updated_to: Option<i64>,
    pub contains: Option<String>,
    #[serde(default)]
    pub sort: NoteSort,
    #[serde(default = "default_limit")]
    pub limit: u32,
    #[serde(default)]
    pub offset: u32,
}

const fn default_limit() -> u32 {
    100
}

impl Default for NoteFilter {
    fn default() -> Self {
        Self {
            query: String::new(),
            notebook_id: None,
            stack_id: None,
            tag_ids: Vec::new(),
            include_trash: false,
            trash_only: false,
            created_from: None,
            created_to: None,
            updated_from: None,
            updated_to: None,
            contains: None,
            sort: NoteSort::UpdatedDesc,
            limit: default_limit(),
            offset: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LibrarySnapshot {
    pub stacks: Vec<Stack>,
    pub notebooks: Vec<Notebook>,
    pub tags: Vec<Tag>,
    pub shortcuts: Vec<Shortcut>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub notes_imported: u64,
    pub notes_skipped: u64,
    pub resources_imported: u64,
    pub bytes_imported: u64,
    pub notebook_id: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportProgress {
    pub phase: String,
    pub notebook_id: String,
    pub note_index: u64,
    pub note_title: Option<String>,
    pub notes_imported: u64,
    pub notes_skipped: u64,
    pub resources_imported: u64,
    pub bytes_read: u64,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SyncReport {
    pub pushed: u64,
    pub pulled: u64,
    pub conflicts: u64,
    pub blobs_uploaded: u64,
    pub blobs_downloaded: u64,
    pub synced_at: i64,
    pub duration_ms: u64,
    pub network_ms: u64,
    pub remote_requests: u64,
    pub bytes_uploaded: u64,
    pub bytes_downloaded: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncProgress {
    pub phase: String,
    pub message: String,
    pub current: u64,
    pub total: u64,
    pub elapsed_ms: u64,
    pub network_ms: u64,
    pub remote_requests: u64,
    pub bytes_uploaded: u64,
    pub bytes_downloaded: u64,
}
