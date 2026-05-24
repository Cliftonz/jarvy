//! Structured reporting for unsupported tools.
//!
//! When a user requests a tool that Jarvy doesn't know about, this module
//! produces a single `UnsupportedToolReport` payload that can be rendered
//! as human-readable text or JSON. The JSON form is the contract for AI
//! agents driving Jarvy via the MCP feedback loop — they read it, decide
//! whether to file a request or scaffold the tool locally, and try again.
//!
//! The report carries:
//! - `tool` / `version`: what was requested
//! - `suggestions`: top-N fuzzy matches from the registry (typos like
//!   `gti` → `git` resolve here without a round-trip)
//! - `request_url`: pre-filled GitHub issue using `tool_request.yml`
//! - `scaffold_cmd`: workspace command to generate a tool stub locally
//! - `docs_url`: tools docs
//! - `exit_code`: stable [`crate::error_codes::TOOL_UNSUPPORTED`]
//! - `kind`: discriminator (`"unsupported_tool"`) for AI parsers

#![allow(dead_code)] // Public API consumed by setup_cmd and tools_cmd

use serde::Serialize;

use crate::error_codes;
use crate::tools::spec;

/// GitHub repo slug used for the tool-request issue template.
/// Single source of truth — referenced by setup error messages and the
/// `jarvy tools --request` subcommand.
pub const REPO_SLUG: &str = "bearbinary/Jarvy";

/// Issue template filename (must match `.github/ISSUE_TEMPLATE/`).
const TEMPLATE_FILE: &str = "tool_request.yml";

/// Docs URL surfaced in the report.
const DOCS_URL: &str = "https://github.com/bearbinary/Jarvy#supported-tools";

/// How this request was (or will be) delivered to the maintainers.
///
/// Telemetry is the canonical channel: it requires no GitHub account
/// from the user (or AI agent) and zero triage work from the maintainer
/// beyond reading the aggregated counter. The GitHub issue URL stays in
/// the payload only as a fallback for users with telemetry disabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestChannel {
    /// Telemetry already fired for this request (explicit `--request`).
    Sent,
    /// Telemetry is enabled and will fire alongside this message
    /// (e.g. setup-path unknown-tool event).
    WillSend,
    /// Telemetry is disabled — only the GitHub URL remains as a route.
    Manual,
}

/// Structured payload describing an unsupported-tool event.
#[derive(Debug, Clone, Serialize)]
pub struct UnsupportedToolReport {
    /// Discriminator for AI parsers; always `"unsupported_tool"`.
    pub kind: &'static str,
    /// Tool name as the user wrote it.
    pub tool: String,
    /// Version string the user requested, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Top-N closest registered tool names (lowercased), sorted best-first.
    pub suggestions: Vec<String>,
    /// Canonical delivery channel — `telemetry` whenever possible,
    /// `manual` when telemetry is off and the user must use the URL.
    pub channel: &'static str,
    /// Fallback GitHub issue URL using the tool-request template. Used
    /// only when telemetry is unavailable; AI agents should prefer the
    /// telemetry channel because it requires no GitHub account.
    pub fallback_issue_url: String,
    /// Workspace command that scaffolds a new tool module locally.
    pub scaffold_cmd: String,
    /// Docs link with the canonical supported-tools list.
    pub docs_url: String,
    /// Stable process exit code matching [`error_codes::TOOL_UNSUPPORTED`].
    pub exit_code: i32,
}

/// Build a full report for a single unsupported tool request.
///
/// `channel` describes whether telemetry has fired / will fire / is off.
/// The choice flows into both the JSON `channel` field (for AI parsers)
/// and the human renderer (which de-emphasizes the GitHub URL whenever
/// telemetry covers the request).
pub fn build_report(
    tool: &str,
    version: Option<&str>,
    channel: RequestChannel,
) -> UnsupportedToolReport {
    UnsupportedToolReport {
        kind: "unsupported_tool",
        tool: tool.to_string(),
        version: version.map(str::to_string),
        suggestions: fuzzy_suggest(tool, 3),
        channel: match channel {
            RequestChannel::Sent | RequestChannel::WillSend => "telemetry",
            RequestChannel::Manual => "manual",
        },
        fallback_issue_url: issue_url(tool, version),
        scaffold_cmd: format!("cargo run -p cargo-jarvy -- new-tool {}", tool),
        docs_url: DOCS_URL.to_string(),
        exit_code: error_codes::TOOL_UNSUPPORTED,
    }
}

