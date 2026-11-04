use std::{
    fs::File,
    io::{BufReader, BufWriter, Write},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use bevernote_core::{
    enex::{export_enex, import_enex_with_progress},
    error::{BeverError, Result as BeverResult},
    models::{
        ImportProgress, ImportReport, LibrarySnapshot, Note, NoteFilter, NoteSummary, SaveNote,
        SyncReport,
    },
    pcloud::{PCloudClient, PCloudCredentials, DEFAULT_CLIENT_ID, DEFAULT_REDIRECT_URI},
    sync::sync_with_pcloud,
    Library,
};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;
use url::Url;

const KEYRING_SERVICE: &str = "app.bevernote.desktop";
const KEYRING_ACCOUNT: &str = "pcloud";

struct AppState {
    library: Arc<Library>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PCloudStatus {
    connected: bool,
    email: Option<String>,
    hostname: String,
    last_sync_at: Option<i64>,
}

fn string_error<T>(result: bevernote_core::Result<T>) -> Result<T, String> {
    result.map_err(|error| error.to_string())
}

#[tauri::command]
fn bootstrap(state: State<'_, AppState>) -> Result<LibrarySnapshot, String> {
    string_error(state.library.snapshot())
}

#[tauri::command]
fn list_notes(state: State<'_, AppState>, filter: NoteFilter) -> Result<Vec<NoteSummary>, String> {
    string_error(state.library.list_notes(&filter))
}

#[tauri::command]
fn get_note(state: State<'_, AppState>, id: String) -> Result<Note, String> {
    string_error(state.library.get_note(&id))
}

#[tauri::command]
fn create_note(state: State<'_, AppState>, notebook_id: Option<String>) -> Result<Note, String> {
    string_error(state.library.create_note(notebook_id.as_deref()))
}

#[tauri::command]
fn save_note(state: State<'_, AppState>, input: SaveNote) -> Result<Note, String> {
    string_error(state.library.save_note(&input))
}

#[tauri::command]
fn set_note_trashed(state: State<'_, AppState>, id: String, trashed: bool) -> Result<(), String> {
    string_error(state.library.set_note_trashed(&id, trashed))
}

#[tauri::command]
fn delete_note_permanently(state: State<'_, AppState>, id: String) -> Result<(), String> {
    string_error(state.library.delete_note_permanently(&id))
}

#[tauri::command]
fn create_notebook(
    state: State<'_, AppState>,
    name: String,
    stack_id: Option<String>,
) -> Result<LibrarySnapshot, String> {
    string_error(
        state
            .library
            .ensure_notebook(&name, stack_id.as_deref())
            .and_then(|_| state.library.snapshot()),
    )
}

#[tauri::command]
fn set_notebook_stack(
    state: State<'_, AppState>,
    notebook_id: String,
    stack_id: Option<String>,
) -> Result<LibrarySnapshot, String> {
    string_error(
        state
            .library
            .set_notebook_stack(&notebook_id, stack_id.as_deref())
            .and_then(|_| state.library.snapshot()),
    )
}

#[tauri::command]
fn create_stack(state: State<'_, AppState>, name: String) -> Result<LibrarySnapshot, String> {
    string_error(
        state
            .library
            .create_stack(&name)
            .and_then(|_| state.library.snapshot()),
    )
}

#[tauri::command]
fn create_tag(
    state: State<'_, AppState>,
    name: String,
    parent_id: Option<String>,
) -> Result<LibrarySnapshot, String> {
    string_error(
        state
            .library
            .create_tag(&name, parent_id.as_deref())
            .and_then(|_| state.library.snapshot()),
    )
}

#[tauri::command]
fn toggle_shortcut(
    state: State<'_, AppState>,
    target_type: String,
    target_id: String,
    label: String,
) -> Result<LibrarySnapshot, String> {
    string_error(
        state
            .library
            .toggle_shortcut(&target_type, &target_id, &label)
            .and_then(|_| state.library.snapshot()),
    )
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ImportEvent {
    import_id: String,
    #[serde(flatten)]
    progress: ImportProgress,
}

#[tauri::command]
async fn import_enex_file(
    app: AppHandle,
    state: State<'_, AppState>,
    path: PathBuf,
    notebook_name: Option<String>,
    import_id: String,
) -> Result<ImportReport, String> {
    let library = Arc::clone(&state.library);
    tauri::async_runtime::spawn_blocking(move || {
        import_enex_with_progress(&library, path, notebook_name.as_deref(), move |progress| {
            let _ = app.emit(
                "bevernote:import-progress",
                ImportEvent {
                    import_id: import_id.clone(),
                    progress,
                },
            );
        })
    })
    .await
    .map_err(|error| error.to_string())
    .and_then(string_error)
}

#[tauri::command]
fn organize_notes(
    state: State<'_, AppState>,
    note_ids: Vec<String>,
    action: String,
    notebook_id: Option<String>,
    tags: Vec<String>,
) -> Result<u64, String> {
    let result = match action.as_str() {
        "move" | "copy" => {
            let notebook_id = notebook_id
                .filter(|id| !id.is_empty())
                .ok_or_else(|| "Choose a destination notebook.".to_owned())?;
            if action == "move" {
                state.library.move_notes(&note_ids, &notebook_id)
            } else {
                state.library.copy_notes(&note_ids, &notebook_id)
            }
        }
        "addTags" => state.library.add_tags_to_notes(&note_ids, &tags),
        _ => Err(BeverError::Invalid(
            "unknown note organization action".into(),
        )),
    };
    string_error(result)
}

#[tauri::command]
fn update_note_timestamps(
    state: State<'_, AppState>,
    id: String,
    created_at: i64,
    updated_at: i64,
) -> Result<Note, String> {
    string_error(
        state
            .library
            .update_note_timestamps(&id, created_at, updated_at),
    )
}

#[tauri::command]
fn ocr_enabled(state: State<'_, AppState>) -> Result<bool, String> {
    string_error(
        state
            .library
            .setting("ocr_enabled")
            .map(|value| value.as_deref() != Some("false")),
    )
}

#[tauri::command]
fn set_ocr_enabled(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    string_error(
        state
            .library
            .set_setting("ocr_enabled", if enabled { "true" } else { "false" }),
    )
}

#[tauri::command]
fn export_enex_file(
    state: State<'_, AppState>,
    note_ids: Vec<String>,
    path: PathBuf,
) -> Result<(), String> {
    let notes = string_error(
        note_ids
            .iter()
            .map(|id| state.library.get_note(id))
            .collect::<bevernote_core::Result<Vec<_>>>(),
    )?;
    string_error(export_enex(&state.library, &notes, path))
}

#[tauri::command]
fn attach_file(state: State<'_, AppState>, note_id: String, path: PathBuf) -> Result<Note, String> {
    string_error(
        state
            .library
            .attach_file(&note_id, path)
            .and_then(|_| state.library.get_note(&note_id)),
    )
}

fn safe_attachment_name(name: Option<&str>) -> String {
    let cleaned = name
        .unwrap_or("attachment")
        .chars()
        .map(|character| {
            if matches!(
                character,
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
            ) || character.is_control()
            {
                '_'
            } else {
                character
            }
        })
        .collect::<String>();
    let cleaned = cleaned.trim().trim_matches(['.', ' ']).trim();
    let mut name = if cleaned.is_empty() {
        "attachment".to_owned()
    } else {
        cleaned.chars().take(160).collect()
    };
    let stem = name.split('.').next().unwrap_or("").to_ascii_uppercase();
    if matches!(
        stem.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    ) {
        name.insert(0, '_');
    }
    name
}

fn copy_attachment(library: &Library, resource_id: &str, destination: &Path) -> BeverResult<()> {
    let resource = library.resource(resource_id)?;
    let source = File::open(library.blob_dir().join(&resource.sha256))?;
    let target = File::create(destination)?;
    let mut reader = BufReader::new(source);
    let mut writer = BufWriter::new(target);
    std::io::copy(&mut reader, &mut writer)?;
    writer.flush()?;
    Ok(())
}

#[tauri::command]
fn open_attachment(app: AppHandle, resource_id: String) -> Result<(), String> {
    let result = (|| {
        let state = app.state::<AppState>();
        let resource = state.library.resource(&resource_id)?;
        let directory = app
            .path()
            .app_cache_dir()
            .map_err(|error| {
                BeverError::Invalid(format!("cannot locate attachment cache: {error}"))
            })?
            .join("attachments")
            .join(&resource.id);
        std::fs::create_dir_all(&directory)?;
        let destination = directory.join(safe_attachment_name(resource.file_name.as_deref()));
        copy_attachment(&state.library, &resource.id, &destination)?;
        app.opener()
            .open_path(destination.to_string_lossy(), None::<&str>)
            .map_err(|error| BeverError::Invalid(format!("cannot open attachment: {error}")))
    })();
    string_error(result)
}

#[tauri::command]
fn save_attachment(
    state: State<'_, AppState>,
    resource_id: String,
    destination: PathBuf,
) -> Result<(), String> {
    string_error(copy_attachment(&state.library, &resource_id, &destination))
}

#[tauri::command]
async fn sync_now(state: State<'_, AppState>) -> Result<SyncReport, String> {
    let library = Arc::clone(&state.library);
    tauri::async_runtime::spawn_blocking(move || {
        let credentials = load_credentials(&library)?;
        let report = sync_with_pcloud(&library, credentials)?;
        Ok(report)
    })
    .await
    .map_err(|error| error.to_string())
    .and_then(string_error)
}

#[tauri::command]
async fn pcloud_status(state: State<'_, AppState>) -> Result<PCloudStatus, String> {
    string_error(pcloud_status_inner(&state.library))
}

fn pcloud_status_inner(library: &Library) -> BeverResult<PCloudStatus> {
    let hostname = library
        .setting("pcloud_hostname")?
        .unwrap_or_else(|| "api.pcloud.com".to_owned());
    Ok(PCloudStatus {
        connected: read_access_token()?.is_some(),
        email: library.setting("pcloud_email")?,
        last_sync_at: library
            .setting("last_sync_at")?
            .and_then(|value| value.parse().ok()),
        hostname,
    })
}

#[tauri::command]
async fn connect_pcloud(app: AppHandle) -> Result<PCloudStatus, String> {
    let server = tiny_http::Server::http("127.0.0.1:53682").map_err(|error| {
        format!("cannot start OAuth loopback listener; close other Bevernote connection attempts and retry: {error}")
    })?;
    let expected_state = uuid::Uuid::new_v4().to_string();
    let mut authorization = Url::parse("https://my.pcloud.com/oauth2/authorize")
        .expect("valid pCloud authorization URL");
    authorization
        .query_pairs_mut()
        .append_pair("client_id", DEFAULT_CLIENT_ID)
        .append_pair("response_type", "token")
        .append_pair("redirect_uri", DEFAULT_REDIRECT_URI)
        .append_pair("state", &expected_state);
    app.opener()
        .open_url(authorization.as_str(), None::<&str>)
        .map_err(|error| error.to_string())?;
    let credentials = tauri::async_runtime::spawn_blocking(move || {
        receive_loopback_credentials(server, &expected_state)
    })
    .await
    .map_err(|error| error.to_string())
    .and_then(string_error)?;
    let library = Arc::clone(&app.state::<AppState>().library);
    let status = tauri::async_runtime::spawn_blocking(move || {
        // The OAuth provider has already authorized this token. Do not make
        // connection completion depend on an optional profile/network request.
        save_credentials(&library, &credentials, None)?;
        status_for_saved_credentials(&library, &credentials, None)
    })
    .await
    .map_err(|error| error.to_string())
    .and_then(string_error)?;
    let _ = app.emit("bevernote:pcloud-status", &status);
    Ok(status)
}

#[tauri::command]
async fn connect_pcloud_token(
    state: State<'_, AppState>,
    access_token: String,
    hostname: String,
) -> Result<PCloudStatus, String> {
    let credentials = PCloudCredentials {
        access_token: access_token.trim().to_owned(),
        hostname,
    };
    if credentials.access_token.is_empty() {
        return Err("Enter a pCloud access token.".into());
    }
    let client = string_error(PCloudClient::new(credentials.clone()))?;
    let library = Arc::clone(&state.library);
    tauri::async_runtime::spawn_blocking(move || {
        let email = client.user_email()?;
        save_credentials(&library, &credentials, email.as_deref())?;
        status_for_saved_credentials(&library, &credentials, email)
    })
    .await
    .map_err(|error| error.to_string())
    .and_then(string_error)
}

#[tauri::command]
async fn disconnect_pcloud(state: State<'_, AppState>) -> Result<PCloudStatus, String> {
    string_error(disconnect_pcloud_inner(&state.library))
}

fn disconnect_pcloud_inner(library: &Library) -> BeverResult<PCloudStatus> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT)
        .map_err(keyring_error)?
        .delete_credential()
        .map_err(keyring_error)?;
    library.set_setting("pcloud_email", "")?;
    pcloud_status_inner(library)
}

fn save_credentials(
    library: &Library,
    credentials: &PCloudCredentials,
    email: Option<&str>,
) -> BeverResult<()> {
    credentials.api_base()?;
    if credentials.access_token.trim().is_empty() {
        return Err(BeverError::Invalid(
            "pCloud returned an empty access token".into(),
        ));
    }
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT)
        .map_err(keyring_error)?
        .set_password(&credentials.access_token)
        .map_err(keyring_error)?;
    let saved_token = keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT)
        .map_err(keyring_error)?
        .get_password()
        .map_err(keyring_error)?;
    if saved_token != credentials.access_token {
        return Err(BeverError::Invalid(
            "secure credential storage could not retain the pCloud access token; check your system credential manager and reconnect".into(),
        ));
    }
    library.set_setting("pcloud_hostname", &credentials.hostname)?;
    library.set_setting("pcloud_email", email.unwrap_or(""))?;
    Ok(())
}

