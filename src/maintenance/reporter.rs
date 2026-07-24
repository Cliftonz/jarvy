//! Human + JSON output for freshness reports (PRD-057).

use super::checker::{Direction, Report};

/// One-line summary used by `jarvy setup`'s post-completion banner
/// and by `--quiet` invocations of `jarvy check-updates`. Returns
/// `None` when there's nothing worth telling the user (no updates,
/// no errors) so the setup path can skip printing anything at all.
pub fn setup_summary_line(report: &Report) -> Option<String> {
    if report.summary.updates_available == 0 && report.summary.errors == 0 {
        return None;
    }
    let mut parts = Vec::new();
    if report.summary.updates_available > 0 {
        let phrase = if report.summary.updates_available == 1 {
            "1 tool has a newer version available".to_string()
        } else {
            format!(
                "{} tools have newer versions available",
                report.summary.updates_available
            )
        };
        parts.push(phrase);
    }
    if report.summary.errors > 0 {
        parts.push(format!(
            "{} error{}",
            report.summary.errors,
            if report.summary.errors == 1 { "" } else { "s" }
        ));
    }
    Some(format!(
        "Freshness advisory: {} — run `jarvy check-updates` for details.",
        parts.join(", ")
    ))
}

/// Full human-formatted report body. `include_unchecked = false`
/// (default) collapses the unchecked bucket into a single
/// summary line; `true` breaks it out per tool. Mirrors the
/// PRD-057 shape.
pub fn human_with_options(report: &Report, include_unchecked: bool) -> String {
    render_human(report, include_unchecked)
}

fn render_human(report: &Report, include_unchecked: bool) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "Freshness advisory ({} tool{} checked, {} with newer versions):\n\n",
        report.summary.tools_checked,
        if report.summary.tools_checked == 1 {
            ""
        } else {
            "s"
        },
        report.summary.updates_available
    ));

    if !report.updates.is_empty() {
        for check in &report.updates {
            let arrow = match check.direction {
                Direction::Upgrade => "→",
                Direction::Downgrade => "↓",
                _ => "?",
            };
            out.push_str(&format!(
                "  {name:<20} {installed:<12} {arrow} {latest:<12} ({backend})\n",
                name = check.tool,
                installed = check.installed.as_deref().unwrap_or("?"),
                arrow = arrow,
                latest = check.latest.as_deref().unwrap_or("?"),
                backend = check.backend,
            ));
        }
        out.push('\n');
    }

    if !report.errors.is_empty() {
        out.push_str(&format!("Errors ({}):\n", report.errors.len()));
        for check in &report.errors {
            out.push_str(&format!(
                "  {} ({}): {}\n",
                check.tool,
                check.backend,
                check.error_kind.as_deref().unwrap_or("unknown")
            ));
        }
        out.push('\n');
    }

    if !report.unchecked.is_empty() {
        if include_unchecked {
            out.push_str(&format!(
                "{} tool{} skipped:\n",
                report.unchecked.len(),
                if report.unchecked.len() == 1 { "" } else { "s" }
            ));
            for skipped in &report.unchecked {
                match &skipped.manager {
                    Some(m) => out.push_str(&format!(
                        "  - {} ({}, via {})\n",
                        skipped.tool, skipped.reason, m
                    )),
                    None => out.push_str(&format!("  - {} ({})\n", skipped.tool, skipped.reason)),
                }
            }
            out.push('\n');
        } else {
            // Default: collapse into one line so casual runs
            // aren't flooded with rustup / nvm / pyenv noise.
            // `--include-unchecked` opts into the per-tool list.
            out.push_str(&format!(
                "{} tool{} skipped (version manager / custom install; pass \
                 `--include-unchecked` to list).\n\n",
                report.unchecked.len(),
                if report.unchecked.len() == 1 { "" } else { "s" }
            ));
        }
    }

    out.push_str("Run `jarvy check-updates --format json` for machine-readable output.\n");
    out.push_str("Configure via [maintenance] in jarvy.toml.\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::maintenance::checker::{ToolCheck, UncheckedTool};

    fn sample_report() -> Report {
        let mut r = Report::empty();
        r.updates.push(ToolCheck {
            tool: "jq".to_string(),
            backend: "brew".to_string(),
            installed: Some("1.7.1".to_string()),
            latest: Some("1.8.0".to_string()),
            error_kind: None,
            from_cache: false,
            direction: Direction::Upgrade,
        });
        r.up_to_date.push(ToolCheck {
            tool: "git".to_string(),
            backend: "brew".to_string(),
            installed: Some("2.45.0".to_string()),
            latest: Some("2.45.0".to_string()),
            error_kind: None,
            from_cache: false,
            direction: Direction::UpToDate,
        });
        r.unchecked.push(UncheckedTool {
            tool: "rust".to_string(),
            reason: "version_manager".to_string(),
            manager: Some("rustup".to_string()),
        });
        r.summary.tools_checked = 2;
        r.summary.updates_available = 1;
        r.summary.unchecked = 1;
        r
    }

    #[test]
    fn setup_summary_line_singular_form() {
        let r = sample_report();
        let line = setup_summary_line(&r).unwrap();
        assert!(
            line.contains("1 tool has a newer version available"),
            "{line}"
        );
    }

    #[test]
    fn setup_summary_line_returns_none_when_clean() {
        let mut r = Report::empty();
        r.summary.tools_checked = 5;
        assert!(setup_summary_line(&r).is_none());
    }

    #[test]
    fn human_renders_updates_and_unchecked() {
        let r = sample_report();
        let text = human_with_options(&r, true);
        assert!(text.contains("jq"));
        assert!(text.contains("1.7.1"));
        assert!(text.contains("1.8.0"));
        assert!(text.contains("brew"));
        assert!(text.contains("rust"));
        assert!(text.contains("rustup"));
    }
}
