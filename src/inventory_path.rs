//! Inventory path resolution matching govmomi `mo.Ancestors` + `internal.InventoryPath`
//! (batched `PropertyCollector::retrieve_properties_ex`).

use anyhow::{Context, Result};
use log::debug;
use std::collections::HashMap;
use std::sync::Arc;
use vim_rs::core::client::Client;
use vim_rs::mo::PropertyCollector;
use vim_rs::types::boxed_types::ValueElements;
use vim_rs::types::enums::MoTypesEnum;
use vim_rs::types::structs::{
    ManagedObjectReference, ObjectContent, ObjectSpec, PropertyFilterSpec, PropertySpec,
    RetrieveOptions, SelectionSpec, TraversalSpec,
};
use vim_rs::types::traits::SelectionSpecTrait;
use vim_rs::types::vim_any::VimAny;

fn is_hidden_root_folder(mor: &ManagedObjectReference) -> bool {
    mor.r#type == MoTypesEnum::Folder && mor.value == "group-d1"
}

#[derive(Clone, Debug)]
struct AncestorParsed {
    mor: ManagedObjectReference,
    name: String,
    /// After applying VirtualMachine `parentVApp` when `parent` is unset (govmomi `Ancestors`).
    parent_resolved: Option<ManagedObjectReference>,
}

/// Build path like govmomi `internal.InventoryPath`: skip entities with no parent (inventory root),
/// join names with `/`, leading `/`. `leaf_to_root` is ordered from the start object up toward the root.
fn inventory_path_govmomi_style(leaf_to_root: &[AncestorParsed]) -> String {
    let mut parts = Vec::new();
    for row in leaf_to_root.iter().rev() {
        if row.parent_resolved.is_none() {
            continue;
        }
        parts.push(row.name.as_str());
    }
    if parts.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", parts.join("/"))
    }
}

fn ancestors_filter_spec(start: ManagedObjectReference) -> PropertyFilterSpec {
    let traverse_name = "traverseParent";
    let traverse_parent_ref: Box<dyn SelectionSpecTrait> = Box::new(SelectionSpec {
        name: Some(traverse_name.to_string()),
    });

    PropertyFilterSpec {
        object_set: vec![ObjectSpec {
            obj: start,
            skip: Some(false),
            select_set: Some(vec![
                Box::new(TraversalSpec {
                    selection_spec_: SelectionSpec {
                        name: Some(traverse_name.to_string()),
                    },
                    r#type: "ManagedEntity".to_string(),
                    path: "parent".to_string(),
                    skip: Some(false),
                    select_set: Some(vec![Box::new(SelectionSpec {
                        name: Some(traverse_name.to_string()),
                    })]),
                }),
                Box::new(TraversalSpec {
                    selection_spec_: SelectionSpec { name: None },
                    r#type: "VirtualMachine".to_string(),
                    path: "parentVApp".to_string(),
                    skip: Some(false),
                    select_set: Some(vec![traverse_parent_ref]),
                }),
            ]),
        }],
        prop_set: vec![
            PropertySpec {
                r#type: "ManagedEntity".to_string(),
                all: Some(false),
                path_set: Some(vec!["name".to_string(), "parent".to_string()]),
            },
            PropertySpec {
                r#type: "VirtualMachine".to_string(),
                all: Some(false),
                path_set: Some(vec!["parentVApp".to_string()]),
            },
        ],
        report_missing_objects_in_results: Some(true),
    }
}

fn parse_dynamic_parent(val: &VimAny) -> Result<Option<ManagedObjectReference>> {
    match val {
        VimAny::Object(obj) => {
            let mor = obj
                .as_ref()
                .as_any_ref()
                .downcast_ref::<ManagedObjectReference>()
                .context("expected ManagedObjectReference for parent / parentVApp")?;
            Ok(Some(mor.clone()))
        }
        VimAny::Value(ValueElements::ArrayOfManagedObjectReference(refs)) if refs.len() == 1 => {
            Ok(Some(refs[0].clone()))
        }
        VimAny::Value(_) => Ok(None),
    }
}

