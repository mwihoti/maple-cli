use maple_protocol::{catalog, http_error, request, stream::Decoder};
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn request() -> Value {
    json!({"model":"fixture-model","instructions":"Follow permissions.",
        "input":[{"type":"message","role":"user","content":[{"type":"input_text","text":"Fix the test"}]}],
        "tools":[{"type":"function","name":"exec_command","description":"Run a command",
        "parameters":{"type":"object","properties":{"cmd":{"type":"string"}},"required":["cmd"]}},
        {"type":"custom","name":"apply_patch","description":"Apply a patch"}],"tool_choice":"auto"})
}

fn frame(delta: Value, finish: Value) -> Vec<u8> {
    format!(
        "data: {}\n\n",
        json!({"id":"chat-fixture","choices":[{"index":0,"delta":delta,"finish_reason":finish}]})
    )
    .into_bytes()
}

fn done(events: &[Value]) -> Vec<Value> {
    events
        .iter()
        .filter(|e| e["type"] == "response.output_item.done")
        .map(|e| e["item"].clone())
        .collect()
}

#[test]
fn instructions_and_developer_user_assistant_roles_are_preserved() {
    let mut r = request();
    r["input"] = json!([
        {"role":"developer","content":[{"type":"input_text","text":"Use tests"}]},
        {"role":"user","content":"Inspect"},
        {"role":"assistant","content":[{"type":"output_text","text":"Checking now"}]},
        {"role":"system","content":"Another system instruction"}
    ]);
    let c = request::translate(&r).unwrap();
    assert_eq!(
        c.body["messages"],
        json!([
            {"role":"system","content":"Follow permissions."},
            {"role":"developer","content":"Use tests"},
            {"role":"user","content":"Inspect"},
            {"role":"assistant","content":"Checking now"},
            {"role":"system","content":"Another system instruction"}
        ])
    );
    assert!(c.body.get("store").is_none());
    assert!(c.body.get("previous_response_id").is_none());
}

