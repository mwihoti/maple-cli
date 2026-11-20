use crate::request::Tool;
use crate::{Error, Result, string};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

const MAX_FRAME: usize = 8 * 1024 * 1024;
const MAX_OUTPUT: usize = 32 * 1024 * 1024;
const MAX_CALLS: u64 = 1024;

#[derive(Default)]
struct Call {
    id: String,
    name: String,
    arguments: String,
}

/// A decoder is owned by one HTTP response and is discarded on cancellation.
/// Only a valid terminal frame releases completed tool calls to the agent.
pub struct Decoder {
    tools: BTreeMap<String, Tool>,
    line: Vec<u8>,
    data: Vec<String>,
    frame_bytes: usize,
    skip_lf: bool,
    response_id: Option<String>,
    content: String,
    reasoning: String,
    calls: BTreeMap<u64, Call>,
    usage: Option<Value>,
    finish: Option<String>,
    done: bool,
    failed: bool,
    output_bytes: usize,
}

impl Decoder {
    pub fn new(tools: BTreeMap<String, Tool>) -> Self {
        Self {
            tools,
            line: Vec::new(),
            data: Vec::new(),
            frame_bytes: 0,
            skip_lf: false,
            response_id: None,
            content: String::new(),
            reasoning: String::new(),
            calls: BTreeMap::new(),
            usage: None,
            finish: None,
            done: false,
            failed: false,
            output_bytes: 0,
        }
    }

    pub fn is_done(&self) -> bool {
        self.done
    }

    pub fn feed(&mut self, bytes: &[u8]) -> Result<Vec<Value>> {
        if self.failed {
            return Err(Error("stream already failed"));
        }
        let result = self.feed_inner(bytes);
        if result.is_err() {
            self.failed = true;
        }
        result
    }

    fn feed_inner(&mut self, bytes: &[u8]) -> Result<Vec<Value>> {
        let mut out = Vec::new();
        for &byte in bytes {
            if self.done {
                break;
            }
            if self.skip_lf {
                self.skip_lf = false;
                if byte == b'\n' {
                    continue;
                }
            }
            if byte == b'\r' || byte == b'\n' {
                self.skip_lf = byte == b'\r';
                let line = std::mem::take(&mut self.line);
                let line =
                    String::from_utf8(line).map_err(|_| Error("invalid UTF-8 in SSE frame"))?;
                if line.is_empty() {
                    if !self.data.is_empty() {
                        let data = std::mem::take(&mut self.data).join("\n");
                        out.extend(self.frame(&data)?);
                    }
                    self.frame_bytes = 0;
                } else if let Some(data) = line.strip_prefix("data:") {
                    let data = data.strip_prefix(' ').unwrap_or(data);
                    self.frame_bytes = self.frame_bytes.saturating_add(data.len());
                    if self.frame_bytes > MAX_FRAME {
                        return Err(Error("SSE frame exceeds adapter limit"));
                    }
                    self.data.push(data.to_owned());
                }
            } else {
                if self.line.len() >= MAX_FRAME {
                    return Err(Error("SSE line exceeds adapter limit"));
                }
                self.line.push(byte);
            }
        }
        Ok(out)
    }

    pub fn eof(&self) -> Result<()> {
        if self.done && !self.failed {
            Ok(())
        } else {
            Err(Error(
                "stream interrupted before a valid terminal frame; no partial tool calls were released",
            ))
        }
    }

