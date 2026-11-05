//! Minimal native lifetime adapter for the shared synchronization engine.
//!
//! Only Android needs a foreground service. Desktop uses its tray lifecycle;
//! future platforms can extend this adapter without changing the sync engine.
#[cfg(target_os = "android")]
use bevernote_core::{
    error::{BeverError, Result as BeverResult},
    models::SyncProgress,
};
#[cfg(target_os = "android")]
use serde::Serialize;
#[cfg(target_os = "android")]
use tauri::plugin::{PluginApi, PluginHandle};

#[cfg(target_os = "android")]
pub(crate) struct SyncPlatform {
    handle: PluginHandle<tauri::Wry>,
}

#[cfg(target_os = "android")]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ServiceMessage {
    message: String,
    current: u64,
    total: u64,
}

#[cfg(target_os = "android")]
impl SyncPlatform {
    pub(crate) fn register<C: serde::de::DeserializeOwned>(
        api: PluginApi<tauri::Wry, C>,
    ) -> BeverResult<Self> {
        let handle = api
            .register_android_plugin("app.bevernote.desktop", "SyncPlugin")
            .map_err(|error| BeverError::Invalid(error.to_string()))?;
        Ok(Self { handle })
    }

    pub(crate) fn start(&self) -> BeverResult<()> {
        self.handle
            .run_mobile_plugin::<()>(
                "start",
                ServiceMessage {
                    message: "Preparing synchronization…".into(),
                    current: 0,
                    total: 0,
                },
            )
            .map_err(|error| {
                BeverError::Invalid(format!("cannot start Android background sync: {error}"))
            })
    }

    pub(crate) fn progress(&self, progress: &SyncProgress) {
        let _ = self.handle.run_mobile_plugin::<()>(
            "progress",
            ServiceMessage {
                message: progress.message.clone(),
                current: progress.current,
                total: progress.total,
            },
        );
    }

    pub(crate) fn finish(&self, message: &str) {
        let _ = self.handle.run_mobile_plugin::<()>(
            "finish",
            ServiceMessage {
                message: message.into(),
                current: 0,
                total: 0,
            },
        );
    }
}

#[cfg(not(target_os = "android"))]
pub(crate) struct SyncPlatform;

#[cfg(not(target_os = "android"))]
impl SyncPlatform {
    pub(crate) fn start(&self) -> bevernote_core::Result<()> {
        Ok(())
    }

    pub(crate) fn progress(&self, _progress: &bevernote_core::models::SyncProgress) {}

    pub(crate) fn finish(&self, _message: &str) {}
}
