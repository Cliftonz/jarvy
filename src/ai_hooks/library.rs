//! Built-in, curated hook scripts.
//!
//! Each [`LibraryHook`] is a Jarvy-authored guard that ships with the binary.
//! The user references it by name (`use = "block-rm-rf"`) and Jarvy expands
//! the entry into a concrete command for each target agent. Library hooks
//! are the only entries that pass the audit gate without
//! `allow_custom_commands = true`.
//!
//! # Robust command extraction
//!
//! Every script that needs to inspect the agent's command extracts it via
//! `jq`, falling back to a tightened `sed` regex when `jq` is missing.
//! The old `sed -n 's/.*"command":"\([^"]*\)".*/\1/p'` parser truncated
//! on the first raw `"` — a prompt-injection attacker could ship
//! `{"command":"echo \"safe\" ; rm -rf /"}` and the deny regex would see
//! only `echo \`. The jq-based extractor handles JSON-escaped quotes
//! correctly. See `docs/ai-hooks.md` for the bypass history.

use super::event::HookEvent;

/// Static definition of a library hook entry.
#[derive(Clone, Debug)]
pub struct LibraryHook {
    pub name: &'static str,
    pub description: &'static str,
    pub event: HookEvent,
    pub matcher: Option<&'static str>,
    pub bash: &'static str,
    pub powershell: &'static str,
    pub timeout_ms: u64,
}

/// All built-in library hooks. Append-only; new hooks add a row here
/// + a positive/negative case in `tests/ai_hooks_library_matrix.rs`.
pub const LIBRARY: &[LibraryHook] = &[
    LibraryHook {
        name: "block-rm-rf",
        description: "Block destructive recursive removes (rm -rf /, sudo rm -rf, /bin/rm -rf).",
        event: HookEvent::PreToolUse,
        matcher: Some("Bash"),
        bash: BLOCK_RM_RF_BASH,
        powershell: BLOCK_RM_RF_PS,
        timeout_ms: 5_000,
    },
    LibraryHook {
        name: "block-force-push",
        description: "Block `git push --force` / `-f` / `--set-upstream` to protected refs.",
        event: HookEvent::PreToolUse,
        matcher: Some("Bash"),
        bash: BLOCK_FORCE_PUSH_BASH,
        powershell: BLOCK_FORCE_PUSH_PS,
        timeout_ms: 5_000,
    },
    LibraryHook {
        name: "block-secrets-commit",
        description: "Block `git commit` when staged diff contains AWS / GitHub / generic API key shapes.",
        event: HookEvent::PreToolUse,
        matcher: Some("Bash"),
        bash: BLOCK_SECRETS_BASH,
        powershell: BLOCK_SECRETS_PS,
        timeout_ms: 10_000,
    },
    LibraryHook {
        name: "block-curl-bash-pipe",
        description: "Block `curl | bash` / `wget | sh` patterns (untrusted remote execution).",
        event: HookEvent::PreToolUse,
        matcher: Some("Bash"),
        bash: BLOCK_CURL_PIPE_BASH,
        powershell: BLOCK_CURL_PIPE_PS,
        timeout_ms: 5_000,
    },
    LibraryHook {
        name: "block-prod-db-write",
        description: "Block writes to URLs containing `prod`, `production`, or RDS hostnames.",
        event: HookEvent::PreToolUse,
        matcher: Some("Bash"),
        bash: BLOCK_PROD_DB_BASH,
        powershell: BLOCK_PROD_DB_PS,
        timeout_ms: 5_000,
    },
    LibraryHook {
        name: "block-git-reset-hard",
        description: "Block `git reset --hard` (silent destruction of uncommitted work).",
        event: HookEvent::PreToolUse,
        matcher: Some("Bash"),
        bash: BLOCK_GIT_RESET_BASH,
        powershell: BLOCK_GIT_RESET_PS,
        timeout_ms: 5_000,
    },
    LibraryHook {
        name: "block-protected-branch-commit",
        description: "Block direct `git push` to `main`/`master`/`production` (including HEAD: refspecs).",
        event: HookEvent::PreToolUse,
        matcher: Some("Bash"),
        bash: BLOCK_PROTECTED_BRANCH_BASH,
        powershell: BLOCK_PROTECTED_BRANCH_PS,
        timeout_ms: 5_000,
    },
    LibraryHook {
        name: "block-kubectl-delete",
        description: "Block `kubectl delete` (namespace/deployment/cluster wipes).",
        event: HookEvent::PreToolUse,
        matcher: Some("Bash"),
        bash: BLOCK_KUBECTL_DELETE_BASH,
        powershell: BLOCK_KUBECTL_DELETE_PS,
        timeout_ms: 5_000,
    },
    LibraryHook {
        name: "block-docker-prune",
        description: "Block `docker system/volume/image prune` (volume + state loss).",
        event: HookEvent::PreToolUse,
        matcher: Some("Bash"),
        bash: BLOCK_DOCKER_PRUNE_BASH,
        powershell: BLOCK_DOCKER_PRUNE_PS,
        timeout_ms: 5_000,
    },
    LibraryHook {
        name: "block-drop-table",
        description: "Block `DROP TABLE`, `TRUNCATE`, `DELETE FROM` (no WHERE) in shell SQL.",
        event: HookEvent::PreToolUse,
        matcher: Some("Bash"),
        bash: BLOCK_DROP_TABLE_BASH,
        powershell: BLOCK_DROP_TABLE_PS,
        timeout_ms: 5_000,
    },
    LibraryHook {
        name: "block-edit-env-files",
        description: "Block edits/writes to `.env*`, `*.pem`, `*.key`, credential files (Edit/Write/MultiEdit).",
        event: HookEvent::PreToolUse,
        // `None` = all tools — script inspects file_path and exits 0 for
        // non-write tools. Catches Edit, Write, MultiEdit, Patch
        // uniformly. Per-agent tool naming differs (see docs/ai-hooks.md).
        matcher: None,
        bash: BLOCK_EDIT_ENV_BASH,
        powershell: BLOCK_EDIT_ENV_PS,
        timeout_ms: 5_000,
    },
    LibraryHook {
        name: "block-read-secret-files",
        description: "Block reading `.env`, `~/.ssh/`, `~/.aws/`, `~/.kube/`, `~/.gnupg/`.",
        event: HookEvent::PreToolUse,
        matcher: Some("Read"),
        bash: BLOCK_READ_SECRET_BASH,
        powershell: BLOCK_READ_SECRET_PS,
        timeout_ms: 5_000,
    },
    LibraryHook {
        name: "block-cat-env-files",
        description: "Block `cat .env`, `printenv`, `env` shell exfiltration.",
        event: HookEvent::PreToolUse,
        matcher: Some("Bash"),
        bash: BLOCK_CAT_ENV_BASH,
        powershell: BLOCK_CAT_ENV_PS,
        timeout_ms: 5_000,
    },
    LibraryHook {
        name: "block-malware-install",
        description: "Block `npm install`/`pip install`/`cargo install` of names in a static malware deny list.",
        event: HookEvent::PreToolUse,
        matcher: Some("Bash"),
        bash: BLOCK_MALWARE_BASH,
        powershell: BLOCK_MALWARE_PS,
        timeout_ms: 5_000,
    },
    LibraryHook {
        name: "audit-log",
        description: "Append every tool call to `~/.jarvy/logs/ai-hooks-audit.jsonl`. Redacts secrets, 0600 perms.",
        event: HookEvent::PreToolUse,
        matcher: None,
        bash: AUDIT_LOG_BASH,
        powershell: AUDIT_LOG_PS,
        timeout_ms: 3_000,
    },
    LibraryHook {
        name: "commit-message-format-guard",
        description: "Block `git commit -m` without a Conventional Commits prefix.",
        event: HookEvent::PreToolUse,
        matcher: Some("Bash"),
        bash: COMMIT_FORMAT_BASH,
        powershell: COMMIT_FORMAT_PS,
        timeout_ms: 5_000,
    },
];

