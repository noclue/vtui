//! VM power actions and VIM execution (MVP: fire task / guest op, do not wait on tasks).
//!
//! VM name and `disabledMethod` are loaded in one `PropertyCollector::RetrievePropertiesEx` round
//! trip via [`vim_rs::core::pc_retrieve::ObjectRetriever`] (see vim_rs README / `vim_retrievable`).

use anyhow::Context;
use log::{debug, warn};
use std::sync::Arc;
use vim_rs::core::client::Client;
use vim_rs::core::pc_retrieve::ObjectRetriever;
use vim_rs::mo::VirtualMachine;
use vim_rs::types::structs::ManagedObjectReference;
use vim_rs::vim_retrievable;

vim_retrievable!(
    struct VmPowerActionProps: VirtualMachine {
        name = "name",
        disabled_method = "disabled_method",
    }
);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmPowerAction {
    PowerOn,
    ShutdownGuest,
    HardPowerOff,
    GuestReboot,
    HardReset,
    Suspend,
}

impl VmPowerAction {
    pub const ALL: [VmPowerAction; 6] = [
        VmPowerAction::PowerOn,
        VmPowerAction::ShutdownGuest,
        VmPowerAction::HardPowerOff,
        VmPowerAction::GuestReboot,
        VmPowerAction::HardReset,
        VmPowerAction::Suspend,
    ];

    pub fn label(self) -> &'static str {
        match self {
            VmPowerAction::PowerOn => "Power On / Start",
            VmPowerAction::ShutdownGuest => "Shutdown Guest",
            VmPowerAction::HardPowerOff => "Hard Power Off",
            VmPowerAction::GuestReboot => "Guest Reboot",
            VmPowerAction::HardReset => "Hard Reset",
            VmPowerAction::Suspend => "Suspend",
        }
    }

    /// Token as returned in `VirtualMachine::disabled_method` for this VM.
    pub fn disabled_method_token(self) -> &'static str {
        match self {
            VmPowerAction::PowerOn => "PowerOnVM_Task",
            VmPowerAction::ShutdownGuest => "ShutdownGuest",
            VmPowerAction::HardPowerOff => "PowerOffVM_Task",
            VmPowerAction::GuestReboot => "RebootGuest",
            VmPowerAction::HardReset => "ResetVM_Task",
            VmPowerAction::Suspend => "SuspendVM_Task",
        }
    }

    pub fn requires_confirmation(self) -> bool {
        !matches!(self, VmPowerAction::PowerOn)
    }

    /// Actions to show given `disabled_method` from the server (hide if token is listed).
    pub fn visible(disabled: &Option<Vec<String>>) -> Vec<VmPowerAction> {
        let disabled_set: std::collections::HashSet<&str> = disabled
            .as_ref()
            .map(|v| v.iter().map(|s| s.trim()).collect())
            .unwrap_or_default();
        Self::ALL
            .into_iter()
            .filter(|a| !disabled_set.contains(a.disabled_method_token()))
            .collect()
    }
}

fn vm_ref_label(vm: &ManagedObjectReference) -> String {
    format!("{}:{}", vm.r#type.as_str(), vm.value)
}

/// Prefetch VM name, `disabled_method`, and inventory path for the actions menu.
///
/// Log target `vm_actions`: set `LOG_LEVEL=debug` to see each SOAP/JSON step (useful when XML
/// deserialization fails for a specific property).
pub async fn prefetch_vm_action_context(
    client: Arc<Client>,
    vm: ManagedObjectReference,
) -> anyhow::Result<VmActionContext> {
    let label = vm_ref_label(&vm);
    debug!(target: "vm_actions", "prefetch_vm_action_context: start vm={label}");

    debug!(
        target: "vm_actions",
        "prefetch_vm_action_context: ObjectRetriever.retrieve_objects_from_list (name + disabledMethod) vm={label}"
    );
    let retriever = ObjectRetriever::new(client.clone()).map_err(anyhow::Error::from)?;
    let mut rows = retriever
        .retrieve_objects_from_list::<VmPowerActionProps>(&[vm.clone()])
        .await
        .map_err(anyhow::Error::from)
        .with_context(|| {
            format!(
                "prefetch failed retrieving name/disabledMethod for {label}"
            )
        })?;
    let row = rows
        .pop()
        .with_context(|| format!("prefetch: empty retrieve result for {label}"))?;
    let name = row.name;
    let disabled_method = row.disabled_method;
    debug!(
        target: "vm_actions",
        "prefetch_vm_action_context: retrieve ok name={name:?} disabled_method count={} vm={label}",
        disabled_method.as_ref().map(|v| v.len()).unwrap_or(0)
    );

    debug!(target: "vm_actions", "prefetch_vm_action_context: calling resolve_inventory_path vm={label}");
    let inventory_path = crate::inventory_path::resolve_inventory_path(client, vm.clone())
        .await
        .with_context(|| format!("prefetch failed at resolve_inventory_path for {label}"))?;
    debug!(
        target: "vm_actions",
        "prefetch_vm_action_context: resolve_inventory_path ok path={inventory_path:?} vm={label}"
    );

    debug!(target: "vm_actions", "prefetch_vm_action_context: complete vm={label}");
    Ok(VmActionContext {
        vm,
        name,
        disabled_method,
        inventory_path,
    })
}

#[derive(Debug, Clone)]
pub struct VmActionContext {
    pub vm: ManagedObjectReference,
    pub name: String,
    pub disabled_method: Option<Vec<String>>,
    pub inventory_path: String,
}

pub async fn execute_vm_power_action(
    client: Arc<Client>,
    vm: &ManagedObjectReference,
    action: VmPowerAction,
) -> anyhow::Result<()> {
    let label = vm_ref_label(vm);
    debug!(
        target: "vm_actions",
        "execute_vm_power_action: vm={label} action={:?}",
        action
    );
    let vm_mo = VirtualMachine::new(client.clone(), &vm.value);
    let r = match action {
        VmPowerAction::PowerOn => vm_mo.power_on_vm_task(None).await.map(|_| ()),
        VmPowerAction::ShutdownGuest => vm_mo.shutdown_guest().await,
        VmPowerAction::HardPowerOff => vm_mo.power_off_vm_task().await.map(|_| ()),
        VmPowerAction::GuestReboot => vm_mo.reboot_guest().await,
        VmPowerAction::HardReset => vm_mo.reset_vm_task().await.map(|_| ()),
        VmPowerAction::Suspend => vm_mo.suspend_vm_task().await.map(|_| ()),
    };
    match &r {
        Ok(()) => debug!(
            target: "vm_actions",
            "execute_vm_power_action: ok vm={label} action={action:?}"
        ),
        Err(e) => warn!(
            target: "vm_actions",
            "execute_vm_power_action: failed vm={label} action={action:?}: {e:#}"
        ),
    }
    r.map_err(Into::into)
}