    fn frame(&mut self, data: &str) -> Result<Vec<Value>> {
        if data.trim() == "[DONE]" {
            return self.complete();
        }
        let v: Value =
            serde_json::from_str(data).map_err(|_| Error("malformed chat completion stream"))?;
        if v.get("error").is_some() {
            return Err(Error("provider reported a streaming error"));
        }
        let mut out = Vec::new();
        let id = string(&v, "id")?;
        if let Some(existing) = &self.response_id {
            if id != existing {
                return Err(Error("response identifier changed during streaming"));
            }
        } else {
            self.response_id = Some(id.to_owned());
            out.push(json!({"type":"response.created","response":{"id":id}}));
        }
        if let Some(usage) = v.get("usage").filter(|u| !u.is_null()) {
            self.usage = Some(translate_usage(usage)?);
        }
        let choices = v["choices"]
            .as_array()
            .ok_or(Error("chat response lacks choices"))?;
        if choices.is_empty() {
            if v.get("usage").is_none_or(Value::is_null) {
                return Err(Error("empty choices without usage metadata"));
            }
            return Ok(out);
        }
        if choices.len() != 1 || choices[0]["index"].as_u64() != Some(0) {
            return Err(Error("multiple choices are unsupported"));
        }
        if self.finish.is_some() {
            return Err(Error("received output after finish_reason"));
        }
        let choice = &choices[0];
        let delta = choice["delta"]
            .as_object()
            .ok_or(Error("invalid chat delta"))?;
        if delta.get("role").is_some_and(|r| r != "assistant") {
            return Err(Error("unexpected streamed role"));
        }
        for (field, target, event_type) in [
            ("content", &mut self.content, "response.output_text.delta"),
            (
                "reasoning",
                &mut self.reasoning,
                "response.reasoning_text.delta",
            ),
        ] {
            if let Some(value) = delta.get(field).filter(|v| !v.is_null()) {
                let text = value.as_str().ok_or(Error("invalid streamed text"))?;
                // Chat Completions servers commonly open with `"content": ""`
                // alongside the role. Emitting that as a delta would precede the
                // output_item.added event, which Codex treats as a protocol error.
                if text.is_empty() {
                    continue;
                }
                self.output_bytes = self.output_bytes.saturating_add(text.len());
                if self.output_bytes > MAX_OUTPUT {
                    return Err(Error("response exceeds adapter memory limit"));
                }
                if target.is_empty() && !text.is_empty() {
                    let item = if field == "content" {
                        json!({"type":"message","id":format!("maple_message_{id}"),"role":"assistant","content":[]})
                    } else {
                        json!({"type":"reasoning","id":format!("maple_reasoning_{id}"),"summary":[],"content":[]})
                    };
                    out.push(json!({"type":"response.output_item.added","item":item}));
                }
                target.push_str(text);
                out.push(json!({"type":event_type,"delta":text,"content_index":0}));
            }
        }
        if delta.get("refusal").is_some_and(|v| !v.is_null()) {
            return Err(Error("model refused the request"));
        }
        if delta.contains_key("function_call") {
            return Err(Error("legacy function_call deltas are unsupported"));
        }
        if let Some(values) = delta.get("tool_calls").filter(|v| !v.is_null()) {
            for value in values.as_array().ok_or(Error("invalid tool call delta"))? {
                let index = value["index"]
                    .as_u64()
                    .filter(|n| *n < MAX_CALLS)
                    .ok_or(Error("invalid tool call index"))?;
                if value.get("type").is_some_and(|v| v != "function") {
                    return Err(Error("unsupported tool call type"));
                }
                let call = self.calls.entry(index).or_default();
                for (src, dst) in [
                    (value.get("id"), &mut call.id),
                    (value.pointer("/function/name"), &mut call.name),
                    (value.pointer("/function/arguments"), &mut call.arguments),
                ] {
                    if let Some(src) = src {
                        let fragment = src.as_str().ok_or(Error("invalid tool delta fragment"))?;
                        self.output_bytes = self.output_bytes.saturating_add(fragment.len());
                        if self.output_bytes > MAX_OUTPUT {
                            return Err(Error("response exceeds adapter memory limit"));
                        }
                        dst.push_str(fragment);
                    }
                }
                if let Some(args) = value.pointer("/function/arguments").and_then(Value::as_str) {
                    out.push(json!({"type":"response.function_call_arguments.delta",
                        "item_id":format!("maple_call_{id}_{index}"),"delta":args}));
                }
            }
        }
        if let Some(reason) = choice.get("finish_reason").filter(|r| !r.is_null()) {
            self.finish = Some(
                reason
                    .as_str()
                    .ok_or(Error("invalid finish reason"))?
                    .to_owned(),
            );
        }
        Ok(out)
    }

