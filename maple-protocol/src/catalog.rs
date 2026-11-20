use crate::{Error, Result, string};
use serde_json::{Value, json};
use std::collections::BTreeSet;

/// Convert the documented Maple models envelope to Codex-native metadata.
/// Unknown capability data stays unknown. Only explicitly tool-capable chat
/// entries appear in the autonomous-agent selector.
pub fn translate(value: &Value) -> Result<Value> {
    let entries = value["data"]
        .as_array()
        .ok_or(Error("model catalog lacks data array"))?;
    let mut seen = BTreeSet::new();
    let mut models = Vec::new();
    for entry in entries {
        let id = string(entry, "id")?;
        if !seen.insert(id) {
            return Err(Error("model catalog contains duplicate identifiers"));
        }
        let chat = entry.pointer("/capabilities/chat").and_then(Value::as_bool);
        let tools = entry
            .pointer("/capabilities/tool_use")
            .and_then(Value::as_bool);
        let generate = entry["tasks"]
            .as_array()
            .is_some_and(|t| t.iter().any(|v| v == "generate"));
        if tools != Some(true) || chat == Some(false) || !(chat == Some(true) || generate) {
            continue;
        }
        if entry["enabled"] == false || entry["deprecated"] == true {
            continue;
        }
        let context = entry.get("context_window").filter(|v| !v.is_null());
        if context.is_some_and(|v| v.as_i64().is_none_or(|n| n <= 0)) {
            return Err(Error("invalid context window metadata"));
        }
        let reasoning_levels = entry
            .pointer("/reasoning/supported_efforts")
            .and_then(Value::as_array);
        let supported = reasoning_levels
            .into_iter()
            .flatten()
            .map(|v| {
                let effort = v
                    .as_str()
                    .ok_or(Error("invalid reasoning effort metadata"))?;
                Ok(json!({"effort":effort,"description":effort}))
            })
            .collect::<Result<Vec<_>>>()?;
        let mut model = json!({
            "slug":id,"display_name":entry["display_name"].as_str().unwrap_or(id),
            "description":entry.get("description"),
            "default_reasoning_level":entry.pointer("/reasoning/default_effort"),
            "supported_reasoning_levels":supported,
            "shell_type":"unified_exec","visibility":"list","supported_in_api":true,
            "priority":models.len(),"availability_nux":null,"upgrade":null,
            "model_messages":{"instructions_template":"You are a coding assistant. Inspect the repository, make precise changes, and verify your work with the available tools. Follow all supplied instructions and permission requirements."},
            "include_apps_usage_instructions":false,"supports_reasoning_summary_parameter":false,
            "support_verbosity":false,"default_verbosity":null,"apply_patch_tool_type":"freeform",
            "truncation_policy":{"mode":"bytes","limit":10000},
            "context_window":context,"max_context_window":context,
            "experimental_supported_tools":[],"tool_mode":"direct",
            "input_modalities":["text"],
            "effective_context_window_percent":90
        });
        if entry.pointer("/capabilities/vision") == Some(&json!(true)) {
            model["input_modalities"] = json!(["text", "image"]);
        }
        models.push(model);
    }
    if models.is_empty() {
        return Err(Error(
            "catalog has no explicitly tool-capable chat models; check proxy capability metadata",
        ));
    }
    Ok(json!({"models":models}))
}