#[test]
fn multiple_calls_and_results_survive_model_switch_and_history_reload() {
    let mut r = request();
    r["input"] = json!([
        {"type":"message","role":"user","content":"Fix"},
        {"type":"function_call","call_id":"call-a","name":"exec_command","arguments":"{\"cmd\":\"cat main.rs\"}"},
        {"type":"custom_tool_call","call_id":"call-b","name":"apply_patch","input":"*** Begin Patch\n*** End Patch"},
        {"type":"function_call_output","call_id":"call-a","output":"source contents"},
        {"type":"custom_tool_call_output","call_id":"call-b","output":[{"type":"input_text","text":"applied"}]},
        {"type":"message","role":"user","content":"Now run tests"}
    ]);
    let first = request::translate(&r).unwrap();
    assert_eq!(first.body["messages"][2]["role"], "assistant");
    assert_eq!(
        first.body["messages"][2]["tool_calls"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(first.body["messages"][3]["tool_call_id"], "call-a");
    assert_eq!(first.body["messages"][4]["content"], "applied");
    r["model"] = json!("another-fixture-model");
    let serialized = serde_json::to_vec(&r).unwrap();
    let reloaded: Value = serde_json::from_slice(&serialized).unwrap();
    let next = request::translate(&reloaded).unwrap();
    assert_eq!(first.body["messages"], next.body["messages"]);
    assert_ne!(first.body["model"], next.body["model"]);
}

#[test]
fn rejects_orphan_duplicate_and_unresolved_calls() {
    for history in [
        json!([{"type":"function_call_output","call_id":"missing","output":"ok"}]),
        json!([{"type":"function_call","call_id":"a","name":"exec_command","arguments":"{}"}]),
        json!([
            {"type":"function_call","call_id":"a","name":"exec_command","arguments":"{}"},
            {"type":"function_call_output","call_id":"a","output":"ok"},
            {"type":"function_call","call_id":"a","name":"exec_command","arguments":"{}"},
            {"type":"function_call_output","call_id":"a","output":"ok"}]),
    ] {
        let mut r = request();
        r["input"] = history;
        assert!(request::translate(&r).is_err());
    }
}

#[test]
fn rejects_unsupported_features_and_encrypted_history() {
    for field in [
        "compaction",
        "local_shell_call",
        "tool_search_call",
        "additional_tools",
    ] {
        let mut r = request();
        r["input"] = json!([{"type":field}]);
        assert!(request::translate(&r).is_err());
    }
    let mut r = request();
    r["input"] = json!([{"type":"reasoning","summary":[],"encrypted_content":"opaque"}]);
    assert!(request::translate(&r).is_err());
    let mut r = request();
    r["previous_response_id"] = json!("earlier");
    assert!(request::translate(&r).is_err());
    let mut r = request();
    r["tools"] = json!([{"type":"web_search"}]);
    assert!(request::translate(&r).is_err());
    let mut r = request();
    r["text"] = json!({"format":{"type":"json_schema"}});
    assert!(request::translate(&r).is_err());
}

#[test]
fn inline_images_preserved_and_uploaded_files_rejected() {
    let mut r = request();
    r["input"][0]["content"].as_array_mut().unwrap().push(
        json!({"type":"input_image","image_url":"data:image/png;base64,fixture","detail":"high"}),
    );
    assert_eq!(
        request::translate(&r).unwrap().body["messages"][1]["content"][1]["image_url"]["detail"],
        "high"
    );
    r["input"][0]["content"][1] = json!({"type":"input_image","file_id":"uploaded"});
    assert!(request::translate(&r).is_err());
}

#[test]
fn tool_result_images_follow_the_tool_batch_in_a_user_message() {
    let mut r = request();
    let image = json!({"type":"input_image","image_url":"data:image/png;base64,fixture"});
    r["input"] = json!([
        {"type":"message","role":"user","content":"look"},
        {"type":"function_call","name":"view_image","call_id":"a","arguments":"{}"},
        {"type":"function_call","name":"view_image","call_id":"b","arguments":"{}"},
        {"type":"function_call_output","call_id":"a","output":[image]},
        {"type":"function_call_output","call_id":"b","output":"done"},
    ]);
    let body = request::translate(&r).unwrap().body;
    let messages = body["messages"].as_array().unwrap();
    let n = messages.len();
    assert_eq!(messages[n - 3]["role"], "tool");
    assert!(messages[n - 3]["content"].is_string());
    assert_eq!(messages[n - 2]["role"], "tool");
    assert_eq!(messages[n - 1]["role"], "user");
    assert_eq!(
        messages[n - 1]["content"][1]["image_url"]["url"],
        "data:image/png;base64,fixture"
    );
}

#[test]
fn tool_result_image_keeps_its_detail_setting() {
    let mut r = request();
    r["input"] = json!([
        {"type":"message","role":"user","content":"look"},
        {"type":"function_call","name":"view_image","call_id":"a","arguments":"{}"},
        {"type":"function_call_output","call_id":"a","output":[
            {"type":"input_image","image_url":"data:image/png;base64,fixture","detail":"low"}]},
    ]);
    let body = request::translate(&r).unwrap().body;
    let last = body["messages"].as_array().unwrap().last().unwrap();
    assert_eq!(last["content"][1]["image_url"]["detail"], "low");
}

#[test]
fn conversation_continues_after_a_tool_result_image() {
    let mut r = request();
    r["input"] = json!([
        {"type":"message","role":"user","content":"look"},
        {"type":"function_call","name":"view_image","call_id":"a","arguments":"{}"},
        {"type":"function_call_output","call_id":"a","output":[
            {"type":"input_text","text":"viewed"},
            {"type":"input_image","image_url":"data:image/png;base64,fixture"}]},
        {"type":"message","role":"assistant","content":[{"type":"output_text","text":"red"}]},
        {"type":"message","role":"user","content":"thanks"},
    ]);
    let body = request::translate(&r).unwrap().body;
    let roles: Vec<&str> = body["messages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["role"].as_str().unwrap())
        .rev()
        .take(5)
        .collect();
    assert_eq!(roles, ["user", "assistant", "user", "tool", "assistant"]);
}

#[test]
fn uploaded_files_in_tool_results_are_still_rejected() {
    let mut r = request();
    r["input"] = json!([
        {"type":"message","role":"user","content":"look"},
        {"type":"function_call","name":"view_image","call_id":"a","arguments":"{}"},
        {"type":"function_call_output","call_id":"a","output":[
            {"type":"input_file","file_id":"uploaded"}]},
    ]);
    assert!(request::translate(&r).is_err());
}

#[test]
fn assistant_images_are_still_rejected() {
    let mut r = request();
    r["input"] = json!([
        {"type":"message","role":"assistant","content":[
            {"type":"input_image","image_url":"data:image/png;base64,fixture"}]},
    ]);
    assert!(request::translate(&r).is_err());
}

#[test]
fn namespace_tools_round_trip_to_original_dispatch_names() {
    let mut r = request();
    let tool = r["tools"][0].clone();
    r["tools"] = json!([{"type":"namespace","name":"functions","tools":[tool]}]);
    let c = request::translate(&r).unwrap();
    let alias = c.body["tools"][0]["function"]["name"]
        .as_str()
        .unwrap()
        .to_owned();
    let mut d = Decoder::new(c.tools);
    let mut events=d.feed(&frame(json!({"tool_calls":[{"index":0,"id":"a","type":"function","function":{"name":alias,"arguments":"{}"}}]}),json!("tool_calls"))).unwrap();
    events.extend(d.feed(b"data: [DONE]\n\n").unwrap());
    let call = &done(&events)[0];
    assert_eq!(call["name"], "exec_command");
    assert_eq!(call["namespace"], "functions");
    assert_eq!(call["call_id"], "a");
}

#[test]
fn streaming_unicode_survives_every_possible_byte_boundary() {
    let mut bytes = frame(
        json!({"role":"assistant","content":"Hello 🍁\n世界"}),
        json!("stop"),
    );
    bytes.extend(b"data: [DONE]\n\n");
    for boundary in 0..=bytes.len() {
        let mut d = Decoder::new(BTreeMap::new());
        let mut events = d.feed(&bytes[..boundary]).unwrap();
        events.extend(d.feed(&bytes[boundary..]).unwrap());
        d.eof().unwrap();
        assert_eq!(done(&events)[0]["content"][0]["text"], "Hello 🍁\n世界");
        assert_eq!(events.last().unwrap()["type"], "response.completed");
    }
}

#[test]
fn sse_comments_crlf_cr_and_multiline_frames_are_supported() {
    for newline in ["\n", "\r\n", "\r"] {
        let text = format!(
            ": heartbeat{newline}{newline}data: {{\"id\":\"fixture\",{newline}data: \"choices\":[{{\"index\":0,\"delta\":{{\"content\":\"ok\"}},\"finish_reason\":\"stop\"}}]}}{newline}{newline}data: [DONE]{newline}{newline}"
        );
        let mut d = Decoder::new(BTreeMap::new());
        let mut events = Vec::new();
        for byte in text.bytes() {
            events.extend(d.feed(&[byte]).unwrap());
        }
        d.eof().unwrap();
        assert_eq!(done(&events)[0]["content"][0]["text"], "ok");
    }
}

#[test]
fn incremental_interleaved_tool_arguments_preserve_ids_and_raw_custom_input() {
    let c = request::translate(&request()).unwrap();
    let exec = c.body["tools"][0]["function"]["name"].clone();
    let patch = c.body["tools"][1]["function"]["name"].clone();
    let mut d = Decoder::new(c.tools);
    let first=d.feed(&frame(json!({"tool_calls":[
        {"index":1,"id":"patch-b","type":"function","function":{"name":patch,"arguments":"{\"input\":\"*** Begin"}},
        {"index":0,"id":"exec-a","type":"function","function":{"name":exec,"arguments":"{\"cmd\":"}}]}),Value::Null)).unwrap();
    assert!(done(&first).is_empty());
    let second = d
        .feed(&frame(
            json!({"tool_calls":[
        {"index":0,"function":{"arguments":"\"cargo test\"}"}},
        {"index":1,"function":{"arguments":" Patch\\n*** End Patch\"}"}}]}),
            json!("tool_calls"),
        ))
        .unwrap();
    assert!(done(&second).is_empty());
    let events = d.feed(b"data: [DONE]\n\n").unwrap();
    let items = done(&events);
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["call_id"], "exec-a");
    assert_eq!(items[0]["arguments"], "{\"cmd\":\"cargo test\"}");
    assert_eq!(items[1]["type"], "custom_tool_call");
    assert_eq!(items[1]["input"], "*** Begin Patch\n*** End Patch");
    assert_eq!(events.last().unwrap()["response"]["end_turn"], false);
}

#[test]
fn disconnect_never_releases_a_tool_call_even_after_finish_reason() {
    let c = request::translate(&request()).unwrap();
    let name = c.body["tools"][0]["function"]["name"].clone();
    let mut d = Decoder::new(c.tools);
    let events = d
        .feed(&frame(
            json!({"tool_calls":[{"index":0,"id":"a","function":{"name":name,"arguments":"{}"}}]}),
            json!("tool_calls"),
        ))
        .unwrap();
    assert!(done(&events).is_empty());
    assert!(d.eof().is_err());
}

#[test]
fn malformed_or_unknown_tool_in_multi_call_response_releases_none() {
    for bad_arguments in ["{", "[]", "null"] {
        let c = request::translate(&request()).unwrap();
        let name = c.body["tools"][0]["function"]["name"].clone();
        let mut d = Decoder::new(c.tools);
        d.feed(&frame(
            json!({"tool_calls":[
            {"index":0,"id":"a","function":{"name":name,"arguments":"{}"}},
            {"index":1,"id":"b","function":{"name":name,"arguments":bad_arguments}}]}),
            json!("tool_calls"),
        ))
        .unwrap();
        assert!(d.feed(b"data: [DONE]\n\n").is_err());
        assert!(!d.is_done());
    }
    let mut d = Decoder::new(BTreeMap::new());
    d.feed(&frame(json!({"tool_calls":[{"index":0,"id":"a","function":{"name":"unauthorized","arguments":"{}"}}]}),json!("tool_calls"))).unwrap();
    // An unknown tool becomes a call Maple Code cannot dispatch, not a dead turn.
    let events = d.feed(b"data: [DONE]\n\n").unwrap();
    let item = &done(&events)[0];
    assert_eq!(item["name"], "unavailable_tool");
    assert_eq!(item["arguments"], "{\"requested\":\"unauthorized\"}");
}

#[test]
fn readable_tool_names_resolve_to_advertised_tools_only() {
    let mut r = request();
    let tool = r["tools"][0].clone();
    let plain = tool["name"].as_str().unwrap().to_owned();
    r["tools"] = json!([{"type":"namespace","name":"functions","tools":[tool]}]);
    for requested in [
        plain.clone(),
        format!("functions.{plain}"),
        format!("functions__{plain}"),
    ] {
        let c = request::translate(&r).unwrap();
        let mut d = Decoder::new(c.tools);
        d.feed(&frame(
            json!({"tool_calls":[{"index":0,"id":"a","function":{"name":requested,"arguments":"{}"}}]}),
            json!("tool_calls"),
        ))
        .unwrap();
        let events = d.feed(b"data: [DONE]\n\n").unwrap();
        let item = &done(&events)[0];
        assert_eq!(item["name"], plain);
        assert_eq!(item["namespace"], "functions");
    }
}

#[test]
fn ambiguous_readable_tool_names_are_not_resolved() {
    let mut r = request();
    let tool = r["tools"][0].clone();
    let plain = tool["name"].as_str().unwrap().to_owned();
    r["tools"] = json!([
        {"type":"namespace","name":"one","tools":[tool.clone()]},
        {"type":"namespace","name":"two","tools":[tool]}]);
    let c = request::translate(&r).unwrap();
    let mut d = Decoder::new(c.tools);
    d.feed(&frame(
        json!({"tool_calls":[{"index":0,"id":"a","function":{"name":plain,"arguments":"{}"}}]}),
        json!("tool_calls"),
    ))
    .unwrap();
    let events = d.feed(b"data: [DONE]\n\n").unwrap();
    assert_eq!(done(&events)[0]["name"], "unavailable_tool");
}

#[test]
fn duplicate_tool_ids_and_missing_ids_are_rejected() {
    for id in ["same", ""] {
        let c = request::translate(&request()).unwrap();
        let name = c.body["tools"][0]["function"]["name"].clone();
        let mut d = Decoder::new(c.tools);
        d.feed(&frame(
            json!({"tool_calls":[
            {"index":0,"id":id,"function":{"name":name,"arguments":"{}"}},
            {"index":1,"id":id,"function":{"name":name,"arguments":"{}"}}]}),
            json!("tool_calls"),
        ))
        .unwrap();
        assert!(d.feed(b"data: [DONE]\n\n").is_err());
    }
}

#[test]
fn truncation_refusal_filter_and_unknown_finish_are_not_success() {
    for finish in ["length", "content_filter", "function_call", "unknown"] {
        let mut d = Decoder::new(BTreeMap::new());
        let events = d
            .feed(&frame(json!({"content":"partial"}), json!(finish)))
            .unwrap();
        assert!(done(&events).is_empty());
        assert!(d.feed(b"data: [DONE]\n\n").is_err());
    }
    let mut d = Decoder::new(BTreeMap::new());
    assert!(
        d.feed(&frame(json!({"refusal":"blocked"}), json!("stop")))
            .is_err()
    );
}

#[test]
fn malformed_frames_invalid_utf8_and_inconsistent_response_ids_fail_closed() {
    for data in [
        b"data: {invalid\n\n".as_slice(),
        b"data: \xff\n\n",
        b"data: [DONE]\n\n",
        b"data: {\"error\":{\"message\":\"sensitive-token\"}}\n\n",
    ] {
        let mut d = Decoder::new(BTreeMap::new());
        let error = d.feed(data).unwrap_err();
        assert!(!error.to_string().contains("sensitive-token"));
        assert!(
            d.feed(&frame(json!({"content":"recovery"}), json!("stop")))
                .is_err()
        );
    }
    let mut d = Decoder::new(BTreeMap::new());
    d.feed(&frame(json!({"content":"first"}), Value::Null))
        .unwrap();
    assert!(
        d.feed(b"data: {\"id\":\"different\",\"choices\":[]}\n\n")
            .is_err()
    );
}

#[test]
fn reasoning_and_usage_metadata_are_preserved_without_inventing_totals() {
    let mut d = Decoder::new(BTreeMap::new());
    let mut events = d
        .feed(&frame(
            json!({"reasoning":"Consider tests", "content":"Done"}),
            json!("stop"),
        ))
        .unwrap();
    events.extend(d.feed(format!("data: {}\n\n",json!({"id":"chat-fixture","choices":[],"usage":{
        "prompt_tokens":20,"completion_tokens":12,"total_tokens":32,
        "prompt_tokens_details":{"cached_tokens":8},"completion_tokens_details":{"reasoning_tokens":9}}})).as_bytes()).unwrap());
    events.extend(d.feed(b"data: [DONE]\n\n").unwrap());
    assert_eq!(done(&events)[0]["type"], "reasoning");
    let usage = &events.last().unwrap()["response"]["usage"];
    assert_eq!(usage["input_tokens"], 20);
    assert_eq!(usage["output_tokens_details"]["reasoning_tokens"], 9);
    let d = Decoder::new(BTreeMap::new());
    let events = d
        .completion(&json!({"id":"plain","choices":[{
        "message":{"role":"assistant","content":"ok"},"finish_reason":"stop"}]}))
        .unwrap();
    assert!(events.last().unwrap()["response"]["usage"].is_null());
}

#[test]
fn non_streaming_completion_supports_real_tool_calls() {
    let c = request::translate(&request()).unwrap();
    let name = c.body["tools"][0]["function"]["name"].clone();
    let events=Decoder::new(c.tools).completion(&json!({"id":"single","choices":[{
        "message":{"role":"assistant","content":null,"tool_calls":[{"id":"a","type":"function",
            "function":{"name":name,"arguments":"{\"cmd\":\"cargo test\"}"}}]},"finish_reason":"tool_calls"}]})).unwrap();
    assert_eq!(done(&events)[0]["call_id"], "a");
}

fn catalog() -> Value {
    json!({"object":"list","data":[
        {"id":"fixture-agent","display_name":"Fixture Agent","tasks":["generate"],
        "capabilities":{"chat":true,"tool_use":true,"vision":false},"context_window":4096,
        "reasoning":{"supported_efforts":["low","high"],"default_effort":"low"}},
        {"id":"fixture-embed","tasks":["embed"],"capabilities":{"tool_use":false}},
        {"id":"fixture-no-tools","tasks":["generate"],"capabilities":{"tool_use":false}},
        {"id":"fixture-unknown","tasks":["generate"]}
    ]})
}

#[test]
fn catalog_filters_by_explicit_capabilities_without_assuming_entitlements() {
    let catalog = catalog::translate(&catalog()).unwrap();
    let entries = catalog["models"].as_array().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["slug"], "fixture-agent");
    assert_eq!(entries[0]["context_window"], 4096);
    assert_eq!(
        entries[0]["supported_reasoning_levels"][1]["effort"],
        "high"
    );
    assert_eq!(entries[0]["tool_mode"], "direct");
}

#[test]
fn new_catalog_models_need_no_code_release_and_unknown_context_stays_unknown() {
    let mut c = catalog();
    c["data"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"new-fixture","tasks":["generate"],
        "capabilities":{"tool_use":true,"vision":true}}));
    let result = catalog::translate(&c).unwrap();
    let new = &result["models"][1];
    assert_eq!(new["slug"], "new-fixture");
    assert!(new["context_window"].is_null());
    assert_eq!(new["input_modalities"], json!(["text", "image"]));
    assert!(new["default_reasoning_level"].is_null());
}

