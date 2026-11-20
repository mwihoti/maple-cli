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
