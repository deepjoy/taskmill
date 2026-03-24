//! Parent-child resolution after task completion or failure.

use std::sync::Arc;

use crate::store::TaskStore;
use crate::task::{IoBudget, ParentResolution};

use super::super::dispatch::ActiveTaskMap;
use super::super::{emit_event, SchedulerEvent};

/// Check if a waiting parent is ready for finalization or has failed,
/// and dispatch the finalize phase if ready.
pub(crate) async fn handle_parent_resolution(
    parent_id: i64,
    store: &TaskStore,
    active: &ActiveTaskMap,
    event_tx: &tokio::sync::broadcast::Sender<SchedulerEvent>,
    _max_retries: i32,
    work_notify: &Arc<tokio::sync::Notify>,
) {
    match store.try_resolve_parent(parent_id).await {
        Ok(Some(ParentResolution::ReadyToFinalize)) => {
            // Enqueue parent for finalize dispatch.
            active.pending_finalizers.lock().unwrap().insert(parent_id);
            // Wake the scheduler to dispatch the finalize phase.
            work_notify.notify_one();
        }
        Ok(Some(ParentResolution::Failed(reason))) => {
            // All children done but some failed — fail the parent.
            if let Ok(Some(parent)) = store.task_by_id(parent_id).await {
                if let Err(e) = store
                    .fail_with_record(
                        &parent,
                        &reason,
                        false,
                        0,
                        &IoBudget::default(),
                        &Default::default(),
                    )
                    .await
                {
                    tracing::error!(parent_id, error = %e, "failed to record parent failure");
                }
                emit_event(event_tx, SchedulerEvent::Failed {
                    header: parent.event_header(),
                    error: reason,
                    will_retry: false,
                    retry_after: None,
                });
            }
        }
        Ok(Some(ParentResolution::StillWaiting)) | Ok(None) => {
            // Children still active or parent not found — nothing to do.
        }
        Err(e) => {
            tracing::error!(parent_id, error = %e, "failed to resolve parent");
        }
    }
}
