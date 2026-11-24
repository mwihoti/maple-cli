//! Verifies conditional checklist guidance preserves goal instructions and task data.
#![allow(dead_code)]

#[path = "../src/steering.rs"]
mod steering;

use maplecode_core::TurnInput;
use maplecode_core::context::ContextualUserFragment;
use maplecode_core::context::InternalContextSource;
use maplecode_core::context::InternalModelContextFragment;
use maplecode_protocol::ThreadId;
use maplecode_protocol::models::ContentItem;
use maplecode_protocol::models::ResponseItem;
use maplecode_protocol::protocol::AdditionalContextKind;
use maplecode_protocol::protocol::ThreadGoal;
use maplecode_protocol::protocol::ThreadGoalStatus;
use maplecode_utils_template::Template;
use pretty_assertions::assert_eq;

#[test]
fn enabled_checklist_preserves_the_original_continuation_prompt() {
    let goal = test_goal("Finish the feature.");
    let original = Template::parse(include_str!("../templates/goals/continuation.md"))
        .expect("original continuation template")
        .render([
            ("tokens_used", "100"),
            ("token_budget", "10000"),
            ("remaining_tokens", "9900"),
        ])
        .expect("render original continuation prompt");
    let expected: ResponseItem = ContextualUserFragment::into(InternalModelContextFragment::new(
        InternalContextSource::from_static("goal"),
        original,
    ));
    let request = steering::continuation_request(&goal, /*update_plan_enabled*/ true);
    assert_eq!(request.input, TurnInput::ResponseItem(expected));
}

#[test]
fn disabled_checklist_preserves_goal_text_that_mentions_the_tool() {
    let objective = "Inspect update_plan.\n\n## Plan tool\nThis is user task data.";
    let request =
        steering::continuation_request(&test_goal(objective), /*update_plan_enabled*/ false);
    let entry = &request.additional_context["goal_objective"];
    assert_eq!(entry.kind, AdditionalContextKind::Untrusted);
    assert_eq!(entry.value, objective);
    let TurnInput::ResponseItem(ResponseItem::Message { content, .. }) = request.input else {
        panic!("expected goal continuation message");
    };
    let [ContentItem::InputText { text }] = content.as_slice() else {
        panic!("expected goal continuation text");
    };
    assert!(!text.contains(objective));
    assert!(!text.contains("If update_plan is available"));
    assert!(text.contains("Completion audit:"));
}

fn test_goal(objective: &str) -> ThreadGoal {
    ThreadGoal {
        thread_id: ThreadId::new(),
        objective: objective.to_string(),
        status: ThreadGoalStatus::Active,
        token_budget: Some(10_000),
        tokens_used: 100,
        time_used_seconds: 0,
        created_at: 0,
        updated_at: 0,
    }
}