#[test]
fn malformed_catalog_and_invalid_context_are_errors() {
    for c in [
        json!({}),
        json!({"data":[]}),
        json!({"data":[{"id":"unknown"}]}),
    ] {
        assert!(catalog::translate(&c).is_err());
    }
    let mut c = catalog();
    c["data"][0]["context_window"] = json!(-1);
    assert!(catalog::translate(&c).is_err());
    let mut c = catalog();
    let entry = c["data"][0].clone();
    c["data"].as_array_mut().unwrap().push(entry);
    assert!(catalog::translate(&c).is_err());
}

#[test]
fn provider_errors_never_echo_credentials_or_raw_payloads() {
    assert!(http_error(401).to_string().contains("MAPLE_API_KEY"));
    assert!(http_error(403).to_string().contains("entitlement"));
    assert!(http_error(429).to_string().contains("rate limit"));
    for status in [400, 401, 403, 404, 408, 413, 429, 500, 502, 504] {
        let text = format!("{:?}: {}", http_error(status), http_error(status));
        assert!(!text.contains("Bearer"));
        assert!(!text.contains("sensitive-token"));
    }
    let mut r = request();
    r["input"] = json!([{"type":"unknown-sensitive-token"}]);
    assert!(!format!("{:?}", request::translate(&r).err()).contains("unknown-sensitive-token"));
}

