//! `metrics` crate integration — feature-gated metric emission.
//!
//! This module provides [`MetricsEmitter`], a thin wrapper that emits metrics
//! via the `metrics` crate facade. All methods compile to nothing when no
//! recorder is installed (the `metrics` crate's built-in no-op path).

use std::time::Duration;

use metrics::{counter, gauge, histogram, Label};

/// Thin wrapper that formats metric names with an optional prefix and
/// attaches global labels. All methods are `#[inline]` for zero overhead
/// when the recorder is a no-op.
#[allow(dead_code)]
pub(crate) struct MetricsEmitter {
    prefix: String,
    global_labels: Vec<Label>,
}

#[allow(dead_code)]
impl MetricsEmitter {
    pub(crate) fn new(prefix: Option<String>, global_labels: Vec<(String, String)>) -> Self {
        let prefix = match prefix {
            Some(p) => format!("{p}_taskmill_"),
            None => "taskmill_".to_string(),
        };
        let global_labels = global_labels
            .into_iter()
            .map(|(k, v)| Label::new(k, v))
            .collect();
        Self {
            prefix,
            global_labels,
        }
    }

    fn name(&self, suffix: &str) -> String {
        format!("{}{suffix}", self.prefix)
    }

    fn labels(&self, extra: &[(&str, &str)]) -> Vec<Label> {
        let mut labels: Vec<Label> = self.global_labels.clone();
        for (k, v) in extra {
            labels.push(Label::new(k.to_string(), v.to_string()));
        }
        labels
    }

    /// Register metric descriptions. Called once at scheduler build time.
    pub(crate) fn describe_metrics(&self) {
        use metrics::{describe_counter, describe_gauge, describe_histogram, Unit};

        // Counters
        describe_counter!(
            self.name("tasks_submitted_total"),
            Unit::Count,
            "Total tasks accepted into the queue"
        );
        describe_counter!(
            self.name("tasks_dispatched_total"),
            Unit::Count,
            "Total tasks that entered Running state"
        );
        describe_counter!(
            self.name("tasks_completed_total"),
            Unit::Count,
            "Total successful completions"
        );
        describe_counter!(
            self.name("tasks_failed_total"),
            Unit::Count,
            "Total failures, split by retryability"
        );
        describe_counter!(
            self.name("tasks_retried_total"),
            Unit::Count,
            "Total retry requeue attempts"
        );
        describe_counter!(
            self.name("tasks_dead_lettered_total"),
            Unit::Count,
            "Tasks that exhausted retries"
        );
        describe_counter!(
            self.name("tasks_superseded_total"),
            Unit::Count,
            "Tasks replaced by newer submissions with the same dedup key"
        );
        describe_counter!(
            self.name("tasks_cancelled_total"),
            Unit::Count,
            "Explicit cancellations"
        );
        describe_counter!(
            self.name("tasks_expired_total"),
            Unit::Count,
            "Tasks that hit TTL before dispatch"
        );
        describe_counter!(
            self.name("tasks_preempted_total"),
            Unit::Count,
            "Tasks preempted by higher-priority work"
        );
        describe_counter!(
            self.name("batches_submitted_total"),
            Unit::Count,
            "Total batch submission calls"
        );
        describe_counter!(
            self.name("gate_denials_total"),
            Unit::Count,
            "Dispatch gate rejections by reason"
        );
        describe_counter!(
            self.name("rate_limit_throttles_total"),
            Unit::Count,
            "Rate limit token exhaustion events per scope"
        );
        describe_counter!(
            self.name("group_pauses_total"),
            Unit::Count,
            "Group pause events"
        );
        describe_counter!(
            self.name("group_resumes_total"),
            Unit::Count,
            "Group resume events"
        );
        describe_counter!(
            self.name("dependency_failures_total"),
            Unit::Count,
            "Blocked tasks cancelled because a dependency failed"
        );
        describe_counter!(
            self.name("recurring_skipped_total"),
            Unit::Count,
            "Recurring instances skipped due to pile-up prevention"
        );

        // Gauges
        describe_gauge!(
            self.name("tasks_pending"),
            Unit::Count,
            "Current queue depth"
        );
        describe_gauge!(
            self.name("tasks_running"),
            Unit::Count,
            "Current running task count"
        );
        describe_gauge!(
            self.name("tasks_blocked"),
            Unit::Count,
            "Tasks waiting on unmet dependencies"
        );
        describe_gauge!(
            self.name("tasks_paused"),
            Unit::Count,
            "Tasks in pause state"
        );
        describe_gauge!(
            self.name("tasks_waiting"),
            Unit::Count,
            "Parent tasks waiting for children"
        );
        describe_gauge!(
            self.name("max_concurrency"),
            Unit::Count,
            "Current concurrency cap"
        );
        describe_gauge!(
            self.name("pressure"),
            Unit::Count,
            "Aggregate backpressure (0.0-1.0)"
        );
        describe_gauge!(
            self.name("pressure_source"),
            Unit::Count,
            "Per-source pressure level"
        );
        describe_gauge!(
            self.name("groups_paused_count"),
            Unit::Count,
            "Number of currently paused groups"
        );
        describe_gauge!(
            self.name("rate_limit_tokens_available"),
            Unit::Count,
            "Current available tokens per rate-limit bucket"
        );
        describe_gauge!(
            self.name("module_tasks_running"),
            Unit::Count,
            "Running tasks per registered module"
        );

        // Histograms
        describe_histogram!(
            self.name("task_duration_seconds"),
            Unit::Seconds,
            "Wall-clock execution time"
        );
        describe_histogram!(
            self.name("task_queue_wait_seconds"),
            Unit::Seconds,
            "Time from submission to dispatch start"
        );
        describe_histogram!(
            self.name("batch_size"),
            Unit::Count,
            "Number of tasks per batch submission call"
        );
        describe_histogram!(
            self.name("completion_batch_size"),
            Unit::Count,
            "Number of completions coalesced per drain cycle"
        );
        describe_histogram!(
            self.name("failure_batch_size"),
            Unit::Count,
            "Number of failures coalesced per drain cycle"
        );
    }