/// Lookup by name (case-insensitive).
pub fn find(name: &str) -> Option<&'static LibraryHook> {
    LIBRARY.iter().find(|h| h.name.eq_ignore_ascii_case(name))
}

// ---------------------------------------------------------------------------
// Hook bodies
//
// Every command-introspecting hook starts with a shared `_extract` shim
// that prefers `jq` (handles JSON-escaped quotes) and falls back to a
// tightened sed that at least anchors on the boundary of the field. The
// regexes that follow are case-insensitive (`grep -Ei`) to catch
// `RM -rf`, `Git Push --force`, etc.
//
// The shim is inlined into each hook body so the library remains a flat
// `&'static str` registry — no runtime templating, no dependencies.
// ---------------------------------------------------------------------------

const EXTRACT_CMD: &str = r#"_jarvy_extract_cmd() {
  local payload="$1"
  if command -v jq >/dev/null 2>&1; then
    printf '%s' "$payload" \
      | jq -r '(.tool_input.command // .command // .command_line // empty)' 2>/dev/null \
      | head -1
  else
    # Best-effort fallback when jq is missing. Greedy match to the LAST
    # closing `"` on the line, then strip trailing keys. Still bypassable
    # on adversarial payloads — install jq for robust enforcement.
    printf '%s' "$payload" \
      | sed -n 's/.*"\(command\|command_line\)"[[:space:]]*:[[:space:]]*"\(.*\)"[^"]*$/\2/p' \
      | head -1
  fi
}"#;

const EXTRACT_PATH: &str = r#"_jarvy_extract_path() {
  local payload="$1"
  if command -v jq >/dev/null 2>&1; then
    printf '%s' "$payload" \
      | jq -r '(.tool_input.file_path // .tool_input.path // .file_path // .path // empty)' 2>/dev/null \
      | head -1
  else
    printf '%s' "$payload" \
      | sed -n 's/.*"\(file_path\|path\)"[[:space:]]*:[[:space:]]*"\(.*\)"[^"]*$/\2/p' \
      | head -1
  fi
}"#;

const BLOCK_RM_RF_BASH: &str = r#"#!/usr/bin/env bash
set -u
payload="$(cat)"
_jarvy_extract_cmd() {
  local p="$1"
  if command -v jq >/dev/null 2>&1; then
    printf '%s' "$p" | jq -r '(.tool_input.command // .command // .command_line // empty)' 2>/dev/null | head -1
  else
    printf '%s' "$p" | sed -n 's/.*"\(command\|command_line\)"[[:space:]]*:[[:space:]]*"\(.*\)"[^"]*$/\2/p' | head -1
  fi
}
cmd="$(_jarvy_extract_cmd "$payload")"
# Case-insensitive, allow absolute paths (/bin/rm), allow rm short forms (-rf, -fr, -r -f).
if printf '%s' "$cmd" | grep -Eiq '(^|[[:space:]/])(sudo[[:space:]]+)?(rm|/bin/rm|/usr/bin/rm)[[:space:]]+(-[a-zA-Z]*[rfRF][a-zA-Z]*[rfRF][a-zA-Z]*|-r[[:space:]]+-f|-f[[:space:]]+-r|--recursive[[:space:]]+--force|--force[[:space:]]+--recursive)'; then
  echo "jarvy: refusing rm -rf via AI agent (configure allow list to override)" >&2
  exit 2
fi
exit 0
"#;