    fn complete(&mut self) -> Result<Vec<Value>> {
        let id = self
            .response_id
            .as_deref()
            .ok_or(Error("empty chat stream"))?;
        let finish = self
            .finish
            .as_deref()
            .ok_or(Error("stream ended without finish_reason"))?;
        if !matches!(finish, "stop" | "tool_calls") {
            return Err(Error(match finish {
                "length" => "model reached its output limit; partial tool calls were discarded",
                "content_filter" => "provider content filter interrupted the response",
                _ => "unsupported completion finish reason",
            }));
        }
        if (finish == "tool_calls") != !self.calls.is_empty() {
            return Err(Error("finish reason does not match returned tool calls"));
        }
        let mut items = Vec::new();
        if !self.reasoning.is_empty() {
            items.push(json!({"type":"reasoning","id":format!("maple_reasoning_{id}"),
                "summary":[],"content":[{"type":"reasoning_text","text":self.reasoning}],"encrypted_content":null}));
        }
        if !self.content.is_empty() {
            items.push(
                json!({"type":"message","id":format!("maple_message_{id}"),"role":"assistant",
                "content":[{"type":"output_text","text":self.content}],"status":"completed"}),
            );
        }
        let mut ids = BTreeSet::new();
        for (index, call) in &self.calls {
            if call.id.is_empty() || !ids.insert(&call.id) {
                return Err(Error("missing or duplicate tool call identifier"));
            }
            let tool = self
                .tools
                .get(&call.name)
                .ok_or(Error("model requested an unadvertised tool"))?;
            let args: Value = serde_json::from_str(&call.arguments)
                .map_err(|_| Error("tool arguments are not valid JSON"))?;
            if !args.is_object() {
                return Err(Error("tool arguments must be a JSON object"));
            }
            let mut item = json!({"id":format!("maple_call_{id}_{index}"),"call_id":call.id,
                "name":tool.name,"namespace":tool.namespace,"status":"completed"});
            if tool.custom {
                let input = args["input"]
                    .as_str()
                    .ok_or(Error("custom tool call lacks raw input"))?;
                if args.as_object().is_none_or(|o| o.len() != 1) {
                    return Err(Error("custom tool call contains unknown arguments"));
                }
                item["type"] = json!("custom_tool_call");
                item["input"] = json!(input);
            } else {
                item["type"] = json!("function_call");
                item["arguments"] = json!(call.arguments);
            }
            items.push(item);
        }
        if items.is_empty() {
            return Err(Error("model returned no content or tool calls"));
        }
        // Validate every call before releasing any completed item. One malformed
        // call must not permit another call in the same response to execute.
        let mut out = Vec::new();
        for item in &items {
            out.push(json!({"type":"response.output_item.done","item":item}));
        }
        out.push(json!({"type":"response.completed","response":{
            "id":id,"output":items,"usage":self.usage,"end_turn":self.calls.is_empty()}}));
        self.done = true;
        Ok(out)
    }

    /// Non-streaming completions share exactly the same validation path.
    pub fn completion(mut self, value: &Value) -> Result<Vec<Value>> {
        let choices = value["choices"]
            .as_array()
            .filter(|v| v.len() == 1)
            .ok_or(Error("invalid non-streaming choices"))?;
        let choice = &choices[0];
        let mut message = choice["message"].clone();
        let obj = message
            .as_object_mut()
            .ok_or(Error("invalid completion message"))?;
        if let Some(calls) = obj.get_mut("tool_calls") {
            for (i, call) in calls
                .as_array_mut()
                .ok_or(Error("invalid completion calls"))?
                .iter_mut()
                .enumerate()
            {
                call["index"] = json!(i);
            }
        }
        let mut out = self.frame(
            &json!({"id":value["id"],"choices":[{"index":0,
            "delta":message,"finish_reason":choice["finish_reason"]}],"usage":value["usage"]})
            .to_string(),
        )?;
        out.extend(self.complete()?);
        Ok(out)
    }
}

fn translate_usage(v: &Value) -> Result<Value> {
    for key in ["prompt_tokens", "completion_tokens", "total_tokens"] {
        if v[key].as_i64().is_none_or(|n| n < 0) {
            return Err(Error("invalid usage metadata"));
        }
    }
    let mut usage = json!({"input_tokens":v["prompt_tokens"],"output_tokens":v["completion_tokens"],
        "total_tokens":v["total_tokens"]});
    if let Some(n) = v.pointer("/prompt_tokens_details/cached_tokens") {
        if n.as_i64().is_none_or(|n| n < 0) {
            return Err(Error("invalid cached-token metadata"));
        }
        usage["input_tokens_details"] = json!({"cached_tokens":n});
    }
    if let Some(n) = v.pointer("/completion_tokens_details/reasoning_tokens") {
        if n.as_i64().is_none_or(|n| n < 0) {
            return Err(Error("invalid reasoning-token metadata"));
        }
        usage["output_tokens_details"] = json!({"reasoning_tokens":n});
    }
    Ok(usage)
}

pub fn encode_events(events: &[Value]) -> Vec<u8> {
    let mut out = Vec::new();
    for event in events {
        out.extend_from_slice(b"data: ");
        out.extend_from_slice(event.to_string().as_bytes());
        out.extend_from_slice(b"\n\n");
    }
    out
}
