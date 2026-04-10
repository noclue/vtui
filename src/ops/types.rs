use crate::operation_types::OperationId;
use crate::vm_power_actions::VmPowerAction;
use vim_rs::types::structs::ManagedObjectReference;

/// Request sent from the UI to the ops supervisor (bounded queue).
#[derive(Debug)]
pub enum OperationRequest {
    PrefetchVmActions {
        request_id: OperationId,
        vm: ManagedObjectReference,
    },
    ExecuteInventoryOperation {
        op_id: OperationId,
        op: InventoryOperation,
    },
}

/// Inventory-scoped operation (VM first; host/datastore later).
#[derive(Debug, Clone)]
pub enum InventoryOperation {
    Vm {
        vm: ManagedObjectReference,
        action: VmPowerAction,
    },
}