const BLOCK_RM_RF_PS: &str = r#"$payload = [Console]::In.ReadToEnd()
function Extract-Command($p) {
  if (Get-Command jq -ErrorAction SilentlyContinue) {
    return ($p | jq -r '(.tool_input.command // .command // .command_line // empty)' 2>$null | Select-Object -First 1)
  }
  if ($p -match '"command"\s*:\s*"((?:[^"\\]|\\.)*)"') { return $Matches[1].Replace('\"','"') }
  return ''
}
$cmd = Extract-Command $payload
if ($cmd -match '(?i)(^|[\s/])(sudo\s+)?(rm|Remove-Item)\s+(-[a-zA-Z]*[rfRF][a-zA-Z]*[rfRF][a-zA-Z]*|-Recurse\s+-Force|--recursive\s+--force)') {
  [Console]::Error.WriteLine('jarvy: refusing rm -rf via AI agent')
  exit 2
}
exit 0
"#;

const BLOCK_FORCE_PUSH_BASH: &str = r#"#!/usr/bin/env bash
set -u
payload="$(cat)"
_jarvy_extract_cmd() {
  local p="$1"
  if command -v jq >/dev/null 2>&1; then
    printf '%s' "$p" | jq -r '(.tool_input.command // .command // .command_line // empty)' 2>/dev/null | head -1
  else
    printf '%s' "$p" | sed -n 's/.*"\(command\|command_line\)"[[:space:]]*:[[:space:]]*"\(.*\)"[^"]*$/\2/p' | head -1
  fi
}
cmd="$(_jarvy_extract_cmd "$payload")"
if printf '%s' "$cmd" | grep -Eiq 'git[[:space:]]+push[[:space:]]+([^&|;]*[[:space:]])?(-f([[:space:]]|$)|--force([[:space:]]|$)|--force-with-lease)'; then
  echo "jarvy: refusing git push --force (rebase or open a PR instead)" >&2
  exit 2
fi
exit 0
"#;

const BLOCK_FORCE_PUSH_PS: &str = r#"$payload = [Console]::In.ReadToEnd()
function Extract-Command($p) {
  if (Get-Command jq -ErrorAction SilentlyContinue) {
    return ($p | jq -r '(.tool_input.command // .command // .command_line // empty)' 2>$null | Select-Object -First 1)
  }
  if ($p -match '"command"\s*:\s*"((?:[^"\\]|\\.)*)"') { return $Matches[1].Replace('\"','"') }
  return ''
}
$cmd = Extract-Command $payload
if ($cmd -match '(?i)git\s+push\s+([^&|;]*\s)?(-f(\s|$)|--force(\s|$)|--force-with-lease)') {
  [Console]::Error.WriteLine('jarvy: refusing git push --force')
  exit 2
}
exit 0
"#;

const BLOCK_SECRETS_BASH: &str = r#"#!/usr/bin/env bash
set -u
payload="$(cat)"
_jarvy_extract_cmd() {
  local p="$1"
  if command -v jq >/dev/null 2>&1; then
    printf '%s' "$p" | jq -r '(.tool_input.command // .command // .command_line // empty)' 2>/dev/null | head -1
  else
    printf '%s' "$p" | sed -n 's/.*"\(command\|command_line\)"[[:space:]]*:[[:space:]]*"\(.*\)"[^"]*$/\2/p' | head -1
  fi
}
cmd="$(_jarvy_extract_cmd "$payload")"
# Tight literal boundary — only act on real `git commit ` invocations,
# not `cargo git commit-stats` or `npm install git-commit-msg`.
case "$cmd" in
  "git commit "*|"git commit"|"git ci "*|"git ci") ;;
  *) exit 0 ;;
esac
diff="$(git diff --cached 2>/dev/null || true)"
if printf '%s' "$diff" | grep -Eq '(AKIA[0-9A-Z]{16}|ghp_[A-Za-z0-9]{30,}|gho_[A-Za-z0-9]{30,}|sk-[A-Za-z0-9]{30,}|-----BEGIN[[:space:]]+(RSA|OPENSSH|EC|DSA|PGP|PRIVATE)[[:space:]]+(KEY|PRIVATE KEY)-----)'; then
  echo "jarvy: refusing commit — staged diff appears to contain a secret" >&2
  exit 2
fi
exit 0
"#;

const BLOCK_SECRETS_PS: &str = r#"$payload = [Console]::In.ReadToEnd()
function Extract-Command($p) {
  if (Get-Command jq -ErrorAction SilentlyContinue) {
    return ($p | jq -r '(.tool_input.command // .command // .command_line // empty)' 2>$null | Select-Object -First 1)
  }
  if ($p -match '"command"\s*:\s*"((?:[^"\\]|\\.)*)"') { return $Matches[1].Replace('\"','"') }
  return ''
}
$cmd = Extract-Command $payload
if (-not ($cmd -match '^(git commit( |$)|git ci( |$))')) { exit 0 }
$diff = git diff --cached 2>$null
if ($diff -match '(AKIA[0-9A-Z]{16}|ghp_[A-Za-z0-9]{30,}|gho_[A-Za-z0-9]{30,}|sk-[A-Za-z0-9]{30,}|-----BEGIN\s+(RSA|OPENSSH|EC|DSA|PGP|PRIVATE)\s+(KEY|PRIVATE KEY)-----)') {
  [Console]::Error.WriteLine('jarvy: refusing commit — staged diff appears to contain a secret')
  exit 2
}
exit 0
"#;

const BLOCK_CURL_PIPE_BASH: &str = r#"#!/usr/bin/env bash
set -u
payload="$(cat)"
_jarvy_extract_cmd() {
  local p="$1"
  if command -v jq >/dev/null 2>&1; then
    printf '%s' "$p" | jq -r '(.tool_input.command // .command // .command_line // empty)' 2>/dev/null | head -1
  else
    printf '%s' "$p" | sed -n 's/.*"\(command\|command_line\)"[[:space:]]*:[[:space:]]*"\(.*\)"[^"]*$/\2/p' | head -1
  fi
}
cmd="$(_jarvy_extract_cmd "$payload")"
if printf '%s' "$cmd" | grep -Eiq '(curl|wget)[[:space:]][^|;&]*\|[[:space:]]*(bash|sh|zsh|fish)([[:space:]]|$)'; then
  echo "jarvy: refusing curl|bash pipe — download, inspect, then run" >&2
  exit 2
