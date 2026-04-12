use crate::event::{AppEvent, Event};
use crate::ops::types::{InventoryOperation, OperationRequest};
use crate::vm_power_actions::{execute_vm_power_action, prefetch_vm_action_context};
use crate::vm_summary::fetch_vm_summary;
use log::{debug, warn};
use std::sync::Arc;
use tokio::sync::{Semaphore, mpsc};
use vim_rs::core::client::VimClientHandle;

const DEFAULT_MAX_CONCURRENT_OPS: usize = 8;

pub async fn run_ops_supervisor(
    client: VimClientHandle,
    event_tx: mpsc::UnboundedSender<Event>,
    mut rx: mpsc::Receiver<OperationRequest>,
) {
    let sem = Arc::new(Semaphore::new(DEFAULT_MAX_CONCURRENT_OPS));

    while let Some(req) = rx.recv().await {
        let permit = match sem.clone().acquire_owned().await {
            Ok(p) => p,
            Err(_) => break,
        };
        let client = client.clone();
        let event_tx = event_tx.clone();
        tokio::spawn(async move {
            let _permit = permit;
            match req {
                OperationRequest::PrefetchVmActions { request_id, vm } => {
                    let res = prefetch_vm_action_context(client, vm).await;
                    let ev = match res {
                        Ok(ctx) => AppEvent::VmActionPrefetchSucceeded {
                            request_id,
                            context: ctx,
                        },
                        Err(e) => AppEvent::VmActionPrefetchFailed {
                            request_id,
                            error: format!("{e:#}"),
                        },
                    };
                    let _ = event_tx.send(Event::App(Box::new(ev)));
                }
                OperationRequest::PrefetchVmSummary { request_id, vm } => {
                    let vm_label = format!("{}:{}", vm.r#type.as_str(), vm.value);
                    debug!(
                        target: "vm_summary",
                        "ops: PrefetchVmSummary task start request_id={request_id} vm={vm_label}"
                    );
                    let res = fetch_vm_summary(client, vm).await;
                    let ev = match res {
                        Ok(summary) => {
                            debug!(
                                target: "vm_summary",
                                "ops: PrefetchVmSummary task ok request_id={request_id} vm={vm_label} name={}",
                                summary.vm_name
                            );
                            AppEvent::VmSummarySucceeded {
                                request_id,
                                summary,
                            }
                        }
                        Err(e) => {
                            warn!(
                                target: "vm_summary",
                                "ops: PrefetchVmSummary task failed request_id={request_id} vm={vm_label}: {e:#}"
                            );
                            AppEvent::VmSummaryFailed {
                                request_id,
                                error: format!("{e:#}"),
                            }
                        }
                    };
                    let _ = event_tx.send(Event::App(Box::new(ev)));
                }
                OperationRequest::ExecuteInventoryOperation { op_id, op } => match op {
                    InventoryOperation::Vm { vm, action } => {
                        let res = execute_vm_power_action(client, &vm, action).await;
                        let ev = match res {
                            Ok(()) => AppEvent::OperationSucceeded {
                                op_id,
                                message: format!("{} completed.", action.label()),
                            },
                            Err(e) => AppEvent::OperationFailed {
                                op_id,
                                error: format!("{e:#}"),
                            },
                        };
                        let _ = event_tx.send(Event::App(Box::new(ev)));
                    }
                },
            }
        });
    }
}
