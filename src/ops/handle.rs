use crate::ops::types::OperationRequest;
use tokio::sync::mpsc;

/// Application-facing handle for submitting work to the ops supervisor.
pub struct OpsHandle {
    tx: Option<mpsc::Sender<OperationRequest>>,
}

impl OpsHandle {
    pub(crate) fn new(tx: mpsc::Sender<OperationRequest>) -> Self {
        Self { tx: Some(tx) }
    }

    pub async fn submit(&mut self, req: OperationRequest) -> Result<(), OpsSubmitError> {
        let Some(tx) = self.tx.as_ref() else {
            return Err(OpsSubmitError);
        };
        tx.send(req).await.map_err(|_| OpsSubmitError)
    }

    /// Drop the sender so the supervisor stops accepting new work and exits its receive loop.
    pub fn shutdown(&mut self) {
        self.tx.take();
    }
}

#[derive(Debug, Clone, Copy)]
pub struct OpsSubmitError;