fi
exit 0
"#;

const BLOCK_CURL_PIPE_PS: &str = r#"$payload = [Console]::In.ReadToEnd()
function Extract-Command($p) {
  if (Get-Command jq -ErrorAction SilentlyContinue) {
    return ($p | jq -r '(.tool_input.command // .command // .command_line // empty)' 2>$null | Select-Object -First 1)
  }
  if ($p -match '"command"\s*:\s*"((?:[^"\\]|\\.)*)"') { return $Matches[1].Replace('\"','"') }
  return ''
}
$cmd = Extract-Command $payload
if ($cmd -match '(?i)(curl|wget|Invoke-WebRequest|iwr)[^|;&]*\|\s*(bash|sh|powershell|pwsh|iex)') {
  [Console]::Error.WriteLine('jarvy: refusing curl|bash pipe')
  exit 2
}
exit 0
"#;

const BLOCK_PROD_DB_BASH: &str = r#"#!/usr/bin/env bash
set -u
payload="$(cat)"
_jarvy_extract_cmd() {
  local p="$1"
  if command -v jq >/dev/null 2>&1; then
    printf '%s' "$p" | jq -r '(.tool_input.command // .command // .command_line // empty)' 2>/dev/null | head -1
  else
    printf '%s' "$p" | sed -n 's/.*"\(command\|command_line\)"[[:space:]]*:[[:space:]]*"\(.*\)"[^"]*$/\2/p' | head -1
  fi
}
cmd="$(_jarvy_extract_cmd "$payload")"
if printf '%s' "$cmd" | grep -Eiq '(psql|mysql|mongo|redis-cli)[[:space:]][^|;&]*(prod|production|\.rds\.amazonaws\.com)'; then
  echo "jarvy: refusing direct write to production database host" >&2
  exit 2
fi
exit 0
"#;

const BLOCK_PROD_DB_PS: &str = r#"$payload = [Console]::In.ReadToEnd()
function Extract-Command($p) {
  if (Get-Command jq -ErrorAction SilentlyContinue) {
    return ($p | jq -r '(.tool_input.command // .command // .command_line // empty)' 2>$null | Select-Object -First 1)
  }
  if ($p -match '"command"\s*:\s*"((?:[^"\\]|\\.)*)"') { return $Matches[1].Replace('\"','"') }
  return ''
}
$cmd = Extract-Command $payload
if ($cmd -match '(?i)(psql|mysql|mongo|redis-cli)[^|;&]*(prod|production|\.rds\.amazonaws\.com)') {
  [Console]::Error.WriteLine('jarvy: refusing direct write to production database host')
  exit 2
}
exit 0
"#;

const BLOCK_GIT_RESET_BASH: &str = r#"#!/usr/bin/env bash
set -u
payload="$(cat)"
_jarvy_extract_cmd() {
  local p="$1"
  if command -v jq >/dev/null 2>&1; then
    printf '%s' "$p" | jq -r '(.tool_input.command // .command // .command_line // empty)' 2>/dev/null | head -1
  else
    printf '%s' "$p" | sed -n 's/.*"\(command\|command_line\)"[[:space:]]*:[[:space:]]*"\(.*\)"[^"]*$/\2/p' | head -1
  fi
}
cmd="$(_jarvy_extract_cmd "$payload")"
if printf '%s' "$cmd" | grep -Eiq 'git[[:space:]]+reset[[:space:]]+(--hard|-h([[:space:]]|$))'; then
  echo "jarvy: refusing git reset --hard (use stash + reset --mixed instead)" >&2
  exit 2
fi
exit 0
"#;

const BLOCK_GIT_RESET_PS: &str = r#"$payload = [Console]::In.ReadToEnd()
function Extract-Command($p) {
  if (Get-Command jq -ErrorAction SilentlyContinue) {
    return ($p | jq -r '(.tool_input.command // .command // .command_line // empty)' 2>$null | Select-Object -First 1)
  }
  if ($p -match '"command"\s*:\s*"((?:[^"\\]|\\.)*)"') { return $Matches[1].Replace('\"','"') }
  return ''
}
$cmd = Extract-Command $payload
if ($cmd -match '(?i)git\s+reset\s+--hard') {
  [Console]::Error.WriteLine('jarvy: refusing git reset --hard')
  exit 2
}
exit 0
"#;

const BLOCK_PROTECTED_BRANCH_BASH: &str = r#"#!/usr/bin/env bash
set -u
payload="$(cat)"
_jarvy_extract_cmd() {
  local p="$1"
  if command -v jq >/dev/null 2>&1; then
    printf '%s' "$p" | jq -r '(.tool_input.command // .command // .command_line // empty)' 2>/dev/null | head -1
  else
    printf '%s' "$p" | sed -n 's/.*"\(command\|command_line\)"[[:space:]]*:[[:space:]]*"\(.*\)"[^"]*$/\2/p' | head -1
  fi
}
cmd="$(_jarvy_extract_cmd "$payload")"
# Cover: simple `git push origin main`, refspec `HEAD:main`, force refspec `+main`,
# `--set-upstream origin main`, `-u origin main`.
if printf '%s' "$cmd" | grep -Eiq 'git[[:space:]]+push([[:space:]]+(-u|--set-upstream))?[[:space:]]+([^[:space:]]+[[:space:]]+)?(\+?(main|master|production|release)|HEAD:(main|master|production|release))([[:space:]]|$)'; then
  echo "jarvy: refusing direct push to protected branch (open a PR instead)" >&2
  exit 2
fi
exit 0
"#;

