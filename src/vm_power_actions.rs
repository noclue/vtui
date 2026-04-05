//! VM power actions and VIM execution (MVP: fire task / guest op, do not wait on tasks).
//!
//! VM name and `disabledMethod` are loaded in one `PropertyCollector::RetrievePropertiesEx` round
//! trip (list view + property spec), same pattern as [`vim_rs::core::pc_retrieve::ObjectRetriever`]
//! but against [`VimClientHandle`] so callers can inject a mock [`vim_rs::core::client::VimClient`].

use anyhow::Context;
use log::{debug, warn};
use vim_rs::core::client::VimClientHandle;
use vim_rs::core::error::Error as VimError;
use vim_rs::core::error::Result as VimResult;
use vim_rs::core::pc_helpers::BoxableError;
use vim_rs::core::pc_retrieve::Retrievable;
use vim_rs::mo::{PropertyCollector, View, ViewManager, VirtualMachine};
use vim_rs::types::structs::ManagedObjectReference;
use vim_rs::vim_retrievable;

vim_retrievable!(
    struct VmPowerActionProps: VirtualMachine {
        name = "name",
        disabled_method = "disabled_method",
    }
);

/// Mirrors `vim_rs` `pc_helpers::obj_spec_for_view` (crate-private there): traverse `view` from a list/container view MO.
fn obj_spec_for_view(
    view_moref: ManagedObjectReference,
) -> Vec<vim_rs::types::structs::ObjectSpec> {
    let r#type = view_moref.r#type.clone();
    vec![vim_rs::types::structs::ObjectSpec {
        obj: view_moref,
        skip: Some(false),
        select_set: Some(vec![Box::new(vim_rs::types::structs::TraversalSpec {
            selection_spec_: vim_rs::types::structs::SelectionSpec {
                name: Some("traverseEntities".to_string()),
            },
            r#type: r#type.as_str().to_string(),
            path: "view".to_string(),
            skip: Some(false),
            select_set: None,
        })]),
    }]
}

// Mirrors `vim_rs` `pc_helpers::retrieve_objects_from_list` (crate-private there): retrieve objects from a list/container view MO.
async fn retrieve_objects_from_list<T>(
    client: VimClientHandle,
    objs: &[ManagedObjectReference],
) -> VimResult<Vec<T>>
where
    T: Retrievable,
    <T as TryFrom<vim_rs::types::structs::ObjectContent>>::Error: BoxableError,
{
    let pc_mo_id = client.service_content().property_collector.value.as_str();
    let property_collector = PropertyCollector::new(client.clone(), pc_mo_id);
    let Some(view_manager_moref) = &client.service_content().view_manager else {
        return Err(VimError::internal("cannot find view_manager".to_string()));
    };
    let view_manager = ViewManager::new(client.clone(), &view_manager_moref.value);
    let view_moref = view_manager.create_list_view(Some(objs)).await?;
    let view = View::new(client.clone(), &view_moref.value);
    let object_set = obj_spec_for_view(view_moref);
    let spec_set = vec![vim_rs::types::structs::PropertyFilterSpec {
        object_set,
        prop_set: vec![T::prop_spec()],
        report_missing_objects_in_results: Some(true),
    }];
    let options = vim_rs::types::structs::RetrieveOptions {
        max_objects: Some(100),
    };
    let inner = async {
        let retrieve_result = property_collector
            .retrieve_properties_ex(&spec_set, &options)
            .await?;
        let Some(mut res) = retrieve_result else {
            return Ok(Vec::new());
        };
        let mut out: Vec<T> = Vec::new();
        loop {
            for obj in res.objects {
                out.push(obj.try_into().map_err(|e| {
                    VimError::from(Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
                })?);
            }
            let Some(token) = res.token else {
                break;
            };
            res = property_collector
                .continue_retrieve_properties_ex(&token)
                .await?;
        }
        Ok(out)
    };
    let out = inner.await;
    view.destroy_view().await?;
    out
}

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
    client: VimClientHandle,
    vm: ManagedObjectReference,
) -> anyhow::Result<VmActionContext> {
    let label = vm_ref_label(&vm);
    debug!(target: "vm_actions", "prefetch_vm_action_context: start vm={label}");

    debug!(
        target: "vm_actions",
        "prefetch_vm_action_context: retrieve_objects_from_list (name + disabledMethod) vm={label}"
    );
    // Revert this code to ObjectRetriever::retrieve_objects_from_list when it supports the Arc<dyn VimClient> argument.
    let mut rows =
        retrieve_objects_from_list::<VmPowerActionProps>(client.clone(), std::slice::from_ref(&vm))
            .await
            .map_err(anyhow::Error::from)
            .with_context(|| {
                format!("prefetch failed retrieving name/disabledMethod for {label}")
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
        disabled_method,
        inventory_path,
    })
}

#[derive(Debug, Clone)]
pub struct VmActionContext {
    pub vm: ManagedObjectReference,
    pub disabled_method: Option<Vec<String>>,
    pub inventory_path: String,
}

pub async fn execute_vm_power_action(
    client: VimClientHandle,
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
