//! Background vSphere operations (supervisor + bounded queue).

mod handle;
mod supervisor;
pub mod types;

pub use handle::{OpsHandle, OpsSubmitError};
pub use supervisor::run_ops_supervisor;

use tokio::sync::mpsc;
use vim_rs::core::client::VimClientHandle;

use crate::event::Event;

const OPS_QUEUE_CAPACITY: usize = 256;

/// Spawn the ops supervisor and return a handle plus its [`tokio::task::JoinHandle`].
pub fn spawn_ops_supervisor(
    client: VimClientHandle,
    event_tx: mpsc::UnboundedSender<Event>,
) -> (OpsHandle, tokio::task::JoinHandle<()>) {
    let (tx, rx) = mpsc::channel(OPS_QUEUE_CAPACITY);
    let worker_client = client.clone();
    let worker_tx = event_tx.clone();
    let join = tokio::spawn(async move {
        run_ops_supervisor(worker_client, worker_tx, rx).await;
    });
    (OpsHandle::new(tx), join)
}