const BLOCK_PROTECTED_BRANCH_PS: &str = r#"$payload = [Console]::In.ReadToEnd()
function Extract-Command($p) {
  if (Get-Command jq -ErrorAction SilentlyContinue) {
    return ($p | jq -r '(.tool_input.command // .command // .command_line // empty)' 2>$null | Select-Object -First 1)
  }
  if ($p -match '"command"\s*:\s*"((?:[^"\\]|\\.)*)"') { return $Matches[1].Replace('\"','"') }
  return ''
}
$cmd = Extract-Command $payload
if ($cmd -match '(?i)git\s+push(\s+(-u|--set-upstream))?\s+([^\s]+\s+)?(\+?(main|master|production|release)|HEAD:(main|master|production|release))(\s|$)') {
  [Console]::Error.WriteLine('jarvy: refusing direct push to protected branch')
  exit 2
}
exit 0
"#;

const BLOCK_KUBECTL_DELETE_BASH: &str = r#"#!/usr/bin/env bash
set -u
payload="$(cat)"
_jarvy_extract_cmd() {
  local p="$1"
  if command -v jq >/dev/null 2>&1; then
    printf '%s' "$p" | jq -r '(.tool_input.command // .command // .command_line // empty)' 2>/dev/null | head -1
  else
    printf '%s' "$p" | sed -n 's/.*"\(command\|command_line\)"[[:space:]]*:[[:space:]]*"\(.*\)"[^"]*$/\2/p' | head -1
  fi
}
cmd="$(_jarvy_extract_cmd "$payload")"
if printf '%s' "$cmd" | grep -Eiq 'kubectl[[:space:]]+(--[a-z=_-]+[[:space:]]+)?delete[[:space:]]+(namespace|ns|deployment|deploy|cluster|all|--all)'; then
  echo "jarvy: refusing kubectl delete (high blast radius — use --dry-run=client first)" >&2
  exit 2
fi
exit 0
"#;

const BLOCK_KUBECTL_DELETE_PS: &str = r#"$payload = [Console]::In.ReadToEnd()
function Extract-Command($p) {
  if (Get-Command jq -ErrorAction SilentlyContinue) {
    return ($p | jq -r '(.tool_input.command // .command // .command_line // empty)' 2>$null | Select-Object -First 1)
  }
  if ($p -match '"command"\s*:\s*"((?:[^"\\]|\\.)*)"') { return $Matches[1].Replace('\"','"') }
  return ''
}
$cmd = Extract-Command $payload
if ($cmd -match '(?i)kubectl\s+(--[a-z=_-]+\s+)?delete\s+(namespace|ns|deployment|deploy|cluster|all|--all)') {
  [Console]::Error.WriteLine('jarvy: refusing kubectl delete')
  exit 2
}
exit 0
"#;

const BLOCK_DOCKER_PRUNE_BASH: &str = r#"#!/usr/bin/env bash
set -u
payload="$(cat)"
_jarvy_extract_cmd() {
  local p="$1"
  if command -v jq >/dev/null 2>&1; then
    printf '%s' "$p" | jq -r '(.tool_input.command // .command // .command_line // empty)' 2>/dev/null | head -1
  else
    printf '%s' "$p" | sed -n 's/.*"\(command\|command_line\)"[[:space:]]*:[[:space:]]*"\(.*\)"[^"]*$/\2/p' | head -1
  fi
}
cmd="$(_jarvy_extract_cmd "$payload")"
if printf '%s' "$cmd" | grep -Eiq 'docker[[:space:]]+(system|volume|image|builder|container|network)[[:space:]]+prune'; then
  echo "jarvy: refusing docker prune (destroys volumes/build cache)" >&2
  exit 2
fi
exit 0
"#;

const BLOCK_DOCKER_PRUNE_PS: &str = r#"$payload = [Console]::In.ReadToEnd()
function Extract-Command($p) {
  if (Get-Command jq -ErrorAction SilentlyContinue) {
    return ($p | jq -r '(.tool_input.command // .command // .command_line // empty)' 2>$null | Select-Object -First 1)
  }
  if ($p -match '"command"\s*:\s*"((?:[^"\\]|\\.)*)"') { return $Matches[1].Replace('\"','"') }
  return ''
}
$cmd = Extract-Command $payload
if ($cmd -match '(?i)docker\s+(system|volume|image|builder|container|network)\s+prune') {
  [Console]::Error.WriteLine('jarvy: refusing docker prune')
  exit 2
}
exit 0
"#;

const BLOCK_DROP_TABLE_BASH: &str = r#"#!/usr/bin/env bash
set -u
payload="$(cat)"
_jarvy_extract_cmd() {
  local p="$1"
  if command -v jq >/dev/null 2>&1; then
    printf '%s' "$p" | jq -r '(.tool_input.command // .command // .command_line // empty)' 2>/dev/null | head -1
  else
    printf '%s' "$p" | sed -n 's/.*"\(command\|command_line\)"[[:space:]]*:[[:space:]]*"\(.*\)"[^"]*$/\2/p' | head -1
  fi
}
cmd="$(_jarvy_extract_cmd "$payload")"
if printf '%s' "$cmd" | grep -Eiq '(drop[[:space:]]+(table|database|schema)|truncate[[:space:]]+table|delete[[:space:]]+from[[:space:]]+[a-zA-Z_][a-zA-Z0-9_]*[[:space:]]*;?[[:space:]]*$)'; then
  echo "jarvy: refusing destructive SQL (DROP/TRUNCATE/DELETE without WHERE)" >&2
  exit 2
fi
exit 0
"#;