#[test]
fn context_quota_and_capability_failures_use_safe_actionable_diagnostics() {
    for (code, expected) in [
        ("context_length_exceeded", "context limit"),
        ("model_not_found", "entitlement"),
        ("insufficient_quota", "remaining credits"),
        ("unsupported_parameter", "capability"),
    ] {
        let body = json!({"error":{"code":code,"message":"Bearer sensitive-token"}}).to_string();
        let message = maple_protocol::provider_error(400, Some(&body)).to_string();
        assert!(message.contains(expected));
        assert!(!message.contains("sensitive-token"));
    }
    assert_eq!(
        maple_protocol::provider_error(401, Some("plain text secret")),
        http_error(401)
    );
}

#[test]
fn a_fresh_decoder_does_not_inherit_cancelled_partial_output() {
    let mut cancelled = Decoder::new(BTreeMap::new());
    cancelled
        .feed(&frame(json!({"content":"old partial"}), Value::Null))
        .unwrap();
    drop(cancelled);
    let mut fresh = Decoder::new(BTreeMap::new());
    fresh
        .feed(&frame(json!({"content":"new response"}), json!("stop")))
        .unwrap();
    let events = fresh.feed(b"data: [DONE]\n\n").unwrap();
    assert_eq!(done(&events)[0]["content"][0]["text"], "new response");
}

#[test]
fn empty_opening_content_delta_does_not_precede_item_added() {
    let mut d = Decoder::new(BTreeMap::new());
    let mut events = d
        .feed(&frame(
            json!({"role":"assistant","content":""}),
            Value::Null,
        ))
        .unwrap();
    events.extend(
        d.feed(&frame(json!({"reasoning":"think"}), Value::Null))
            .unwrap(),
    );
    events.extend(
        d.feed(&frame(json!({"content":"hello"}), json!("stop")))
            .unwrap(),
    );
    events.extend(d.feed(b"data: [DONE]\n\n").unwrap());
    let mut open = std::collections::HashSet::new();
    for e in &events {
        match e["type"].as_str().unwrap() {
            "response.output_item.added" => {
                open.insert(e["item"]["type"].as_str().unwrap().to_owned());
            }
            "response.output_text.delta" => {
                assert_ne!(e["delta"], "");
                assert!(open.contains("message"), "text delta before message item");
            }
            "response.reasoning_text.delta" => {
                assert!(open.contains("reasoning"), "reasoning delta before item");
            }
            _ => {}
        }
    }
    assert_eq!(done(&events).len(), 2);
}
