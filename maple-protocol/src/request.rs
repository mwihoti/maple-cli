use crate::{Error, Result, string};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone)]
pub struct Tool {
    pub name: String,
    pub namespace: Option<String>,
    pub custom: bool,
}

pub struct ChatRequest {
    pub body: Value,
    pub tools: BTreeMap<String, Tool>,
}

// Stable wire names allow history to survive model changes and process restarts.
// Collision checks below prevent ambiguous dispatch. Provider-controlled aliases
// are never treated as executable names.
fn alias(name: &str, namespace: Option<&str>) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for b in namespace
        .unwrap_or("")
        .bytes()
        .chain([0])
        .chain(name.bytes())
    {
        hash = (hash ^ u64::from(b)).wrapping_mul(0x100000001b3);
    }
    format!("maple_{hash:016x}")
}

const TOOL_IMAGE_NOTE: &str = "[image returned by this tool; attached in the next user message]";
const TOOL_IMAGE_INTRO: &str = "Image output of the preceding tool call:";

fn declarations(
    values: &[Value],
    namespace: Option<&str>,
    registry: &mut BTreeMap<String, Tool>,
    out: &mut Vec<Value>,
) -> Result<()> {
    for v in values {
        let kind = string(v, "type")?;
        if kind == "namespace" {
            if namespace.is_some() {
                return Err(Error("nested tool namespaces are unsupported"));
            }
            declarations(
                v["tools"]
                    .as_array()
                    .ok_or(Error("invalid namespace tools"))?,
                Some(string(v, "name")?),
                registry,
                out,
            )?;
            continue;
        }
        if !matches!(kind, "function" | "custom") {
            return Err(Error(
                "hosted or deferred tools are unsupported; use direct local tools",
            ));
        }
        let name = string(v, "name")?;
        let wire_name = alias(name, namespace);
        let tool = Tool {
            name: name.to_owned(),
            namespace: namespace.map(str::to_owned),
            custom: kind == "custom",
        };
        if registry.insert(wire_name.clone(), tool).is_some() {
            return Err(Error("duplicate or colliding tool definitions"));
        }
        let parameters = if kind == "custom" {
            json!({"type":"object","properties":{"input":{"type":"string",
                "description":"The complete raw tool input, preserving newlines and syntax."}},
                "required":["input"],"additionalProperties":false})
        } else {
            v.get("parameters")
                .filter(|p| p.is_object())
                .cloned()
                .ok_or(Error("function tool lacks a JSON schema"))?
        };
        // Do not claim Maple supports strict/grammar enforcement. Core still
        // validates the actual tool arguments and applies its approval policy.
        let description = format!(
            "{}{}{}",
            name,
            if kind == "custom" {
                ": Supply the raw input in the input field. "
            } else {
                ": "
            },
            v["description"].as_str().unwrap_or("")
        );
        out.push(json!({"type":"function","function":{
            "name":wire_name,"description":description,"parameters":parameters}}));
    }
    Ok(())
}

fn image_part(item: &Value) -> Result<Value> {
    let image = string(item, "image_url")?;
    let mut part = json!({"type":"image_url","image_url":{"url":image}});
    if let Some(detail) = item.get("detail").filter(|v| !v.is_null()) {
        if !matches!(detail.as_str(), Some("auto" | "low" | "high")) {
            return Err(Error("unsupported image detail"));
        }
        part["image_url"]["detail"] = detail.clone();
    }
    Ok(part)
}

/// Translates message content. Chat Completions tool results are text only, so
/// images in a tool result are moved to `tool_images` and replaced by a note;
/// the caller delivers them in a user message once all tool calls are resolved.
fn content(v: &Value, role: &str, tool_images: &mut Vec<Value>) -> Result<Value> {
    if v.is_string() {
        return Ok(v.clone());
    }
    let items = v.as_array().ok_or(Error("invalid message content"))?;
    let mut parts = Vec::new();
    for item in items {
        match string(item, "type")? {
            "input_text" | "output_text" => parts.push(json!({"type":"text","text":
                item["text"].as_str().ok_or(Error("invalid text content"))?})),
            "input_image" if role == "user" => parts.push(image_part(item)?),
            "input_image" if role == "tool" => {
                tool_images.push(image_part(item)?);
                parts.push(json!({"type":"text","text":TOOL_IMAGE_NOTE}));
            }
            _ => {
                return Err(Error(
                    "encrypted, audio, uploaded-file or non-user image content is unsupported",
                ));
            }
        }
    }
    // Most chat endpoints accept plain strings for all text roles, but not
    // content-part arrays for tool results or assistant output.
    if parts.iter().all(|p| p["type"] == "text") {
        return Ok(Value::String(
            parts
                .iter()
                .map(|p| p["text"].as_str().unwrap_or(""))
                .collect::<Vec<_>>()
                .join("\n"),
        ));
    }
    Ok(Value::Array(parts))
}