const BLOCK_DROP_TABLE_PS: &str = r#"$payload = [Console]::In.ReadToEnd()
function Extract-Command($p) {
  if (Get-Command jq -ErrorAction SilentlyContinue) {
    return ($p | jq -r '(.tool_input.command // .command // .command_line // empty)' 2>$null | Select-Object -First 1)
  }
  if ($p -match '"command"\s*:\s*"((?:[^"\\]|\\.)*)"') { return $Matches[1].Replace('\"','"') }
  return ''
}
$cmd = Extract-Command $payload
if ($cmd -match '(?i)(drop\s+(table|database|schema)|truncate\s+table|delete\s+from\s+[a-zA-Z_][a-zA-Z0-9_]*\s*;?\s*$)') {
  [Console]::Error.WriteLine('jarvy: refusing destructive SQL')
  exit 2
}
exit 0
"#;

const BLOCK_EDIT_ENV_BASH: &str = r#"#!/usr/bin/env bash
set -u
payload="$(cat)"
_jarvy_extract_path() {
  local p="$1"
  if command -v jq >/dev/null 2>&1; then
    printf '%s' "$p" | jq -r '(.tool_input.file_path // .tool_input.path // .file_path // .path // empty)' 2>/dev/null | head -1
  else
    printf '%s' "$p" | sed -n 's/.*"\(file_path\|path\)"[[:space:]]*:[[:space:]]*"\(.*\)"[^"]*$/\2/p' | head -1
  fi
}
# Only act on write-side tools. Non-write tools (Read, Bash, Glob, ...) get exit 0.
tool="$(printf '%s' "$payload" | jq -r '(.tool_name // empty)' 2>/dev/null | head -1)"
case "$tool" in
  Edit|Write|MultiEdit|Patch|write_to_file|str_replace_editor|write_code|edit_file) ;;
  '') ;;  # Cursor/Windsurf encode intent in event, not tool_name — still inspect path
  *) exit 0 ;;
esac
path="$(_jarvy_extract_path "$payload")"
case "$path" in
  *.env|*.env.*|*/.envrc|*.pem|*.key|*/credentials*|*/secrets*|*kubeconfig*|*.p12|*.pfx|credentials.json|*serviceaccount*.json)
    echo "jarvy: refusing edit of credential file: $path" >&2
    exit 2 ;;
esac
exit 0
"#;

const BLOCK_EDIT_ENV_PS: &str = r#"$payload = [Console]::In.ReadToEnd()
function Extract-Path($p) {
  if (Get-Command jq -ErrorAction SilentlyContinue) {
    return ($p | jq -r '(.tool_input.file_path // .tool_input.path // .file_path // .path // empty)' 2>$null | Select-Object -First 1)
  }
  if ($p -match '"file_path"\s*:\s*"((?:[^"\\]|\\.)*)"') { return $Matches[1].Replace('\"','"') }
  return ''
}
$tool = ''
if (Get-Command jq -ErrorAction SilentlyContinue) {
  $tool = ($payload | jq -r '(.tool_name // empty)' 2>$null | Select-Object -First 1)
}
if ($tool -and $tool -notmatch '^(Edit|Write|MultiEdit|Patch|write_to_file|str_replace_editor|write_code|edit_file)$') { exit 0 }
$path = Extract-Path $payload
if ($path -match '(?i)(\.env(\.|$)|\.envrc|\.pem$|\.key$|credentials|secrets|kubeconfig|\.p12$|\.pfx$)') {
  [Console]::Error.WriteLine("jarvy: refusing edit of credential file: $path")
  exit 2
}
exit 0
"#;

const BLOCK_READ_SECRET_BASH: &str = r#"#!/usr/bin/env bash
set -u
payload="$(cat)"
_jarvy_extract_path() {
  local p="$1"
  if command -v jq >/dev/null 2>&1; then
    printf '%s' "$p" | jq -r '(.tool_input.file_path // .tool_input.path // .file_path // .path // empty)' 2>/dev/null | head -1
  else
    printf '%s' "$p" | sed -n 's/.*"\(file_path\|path\)"[[:space:]]*:[[:space:]]*"\(.*\)"[^"]*$/\2/p' | head -1
  fi
}
path="$(_jarvy_extract_path "$payload")"
case "$path" in
  */.ssh/*|*/.aws/*|*/.kube/*|*/.gnupg/*|*/credentials.json|*/serviceaccount*.json|*.env|*.env.*)
    echo "jarvy: refusing read of credential path: $path" >&2
    exit 2 ;;
esac
exit 0
"#;

const BLOCK_READ_SECRET_PS: &str = r#"$payload = [Console]::In.ReadToEnd()
function Extract-Path($p) {
  if (Get-Command jq -ErrorAction SilentlyContinue) {
    return ($p | jq -r '(.tool_input.file_path // .tool_input.path // .file_path // .path // empty)' 2>$null | Select-Object -First 1)
  }
  if ($p -match '"file_path"\s*:\s*"((?:[^"\\]|\\.)*)"') { return $Matches[1].Replace('\"','"') }
  return ''
}
$path = Extract-Path $payload
if ($path -match '(?i)(\\\.ssh\\|\\\.aws\\|\\\.kube\\|\\\.gnupg\\|credentials\.json$|serviceaccount.*\.json$|\.env(\.|$))') {
  [Console]::Error.WriteLine("jarvy: refusing read of credential path: $path")
  exit 2
}
exit 0
"#;

const BLOCK_CAT_ENV_BASH: &str = r#"#!/usr/bin/env bash
set -u
payload="$(cat)"
_jarvy_extract_cmd() {
  local p="$1"
  if command -v jq >/dev/null 2>&1; then
    printf '%s' "$p" | jq -r '(.tool_input.command // .command // .command_line // empty)' 2>/dev/null | head -1
  else
    printf '%s' "$p" | sed -n 's/.*"\(command\|command_line\)"[[:space:]]*:[[:space:]]*"\(.*\)"[^"]*$/\2/p' | head -1
  fi
}
cmd="$(_jarvy_extract_cmd "$payload")"
if printf '%s' "$cmd" | grep -Eiq '(cat[[:space:]]+([^[:space:]]+/)?\.env|printenv|^[[:space:]]*env[[:space:]]*$|^[[:space:]]*env[[:space:]]*\|)'; then
  echo "jarvy: refusing env exfiltration (cat .env / printenv / env)" >&2
  exit 2
