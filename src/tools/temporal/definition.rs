//! temporal - Temporal workflow CLI
//!
//! Temporal is a durable execution / workflow orchestration engine —
//! think "queue plus retries plus history plus replayable code." The
//! `temporal` CLI talks to a Temporal cluster (or `temporal server
//! start-dev` for local) and is the canonical way to start workflows,
//! query state, and replay history. Common pairing with NATS / Kafka
//! in event-driven microservices.

use crate::define_tool;

define_tool!(TEMPORAL, {
    command: "temporal",
    repo: "temporalio/cli",
    macos: { brew: "temporal" },
    // Linux: install via Linuxbrew or release binary; no native
    // distro package.
    linux: { brew: "temporal" },
    // No first-party winget manifest as of 2026-06; the prior
    // `Temporal.TemporalCLI` id was never claimed. Windows users:
    // install from https://github.com/temporalio/cli/releases.
    category: "workflow",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temporal_registration_shape() {
        assert_eq!(TEMPORAL.command, "temporal");
        assert_eq!(TEMPORAL.category, Some("workflow"));
        let mac = TEMPORAL.macos.expect("temporal must support macOS");
        assert_eq!(mac.brew, Some("temporal"));
        let linux = TEMPORAL.linux.expect("temporal must support Linux");
        assert_eq!(linux.brew, Some("temporal"));
        assert!(TEMPORAL.windows.is_none(), "no first-party winget manifest");
    }
}