    // ── Counters ────────────────────────────────────────────────────

    #[inline]
    pub(crate) fn record_submitted(&self, task_type: &str, module: &str, group: Option<&str>) {
        let labels = self.labels(&[
            ("type", task_type),
            ("module", module),
            ("group", group.unwrap_or("")),
        ]);
        counter!(self.name("tasks_submitted_total"), labels).increment(1);
    }

    #[inline]
    pub(crate) fn record_dispatched(&self, task_type: &str, module: &str, group: Option<&str>) {
        let labels = self.labels(&[
            ("type", task_type),
            ("module", module),
            ("group", group.unwrap_or("")),
        ]);
        counter!(self.name("tasks_dispatched_total"), labels).increment(1);
    }

    #[inline]
    pub(crate) fn record_completed(&self, task_type: &str, module: &str, group: Option<&str>) {
        let labels = self.labels(&[
            ("type", task_type),
            ("module", module),
            ("group", group.unwrap_or("")),
        ]);
        counter!(self.name("tasks_completed_total"), labels).increment(1);
    }

    #[inline]
    pub(crate) fn record_failed(
        &self,
        task_type: &str,
        module: &str,
        group: Option<&str>,
        retryable: &str,
    ) {
        let labels = self.labels(&[
            ("type", task_type),
            ("module", module),
            ("group", group.unwrap_or("")),
            ("retryable", retryable),
        ]);
        counter!(self.name("tasks_failed_total"), labels).increment(1);
    }

    #[inline]
    pub(crate) fn record_retried(&self, task_type: &str, module: &str, group: Option<&str>) {
        let labels = self.labels(&[
            ("type", task_type),
            ("module", module),
            ("group", group.unwrap_or("")),
        ]);
        counter!(self.name("tasks_retried_total"), labels).increment(1);
    }

    #[inline]
    pub(crate) fn record_dead_lettered(&self, task_type: &str, module: &str, group: Option<&str>) {
        let labels = self.labels(&[
            ("type", task_type),
            ("module", module),
            ("group", group.unwrap_or("")),
        ]);
        counter!(self.name("tasks_dead_lettered_total"), labels).increment(1);
    }

    #[inline]
    pub(crate) fn record_superseded(&self, task_type: &str, module: &str, group: Option<&str>) {
        let labels = self.labels(&[
            ("type", task_type),
            ("module", module),
            ("group", group.unwrap_or("")),
        ]);
        counter!(self.name("tasks_superseded_total"), labels).increment(1);
    }

    #[inline]
    pub(crate) fn record_cancelled(&self, task_type: &str, module: &str, group: Option<&str>) {
        let labels = self.labels(&[
            ("type", task_type),
            ("module", module),
            ("group", group.unwrap_or("")),
        ]);
        counter!(self.name("tasks_cancelled_total"), labels).increment(1);
    }

    #[inline]
    pub(crate) fn record_expired(&self, task_type: &str, module: &str, group: Option<&str>) {
        let labels = self.labels(&[
            ("type", task_type),
            ("module", module),
            ("group", group.unwrap_or("")),
        ]);
        counter!(self.name("tasks_expired_total"), labels).increment(1);
    }

    #[inline]
    pub(crate) fn record_preempted(&self, task_type: &str, module: &str) {
        let labels = self.labels(&[("type", task_type), ("module", module)]);
        counter!(self.name("tasks_preempted_total"), labels).increment(1);
    }

    #[inline]
    pub(crate) fn record_batch_submitted(&self) {
        let labels = self.labels(&[]);
        counter!(self.name("batches_submitted_total"), labels).increment(1);
    }

    #[inline]
    pub(crate) fn record_gate_denial(&self, reason: &str) {
        let labels = self.labels(&[("reason", reason)]);
        counter!(self.name("gate_denials_total"), labels).increment(1);
    }

