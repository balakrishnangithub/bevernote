//! Native pCloud OAuth callbacks for Android.
//!
//! Android cannot use the desktop loopback listener. pCloud returns to the
//! `bevernote://oauth/pcloud` deep link, with implicit-grant credentials in the
//! URL fragment. Callback URLs and tokens are never logged or emitted.
use bevernote_core::{
    error::{BeverError, Result as BeverResult},
    Library,
};
use tauri::{App, AppHandle, Emitter, Manager};
use tauri_plugin_deep_link::DeepLinkExt;
use tauri_plugin_opener::OpenerExt;
use url::Url;

use crate::{
    pcloud_authorization_url, pcloud_callback_parameters, pcloud_status_inner, save_credentials,
    status_for_saved_credentials, AppState, PCloudStatus,
};

pub fn begin(app: &AppHandle, library: &Library) -> BeverResult<PCloudStatus> {
    let expected_state = uuid::Uuid::new_v4().to_string();
    library.set_setting("pcloud_oauth_state", &expected_state)?;
    let authorization =
        pcloud_authorization_url(&expected_state, bevernote_core::pcloud::MOBILE_REDIRECT_URI);
    if let Err(error) = app.opener().open_url(authorization.as_str(), None::<&str>) {
        let _ = library.set_setting("pcloud_oauth_state", "");
        return Err(BeverError::Invalid(format!(
            "cannot open the browser for pCloud authorization: {error}"
        )));
    }
    pcloud_status_inner(library)
}

pub fn register(app: &App) -> Result<(), Box<dyn std::error::Error>> {
    let handle = app.handle().clone();
    // Register first so a warm callback cannot race with cold-link retrieval.
    app.deep_link().on_open_url(move |event| {
        for url in event.urls() {
            dispatch(handle.clone(), url);
        }
    });
    if let Some(urls) = app.deep_link().get_current()? {
        for url in urls {
            dispatch(app.handle().clone(), url);
        }
    }
    Ok(())
}

fn dispatch(app: AppHandle, url: Url) {
    let _ = tauri::async_runtime::spawn_blocking(move || match complete(&app, &url) {
        Ok(Some(status)) => {
            let _ = app.emit("bevernote:pcloud-status", &status);
        }
        Ok(None) => {}
        Err(error) => {
            let _ = app.emit("bevernote:pcloud-error", error.to_string());
        }
    });
}

fn complete(app: &AppHandle, url: &Url) -> BeverResult<Option<PCloudStatus>> {
    let library = &app.state::<AppState>().library;
    let Some(expected_state) = library
        .setting("pcloud_oauth_state")?
        .filter(|state| !state.is_empty())
    else {
        // A cold link may be delivered twice. Once consumed, it must not be
        // allowed to overwrite newer credentials or report a stale error.
        return Ok(None);
    };
    let parameters = pcloud_callback_parameters(url)?;
    let credentials = crate::parse_loopback_credentials(&parameters, &expected_state)?;
    save_credentials(library, &credentials, None)?;
    status_for_saved_credentials(library, &credentials, None).map(Some)
}