/// Return up to `limit` closest registered tool names to `query`.
///
/// Uses Levenshtein distance with an early-exit cutoff of
/// `max(2, query.len() / 2)` so we don't suggest wildly unrelated tools
/// for very short queries. Names that share the query as a prefix are
/// preferred (distance halved) — typo-correction usually wants prefix
/// matches over edit-distance ties.
pub fn fuzzy_suggest(query: &str, limit: usize) -> Vec<String> {
    if query.is_empty() || limit == 0 {
        return Vec::new();
    }
    let q = query.to_ascii_lowercase();
    let cutoff = std::cmp::max(2, q.len() / 2);

    let mut scored: Vec<(usize, String)> = spec::list_tool_names()
        .into_iter()
        .filter_map(|name| {
            let mut d = levenshtein(&q, &name);
            // Prefer prefix matches: `gti` vs `git` already wins on
            // edit-distance, but `dock` should rank `docker` above
            // `dotnet` even when distances are tied.
            if name.starts_with(&q) || q.starts_with(&name) {
                d /= 2;
            }
            if d <= cutoff { Some((d, name)) } else { None }
        })
        .collect();

    scored.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    scored.into_iter().take(limit).map(|(_, n)| n).collect()
}

/// Build a pre-filled GitHub issue URL for the tool-request template.
///
/// Query parameters target fields declared in
/// `.github/ISSUE_TEMPLATE/tool_request.yml`:
/// - `template` — selects the form
/// - `title` — pre-filled title (`[Tool]: <name>`)
/// - `tool_name` — auto-populates the first input
pub fn issue_url(tool: &str, version: Option<&str>) -> String {
    let mut url = format!(
        "https://github.com/{}/issues/new?template={}&labels=tool-request,needs-triage",
        REPO_SLUG, TEMPLATE_FILE
    );
    url.push_str("&title=");
    url.push_str(&pct_encode(&format!("[Tool]: {}", tool)));
    url.push_str("&tool_name=");
    url.push_str(&pct_encode(tool));
    if let Some(v) = version {
        url.push_str("&use_case=");
        url.push_str(&pct_encode(&format!(
            "Requested version: {} (auto-filed by `jarvy setup`).",
            v
        )));
    }
    url
}

/// Render the report as a multi-line human-readable block.
///
/// Lead with the delivery confirmation. The GitHub URL is only shown
/// when telemetry is off (`RequestChannel::Manual`) — surfacing it
/// alongside a successful telemetry send would push users into the
/// higher-friction path the user wanted to avoid.
pub fn to_human(report: &UnsupportedToolReport, channel: RequestChannel) -> String {
    let mut out = String::with_capacity(384);
    out.push_str(&format!(
        "[jarvy] tool `{}` is not in the Jarvy registry.\n",
        report.tool
    ));
    if !report.suggestions.is_empty() {
        out.push_str("        Did you mean: ");
        out.push_str(&report.suggestions.join(", "));
        out.push_str("?\n");
    }
    match channel {
        RequestChannel::Sent => {
            out.push_str("        Reported via telemetry — no further action needed.\n");
        }
        RequestChannel::WillSend => {
            out.push_str("        Reporting via telemetry.\n");
        }
        RequestChannel::Manual => {
            // Telemetry off — maintainer gets no signal unless the user
            // acts. Lead with the recommended action: the pre-filled
            // tool-request issue (auto-populates name/title/labels via
            // `.github/ISSUE_TEMPLATE/tool_request.yml`). Mention the
            // telemetry-enable shortcut as a one-time alternative for
            // users who'd rather not file.
            out.push_str("        Telemetry off — please file a tool request (pre-filled):\n");
            out.push_str("        ");
            out.push_str(&report.fallback_issue_url);
            out.push('\n');
            out.push_str(
                "        Or enable telemetry once with `jarvy telemetry enable` to skip the form.\n",
            );
        }
    }
    out.push_str("        Scaffold locally: ");
    out.push_str(&report.scaffold_cmd);
    out.push('\n');
    out
}