fi
exit 0
"#;

const BLOCK_CAT_ENV_PS: &str = r#"$payload = [Console]::In.ReadToEnd()
function Extract-Command($p) {
  if (Get-Command jq -ErrorAction SilentlyContinue) {
    return ($p | jq -r '(.tool_input.command // .command // .command_line // empty)' 2>$null | Select-Object -First 1)
  }
  if ($p -match '"command"\s*:\s*"((?:[^"\\]|\\.)*)"') { return $Matches[1].Replace('\"','"') }
  return ''
}
$cmd = Extract-Command $payload
if ($cmd -match '(?i)(cat\s+([^\s]+/)?\.env|printenv|^\s*env\s*$|Get-ChildItem\s+Env:)') {
  [Console]::Error.WriteLine('jarvy: refusing env exfiltration')
  exit 2
}
exit 0
"#;

const BLOCK_MALWARE_BASH: &str = r#"#!/usr/bin/env bash
set -u
payload="$(cat)"
_jarvy_extract_cmd() {
  local p="$1"
  if command -v jq >/dev/null 2>&1; then
    printf '%s' "$p" | jq -r '(.tool_input.command // .command // .command_line // empty)' 2>/dev/null | head -1
  else
    printf '%s' "$p" | sed -n 's/.*"\(command\|command_line\)"[[:space:]]*:[[:space:]]*"\(.*\)"[^"]*$/\2/p' | head -1
  fi
}
cmd="$(_jarvy_extract_cmd "$payload")"
# Static deny list of historically-malicious packages (extend as needed).
deny='discord-app-screenshare|discord-selfbot-v13|noblox\.js-proxy|colorette-tool|@solana/web3-utils|electron-notify|rustdecimal|crossenv|cross-env\.js|fallguys|http-proxy-middleware-v3'
# Cover npm/pnpm/yarn short and long forms, plus pip and cargo.
if printf '%s' "$cmd" | grep -Eiq "(npm[[:space:]]+(install|i|add)|pnpm[[:space:]]+(install|i|add)|yarn[[:space:]]+(add|install)?|pip[[:space:]]+install|cargo[[:space:]]+install)[[:space:]]+[^|;&]*($deny)"; then
  echo "jarvy: refusing install of known-malicious package" >&2
  exit 2
fi
exit 0
"#;

const BLOCK_MALWARE_PS: &str = r#"$payload = [Console]::In.ReadToEnd()
function Extract-Command($p) {
  if (Get-Command jq -ErrorAction SilentlyContinue) {
    return ($p | jq -r '(.tool_input.command // .command // .command_line // empty)' 2>$null | Select-Object -First 1)
  }
  if ($p -match '"command"\s*:\s*"((?:[^"\\]|\\.)*)"') { return $Matches[1].Replace('\"','"') }
  return ''
}
$cmd = Extract-Command $payload
$deny = 'discord-app-screenshare|discord-selfbot-v13|noblox\.js-proxy|colorette-tool|@solana/web3-utils|electron-notify|rustdecimal|crossenv|cross-env\.js|fallguys|http-proxy-middleware-v3'
if ($cmd -match "(?i)(npm\s+(install|i|add)|pnpm\s+(install|i|add)|yarn\s+(add|install)?|pip\s+install|cargo\s+install)\s+[^|;&]*($deny)") {
  [Console]::Error.WriteLine('jarvy: refusing install of known-malicious package')
  exit 2
}
exit 0
"#;

// audit-log: pipe payload through a redactor (jq-driven if available),
// chmod 0600 the log file, validate the line is JSON before writing.
const AUDIT_LOG_BASH: &str = r#"#!/usr/bin/env bash
set -u
payload="$(cat)"
log_dir="${JARVY_AUDIT_LOG_DIR:-$HOME/.jarvy/logs}"
umask 077
mkdir -p "$log_dir" 2>/dev/null || true
log_file="$log_dir/ai-hooks-audit.jsonl"
ts="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

# Validate the payload is JSON; bail out silently if it isn't (do not
# corrupt the JSONL discipline by appending garbage).
if command -v jq >/dev/null 2>&1; then
  if ! printf '%s' "$payload" | jq -e . >/dev/null 2>&1; then
    exit 0
  fi
  # Redact common secret shapes inside any string field of the payload.
  # Replaces AWS keys, GH tokens, OpenAI keys, generic Bearer tokens.
  redacted="$(
    printf '%s' "$payload" \
      | jq -c '
          def red:
            if type == "string" then
              gsub("AKIA[0-9A-Z]{16}"; "[REDACTED_AWS]")
              | gsub("ghp_[A-Za-z0-9]{30,}"; "[REDACTED_GH]")
              | gsub("gho_[A-Za-z0-9]{30,}"; "[REDACTED_GH]")
              | gsub("sk-[A-Za-z0-9]{30,}"; "[REDACTED_OPENAI]")
              | gsub("Bearer [A-Za-z0-9._-]{10,}"; "Bearer [REDACTED]")
            else . end;
          walk(red)' 2>/dev/null
  )"
  if [ -z "$redacted" ]; then
    # jq exists but the walk filter failed (older jq?) — bail rather
    # than write the raw payload.
    exit 0
  fi
  printf '{"ts":"%s","event":"ai_hooks_audit","payload":%s}\n' "$ts" "$redacted" >> "$log_file"
