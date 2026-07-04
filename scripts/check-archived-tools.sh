#!/usr/bin/env bash
#
# check-archived-tools.sh — flag tools whose upstream GitHub repo has
# been archived (read-only / abandoned) or gone missing, so a maintainer
# can migrate to a successor or drop the tool.
#
# Source of truth is the tool index: every `ToolSpec` declares an
# optional `repo` field (canonical `owner/repo` GitHub slug), surfaced by
# `jarvy tools --index --format json`. This script reads that index and
# queries the GitHub API for each repo's `archived` flag — so coverage is
# exhaustive (every registered tool is considered) rather than dependent
# on docstring grepping. Tools with no declared repo (proprietary,
# GitLab-hosted, base system utilities) are reported as "unchecked" for
# transparency.
#
# Requires: gh (authenticated — GH_TOKEN / GITHUB_TOKEN in CI), jq.
# The tool index is obtained, in order of preference, from:
#   1. $1 (a path to a pre-generated index JSON), or
#   2. $JARVY_INDEX_JSON (same), or
#   3. `jarvy tools --index --format json` if `jarvy` is on PATH, or
#   4. `cargo run --quiet -- tools --index --format json` from the repo.
#
# Usage:
#   scripts/check-archived-tools.sh [INDEX_JSON] [REPORT_FILE]
#
# Exit status:
#   0  no archived / missing upstreams found
#   1  at least one archived / missing upstream found
#   2  hard failure (missing gh/jq, not authenticated, no index)

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
INDEX_ARG="${1:-${JARVY_INDEX_JSON:-}}"
REPORT="${2:-$ROOT/archived-tools-report.md}"

for bin in gh jq; do
  if ! command -v "$bin" >/dev/null 2>&1; then
    echo "error: $bin not found on PATH" >&2
    exit 2
  fi
done
if ! gh auth status >/dev/null 2>&1 && [ -z "${GH_TOKEN:-}${GITHUB_TOKEN:-}" ]; then
  echo "error: gh is not authenticated (set GH_TOKEN or GITHUB_TOKEN)" >&2
  exit 2
fi

# Obtain the tool index JSON.
index_json=""
if [ -n "$INDEX_ARG" ] && [ -f "$INDEX_ARG" ]; then
  index_json="$(cat "$INDEX_ARG")"
elif command -v jarvy >/dev/null 2>&1; then
  index_json="$(jarvy tools --index --format json)"
elif command -v cargo >/dev/null 2>&1; then
  index_json="$(cd "$ROOT" && cargo run --quiet -- tools --index --format json)"
else
  echo "error: no index source (pass INDEX_JSON, or provide jarvy/cargo)" >&2
  exit 2
fi

if ! printf '%s' "$index_json" | jq -e '.tools' >/dev/null 2>&1; then
  echo "error: index JSON has no .tools array" >&2
  exit 2
fi

total_tools="$(printf '%s' "$index_json" | jq '.tools | length')"
# Tools with no declared upstream repo — reported as unchecked.
mapfile -t no_repo < <(printf '%s' "$index_json" | jq -r '.tools[] | select(.repo | not) | .name' | sort)

# Unique declared repos, and a repo -> "tool1, tool2" grouping.
mapfile -t repos < <(printf '%s' "$index_json" | jq -r '.tools[] | select(.repo) | .repo' | sort -u)

# repo<TAB>comma-joined tool names (a repo like kubernetes/kubernetes may
# back several tools, e.g. kubectl).
tools_for_repo() {
  printf '%s' "$index_json" | jq -r --arg r "$1" \
    '[.tools[] | select(.repo == $r) | .name] | join(", ")'
}

archived=()
missing=()

for repo in "${repos[@]}"; do
  if state="$(gh api "repos/$repo" --jq '.archived' 2>/dev/null)"; then
    if [ "$state" = "true" ]; then
      archived+=("$repo|$(tools_for_repo "$repo")")
    fi
  else
    missing+=("$repo|$(tools_for_repo "$repo")")
  fi
done

checked=${#repos[@]}
{
  echo "# Archived / unreachable upstream tool report"
  echo
  echo "_Read the \`repo\` field from \`jarvy tools --index\` and checked each declared upstream's archive status via the GitHub API._"
  echo
  if [ "${#archived[@]}" -eq 0 ] && [ "${#missing[@]}" -eq 0 ]; then
    echo "✅ No archived or unreachable upstream repositories found."
    echo
  fi
  if [ "${#archived[@]}" -gt 0 ]; then
    echo "## 🗄️ Archived upstreams (${#archived[@]})"
    echo
    echo "These repos are read-only upstream. Migrate to a maintained successor or drop the tool."
    echo
    echo "| Upstream repo | Backing tool(s) |"
    echo "|---------------|-----------------|"
    for entry in "${archived[@]}"; do
      # shellcheck disable=SC2016  # backticks are literal Markdown, not shell expansion
      printf '| [`%s`](https://github.com/%s) | %s |\n' "${entry%%|*}" "${entry%%|*}" "${entry#*|}"
    done
    echo
  fi
  if [ "${#missing[@]}" -gt 0 ]; then
    echo "## ❓ Unreachable upstreams (${#missing[@]})"
    echo
    echo "Repo returned 404 (renamed, made private, or deleted). Verify the declared \`repo\` is still correct."
    echo
    echo "| Upstream repo | Backing tool(s) |"
    echo "|---------------|-----------------|"
    for entry in "${missing[@]}"; do
      # shellcheck disable=SC2016  # backticks are literal Markdown, not shell expansion
      printf '| `%s` | %s |\n' "${entry%%|*}" "${entry#*|}"
    done
    echo
  fi
  echo "---"
  echo
  echo "**Coverage:** ${checked} unique upstream repos checked across ${total_tools} tools. "
  echo "${#no_repo[@]} tool(s) declare no GitHub upstream and are not checked (proprietary, GitLab-hosted, or base system utilities):"
  echo
  if [ "${#no_repo[@]}" -gt 0 ]; then
    # shellcheck disable=SC2016  # backticks are literal Markdown, not shell expansion
    printf '`%s`' "${no_repo[0]}"
    # shellcheck disable=SC2016  # backticks are literal Markdown, not shell expansion
    for t in "${no_repo[@]:1}"; do printf ', `%s`' "$t"; done
    echo
  fi
} > "$REPORT"

echo "Report written to $REPORT"
echo "archived=${#archived[@]} missing=${#missing[@]} repos_checked=${checked} no_repo=${#no_repo[@]} total=${total_tools}"

if [ "${#archived[@]}" -gt 0 ] || [ "${#missing[@]}" -gt 0 ]; then
  exit 1
fi
exit 0
