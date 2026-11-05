//! Process-wide ownership of the single pCloud synchronization worker.
//!
//! The worker deliberately stays in this process: closing a window never owns
//! or cancels the engine. This controller only serializes claims and snapshots.
use std::sync::Mutex;

use bevernote_core::models::{SyncProgress, SyncReport};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SyncStatus {
    pub(crate) revision: u64,
    pub(crate) run_id: u64,
    pub(crate) running: bool,
    pub(crate) progress: SyncProgress,
    pub(crate) last_report: Option<SyncReport>,
    pub(crate) error: Option<String>,
}

impl Default for SyncStatus {
    fn default() -> Self {
        Self {
            revision: 0,
            run_id: 0,
            running: false,
            progress: SyncProgress {
                phase: "idle".into(),
                message: "Synchronize when you are ready".into(),
                current: 0,
                total: 0,
                elapsed_ms: 0,
                network_ms: 0,
                remote_requests: 0,
                bytes_uploaded: 0,
                bytes_downloaded: 0,
            },
            last_report: None,
            error: None,
        }
    }
}

#[derive(Debug, Default)]
struct State {
    status: SyncStatus,
    prepared: bool,
    worker_claimed: bool,
}

#[derive(Debug, Default)]
pub(crate) struct SyncController {
    state: Mutex<State>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Preparation {
    pub(crate) run_id: u64,
    pub(crate) should_start: bool,
}

impl SyncController {
    pub(crate) fn status(&self) -> SyncStatus {
        self.state
            .lock()
            .expect("sync mutex poisoned")
            .status
            .clone()
    }

    pub(crate) fn is_running(&self) -> bool {
        self.status().running
    }

    /// Atomically reserves a run. A repeated call returns the current run but
    /// never resets its progress or grants another worker.
    pub(crate) fn prepare(&self) -> Preparation {
        let mut state = self.state.lock().expect("sync mutex poisoned");
        if state.status.running {
            return Preparation {
                run_id: state.status.run_id,
                should_start: false,
            };
        }
        let run_id = state.status.run_id.saturating_add(1).max(1);
        state.status = SyncStatus {
            revision: state.status.revision.saturating_add(1),
            run_id,
            running: true,
            progress: SyncProgress {
                phase: "preparing".into(),
                message: "Preparing synchronization…".into(),
                ..state.status.progress.clone()
            },
            last_report: None,
            error: None,
        };
        state.prepared = true;
        state.worker_claimed = false;
        Preparation {
            run_id,
            should_start: true,
        }
    }

    pub(crate) fn claim_worker(&self, run_id: u64) -> bool {
        let mut state = self.state.lock().expect("sync mutex poisoned");
        if !state.prepared || state.worker_claimed || state.status.run_id != run_id {
            return false;
        }
        state.prepared = false;
        state.worker_claimed = true;
        true
    }

    pub(crate) fn update(&self, run_id: u64, progress: SyncProgress) -> Option<SyncStatus> {
        let mut state = self.state.lock().expect("sync mutex poisoned");
        if state.status.run_id != run_id || !state.status.running {
            return None;
        }
        state.status.progress = progress;
        state.status.revision = state.status.revision.saturating_add(1);
        Some(state.status.clone())
    }

    pub(crate) fn finish(
        &self,
        run_id: u64,
        result: Result<SyncReport, String>,
    ) -> Option<SyncStatus> {
        let mut state = self.state.lock().expect("sync mutex poisoned");
        if state.status.run_id != run_id || !state.status.running {
            return None;
        }
        match result {
            Ok(report) => state.status.last_report = Some(report),
            Err(error) => state.status.error = Some(error),
        }
        state.status.running = false;
        state.status.revision = state.status.revision.saturating_add(1);
        state.prepared = false;
        state.worker_claimed = false;
        Some(state.status.clone())
    }
}

#[cfg(test)]
mod tests {
    use bevernote_core::models::SyncReport;

    use super::SyncController;

    #[test]
    fn duplicate_preparation_and_worker_claims_do_not_restart_sync() {
        let controller = SyncController::default();
        let first = controller.prepare();
        let duplicate = controller.prepare();
        assert!(first.should_start);
        assert!(!duplicate.should_start);
        assert_eq!(first.run_id, duplicate.run_id);
        assert!(controller.claim_worker(first.run_id));
        assert!(!controller.claim_worker(first.run_id));
        assert_eq!(controller.prepare().run_id, first.run_id);
    }

    #[test]
    fn terminal_state_survives_worker_exit_and_next_run_resets_it() {
        let controller = SyncController::default();
        let first = controller.prepare();
        assert!(controller.claim_worker(first.run_id));
        let report = SyncReport {
            pushed: 3,
            ..SyncReport::default()
        };
        controller.finish(first.run_id, Ok(report.clone())).unwrap();
        assert!(!controller.status().running);
        assert_eq!(controller.status().last_report, Some(report));
        let second = controller.prepare();
        assert_ne!(first.run_id, second.run_id);
        assert!(controller.status().running);
        assert_eq!(controller.status().last_report, None);
        assert_eq!(controller.status().error, None);
        controller
            .finish(second.run_id, Err("network unavailable".into()))
            .unwrap();
        assert_eq!(
            controller.status().error.as_deref(),
            Some("network unavailable")
        );
    }

    #[test]
    fn stale_completion_cannot_overwrite_a_new_run() {
        let controller = SyncController::default();
        let first = controller.prepare();
        controller.finish(first.run_id, Err("first failure".into()));
        let second = controller.prepare();
        assert!(controller
            .finish(first.run_id, Ok(SyncReport::default()))
            .is_none());
        assert!(controller.status().running);
        assert_eq!(controller.status().run_id, second.run_id);
        assert_eq!(controller.status().last_report, None);
        assert_eq!(controller.status().error, None);
    }

    #[test]
    fn failed_platform_start_releases_the_claim() {
        let controller = SyncController::default();
        let run = controller.prepare();
        controller
            .finish(run.run_id, Err("foreground service unavailable".into()))
            .unwrap();
        assert!(!controller.status().running);
        assert!(controller.prepare().should_start);
    }
}