pub fn translate(request: &Value) -> Result<ChatRequest> {
    if request
        .get("previous_response_id")
        .is_some_and(|v| !v.is_null())
    {
        return Err(Error(
            "server-side previous_response_id is unsupported; send full history",
        ));
    }
    if request
        .pointer("/text/format")
        .is_some_and(|v| !v.is_null())
    {
        return Err(Error("structured Responses output formats are unsupported"));
    }
    if request.get("access_programs").is_some_and(|v| !v.is_null()) {
        return Err(Error("OpenAI access programs are unsupported"));
    }
    let mut registry = BTreeMap::new();
    let mut tools = Vec::new();
    if let Some(v) = request.get("tools").filter(|v| !v.is_null()) {
        declarations(
            v.as_array().ok_or(Error("invalid tool definitions"))?,
            None,
            &mut registry,
            &mut tools,
        )?;
    }
    let mut messages = Vec::<Value>::new();
    if let Some(instructions) = request
        .get("instructions")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    {
        messages.push(json!({"role":"system","content":instructions}));
    }
    let mut pending = BTreeSet::<String>::new();
    let mut seen = BTreeSet::<String>::new();
    let mut reasoning = String::new();
    let mut calls_open = false;
    let mut tool_images = Vec::<Value>::new();
    let input = if let Some(s) = request["input"].as_str() {
        vec![json!({"type":"message","role":"user","content":s})]
    } else {
        request["input"]
            .as_array()
            .ok_or(Error("invalid conversation history"))?
            .clone()
    };
    for item in &input {
        match item["type"].as_str().unwrap_or("message") {
            "reasoning" => {
                if item.get("encrypted_content").is_some_and(|v| !v.is_null()) {
                    return Err(Error("encrypted reasoning history cannot be translated"));
                }
                if let Some(parts) = item.get("content").and_then(Value::as_array) {
                    for part in parts {
                        reasoning.push_str(
                            part["text"]
                                .as_str()
                                .ok_or(Error("invalid reasoning history"))?,
                        );
                    }
                }
                // Summaries are display metadata, never fabricated private reasoning.
            }
            "message" => {
                if !pending.is_empty() {
                    return Err(Error("conversation has unresolved tool calls"));
                }
                let role = string(item, "role")?;
                if !matches!(role, "system" | "developer" | "user" | "assistant") {
                    return Err(Error("unsupported conversation role"));
                }
                let mut msg = json!({"role":role,"content":content(&item["content"],role,&mut tool_images)?});
                if role == "assistant" && !reasoning.is_empty() {
                    msg["reasoning"] = Value::String(std::mem::take(&mut reasoning));
                }
                messages.push(msg);
                calls_open = role == "assistant";
            }
            "function_call" | "custom_tool_call" => {
                let name = string(item, "name")?;
                let namespace = item["namespace"].as_str();
                let wire_name = alias(name, namespace);
                let custom = item["type"] == "custom_tool_call";
                // History may contain a previously available tool. Stable aliases
                // preserve it without advertising it as a current tool.
                let id = string(item, "call_id")?;
                if !seen.insert(id.to_owned()) {
                    return Err(Error("duplicate tool call identifier in history"));
                }
                pending.insert(id.to_owned());
                let args = if custom {
                    json!({"input": item["input"].as_str().ok_or(Error("invalid custom tool history"))?}).to_string()
                } else {
                    let s = item["arguments"]
                        .as_str()
                        .ok_or(Error("invalid function arguments"))?;
                    if !serde_json::from_str::<Value>(s).is_ok_and(|v| v.is_object()) {
                        return Err(Error("function arguments must be a JSON object"));
                    }
                    s.to_owned()
                };
                if !calls_open {
                    let mut msg = json!({"role":"assistant","content":null});
                    if !reasoning.is_empty() {
                        msg["reasoning"] = Value::String(std::mem::take(&mut reasoning));
                    }
                    messages.push(msg);
                }
                let msg = messages.last_mut().ok_or(Error("invalid tool history"))?;
                if msg.get("tool_calls").is_none() {
                    msg["tool_calls"] = json!([]);
                }
                msg["tool_calls"].as_array_mut().ok_or(Error("invalid tool history"))?.push(
                    json!({"id":id,"type":"function","function":{"name":wire_name,"arguments":args}}));
                calls_open = true;
            }
            "function_call_output" | "custom_tool_call_output" => {
                let id = string(item, "call_id")?;
                if !pending.remove(id) {
                    return Err(Error("tool result has no matching call identifier"));
                }
                let output = content(&item["output"], "tool", &mut tool_images)?;
                messages.push(json!({"role":"tool","tool_call_id":id,"content":output}));
                // Tool results must directly follow their calls, so images wait
                // until the whole batch of calls has been answered.
                if pending.is_empty() && !tool_images.is_empty() {
                    let mut parts = vec![json!({"type":"text","text":TOOL_IMAGE_INTRO})];
                    parts.append(&mut tool_images);
                    messages.push(json!({"role":"user","content":parts}));
                }
                calls_open = false;
            }
            _ => {
                return Err(Error(
                    "unsupported Responses history item; start a compatible session",
                ));
            }
        }
    }
    if !pending.is_empty() {
        return Err(Error("conversation has unresolved tool calls"));
    }
    let mut body = json!({"model":string(request,"model")?,"messages":messages,"stream":true,
        "stream_options":{"include_usage":true}});
    if !tools.is_empty() {
        body["tools"] = Value::Array(tools);
        let choice = request.get("tool_choice").unwrap_or(&Value::Null);
        body["tool_choice"] = match choice.as_str() {
            Some("auto" | "none" | "required") => choice.clone(),
            None if choice.is_null() => json!("auto"),
            _ => return Err(Error("unsupported tool choice")),
        };
    }
    if let Some(effort) = request
        .pointer("/reasoning/effort")
        .filter(|v| !v.is_null())
    {
        body["reasoning_effort"] = effort.clone();
    }
    if let Some(limit) = request.get("max_output_tokens").filter(|v| !v.is_null()) {
        if limit.as_u64().is_none_or(|n| n == 0) {
            return Err(Error("invalid output token limit"));
        }
        body["max_tokens"] = limit.clone();
    }
    Ok(ChatRequest {
        body,
        tools: registry,
    })
}