fn status_for_saved_credentials(
    library: &Library,
    credentials: &PCloudCredentials,
    email: Option<String>,
) -> BeverResult<PCloudStatus> {
    Ok(PCloudStatus {
        connected: true,
        email,
        hostname: credentials.hostname.clone(),
        last_sync_at: library
            .setting("last_sync_at")?
            .and_then(|value| value.parse().ok()),
    })
}

fn read_access_token() -> BeverResult<Option<String>> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT).map_err(keyring_error)?;
    match entry.get_password() {
        Ok(token) if !token.trim().is_empty() => Ok(Some(token)),
        Ok(_) | Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(keyring_error(error)),
    }
}

fn load_credentials(library: &Library) -> BeverResult<PCloudCredentials> {
    let access_token = read_access_token()?
        .ok_or_else(|| BeverError::Invalid("connect pCloud before synchronizing".into()))?;
    let hostname = library
        .setting("pcloud_hostname")?
        .unwrap_or_else(|| "api.pcloud.com".to_owned());
    Ok(PCloudCredentials {
        access_token,
        hostname,
    })
}

fn keyring_error(error: keyring::Error) -> BeverError {
    BeverError::Invalid(format!("secure credential storage: {error}"))
}

fn receive_loopback_credentials(
    server: tiny_http::Server,
    expected_state: &str,
) -> BeverResult<PCloudCredentials> {
    let deadline = std::time::Instant::now() + Duration::from_secs(300);
    while std::time::Instant::now() < deadline {
        let Some(mut request) = server
            .recv_timeout(Duration::from_secs(1))
            .map_err(BeverError::Io)?
        else {
            continue;
        };
        let url = Url::parse(&format!("http://127.0.0.1:53682{}", request.url()))
            .map_err(|error| BeverError::Invalid(error.to_string()))?;
        if url.path() == "/" {
            let response = tiny_http::Response::from_string(
                r#"<!doctype html><meta charset="utf-8"><meta name="referrer" content="no-referrer"><title>Connecting Bevernote</title><p>Completing pCloud connection…</p><script>const message=document.querySelector('p');if(location.hash){const body=new URLSearchParams(location.hash.slice(1));history.replaceState(null,'','/');fetch('/callback',{method:'POST',headers:{'Content-Type':'application/x-www-form-urlencoded'},body}).then(async response=>{message.textContent=await response.text()}).catch(()=>{message.textContent='Authorization could not reach Bevernote. Return to the app and retry.'})}else{message.textContent='Return to Bevernote to continue.'}</script>"#,
            )
            .with_header(
                tiny_http::Header::from_bytes("Content-Type", "text/html; charset=utf-8")
                    .expect("valid static response header"),
            )
            .with_header(
                tiny_http::Header::from_bytes("Cache-Control", "no-store")
                    .expect("valid static response header"),
            );
            request.respond(response).map_err(BeverError::Io)?;
            continue;
        }
        if url.path() != "/callback" {
            continue;
        }
        let parameters = if request.method() == &tiny_http::Method::Post {
            let mut body = String::new();
            request
                .as_reader()
                .read_to_string(&mut body)
                .map_err(BeverError::Io)?;
            url::form_urlencoded::parse(body.as_bytes())
                .into_owned()
                .collect::<std::collections::HashMap<_, _>>()
        } else {
            url.query_pairs()
                .into_owned()
                .collect::<std::collections::HashMap<_, _>>()
        };
        let credentials = parse_loopback_credentials(&parameters, expected_state);
        let response = if credentials.is_ok() {
            "pCloud authorization received. Return to Bevernote to finish connecting."
        } else {
            "Bevernote could not verify this authorization. Return to the app and try again."
        };
        request
            .respond(
                tiny_http::Response::from_string(response).with_header(
                    tiny_http::Header::from_bytes("Cache-Control", "no-store")
                        .expect("valid static response header"),
                ),
            )
            .map_err(BeverError::Io)?;
        return credentials;
    }
    Err(BeverError::Invalid(
        "pCloud connection timed out; enable implicit grant or use an access token".into(),
    ))
}

