//! Detection rule engine for `jarvy discover`.
//!
//! Each `DetectionRule` declares marker files / directories that indicate
//! a technology is in use, where to extract its version, and what
//! companion tools to recommend. The bundled `default_rules()` set covers
//! the main ecosystems jarvy supports today (rust, node, python, go,
//! docker, kubectl, terraform, pre-commit). Adding a new ecosystem is one
//! entry in the array — no other code changes needed.

use serde::{Deserialize, Serialize};
use std::path::Path;

use super::scanner::find_first_match;
use super::version::extract_version;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionRule {
    pub name: String,
    pub detect: Vec<DetectionPattern>,
    #[serde(default)]
    pub version_from: Option<VersionSource>,
    #[serde(default)]
    pub suggests: Vec<String>,
    pub category: ToolCategory,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DetectionPattern {
    File {
        file: String,
    },
    Dir {
        dir: String,
    },
    /// Match a file whose contents contain a literal substring. Used
    /// for ecosystems where the marker file is generic (`*.yaml`) but
    /// the content is distinctive (`kind: Deployment` for Kubernetes,
    /// `engines.node` for Node package manifests, etc.). Bounded
    /// reads — only the first MAX_CONTAINING_BYTES (4 KiB) are
    /// scanned per file, which is more than enough for header lines.
    FileContaining {
        file: String,
        containing: String,
    },
}

/// Cap how much of a file we inspect for `FileContaining`. Most
/// markers we care about live in the first few lines; reading 4 KiB
/// is plenty and bounds the cost of a malicious giant marker file.
pub const MAX_CONTAINING_BYTES: usize = 4 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionSource {
    pub file: String,
    #[serde(default)]
    pub pattern: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolCategory {
    Runtime,
    Build,
    Dev,
    Ops,
}

/// One technology successfully detected in the project tree. The
/// `source` field is human-readable (e.g. "Cargo.toml") and surfaces in
/// the `--format pretty` output so users can see why a tool was
/// suggested.
#[derive(Debug, Clone, Serialize)]
pub struct Detection {
    pub tool: String,
    pub version: Option<String>,
    pub source: String,
    pub suggests: Vec<String>,
    pub category: ToolCategory,
}

/// Walk every rule against `project_dir` and return one `Detection` per
/// matched rule. Stable iteration order matches `rules` for
/// deterministic output.
pub fn run(project_dir: &Path, rules: &[DetectionRule]) -> Vec<Detection> {
    let mut out = Vec::new();
    for rule in rules {
        if let Some(matched_source) = rule_match_source(project_dir, rule) {
            let version = rule
                .version_from
                .as_ref()
                .and_then(|vs| extract_version(project_dir, vs));
            out.push(Detection {
                tool: rule.name.clone(),
                version,
                source: matched_source,
                suggests: rule.suggests.clone(),
                category: rule.category,
            });
        }
    }
    out
}

/// First matching pattern wins; we return its source string so the
/// suggestion explainer can cite a real file ("detected from Cargo.toml").
///
/// The source string flows into the rendered `# detected from ...`
/// comment in `discover/generator.rs`. For pattern-supplied filenames
/// (`Cargo.toml`, `package.json`, ...) that's a trusted rule-author
/// literal. For `*.ext` glob matches the on-disk filename is
/// attacker-controllable (POSIX filenames may contain newlines, `"`,
/// and control bytes). We strict-allowlist the matched filename to
/// printable ASCII without quotes/backslashes so a hostile filename
/// like `x.tf\n[packages]\nallow_remote = true\n# .tf` can't inject
/// a TOML section through the rendered comment (review item P0 #2).
fn rule_match_source(project_dir: &Path, rule: &DetectionRule) -> Option<String> {
    for pattern in &rule.detect {
        match pattern {
            DetectionPattern::File { file } => {
                if let Some(p) = find_first_match(project_dir, file) {
                    let name = p.file_name()?.to_string_lossy().into_owned();
                    if let Some(safe) = sanitize_source(&name) {
                        return Some(safe);
                    }
                    // Hostile filename — skip this pattern but still try
                    // siblings so a `*.tf` rule isn't defeated by one
                    // poisoned filename.
                    continue;
                }
            }
            DetectionPattern::Dir { dir } => {
                if project_dir.join(dir).is_dir() {
                    if let Some(safe) = sanitize_source(dir) {
                        return Some(safe);
                    }
                }
            }
            DetectionPattern::FileContaining { file, containing } => {
                if let Some(p) = find_first_match(project_dir, file) {
                    if let Ok(content) = read_bounded(&p, MAX_CONTAINING_BYTES) {
                        if content.contains(containing) {
                            let name = p.file_name()?.to_string_lossy().into_owned();
                            if let Some(safe) = sanitize_source(&name) {
                                return Some(safe);
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// Read at most `cap` bytes of `path` and return as a UTF-8 string.
/// Lossy decoding so a stray non-UTF8 byte doesn't kill the scan.
fn read_bounded(path: &Path, cap: usize) -> std::io::Result<String> {
    use std::io::Read;
    let mut file = std::fs::File::open(path)?;
    let mut buf = vec![0u8; cap];
    let n = file.read(&mut buf)?;
    buf.truncate(n);
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

/// Accept only ASCII-graphic + space — every other byte (newline,
/// CR, NUL, ESC, DEL, `"`, `\`) is refused. Returning `None` on a
/// hostile filename means the rule doesn't fire at all rather than
/// allowing partial / sanitized attribution.
fn sanitize_source(name: &str) -> Option<String> {
    if name.is_empty() || name.len() > 255 {
        return None;
    }
    if !name
        .chars()
        .all(|c| (c.is_ascii_graphic() && c != '"' && c != '\\') || c == ' ')
    {
        return None;
    }
    Some(name.to_string())
}

/// Built-in detection rules covering the ecosystems jarvy ships handlers
/// for today. Names match the canonical jarvy tool name (lowercase,
/// dash-separated) so `analyze()` can validate suggestions against
/// `tools::registry::registered_tool_names()` without aliasing logic.
///
/// Cached behind a `OnceLock` (review item P2 #22) — the ~50 String
/// allocations are paid once per process, not per `analyze()` call.
pub fn default_rules() -> &'static [DetectionRule] {
    use std::sync::OnceLock;
    static RULES: OnceLock<Vec<DetectionRule>> = OnceLock::new();
    RULES.get_or_init(build_default_rules)
}

fn build_default_rules() -> Vec<DetectionRule> {
    vec![
        DetectionRule {
            name: "rust".into(),
            detect: vec![
                DetectionPattern::File {
                    file: "Cargo.toml".into(),
                },
                DetectionPattern::File {
                    file: "Cargo.lock".into(),
                },
                DetectionPattern::File {
                    file: "rust-toolchain.toml".into(),
                },
                DetectionPattern::File {
                    file: "rust-toolchain".into(),
                },
            ],
            version_from: Some(VersionSource {
                file: "rust-toolchain.toml".into(),
                pattern: Some(r#"channel\s*=\s*"([^"]+)""#.into()),
            }),
            // bacon is the modern successor to cargo-watch (better TUI,
            // scoped test reruns); cargo-nextest is the standard test
            // runner. Both route through `cargo install` via
            // `custom_install`, so they're safe to recommend on any
            // platform Rust supports without per-OS package coverage.
            suggests: vec!["bacon".into(), "cargo-nextest".into()],
            category: ToolCategory::Runtime,
        },
        DetectionRule {
            name: "node".into(),
            detect: vec![
                DetectionPattern::File {
                    file: "package.json".into(),
                },
                DetectionPattern::File {
                    file: "package-lock.json".into(),
                },
                DetectionPattern::File {
                    file: "yarn.lock".into(),
                },
                DetectionPattern::File {
                    file: "pnpm-lock.yaml".into(),
                },
                DetectionPattern::File {
                    file: ".nvmrc".into(),
                },
            ],
            version_from: Some(VersionSource {
                file: ".nvmrc".into(),
                pattern: Some(r"v?(\d+(?:\.\d+(?:\.\d+)?)?)".into()),
            }),
            // `pnpm` / `yarn` used to live here as generic suggestions,
            // but the lockfile-specific rules below (`pnpm`, `yarn`,
            // `bun`) upgrade the actually-in-use package manager to a
            // required entry — much more precise than always suggesting
            // both. Leave empty so we don't double-list.
            suggests: vec![],
            category: ToolCategory::Runtime,
        },
        // Lockfile-specific Node package managers. Each lockfile is
        // unambiguous evidence that the corresponding CLI is required —
        // `pnpm-lock.yaml` cannot be consumed by `npm` or `yarn`, and
        // vice-versa — so we lift these from "recommended companion"
        // to "required" directly. The dedup in `analyze_with` prevents
        // any accidental double-listing if a follow-up rule adds a
        // recommendation for the same tool.
        DetectionRule {
            name: "pnpm".into(),
            detect: vec![DetectionPattern::File {
                file: "pnpm-lock.yaml".into(),
            }],
            version_from: None,
            suggests: vec![],
            category: ToolCategory::Build,
        },
        DetectionRule {
            name: "yarn".into(),
            detect: vec![DetectionPattern::File {
                file: "yarn.lock".into(),
            }],
            version_from: None,
            suggests: vec![],
            category: ToolCategory::Build,
        },
        DetectionRule {
            name: "bun".into(),
            detect: vec![
                DetectionPattern::File {
                    file: "bun.lockb".into(),
                },
                DetectionPattern::File {
                    file: "bun.lock".into(),
                },
            ],
            version_from: None,
            suggests: vec![],
            category: ToolCategory::Build,
        },
        // PHP + Composer. `composer.json` / `composer.lock` are the
        // canonical markers; `artisan` (Laravel bootstrapper) and
        // `symfony.lock` also imply a PHP project even without a
        // composer manifest at the top level (rare — most modern PHP
        // projects have composer.json regardless of framework).
        DetectionRule {
            name: "php".into(),
            detect: vec![
                DetectionPattern::File {
                    file: "composer.json".into(),
                },
                DetectionPattern::File {
                    file: "composer.lock".into(),
                },
                DetectionPattern::File {
                    file: "artisan".into(),
                },
                DetectionPattern::File {
                    file: "symfony.lock".into(),
                },
            ],
            version_from: None,
            suggests: vec!["composer".into()],
            category: ToolCategory::Runtime,
        },
        // Standalone composer rule: if `composer.json` exists we
        // strictly need composer (php rule above already asks for the
        // interpreter). Separate rule so lock-only or json-only
        // projects still surface composer as required, not just
        // "commonly used with PHP".
        DetectionRule {
            name: "composer".into(),
            detect: vec![
                DetectionPattern::File {
                    file: "composer.json".into(),
                },
                DetectionPattern::File {
                    file: "composer.lock".into(),
                },
            ],
            version_from: None,
            suggests: vec![],
            category: ToolCategory::Build,
        },
        DetectionRule {
            name: "python".into(),
            detect: vec![
                DetectionPattern::File {
                    file: "pyproject.toml".into(),
                },
                DetectionPattern::File {
                    file: "requirements.txt".into(),
                },
                DetectionPattern::File {
                    file: "Pipfile".into(),
                },
                DetectionPattern::File {
                    file: "setup.py".into(),
                },
                DetectionPattern::File {
                    file: ".python-version".into(),
                },
            ],
            version_from: Some(VersionSource {
                file: ".python-version".into(),
                pattern: None,
            }),
            suggests: vec!["uv".into(), "poetry".into(), "pipx".into()],
            category: ToolCategory::Runtime,
        },
        DetectionRule {
            name: "go".into(),
            detect: vec![
                DetectionPattern::File {
                    file: "go.mod".into(),
                },
                DetectionPattern::File {
                    file: "go.sum".into(),
                },
            ],
            version_from: Some(VersionSource {
                file: "go.mod".into(),
                pattern: Some(r"^go\s+(\d+\.\d+(?:\.\d+)?)".into()),
            }),
            // golangci-lint is the de-facto community linter; air is
            // the standard hot-reload dev loop; delve is the debugger
            // (`dlv`). All three are registered as first-party jarvy
            // tools, so `known_tools.contains(...)` in `analyze_with`
            // will surface them as recommended companions rather than
            // silently dropping them.
            suggests: vec![
                "golangci-lint".into(),
                "air".into(),
                "delve".into(),
            ],
            category: ToolCategory::Runtime,
        },
        DetectionRule {
            name: "ruby".into(),
            detect: vec![
                DetectionPattern::File {
                    file: "Gemfile".into(),
                },
                DetectionPattern::File {
                    file: "Gemfile.lock".into(),
                },
                DetectionPattern::File {
                    file: ".ruby-version".into(),
                },
            ],
            version_from: Some(VersionSource {
                file: ".ruby-version".into(),
                pattern: None,
            }),
            suggests: vec![],
            category: ToolCategory::Runtime,
        },
        // ---------------------------------------------------------------
        // Niche-but-popular languages. Every entry below has a
        // registered first-party jarvy tool (see `src/tools/`) so a
        // detection surfaces as `required` in the report, not the
        // `uninstallable` bucket. Rule order doesn't matter for
        // correctness — `analyze_with` iterates deterministically over
        // the whole set.
        // ---------------------------------------------------------------

        // Deno: `deno.json` / `deno.jsonc` are the config equivalents
        // of Node's `package.json`; `deno.lock` is emitted after the
        // first `deno cache`. Presence of any implies a Deno project.
        DetectionRule {
            name: "deno".into(),
            detect: vec![
                DetectionPattern::File {
                    file: "deno.json".into(),
                },
                DetectionPattern::File {
                    file: "deno.jsonc".into(),
                },
                DetectionPattern::File {
                    file: "deno.lock".into(),
                },
            ],
            version_from: None,
            suggests: vec![],
            category: ToolCategory::Runtime,
        },
        // Elixir: `mix.exs` is the canonical build script and
        // dependency manifest; `mix.lock` pins deps. Elixir compiles
        // to the BEAM so Erlang/OTP is a runtime prereq — surface it
        // as a companion.
        DetectionRule {
            name: "elixir".into(),
            detect: vec![
                DetectionPattern::File {
                    file: "mix.exs".into(),
                },
                DetectionPattern::File {
                    file: "mix.lock".into(),
                },
            ],
            version_from: None,
            suggests: vec!["erlang".into()],
            category: ToolCategory::Runtime,
        },
        // Erlang: `rebar.config` / `rebar3.config` are the standard
        // build configs; `erlang.mk` covers the older-style Makefile
        // build. `.rebar3/` cache dirs are not markers — they only
        // appear after a build, so they'd miss fresh clones.
        DetectionRule {
            name: "erlang".into(),
            detect: vec![
                DetectionPattern::File {
                    file: "rebar.config".into(),
                },
                DetectionPattern::File {
                    file: "rebar3.config".into(),
                },
                DetectionPattern::File {
                    file: "erlang.mk".into(),
                },
            ],
            version_from: None,
            suggests: vec![],
            category: ToolCategory::Runtime,
        },
        // Haskell: `*.cabal` (per-package), `cabal.project` (multi-
        // package), `stack.yaml` (Stack-managed), `package.yaml`
        // (hpack — cabal's YAML front-end). Any single one is enough.
        DetectionRule {
            name: "haskell".into(),
            detect: vec![
                DetectionPattern::File {
                    file: "cabal.project".into(),
                },
                DetectionPattern::File {
                    file: "stack.yaml".into(),
                },
                DetectionPattern::File {
                    file: "package.yaml".into(),
                },
                DetectionPattern::File {
                    file: "*.cabal".into(),
                },
            ],
            version_from: None,
            suggests: vec![],
            category: ToolCategory::Runtime,
        },
        // Crystal: `shard.yml` is Crystal's dep manifest (analogous
        // to Cargo.toml); `shard.lock` is the pinned lockfile.
        DetectionRule {
            name: "crystal".into(),
            detect: vec![
                DetectionPattern::File {
                    file: "shard.yml".into(),
                },
                DetectionPattern::File {
                    file: "shard.lock".into(),
                },
            ],
            version_from: None,
            suggests: vec![],
            category: ToolCategory::Runtime,
        },
        // Gleam: `gleam.toml` is the sole canonical marker. Gleam
        // targets both the BEAM (Erlang) and JavaScript, so Erlang
        // surfaces as a recommended companion for the default target.
        DetectionRule {
            name: "gleam".into(),
            detect: vec![DetectionPattern::File {
                file: "gleam.toml".into(),
            }],
            version_from: None,
            suggests: vec!["erlang".into()],
            category: ToolCategory::Runtime,
        },
        // Lua: no universal single-file marker at the repo root — Lua
        // is a per-file embedded language. `.lua-version` (used by
        // asdf / mise / rtx) is the strongest signal that the repo
        // author wants a specific runtime pinned. `luarocks` is the
        // dominant package manager and is surfaced as a suggestion.
        DetectionRule {
            name: "lua".into(),
            detect: vec![DetectionPattern::File {
                file: ".lua-version".into(),
            }],
            version_from: Some(VersionSource {
                file: ".lua-version".into(),
                pattern: None,
            }),
            suggests: vec!["luarocks".into()],
            category: ToolCategory::Runtime,
        },
        // LuaRocks: `*.rockspec` at the repo root is the canonical
        // marker. Requires lua as a runtime prereq.
        DetectionRule {
            name: "luarocks".into(),
            detect: vec![DetectionPattern::File {
                file: "*.rockspec".into(),
            }],
            version_from: None,
            suggests: vec!["lua".into()],
            category: ToolCategory::Build,
        },
        // Nim: `*.nimble` (Nimble package manifest) is the standard
        // dep manifest; `nim.cfg` sets compiler options — either is
        // strong evidence.
        DetectionRule {
            name: "nim".into(),
            detect: vec![
                DetectionPattern::File {
                    file: "*.nimble".into(),
                },
                DetectionPattern::File {
                    file: "nim.cfg".into(),
                },
            ],
            version_from: None,
            suggests: vec![],
            category: ToolCategory::Runtime,
        },
        // OCaml: `dune-project` is the modern Dune build system
        // marker; `*.opam` is the package manifest for opam. Either
        // is sufficient. `dune-workspace` also exists but is much
        // rarer and would rarely appear without `dune-project`.
        DetectionRule {
            name: "ocaml".into(),
            detect: vec![
                DetectionPattern::File {
                    file: "dune-project".into(),
                },
                DetectionPattern::File {
                    file: "*.opam".into(),
                },
            ],
            version_from: None,
            suggests: vec![],
            category: ToolCategory::Runtime,
        },
        // Scala: `build.sbt` is by far the dominant build tool
        // marker; `project/build.properties` also exists but sits in
        // a subdir the top-level scanner doesn't walk (the sbt
        // marker at the root is enough on its own). Mill's `build.sc`
        // covers the smaller Mill-users population.
        DetectionRule {
            name: "scala".into(),
            detect: vec![
                DetectionPattern::File {
                    file: "build.sbt".into(),
                },
                DetectionPattern::File {
                    file: "build.sc".into(),
                },
            ],
            version_from: None,
            suggests: vec![],
            category: ToolCategory::Runtime,
        },
        // Zig: `build.zig` (build script — required for anything
        // non-trivial); `build.zig.zon` is the dep manifest added in
        // 0.11+. Either is a hard marker.
        DetectionRule {
            name: "zig".into(),
            detect: vec![
                DetectionPattern::File {
                    file: "build.zig".into(),
                },
                DetectionPattern::File {
                    file: "build.zig.zon".into(),
                },
            ],
            version_from: None,
            suggests: vec![],
            category: ToolCategory::Runtime,
        },
        // Julia: `Manifest.toml` is Julia-specific (Rust workspaces
        // don't use that filename, Node/Python/etc. don't either).
        // Prefer it over `Project.toml`, which is ambiguous — many
        // tools use `Project.toml` as a generic name. `JuliaProject
        // .toml` (the disambiguated form Pkg emits when it detects a
        // naming collision) is also treated as a marker.
        DetectionRule {
            name: "julia".into(),
            detect: vec![
                DetectionPattern::File {
                    file: "Manifest.toml".into(),
                },
                DetectionPattern::File {
                    file: "JuliaProject.toml".into(),
                },
            ],
            version_from: None,
            suggests: vec![],
            category: ToolCategory::Runtime,
        },
        // CMake: `CMakeLists.txt` at the repo root is the canonical
        // marker for any CMake-built project (C, C++, embedded,
        // graphics, mixed-language). One-file test covers the vast
        // majority of layouts; workspace subdirs still work as long
        // as one exists at the top.
        DetectionRule {
            name: "cmake".into(),
            detect: vec![DetectionPattern::File {
                file: "CMakeLists.txt".into(),
            }],
            version_from: None,
            suggests: vec![],
            category: ToolCategory::Build,
        },
        // Skaffold: `skaffold.yaml` is the sole canonical marker for
        // Skaffold-driven inner dev loops. The presence of this file
        // also implies Kubernetes tooling is expected (kubectl, helm,
        // etc.) — but the `kubectl` rule below covers that from its
        // own markers, so we don't double-emit here.
        DetectionRule {
            name: "skaffold".into(),
            detect: vec![DetectionPattern::File {
                file: "skaffold.yaml".into(),
            }],
            version_from: None,
            suggests: vec![],
            category: ToolCategory::Ops,
        },
        // Bazelisk (Bazel launcher). Bazel projects always ship at
        // least one of these markers at the repo root: `WORKSPACE` /
        // `WORKSPACE.bazel` (classic), `MODULE.bazel` (bzlmod, the
        // modern default since 7.x), or a `.bazelrc` config file.
        // `BUILD.bazel` typically lives in package subdirs but often
        // also sits at the top level of small projects.
        DetectionRule {
            name: "bazelisk".into(),
            detect: vec![
                DetectionPattern::File {
                    file: "MODULE.bazel".into(),
                },
                DetectionPattern::File {
                    file: "WORKSPACE".into(),
                },
                DetectionPattern::File {
                    file: "WORKSPACE.bazel".into(),
                },
                DetectionPattern::File {
                    file: "BUILD.bazel".into(),
                },
                DetectionPattern::File {
                    file: ".bazelrc".into(),
                },
                DetectionPattern::File {
                    file: ".bazelversion".into(),
                },
            ],
            version_from: None,
            suggests: vec![],
            category: ToolCategory::Build,
        },
        DetectionRule {
            name: "docker".into(),
            detect: vec![
                DetectionPattern::File {
                    file: "Dockerfile".into(),
                },
                DetectionPattern::File {
                    file: "docker-compose.yml".into(),
                },
                DetectionPattern::File {
                    file: "docker-compose.yaml".into(),
                },
                DetectionPattern::File {
                    file: "compose.yml".into(),
                },
                DetectionPattern::File {
                    file: "compose.yaml".into(),
                },
            ],
            version_from: None,
            suggests: vec!["docker-compose".into(), "lazydocker".into()],
            category: ToolCategory::Ops,
        },
        DetectionRule {
            name: "kubectl".into(),
            detect: vec![
                DetectionPattern::Dir { dir: "k8s".into() },
                DetectionPattern::Dir {
                    dir: "kubernetes".into(),
                },
                DetectionPattern::Dir {
                    dir: "manifests".into(),
                },
                // Catch repos that scatter k8s manifests at the root
                // (no k8s/ dir) by looking for the marker fields inside
                // a bare `*.yaml`. FileContaining is bounded to the
                // first 4 KiB so it stays fast on large repos.
                DetectionPattern::FileContaining {
                    file: "*.yaml".into(),
                    containing: "kind: Deployment".into(),
                },
                DetectionPattern::FileContaining {
                    file: "*.yaml".into(),
                    containing: "apiVersion: apps/v1".into(),
                },
            ],
            version_from: None,
            suggests: vec!["helm".into(), "kustomize".into(), "k9s".into()],
            category: ToolCategory::Ops,
        },
        DetectionRule {
            name: "helm".into(),
            detect: vec![
                DetectionPattern::File {
                    file: "Chart.yaml".into(),
                },
                DetectionPattern::Dir {
                    dir: "charts".into(),
                },
            ],
            version_from: None,
            suggests: vec![],
            category: ToolCategory::Ops,
        },
        DetectionRule {
            name: "terraform".into(),
            detect: vec![
                DetectionPattern::File {
                    file: ".terraform.lock.hcl".into(),
                },
                DetectionPattern::File {
                    file: "main.tf".into(),
                },
                DetectionPattern::File {
                    file: "*.tf".into(),
                },
            ],
            version_from: None,
            suggests: vec!["tflint".into(), "terraform-docs".into()],
            category: ToolCategory::Ops,
        },
        DetectionRule {
            name: "pre-commit".into(),
            detect: vec![DetectionPattern::File {
                file: ".pre-commit-config.yaml".into(),
            }],
            version_from: None,
            suggests: vec![],
            category: ToolCategory::Dev,
        },
        // git: any repo with a `.git/` dir needs git installed. This is
        // near-universally present on dev boxes already; the value is
        // in ephemeral environments (fresh containers, Codespaces
        // devcontainers, CI runners) where git is NOT preinstalled.
        // The install path short-circuits on `has("git")` so the cost
        // is one PATH lookup on the common case.
        DetectionRule {
            name: "git".into(),
            detect: vec![DetectionPattern::Dir { dir: ".git".into() }],
            version_from: None,
            suggests: vec![],
            category: ToolCategory::Dev,
        },
        // GitHub: a `.github/` directory signals workflows, issue
        // templates, or CODEOWNERS — all of which are typically
        // driven from the terminal via `gh`. We surface it under the
        // synthetic tool name `gh` (the CLI binary) so the wizard
        // recommends `gh = "latest"` rather than an uninstallable
        // "github" bucket entry. `release-plz` is suggested because
        // release-plz is GitHub-Action-first and roughly 90 % of its
        // users pair it with a `.github/workflows/release-plz.yml` —
        // if a `release-plz.toml` is also present, its own rule below
        // upgrades the suggestion to a required entry.
        DetectionRule {
            name: "gh".into(),
            detect: vec![DetectionPattern::Dir {
                dir: ".github".into(),
            }],
            version_from: None,
            suggests: vec!["release-plz".into()],
            category: ToolCategory::Dev,
        },
        // release-plz: the `release-plz.toml` config file at the repo
        // root is the canonical marker (see
        // <https://release-plz.dev>). No version pinning — the tool
        // is installed from crates.io HEAD via `cargo install
        // --locked` regardless. Categorised as `Dev` because it's
        // release-workflow tooling, not a runtime dependency of the
        // project itself.
        DetectionRule {
            name: "release-plz".into(),
            detect: vec![DetectionPattern::File {
                file: "release-plz.toml".into(),
            }],
            version_from: None,
            suggests: vec![],
            category: ToolCategory::Dev,
        },
        // VSCode: a `.vscode/` dir (settings.json, launch.json,
        // extensions.json, tasks.json) is a strong signal the repo's
        // owner intends VSCode as the primary editor. Optional — a
        // team may share `.vscode/settings.json` for lint-on-save
        // config even while individuals use nvim / Cursor — so ship
        // the recommendation and let the wizard prompt the user.
        DetectionRule {
            name: "vscode".into(),
            detect: vec![DetectionPattern::Dir {
                dir: ".vscode".into(),
            }],
            version_from: None,
            suggests: vec![],
            category: ToolCategory::Dev,
        },
        // Infisical: `.infisical.json` (project link file) or a
        // `.env.infisical` sample lands at the repo root when a
        // project is wired to Infisical for secret injection. Detect
        // either — presence of the CLI is a hard requirement to run
        // `infisical run -- <cmd>` in dev / CI.
        DetectionRule {
            name: "infisical".into(),
            detect: vec![
                DetectionPattern::File {
                    file: ".infisical.json".into(),
                },
                DetectionPattern::File {
                    file: ".env.infisical".into(),
                },
            ],
            version_from: None,
            suggests: vec![],
            category: ToolCategory::Dev,
        },
        DetectionRule {
            name: "make".into(),
            detect: vec![
                DetectionPattern::File {
                    file: "Makefile".into(),
                },
                DetectionPattern::File {
                    file: "makefile".into(),
                },
                DetectionPattern::File {
                    file: "GNUmakefile".into(),
                },
            ],
            version_from: None,
            suggests: vec![],
            category: ToolCategory::Build,
        },
        DetectionRule {
            name: "just".into(),
            detect: vec![DetectionPattern::File {
                file: "Justfile".into(),
            }],
            version_from: None,
            suggests: vec![],
            category: ToolCategory::Build,
        },
        // The following ecosystems trigger detection but typically
        // land in the `uninstallable` bucket because jarvy doesn't
        // ship first-party handlers yet. We still surface them so
        // contributors see "jarvy noticed you have Java but can't
        // install it for you" rather than silently doing nothing.
        DetectionRule {
            name: "maven".into(),
            detect: vec![DetectionPattern::File {
                file: "pom.xml".into(),
            }],
            version_from: None,
            suggests: vec![],
            category: ToolCategory::Build,
        },
        DetectionRule {
            name: "gradle".into(),
            detect: vec![
                DetectionPattern::File {
                    file: "build.gradle".into(),
                },
                DetectionPattern::File {
                    file: "build.gradle.kts".into(),
                },
                DetectionPattern::File {
                    file: "settings.gradle".into(),
                },
            ],
            version_from: None,
            suggests: vec![],
            category: ToolCategory::Build,
        },
        DetectionRule {
            name: "dotnet".into(),
            detect: vec![
                DetectionPattern::File {
                    file: "*.csproj".into(),
                },
                DetectionPattern::File {
                    file: "*.fsproj".into(),
                },
                DetectionPattern::File {
                    file: "global.json".into(),
                },
            ],
            version_from: None,
            suggests: vec![],
            category: ToolCategory::Runtime,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    /// Review P0 #2 — hostile filename with newline + `[section]` must
    /// not become a detection source string. The file-glob path is the
    /// only attacker-controllable detection input; rule-author literals
    /// are trusted.
    #[test]
    fn rejects_hostile_glob_match_filename() {
        let tmp = tempdir().unwrap();
        // Filename literally containing newline + section header.
        fs::write(tmp.path().join("x.tf\n[packages]\nbad = true\n.tf"), "").unwrap();
        let rule = DetectionRule {
            name: "terraform".into(),
            detect: vec![DetectionPattern::File {
                file: "*.tf".into(),
            }],
            version_from: None,
            suggests: vec![],
            category: ToolCategory::Ops,
        };
        // Source MUST be None (no fallback to partial sanitization).
        assert!(rule_match_source(tmp.path(), &rule).is_none());
    }

    #[test]
    fn accepts_well_formed_filename_glob() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join("main.tf"), "").unwrap();
        let rule = DetectionRule {
            name: "terraform".into(),
            detect: vec![DetectionPattern::File {
                file: "*.tf".into(),
            }],
            version_from: None,
            suggests: vec![],
            category: ToolCategory::Ops,
        };
        assert_eq!(
            rule_match_source(tmp.path(), &rule).as_deref(),
            Some("main.tf")
        );
    }

    #[test]
    fn sanitize_source_table() {
        assert_eq!(sanitize_source("Cargo.toml").as_deref(), Some("Cargo.toml"));
        assert_eq!(sanitize_source("k8s").as_deref(), Some("k8s"));
        assert!(sanitize_source("").is_none());
        assert!(sanitize_source("x\nbad").is_none());
        assert!(sanitize_source("has\"quote").is_none());
        assert!(sanitize_source("back\\slash").is_none());
        assert!(sanitize_source("nul\0byte").is_none());
        assert!(sanitize_source(&"x".repeat(256)).is_none());
    }
}