    #[inline]
    pub(crate) fn record_rate_limit_throttle(&self, scope_kind: &str, scope: &str) {
        let labels = self.labels(&[("scope_kind", scope_kind), ("scope", scope)]);
        counter!(self.name("rate_limit_throttles_total"), labels).increment(1);
    }

    #[inline]
    pub(crate) fn record_group_pause(&self, group: &str) {
        let labels = self.labels(&[("group", group)]);
        counter!(self.name("group_pauses_total"), labels).increment(1);
    }

    #[inline]
    pub(crate) fn record_group_resume(&self, group: &str) {
        let labels = self.labels(&[("group", group)]);
        counter!(self.name("group_resumes_total"), labels).increment(1);
    }

    #[inline]
    pub(crate) fn record_dependency_failure(&self) {
        let labels = self.labels(&[]);
        counter!(self.name("dependency_failures_total"), labels).increment(1);
    }

    #[inline]
    pub(crate) fn record_recurring_skipped(&self, task_type: &str, module: &str) {
        let labels = self.labels(&[("type", task_type), ("module", module)]);
        counter!(self.name("recurring_skipped_total"), labels).increment(1);
    }

    // ── Gauges ──────────────────────────────────────────────────────

    #[inline]
    pub(crate) fn set_gauge_pending(&self, value: i64) {
        let labels = self.labels(&[]);
        gauge!(self.name("tasks_pending"), labels).set(value as f64);
    }

    #[inline]
    pub(crate) fn set_gauge_running(&self, value: usize) {
        let labels = self.labels(&[]);
        gauge!(self.name("tasks_running"), labels).set(value as f64);
    }

    #[inline]
    pub(crate) fn set_gauge_blocked(&self, value: i64) {
        let labels = self.labels(&[]);
        gauge!(self.name("tasks_blocked"), labels).set(value as f64);
    }

    #[inline]
    pub(crate) fn set_gauge_paused(&self, value: i64) {
        let labels = self.labels(&[]);
        gauge!(self.name("tasks_paused"), labels).set(value as f64);
    }

    #[inline]
    pub(crate) fn set_gauge_waiting(&self, value: i64) {
        let labels = self.labels(&[]);
        gauge!(self.name("tasks_waiting"), labels).set(value as f64);
    }

    #[inline]
    pub(crate) fn set_gauge_max_concurrency(&self, value: usize) {
        let labels = self.labels(&[]);
        gauge!(self.name("max_concurrency"), labels).set(value as f64);
    }

    #[inline]
    pub(crate) fn set_gauge_pressure(&self, value: f32) {
        let labels = self.labels(&[]);
        gauge!(self.name("pressure"), labels).set(value as f64);
    }

    #[inline]
    pub(crate) fn set_gauge_pressure_source(&self, source: &str, value: f32) {
        let labels = self.labels(&[("source", source)]);
        gauge!(self.name("pressure_source"), labels).set(value as f64);
    }

    #[inline]
    pub(crate) fn set_gauge_groups_paused(&self, value: usize) {
        let labels = self.labels(&[]);
        gauge!(self.name("groups_paused_count"), labels).set(value as f64);
    }

    #[inline]
    pub(crate) fn set_gauge_rate_limit_tokens(&self, scope_kind: &str, scope: &str, value: f64) {
        let labels = self.labels(&[("scope_kind", scope_kind), ("scope", scope)]);
        gauge!(self.name("rate_limit_tokens_available"), labels).set(value);
    }

    #[inline]
    pub(crate) fn set_gauge_module_running(&self, module: &str, value: usize) {
        let labels = self.labels(&[("module", module)]);
        gauge!(self.name("module_tasks_running"), labels).set(value as f64);
    }

    // ── Histograms ──────────────────────────────────────────────────

    #[inline]
    pub(crate) fn record_duration(
        &self,
        duration: Duration,
        task_type: &str,
        module: &str,
        status: &str,
    ) {
        let labels = self.labels(&[("type", task_type), ("module", module), ("status", status)]);
        histogram!(self.name("task_duration_seconds"), labels).record(duration.as_secs_f64());
    }

    #[inline]
    pub(crate) fn record_queue_wait(&self, wait: Duration, task_type: &str, module: &str) {
        let labels = self.labels(&[("type", task_type), ("module", module)]);
        histogram!(self.name("task_queue_wait_seconds"), labels).record(wait.as_secs_f64());
    }

    #[inline]
    pub(crate) fn record_batch_size(&self, size: usize) {
        let labels = self.labels(&[]);
        histogram!(self.name("batch_size"), labels).record(size as f64);
    }

    #[inline]
    pub(crate) fn record_completion_batch_size(&self, size: usize) {
        let labels = self.labels(&[]);
        histogram!(self.name("completion_batch_size"), labels).record(size as f64);
    }

    #[inline]
    pub(crate) fn record_failure_batch_size(&self, size: usize) {
        let labels = self.labels(&[]);
        histogram!(self.name("failure_batch_size"), labels).record(size as f64);
    }
}