/// Render the report as a single-line JSON object suitable for log
/// pipelines and AI parsers. Falls back to a static error string if
/// serialization fails (it shouldn't — all fields are owned `String`s).
pub fn to_json(report: &UnsupportedToolReport) -> String {
    serde_json::to_string(report)
        .unwrap_or_else(|_| r#"{"kind":"unsupported_tool","error":"serialize_failed"}"#.to_string())
}

/// Render an inline `define_tool!` macro stub for the requested tool.
/// Used by `jarvy tools --request <name>` to give contributors a copy-
/// paste starting point.
pub fn scaffold_snippet(tool: &str) -> String {
    let upper = tool.to_ascii_uppercase().replace('-', "_");
    let lower = tool.to_ascii_lowercase();
    format!(
        r#"// src/tools/{lower}/{lower}.rs
use crate::define_tool;

define_tool!({upper}, {{
    command: "{lower}",
    macos: {{ brew: "{lower}" }},
    linux: {{ uniform: "{lower}" }},
    windows: {{ winget: "Publisher.{tool}" }},
}});
"#,
        upper = upper,
        lower = lower,
        tool = tool
    )
}

// --- internal helpers ----------------------------------------------------

/// Minimal percent-encoder for query-string values. Encodes everything
/// outside `[A-Za-z0-9_.~-]` per RFC 3986 unreserved set, plus space as
/// `%20` (not `+`, which is form-encoding, not URL-encoding). Tool names
/// and versions are ASCII, so we don't worry about multi-byte handling.
fn pct_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        let safe = b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~');
        if safe {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}

/// Levenshtein distance between two ASCII-lowercased strings.
/// Iterative two-row implementation — O(n*m) time, O(min(n,m)) space.
fn levenshtein(a: &str, b: &str) -> usize {
    let (a, b) = if a.len() < b.len() { (b, a) } else { (a, b) };
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let n = b_bytes.len();
    if n == 0 {
        return a_bytes.len();
    }
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr: Vec<usize> = vec![0; n + 1];
    for (i, &ac) in a_bytes.iter().enumerate() {
        curr[0] = i + 1;
        for (j, &bc) in b_bytes.iter().enumerate() {
            let cost = usize::from(ac != bc);
            curr[j + 1] = (prev[j + 1] + 1).min(curr[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pct_encode_preserves_unreserved() {
        assert_eq!(pct_encode("abc-XYZ_0.9~"), "abc-XYZ_0.9~");
    }

    #[test]
    fn pct_encode_escapes_space_and_brackets() {
        assert_eq!(pct_encode("[Tool]: foo bar"), "%5BTool%5D%3A%20foo%20bar");
    }

    #[test]
    fn levenshtein_basic() {
        assert_eq!(levenshtein("git", "git"), 0);
        assert_eq!(levenshtein("gti", "git"), 2); // swap counts as 2 edits
        assert_eq!(levenshtein("docker", "docke"), 1);
        assert_eq!(levenshtein("", "abc"), 3);
    }

    #[test]
    fn issue_url_contains_template_and_tool() {
        let url = issue_url("kubectl", Some("1.30"));
        assert!(url.contains("template=tool_request.yml"));
        assert!(url.contains("tool_name=kubectl"));
        assert!(url.contains("title=%5BTool%5D%3A%20kubectl"));
        assert!(url.contains("use_case="));
    }

    #[test]
    fn build_report_carries_exit_code() {
        let r = build_report("definitely-not-a-real-tool", None, RequestChannel::Sent);
        assert_eq!(r.kind, "unsupported_tool");
        assert_eq!(r.exit_code, error_codes::TOOL_UNSUPPORTED);
        assert!(r.scaffold_cmd.contains("definitely-not-a-real-tool"));
        assert_eq!(r.channel, "telemetry");
    }

    #[test]
    fn build_report_manual_channel_when_telemetry_off() {
        let r = build_report("foo", None, RequestChannel::Manual);
        assert_eq!(r.channel, "manual");
    }

    #[test]
    fn fuzzy_suggest_finds_close_match() {
        // git is a known tool — `gti` should suggest it.
        let s = fuzzy_suggest("gti", 3);
        assert!(s.contains(&"git".to_string()), "got: {:?}", s);
    }

    #[test]
    fn fuzzy_suggest_empty_query_returns_empty() {
        assert!(fuzzy_suggest("", 3).is_empty());
    }

    #[test]
    fn scaffold_snippet_includes_upper_and_lower() {
        let s = scaffold_snippet("foo-bar");
        assert!(s.contains("FOO_BAR"));
        assert!(s.contains("\"foo-bar\""));
    }

    #[test]
    fn to_json_roundtrips() {
        let r = build_report("xyz", Some("1.0"), RequestChannel::Sent);
        let j = to_json(&r);
        let v: serde_json::Value = serde_json::from_str(&j).unwrap();
        assert_eq!(v["kind"], "unsupported_tool");
        assert_eq!(v["tool"], "xyz");
        assert_eq!(v["exit_code"], 8);
        assert_eq!(v["channel"], "telemetry");
    }

    #[test]
    fn to_human_omits_url_when_telemetry_handles_it() {
        let r = build_report("foo", None, RequestChannel::Sent);
        let s = to_human(&r, RequestChannel::Sent);
        assert!(s.contains("Reported via telemetry"));
        assert!(
            !s.contains("github.com"),
            "URL should not appear when telemetry handles the request: {}",
            s
        );
    }

    #[test]
    fn to_human_shows_url_only_when_telemetry_off() {
        let r = build_report("foo", None, RequestChannel::Manual);
        let s = to_human(&r, RequestChannel::Manual);
        assert!(s.contains("Telemetry off"));
        assert!(s.contains("github.com"));
        // The issue path should be the recommended action, not a
        // secondary mention — verify the wording reflects that.
        assert!(
            s.contains("please file a tool request"),
            "Manual channel should recommend filing an issue: {}",
            s
        );
        assert!(
            s.contains("pre-filled"),
            "User should know the form is auto-populated"
        );
    }
}