fn parse_object_content(oc: &ObjectContent) -> Result<AncestorParsed> {
    let mor = oc.obj.clone();
    let mut name: Option<String> = None;
    let mut parent: Option<ManagedObjectReference> = None;
    let mut parent_vapp: Option<ManagedObjectReference> = None;

    let props = oc
        .prop_set
        .as_ref()
        .with_context(|| format!("no prop_set for {}:{}", mor.r#type.as_str(), mor.value))?;
    for p in props {
        match p.name.as_str() {
            "name" => {
                if let VimAny::Value(ValueElements::PrimitiveString(s)) = &p.val {
                    name = Some(s.clone());
                }
            }
            "parent" => {
                parent = parse_dynamic_parent(&p.val)?;
            }
            "parentVApp" => {
                parent_vapp = parse_dynamic_parent(&p.val)?;
            }
            _ => {}
        }
    }

    let name = name.unwrap_or_default();
    let mut parent_resolved = parent;
    if parent_resolved.is_none() {
        parent_resolved = parent_vapp;
    }

    Ok(AncestorParsed {
        mor,
        name,
        parent_resolved,
    })
}

async fn retrieve_ancestors_contents(
    client: &Arc<Client>,
    start: ManagedObjectReference,
) -> Result<Vec<ObjectContent>> {
    let label = format!("{}:{}", start.r#type.as_str(), start.value);
    debug!(
        target: "inventory_path",
        "retrieve_ancestors_contents: PropertyCollector.retrieve_properties_ex start={label}"
    );

    let spec_set = vec![ancestors_filter_spec(start)];
    let options = RetrieveOptions {
        max_objects: Some(256),
    };
    let pc_id = client.service_content().property_collector.value.clone();
    let pc = PropertyCollector::new(client.clone(), &pc_id);

    let mut collected = Vec::new();
    let mut res = pc
        .retrieve_properties_ex(&spec_set, &options)
        .await
        .with_context(|| {
            format!("retrieve_ancestors_contents: retrieve_properties_ex failed for {label}")
        })?
        .context("RetrievePropertiesEx returned None")?;

    let mut continuation_rounds = 0u32;
    loop {
        collected.append(&mut res.objects);
        let Some(token) = res.token.take() else {
            break;
        };
        continuation_rounds += 1;
        res = pc
            .continue_retrieve_properties_ex(&token)
            .await
            .with_context(|| {
                format!(
                    "retrieve_ancestors_contents: continue_retrieve_properties_ex (round {continuation_rounds}) failed for {label}"
                )
            })?;
    }

    debug!(
        target: "inventory_path",
        "retrieve_ancestors_contents: ok start={label} objects={} continuation_rounds={continuation_rounds}",
        collected.len()
    );

    Ok(collected)
}

fn build_map(
    contents: Vec<ObjectContent>,
) -> Result<HashMap<ManagedObjectReference, AncestorParsed>> {
    let mut map = HashMap::with_capacity(contents.len());
    for oc in contents {
        let row = parse_object_content(&oc)?;
        map.insert(row.mor.clone(), row);
    }
    Ok(map)
}

fn leaf_to_root_chain(
    map: &HashMap<ManagedObjectReference, AncestorParsed>,
    start: &ManagedObjectReference,
) -> Result<Vec<AncestorParsed>> {
    let mut chain = Vec::new();
    let mut cur = start.clone();
    loop {
        let row = map
            .get(&cur)
            .with_context(|| {
                format!(
                    "batched retrieve did not include {}:{} (cannot complete ancestry)",
                    cur.r#type.as_str(),
                    cur.value
                )
            })?
            .clone();
        let next_parent = row.parent_resolved.clone();
        chain.push(row);
        let Some(p) = next_parent else {
            break;
        };
        if is_hidden_root_folder(&p) {
            break;
        }
        if !map.contains_key(&p) {
            anyhow::bail!(
                "ancestor {}:{} missing from retrieve result",
                p.r#type.as_str(),
                p.value
            );
        }
        cur = p;
    }
    Ok(chain)
}

/// Resolves govmomi-style inventory path for `start` (e.g. `/Datacenter/vm/.../VmName`).
pub async fn resolve_inventory_path(
    client: Arc<Client>,
    start: ManagedObjectReference,
) -> Result<String> {
    let label = format!("{}:{}", start.r#type.as_str(), start.value);
    debug!(
        target: "inventory_path",
        "resolve_inventory_path: start {label}"
    );

    let contents = retrieve_ancestors_contents(&client, start.clone())
        .await
        .with_context(|| format!("resolve_inventory_path: retrieve_ancestors_contents failed for {label}"))?;

    let map = build_map(contents).with_context(|| {
        format!("resolve_inventory_path: build_map / parse_object_content failed for {label}")
    })?;

    let chain = leaf_to_root_chain(&map, &start).with_context(|| {
        format!("resolve_inventory_path: leaf_to_root_chain failed for {label}")
    })?;

    let path = inventory_path_govmomi_style(&chain);
    debug!(
        target: "inventory_path",
        "resolve_inventory_path: ok {label} path={path:?}"
    );
    Ok(path)
}
