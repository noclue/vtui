use miniserde::json::Value;
use vim_rs::types::vim_any::VimAny;

pub fn to_json_value(value: &VimAny, name: &str) -> anyhow::Result<Value> {
    Ok(match value {
        VimAny::Value(val) => {
            let json_str = miniserde::json::to_string(val);
            let json_val: Value = miniserde::json::from_str(&json_str)
                .map_err(|e| anyhow::anyhow!("Failed to parse value JSON: {}", e))?;
            match json_val {
                Value::Object(mut obj) => obj.remove("_value").ok_or_else(|| {
                    anyhow::anyhow!(
                        "Expected JSON object with _value field for property {}",
                        name
                    )
                })?,
                _ => {
                    return Err(anyhow::anyhow!(
                        "Expected JSON object for property '{}', got {:?}",
                        name,
                        json_val
                    ));
                }
            }
        }
        VimAny::Object(obj) => {
            let json_str = miniserde::json::to_string(obj.as_ref());
            miniserde::json::from_str(&json_str).map_err(|e| {
                anyhow::anyhow!(
                    "Failed to convert property '{}' object to JSON: {}",
                    name,
                    e
                )
            })?
        }
    })
}

#[cfg(test)]
mod tests {
    use vim_rs::types::boxed_types::ValueElements;
    use vim_rs::types::structs::{VAppIpAssignmentInfo, VmConfigInfo};

    use super::*;

    #[test]
    fn test_to_json_value() {
        let value = VimAny::Value(ValueElements::PrimitiveString("test © “quoted”".to_string()));
        let json_value = to_json_value(&value, "test").unwrap();
        match json_value {
            Value::String(s) => assert_eq!(s, "test © “quoted”"),
            _ => panic!("Expected String"),
        }
    }
    #[test]
    fn test_to_json_value_object() {
        let value = VimAny::Object(Box::new(VmConfigInfo {
            product: None,
            property: None,
            ip_assignment: VAppIpAssignmentInfo { 
                supported_allocation_scheme: None, 
                ip_allocation_policy: None, 
                supported_ip_protocol: None, 
                ip_protocol: None 
            },
            eula: Some(Vec::from(["test © “quoted”".to_string()])),
            ovf_section: None,
            ovf_environment_transport: None,
            install_boot_required: false,
            install_boot_stop_delay: 0,
        }));
        let json_value = to_json_value(&value, "test").unwrap();
        match json_value {
            Value::Object(obj) => {
                let eula = obj.get("eula").unwrap();
                match eula {
                    Value::Array(arr) => {
                        assert_eq!(arr.len(), 1);
                        match &arr[0] {
                            Value::String(s) => assert_eq!(s, "test © “quoted”"),
                            _ => panic!("Expected String"),
                        }
                    }
                    _ => panic!("Expected Array"),
                }
            }
            _ => panic!("Expected Object"),
        };
    }
}