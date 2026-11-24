//! Hands observed inference spending to one host-owned accounting authority.

use futures::future::BoxFuture;
use maplecode_protocol::ThreadId;
use maplecode_protocol::error::Result;
use maplecode_protocol::protocol::TokenUsage;

/// Provider-reported spending for one response, including compaction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentUsageReport {
    pub thread_id: ThreadId,
    pub turn_id: String,
    /// Per-response usage, not cumulative totals. Remote adapters must explicitly
    /// carry `maplecode_rollout_budget_units`, which `TokenUsage` skips when serialized.
    pub usage: TokenUsage,
}

/// Both decisions mean the host recorded the reported spending.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UsageAccountingDecision {
    Continue,
    Exhausted,
}

/// Replaces this runtime's `AgentControl::record_usage` and native rollout-budget reminders.
///
/// Install an `Arc<dyn AgentUsageAccounting>` in thread extension data before host-owned startup;
/// supply reminder context through context contributors. Core awaits each report. The host
/// owns budget policy, timeouts, report-only retries, reporting IDs, deduplication and recovery.
/// A turn may emit multiple reports; thread/turn IDs give attribution, not unique charges.
/// `Err` may leave acceptance unknown and causes a nonretryable failure without inference
/// retry or Local fallback. Incomplete responses retain their original error; background
/// compaction errors fail that job. This callback neither reserves future spending nor
/// interrupts other agents in the tree.
pub trait AgentUsageAccounting: Send + Sync {
    fn report_usage(
        &self,
        report: AgentUsageReport,
    ) -> BoxFuture<'_, Result<UsageAccountingDecision>>;
}