fn parse_loopback_credentials(
    parameters: &std::collections::HashMap<String, String>,
    expected_state: &str,
) -> BeverResult<PCloudCredentials> {
    if parameters.get("state").map(String::as_str) != Some(expected_state) {
        return Err(BeverError::Invalid(
            "pCloud OAuth state did not match".into(),
        ));
    }
    let access_token = parameters
        .get("access_token")
        .filter(|token| !token.is_empty())
        .cloned()
        .ok_or_else(|| BeverError::Invalid("pCloud did not return an access token; enable implicit grant in the pCloud console".into()))?;
    let hostname = parameters
        .get("hostname")
        .map(String::as_str)
        .or_else(|| match parameters.get("locationid").map(String::as_str) {
            Some("1") => Some("api.pcloud.com"),
            Some("2") => Some("eapi.pcloud.com"),
            _ => None,
        })
        .unwrap_or("api.pcloud.com")
        .to_owned();
    let credentials = PCloudCredentials {
        access_token,
        hostname,
    };
    credentials.api_base()?;
    Ok(credentials)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .register_uri_scheme_protocol("bever", |context, request| {
            let state = context.app_handle().state::<AppState>();
            let reference = request.uri().path().trim_matches('/').to_owned();
            let result = state
                .library
                .resource_location(&reference)
                .and_then(|resource| {
                    state.library.read_blob(&resource.sha256).map(|bytes| {
                        let filename = safe_attachment_name(resource.file_name.as_deref());
                        let encoded = url::form_urlencoded::byte_serialize(filename.as_bytes())
                            .collect::<String>();
                        (bytes, resource.mime, encoded)
                    })
                });
            match result {
                Ok((bytes, mime, filename)) => tauri::http::Response::builder()
                    .header("Content-Type", mime)
                    .header(
                        "Content-Disposition",
                        format!("inline; filename=\"attachment\"; filename*=UTF-8''{filename}"),
                    )
                    .header("Access-Control-Allow-Origin", "*")
                    .header("Content-Security-Policy", "default-src 'none'; sandbox")
                    .header("X-Content-Type-Options", "nosniff")
                    .header("Cache-Control", "private, max-age=31536000, immutable")
                    .body(bytes)
                    .expect("valid resource response"),
                Err(error) => tauri::http::Response::builder()
                    .status(404)
                    .body(error.to_string().into_bytes())
                    .expect("valid error response"),
            }
        })
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let library = Arc::new(Library::open(data_dir)?);
            app.manage(AppState { library });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            bootstrap,
            list_notes,
            get_note,
            create_note,
            save_note,
            set_note_trashed,
            delete_note_permanently,
            create_notebook,
            set_notebook_stack,
            create_stack,
            create_tag,
            toggle_shortcut,
            import_enex_file,
            organize_notes,
            update_note_timestamps,
            ocr_enabled,
            set_ocr_enabled,
            export_enex_file,
            attach_file,
            open_attachment,
            save_attachment,
            sync_now,
            pcloud_status,
            connect_pcloud,
            connect_pcloud_token,
            disconnect_pcloud,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Bevernote");
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{parse_loopback_credentials, safe_attachment_name};

    fn callback_parameters() -> HashMap<String, String> {
        HashMap::from([
            ("access_token".into(), "test-token".into()),
            ("state".into(), "expected-state".into()),
        ])
    }

    #[test]
    fn oauth_callback_uses_the_returned_api_region() {
        let mut parameters = callback_parameters();
        parameters.insert("hostname".into(), "eapi.pcloud.com".into());
        let credentials = parse_loopback_credentials(&parameters, "expected-state").unwrap();
        assert_eq!(credentials.hostname, "eapi.pcloud.com");
        assert_eq!(credentials.access_token, "test-token");

        parameters.remove("hostname");
        parameters.insert("locationid".into(), "2".into());
        let credentials = parse_loopback_credentials(&parameters, "expected-state").unwrap();
        assert_eq!(credentials.hostname, "eapi.pcloud.com");
    }

    #[test]
    fn oauth_callback_rejects_invalid_state_and_api_hosts() {
        let mut parameters = callback_parameters();
        assert!(parse_loopback_credentials(&parameters, "different-state").is_err());
        parameters.insert("hostname".into(), "example.com".into());
        assert!(parse_loopback_credentials(&parameters, "expected-state").is_err());
    }

    #[test]
    fn oauth_loopback_callback_returns_credentials_without_profile_lookup() {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let address = server.server_addr().to_string();
        let receiver = std::thread::spawn(move || {
            super::receive_loopback_credentials(server, "expected-state")
        });
        let body = "access_token=test-token&state=expected-state&hostname=eapi.pcloud.com";
        let request = format!(
            "POST /callback HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/x-www-form-urlencoded\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(), body
        );
        let mut stream = std::net::TcpStream::connect(address).unwrap();
        std::io::Write::write_all(&mut stream, request.as_bytes()).unwrap();
        let mut response = String::new();
        std::io::Read::read_to_string(&mut stream, &mut response).unwrap();
        assert!(response.contains("pCloud authorization received"));
        let credentials = receiver.join().unwrap().unwrap();
        assert_eq!(credentials.access_token, "test-token");
        assert_eq!(credentials.hostname, "eapi.pcloud.com");
    }

    #[test]
    fn attachment_names_are_safe_and_keep_their_extension() {
        assert_eq!(
            safe_attachment_name(Some("Quarterly report.pdf")),
            "Quarterly report.pdf"
        );
        assert_eq!(
            safe_attachment_name(Some("../bad\\name:.pdf")),
            "_bad_name_.pdf"
        );
        assert_eq!(safe_attachment_name(Some("CON.txt")), "_CON.txt");
        assert_eq!(safe_attachment_name(Some("...")), "attachment");
        assert_eq!(safe_attachment_name(None), "attachment");
        assert_eq!(safe_attachment_name(Some("a\0b.txt")), "a_b.txt");
    }
}