else
  # No jq: do a coarse sed-based redact and only write if the payload at
  # least looks like JSON (starts with `{`, ends with `}`).
  case "$payload" in
    \{*\}) ;;
    *) exit 0 ;;
  esac
  redacted="$(
    printf '%s' "$payload" \
      | sed -E 's/AKIA[0-9A-Z]{16}/[REDACTED_AWS]/g; s/ghp_[A-Za-z0-9]{30,}/[REDACTED_GH]/g; s/gho_[A-Za-z0-9]{30,}/[REDACTED_GH]/g; s/sk-[A-Za-z0-9]{30,}/[REDACTED_OPENAI]/g'
  )"
  printf '{"ts":"%s","event":"ai_hooks_audit","payload":%s}\n' "$ts" "$redacted" >> "$log_file"
fi
chmod 600 "$log_file" 2>/dev/null || true
exit 0
"#;

const AUDIT_LOG_PS: &str = r#"$payload = [Console]::In.ReadToEnd()
$logDir = $env:JARVY_AUDIT_LOG_DIR
if (-not $logDir) { $logDir = Join-Path $env:USERPROFILE '.jarvy\logs' }
if (-not (Test-Path $logDir)) { New-Item -ItemType Directory -Path $logDir -Force | Out-Null }
$logFile = Join-Path $logDir 'ai-hooks-audit.jsonl'
$ts = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")

# Validate JSON shape.
$trimmed = $payload.TrimStart()
if (-not ($trimmed.StartsWith('{'))) { exit 0 }

# Coarse redaction.
$redacted = $payload `
  -replace 'AKIA[0-9A-Z]{16}','[REDACTED_AWS]' `
  -replace 'ghp_[A-Za-z0-9]{30,}','[REDACTED_GH]' `
  -replace 'gho_[A-Za-z0-9]{30,}','[REDACTED_GH]' `
  -replace 'sk-[A-Za-z0-9]{30,}','[REDACTED_OPENAI]' `
  -replace 'Bearer [A-Za-z0-9._-]{10,}','Bearer [REDACTED]'

$line = '{"ts":"' + $ts + '","event":"ai_hooks_audit","payload":' + $redacted + '}'
Add-Content -Path $logFile -Value $line

# Best-effort ACL tightening on Windows: restrict to current user.
try {
  $acl = Get-Acl $logFile
  $acl.SetAccessRuleProtection($true, $false)
  $rule = New-Object System.Security.AccessControl.FileSystemAccessRule(
    $env:USERNAME, 'FullControl', 'Allow')
  $acl.SetAccessRule($rule)
  Set-Acl $logFile $acl
} catch {}
exit 0
"#;

const COMMIT_FORMAT_BASH: &str = r#"#!/usr/bin/env bash
set -u
payload="$(cat)"
_jarvy_extract_cmd() {
  local p="$1"
  if command -v jq >/dev/null 2>&1; then
    printf '%s' "$p" | jq -r '(.tool_input.command // .command // .command_line // empty)' 2>/dev/null | head -1
  else
    printf '%s' "$p" | sed -n 's/.*"\(command\|command_line\)"[[:space:]]*:[[:space:]]*"\(.*\)"[^"]*$/\2/p' | head -1
  fi
}
cmd="$(_jarvy_extract_cmd "$payload")"
case "$cmd" in
  "git commit -m"*|"git commit --message"*) ;;
  *) exit 0 ;;
esac
msg="$(printf '%s' "$cmd" | sed -nE 's/.*git commit (-m|--message)[[:space:]]+["'"'"']?([^"'"'"']*).*/\2/p')"
if ! printf '%s' "$msg" | grep -Eq '^(feat|fix|docs|chore|refactor|test|build|ci|perf|style|revert)(\([a-z0-9_-]+\))?!?:[[:space:]]'; then
  echo "jarvy: commit message must use Conventional Commits prefix (feat:/fix:/...)" >&2
  exit 2
fi
exit 0
"#;

const COMMIT_FORMAT_PS: &str = r#"$payload = [Console]::In.ReadToEnd()
function Extract-Command($p) {
  if (Get-Command jq -ErrorAction SilentlyContinue) {
    return ($p | jq -r '(.tool_input.command // .command // .command_line // empty)' 2>$null | Select-Object -First 1)
  }
  if ($p -match '"command"\s*:\s*"((?:[^"\\]|\\.)*)"') { return $Matches[1].Replace('\"','"') }
  return ''
}
$cmd = Extract-Command $payload
if ($cmd -notmatch '^git commit (-m|--message)') { exit 0 }
$msg = ''
if ($cmd -match 'git\s+commit\s+(-m|--message)\s+["'']?([^"'']+)') { $msg = $Matches[2] }
if ($msg -notmatch '^(feat|fix|docs|chore|refactor|test|build|ci|perf|style|revert)(\([a-z0-9_-]+\))?!?:\s') {
  [Console]::Error.WriteLine('jarvy: commit message must use Conventional Commits prefix')
  exit 2
}
exit 0
"#;

// EXTRACT_CMD / EXTRACT_PATH constants are unused at the moment — the
// extractor shim is inlined per script so the library remains a flat
// &'static str registry. Keep the named constants for future templating.
#[allow(dead_code)]
const _UNUSED_SHIMS: (&str, &str) = (EXTRACT_CMD, EXTRACT_PATH);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_case_insensitive() {
        assert!(find("block-rm-rf").is_some());
        assert!(find("BLOCK-RM-RF").is_some());
        assert!(find("nope").is_none());
    }

    #[test]
    fn every_library_entry_has_both_scripts() {
        for h in LIBRARY {
            assert!(!h.bash.is_empty(), "{} missing bash", h.name);
            assert!(!h.powershell.is_empty(), "{} missing powershell", h.name);
            assert!(h.timeout_ms > 0, "{} timeout zero", h.name);
        }
    }

    #[test]
    fn library_names_unique() {
        let mut seen = std::collections::HashSet::new();
        for h in LIBRARY {
            assert!(seen.insert(h.name), "duplicate library hook {}", h.name);
        }
    }
}
