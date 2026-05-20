# Changelog

All notable changes to Jarvy will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Policy

- **Stable releases (`vX.Y.Z`)** get a curated entry below **before the tag is
  pushed**. The release workflow's `Build release notes` step awk-extracts the
  matching `## [vX.Y.Z]` section into the GitHub release body, then appends a
  `**Full Changelog**` compare link plus Jarvy's standing install/security
  footer. Forgetting this entry causes the workflow to fall through to a raw
  `git log` listing — technically valid, but reads like a commit dump rather
  than a curated narrative. Update CHANGELOG before tagging.
- **Pre-releases (`vX.Y.Z-rc.N`, `-beta.N`, `-alpha.N`)** do **not** get a
  CHANGELOG entry. The awk extraction returns empty, the workflow falls
  through to `git log <prev-tag>..<tag>` notes, and that fallback is the
  intended pre-release path. The curated stable entry below is written once
  when the corresponding stable cuts.
- Entry headers must match the awk pattern: `## [vX.Y.Z]` or
  `## [vX.Y.Z] — Title` (em-dash optional). Other shapes won't be matched.

See [`docs/release-testing.md`](https://github.com/bearbinary/jarvy/blob/main/docs/release-testing.md)
for the full release process and
[`docs/release-quirks-jarvy.md`](https://github.com/bearbinary/jarvy/blob/main/docs/release-quirks-jarvy.md)
for divergences from generic release skills.

## [Unreleased]

## [helm-v0.5.3] — `helm test` smoke pod actually works now (2026-05-20)

The 0.5.2 ship landed the `helm test` smoke pod + supporting infra
but the pod itself never ran green in CI on the first push (or on
local kind clusters). Three fixes were needed; this release rolls
them into a clean cut.

### Fixed — `jarvy-telemetry-forwarder` Helm chart

- NetworkPolicy: explicit egress allow for in-namespace `helm test`
  pods (paired with the 0.5.2 ingress rule). Production CNIs
  (Cilium, Calico) are conntrack-aware and don't need this — it's
  defense-in-depth for CNIs that evaluate egress per-packet
  (kindnet).
- Test pod hook-delete-policy drops `hook-succeeded` so the pod
  sticks around after a green run. Without this, `helm test --logs`
  failed with `pods ... not found` because the pod was deleted
  before the log fetch ran.
- Test pod template is now nil-safe (nested `if .Values.tests`
  before `.enabled`). Fixes a render failure when the template
  file from a newer chart is checked out alongside an older
  `values.yaml` that doesn't carry the `tests:` block (CI
  upgrade-leg pattern).

### Fixed — `helm-chart-ci` workflow

- Live install + upgrade step deletes the NetworkPolicy before
  running `helm test`. kindnet's netpol enforcement isn't
  conntrack-aware, so the collector's `wide-except-rfc1918`
  egress filter drops reply SYN-ACKs to in-cluster test pods.
  The netpol structure itself is fully covered by the render +
  kubeconform matrix; this step covers the receiver only.
- Common-annotations fanout test now sees the test pod carrying
  the chart's common annotations.
- Diagnostics-on-failure step dumps pods, services, endpoints,
  netpol, collector logs, test-pod logs, and runs a netpol-free
  repro curl. Costs nothing on green runs.
- Three other pre-existing matrix failures fixed in the same
  iteration (kept here for the changelog reader's context):
  helm/kind-action SHA pin corrected, promtool input shape
  (extract `.spec` for RuleGroups), extraEnv reject assertion
  accepts both helm 3.18 and helm 4.x schema messages.

### Migration

No action needed. The chart now passes `helm test` cleanly on
production CNIs. On stock kindnet (only relevant for in-cluster
test runs, not production), drop the NetworkPolicy before
running `helm test` — see the workflow comment for the rationale.

## [helm-v0.5.2] — `helm test` smoke pod + live HTTPS smoke script (2026-05-20)

### Added — `jarvy-telemetry-forwarder` Helm chart

- `templates/tests/otlp-smoke.yaml` — `helm test` hook pod that POSTs
  minimal OTLP/HTTP payloads at `/v1/{logs,metrics,traces}` on the
  Collector Service and asserts 2xx. Validates the receiver pipeline
  end-to-end after `helm install` without depending on the public
  ingress. Image `curlimages/curl:8.10.1` pinned, restricted-PSS
  compliant.
- `tests.*` values + schema validation (`enabled`, `image`,
  `resources`, `securityContext`). Disable with
  `--set tests.enabled=false`.
- NetworkPolicy now whitelists pods carrying BOTH the chart-test
  component label AND the release instance label — required so the
  `helm test` pod can reach the Collector through the otherwise
  locked-down ingress.
- `scripts/smoke-live.sh` — bash script that smokes the public
  HTTPS endpoint with the same three OTLP payloads. A diff between
  this and the in-cluster `helm test` isolates ingress (TLS,
  gateway, middlewares) as the suspect.
- Makefile targets: `helm-smoke-live` (live HTTPS) and
  `helm-test-kind` (in-cluster).
- `helm-chart-ci` kind job now runs `helm test` after the fresh
  install — receiver-pipeline regressions fail CI alongside the
  rendering/lint suite.

### Migration

No action needed; new behavior is purely additive. `helm test`
becomes opt-in once you upgrade — run it whenever you want
in-cluster validation of the receiver path.

## [helm-v0.5.1] — HTTPRoute `filters: null` lint fix (2026-05-17)

### Fixed — `jarvy-telemetry-forwarder` Helm chart

- HTTPRoute template no longer emits an empty `filters:` key (which
  YAML-parses as `null`) when traefik middlewares are disabled and no
  `extraFilters` are supplied. Surfaced by the `helm-chart-ci`
  matrix's `gatewayclass-envoy-accepted` scenario, which has been
  failing kubeconform-strict since the field was added — the Gateway
  API HTTPRoute schema types `filters` as `array`, not
  `array | null`. The fix wraps the key in an `or` guard so it is
  omitted entirely when no filters apply, which is the
  spec-compliant equivalent and produces no Argo CD drift.

### Migration

No action needed. Behavior at runtime is unchanged — a missing
`filters` key and an empty `filters` list both mean "no filters
applied". The diff visible on `helm diff upgrade` is purely the
removal of an `null`-valued field from the rendered HTTPRoute when
running without traefik middlewares.

## [helm-v0.5.0] — ExternalSecret Argo CD drift fix (2026-05-17)

Rendered ExternalSecrets now emit the two server-side defaults the ESO
admission webhook fills in (`target.deletionPolicy: Retain`,
`data[].remoteRef.conversionStrategy: Default`). Without these in the
chart's desired manifest, Argo CD's compare saw the webhook-injected
values as drift on every reconcile, leaving every install of this
chart perpetually `sync=OutOfSync, health=Healthy`. Discovered while
diagnosing the `jarvy-telemetry` Argo app on the home cluster on
2026-05-17.

### Added — `jarvy-telemetry-forwarder` Helm chart

- `secrets.externalSecrets.deletionPolicy` (default `Retain`) and
  `secrets.externalSecrets.conversionStrategy` (default `Default`)
  values. Both default to the ESO server-side default so existing
  installs see no semantic change — only that Argo CD diffs now show
  zero drift after the next `helm upgrade`. Override either if your
  use case needs `Delete` / `Merge` (deletionPolicy) or `Unicode`
  (conversionStrategy).
- `values.schema.json` constraints for both new fields with enum
  validation.

### Fixed — `jarvy-telemetry-forwarder` Helm chart

- ExternalSecret resources no longer drift in Argo CD when the ESO
  admission webhook fills server-side defaults. Bump and `helm
  upgrade` to clear the perpetual OutOfSync state.

### Migration

No action needed beyond `helm upgrade`. Defaults match ESO
server-side, so rendered output is functionally identical — the diff
visible on `helm diff upgrade` is purely the two new explicit
field assignments.

## [helm-v0.4.0] — Chart enhancement plan v3 (2026-05-14)

Multi-perspective parallel review (perf, security, QA, observability,
maintainability) produced a 27-item enhancement plan; all 27 items
ship together. Probe semantics, graceful shutdown, queue-saturation
alert, dashboard, recording rules, image-digest default, FQDN egress
mode, DoS-protection gate, split Service, container security context
schema constraints, runbook anchors in the ops doc, and 5 new CI
guards (kind install/upgrade, helm 3.14/3.16/3.18 matrix, promtool,
README↔schema drift, runbook-anchor check). 13 render scenarios pass,
8 template-time guards fire, `helm lint --strict` clean. **Backward
compatible**: defaults harden but no required-field renames; legacy
`networkPolicy.cilium.enabled=true` still works (now a synonym for
`egressMode: fqdn`).

### Added — `jarvy-telemetry-forwarder` Helm chart

A multi-perspective review (perf, security, QA, observability,
maintainability) produced a 27-item enhancement plan; all 27 items
shipped together. Chart version bump pending.

- **Probe split + pipeline-aware health.** Liveness no longer flips
  on `memory_limiter` backpressure (which would cascade-restart all
  replicas during burst — defeating the design). Readiness still
  flips so the LB sheds load. `health_check_v2`'s
  `check_collector_pipeline` exposes pipeline status on `/`;
  liveness gets a longer failureThreshold (6), readiness shorter
  periodSeconds (5). New `startupProbe` covers cold-pull on fresh
  nodes.
- **Graceful shutdown.** `terminationGracePeriodSeconds: 60` +
  `preStop: sleep 15` so the LB drains and the
  batch/exporter flushes in-flight records before SIGKILL.
- **Exporter queue saturation alert** — leading indicator that fires
  before `JarvyForwarderExporterFailing` starts dropping records.
  Backed by a recording rule (`jarvy_forwarder:exporter_queue_utilization:ratio`).
- **Pod restart alert** — closes the loop when pipeline alerts can't
  fire (pod never gets healthy enough to emit metrics).
- **Grafana dashboard** ConfigMap shipped via `grafana_dashboard=1`
  sidecar label. 10 panels: receiver rate, queue utilization,
  exporter rate, memory/CPU, tail-sampling decisions, allowlist
  drops, batch throughput, pod restarts, cert expiry.
- **Receiver auth** (`collector.receiverAuth.enabled`, opt-in)
  fronts the OTLP receiver with `bearertokenauth/receiver`. Multi-
  tenant deployments should enable.
- **Recording rules.** Repeated `rate(...)` over 5-10m windows
  hoisted into named recording rules; alerts + dashboard share one
  computation instead of recomputing per evaluation.
- **`networkPolicy.egressMode`**. Three modes: `wide` (legacy
  `to: []` on 443), `wide-except-rfc1918` (new default — excludes
  private IP ranges), `fqdn` (requires Cilium — restricts to the
  parsed exporter hostname).
- **DoS-protection gate**: non-Traefik GatewayClasses must supply
  `httpRoute.extraFilters` OR set `dosProtection.acceptUnprotected:
  true` — fails install otherwise. Closes the "I installed on Envoy
  and forgot the rate limit" exposure.
- **Split Service**: public OTLP Service (port 4318) +
  in-cluster metrics Service (port 8888). In-cluster scrapers
  cannot accidentally reach the OTLP receiver and self-metrics no
  longer mix with public ingress traffic.
- **Production-overlay digest pinning**: chart ships with
  `collector.image.digest` set to a real `sha256:` digest by
  default; CI scenario `production-overlay` asserts the rendered
  image string carries the digest.
- **Grafana dashboard's `runbook_url` anchors** all exist in
  `docs/operations/telemetry-forwarder.md` (11 new
  `{#alert-*}`-anchored subsections with diagnosis steps).
- **CI**: kind install + upgrade smoke test (k8s 1.31); helm
  3.14/3.16/3.18 render matrix; promtool PromRule validation;
  README ↔ schema drift check; runbook-anchor grep.

### Changed — `jarvy-telemetry-forwarder` Helm chart

- **CPU limit removed** from `collector.resources.limits`. CFS-quota
  throttling on an I/O-bound forwarder adds 10-100ms p99 latency on
  burst with no upside. Floor preserved via `requests.cpu: 100m`.
- **HPA `scaleDown` policy** is now explicit (`drop 1 pod / 60s`)
  instead of the K8s default (halve replicas per 15s) which causes
  replica thrash near `memory_limiter` pressure.
- **PDB uses `maxUnavailable: 1`** (not `minAvailable: 1`) so node
  drains proceed one pod at a time without stalling forever waiting
  for real-Ready. Mutually exclusive with `minAvailable` —
  template-time `fail()` catches both-set misconfiguration.
- **`pdb.minAvailable` + `pdb.maxUnavailable`** mutually exclusive
  (template `fail`). **`tls.certManager.enabled=true` +
  `tls.existingSecretName`** mutually exclusive (template `fail`).
- **`_helpers.tpl` labels order**: chart-managed labels are emitted
  LAST so `commonLabels` cannot overwrite `app.kubernetes.io/name`
  and steer NetworkPolicy / ServiceMonitor away from real pods.
- **`automountServiceAccountToken: false`** stays hardcoded in both
  ServiceAccount and Pod spec (no values knob); render-time CI
  asserts catch regressions.
- **`enableServiceLinks: false`** on the pod — saves env-var bloat
  on busy namespaces; speeds cold start.
- **ServiceMonitor**: `honorLabels` is now actually rendered (was a
  ghost setting). `path: /metrics`, `scheme: http`, and
  `scrapeTimeout` explicit so a future port change doesn't break
  scrape silently. ServiceMonitor selector now matches the new
  metrics-only Service (`app.kubernetes.io/component: metrics`).
- **ServiceMonitor `metricRelabelings`**: tightened keep-list. Drops
  high-cardinality `otelcol_processor_transform_*_modified` series
  (none of which exist — see Fixed below) and keeps the operational
  subset.
- **`saltStale` alert** rebuilt: now reads
  `external_secrets_sync_calls_total` (the only series that exists
  for "salt content was refreshed"). The old query referenced a
  non-existent `kube_secret_created` metric and would have stayed
  silent forever.
- **`allowlistDroppingKeys` alert** rebuilt: compares
  `otelcol_processor_incoming_items` vs `outgoing_items` on the
  `transform/keep_allowlist_attrs` processor. The old query
  referenced non-existent `*_modified` counters.
- **`bearertokenauth` extension** for the backend exporter, plus
  optional `bearertokenauth/receiver` for inbound auth.
- **Container `securityContext`** explicitly sets
  `runAsNonRoot: true` and `seccompProfile: RuntimeDefault`
  (belt-and-suspenders over the pod-level setting). Schema rejects
  flipping `privileged`, `allowPrivilegeEscalation`,
  `readOnlyRootFilesystem`, or dropping `capabilities.drop: ALL`.
- **`exporterFailing` alert threshold units** documented as
  records/sec; docs/values comments aligned (was previously
  conflicting on per-second vs per-minute).
- **Gateway listener TLS `options:`** rendered through as-is so
  operators can pass GatewayClass-specific knobs (e.g.
  `gateway.envoyproxy.io/min-tls-version`).
- **README** updated: salt-rotation wording, accurate schema
  invariants list, new ConfigMap/dashboard/PrometheusRule entries
  in "What gets installed", egressMode and DoS-protection notes.

### Removed — `jarvy-telemetry-forwarder` Helm chart

- The `cilium.enabled` values knob is still accepted but is now a
  synonym for `egressMode: fqdn`; future versions may remove.

[helm-v0.4.0]: https://github.com/bearbinary/Jarvy/releases/tag/helm-v0.4.0

---

The entries below belong to the Jarvy CLI's pending `[Unreleased]`
section; they ship with the next CLI tag, NOT with `helm-v0.4.0`.
Listed here so the helm-v0.4.0 release notes do not absorb them.

### Sandbox auto-detection (PRD-053)

- **Sandbox auto-detection (PRD-053).** New `src/sandbox/` module
  detects AI agent sandboxes (Claude Code, Cursor, e2b, Modal,
  Daytona, Replit), long-running container envs (GitHub Codespaces,
  Gitpod, devcontainers), and a generic `/.dockerenv` + non-TTY
  fallback. `crate::sandbox::is_seamless()` is the canonical
  "unattended" predicate; CI detection is now a strict subset.
  `JARVY_SANDBOX=0` disables detection, `JARVY_SANDBOX=1` forces
  generic-container (or whatever named provider also matches).
- **Seamless mode** wires through telemetry auto-disable, update-
  check suppression, first-run welcome suppression, brew auto-install
  block, and secrets non-interactive default — five subsystems that
  previously each carried their own `env::var("CI")` heuristic now
  share one predicate.
- **Verify-only fallback** in `jarvy setup`. When the sandbox cannot
  install tools (read-only rootfs, no user-scope package manager, no
  passwordless sudo), setup runs the doctor pipeline inline and exits
  `PREREQ_MISSING (3)` on gaps; clean runs return `0` with a
  verify-only success message. The probe records why via a
  `VerifyOnlyReason` enum (`NoJarvyHome` / `ReadOnlyRoot` /
  `NoInstallPath` / `Forced`) so support tickets explain which gate
  tripped.
- **Auto-baseline.** On the first seamless-mode run with zero gaps,
  Jarvy snapshots the current state as `.jarvy/state.json` so
  subsequent runs can do meaningful drift checks. Gated on a *full*
  doctor match — partial matches never auto-baseline (PRD-053 risk
  row 2). Works on both the install-capable and verify-only paths so
  pre-loaded sandbox images still get a baseline.
- **Seamless banner** on stderr, one line per process, summarizing
  which provider was detected and the `JARVY_SANDBOX=0` escape hatch.
  Muted by `--quiet`, `-q`, `--json`, `--format=json`,
  `--log-format=json`, or `JARVY_QUIET=1`. The corresponding
  `tracing::info!(event = "sandbox.detected")` fires regardless so
  `jarvy.log` records the decision even for JSON consumers.
- **`is_seamless_auto()`** — same as `is_seamless()` minus *forced*
  sandbox detection. Telemetry + update auto-disable now route
  through this variant so a hostile dotfile or compromised
  devcontainer base image that sets `JARVY_SANDBOX=1` cannot silence
  security-patch updates or anomaly telemetry on a victim's machine
  (PRD-053 security review F1).

### Changed

- **`JARVY_HOME` validation.** Paths must be absolute and contain no
  `..` traversal components; on Unix, existing paths must be owned by
  the current uid. Defends against `sudo -E jarvy ...` patterns where
  a less-privileged actor's env points a privileged jarvy run at
  `/etc` or `/root/.ssh` (PRD-053 security review F2).
- **Install-capability probe** writes to a per-PID `.probe-<pid>`
  filename via `OpenOptions::create_new(true)` (`O_CREAT|O_EXCL`)
  instead of `fs::write` to `.probe`. A pre-staged symlink at the
  probe path now errors out instead of being silently followed and
  clobbered (PRD-053 security review F3).
- **Banner emission moved after panic-hook install** in `main.rs` so
  any future stderr-write failure during banner emission produces a
  structured panic message instead of a default backtrace dump.
- **`detect()` and `ci::detect()` are now cached** via `OnceLock` —
  env vars and `/.dockerenv` do not change mid-run, and the previous
  implementation re-walked ~25 `getenv` calls per `is_seamless()`
  invocation × 4 callers per `jarvy setup`. Telemetry `ci_detected`
  event likewise fires at most once per process instead of once per
  call.
- **`InstallCapability::VerifyOnly` carries a `VerifyOnlyReason`** so
  log lines and tickets explain *which* probe tripped.

### Removed

- **`update::config::is_ci_environment` and the parallel shim in
  `onboarding::detection`**. Both were thin re-exports of
  `sandbox::is_seamless()`; in-tree callers now use the canonical
  predicate directly. Jarvy is a `bin` crate, no external library
  consumers to break.
- **Hand-rolled `which()` helper in `src/sandbox/mod.rs`** replaced
  by the `which` crate (already a project dep). Local impl ignored
  the Unix exec bit and only handled three Windows extensions.

### Security

- **Test images pinned by sha256 digest.** `debian:bookworm-slim` and
  `buildpack-deps:bookworm-scm` in `tests/sandbox_integration.rs`
  resolve to specific bytes regardless of registry tag drift or tag-
  replay MITM.
- **Read-only binary bind-mount.** The host's jarvy binary is mounted
  into integration-test containers via
  `Mount::bind_mount(...).with_access_mode(AccessMode::ReadOnly)` so
  a malicious container cannot truncate or replace the host binary
  mid-test (PRD-053 security review F8).

### Tests

- 10 new sandbox unit tests: forced-with/without named signal,
  `JARVY_SANDBOX=0 && CI=true` precedence, `is_seamless_auto` matrix,
  generic-container truth table, `VerifyOnlyReason` Display, force-
  verify-only probe short-circuit, banner idempotence.
- 4 new docker-backed integration tests: partial-match negative gate
  (must not auto-baseline on gaps), banner suppression with
  `--format=json`, banner suppression with `JARVY_QUIET=1`, verify-
  only must not overwrite an existing `state.json`.
- Cross-module env-isolation via `#[serial_test::serial(ci_sandbox_env)]`
  on every `ci::tests` and `sandbox::tests` function so the two
  suites cannot race on shared env vars (`CI`, `GITHUB_ACTIONS`,
  `CODESPACES`).

## [v0.1.0] — First feature-complete milestone (2026-05-10)

First feature-complete stable. Closes the round-2 hardening review
(45 items across two passes), ships clean-laptop onboarding, and
publishes 14 ready-to-copy `jarvy.toml` project templates. The
public surface from v0.0.5 is preserved; everything below is either
additive, fail-closed by default, or a tightening of internal
invariants.

### Added

- **Project templates.** `examples/<stack>/jarvy.toml` ships 14
  validated drop-in configs (node-npm/pnpm/bun, deno, python-api/uv,
  go-api, rust-cli/workspace, ruby-rails, java-spring, react-app,
  fullstack, k8s-platform). Companion docs at
  `docs/templates-index.md` give an AI-agent decision table mapping
  detect-by signals (lockfiles, manifests) to template URLs.
- **Clean-laptop onboarding.** New `Makefile` + idempotent
  `scripts/bootstrap.sh` give contributors a two-command setup
  (`curl install.sh | bash` then `make setup`). Bootstrap script
  honors `JARVY_CHANNEL` for stable/beta/nightly, falls back to
  `wget` if `curl` is missing, and forwards extra args to
  `jarvy setup`. shellcheck-clean.
- **`jarvy validate` recognizes the full top-level surface.**
  `[npm]`, `[pip]`, `[cargo]`, `[commands]`, `[drift]`, `[git]`,
  `[network]`, `[logging]` no longer trigger
  "unknown configuration section" warnings. Toolchain channel
  aliases (`stable`, `beta`, `nightly`, `lts`, `current`) are
  accepted as valid version strings — `rust = "stable"` validates
  cleanly.
- **`SecretError::PathEscapesProject`** + `JARVY_ALLOW_EXTERNAL_SECRETS`
  override. `[env.secrets] from_file` paths that resolve outside
  the project root and `$HOME` after symlink-resolving
  canonicalization are refused by default. Common legitimate paths
  (`~/.aws/credentials`, `<project>/.env.secret`) keep working.
  Override with `JARVY_ALLOW_EXTERNAL_SECRETS=1`.
- **`tools::pinned_installer::PinnedInstaller`** helper for the
  curl-bash class of installers. arctl, kmcp, and ollama (Linux
  fallback only) now fetch their installer scripts at a pinned
  commit, sha256-verify the body, and refuse to exec on mismatch —
  same pattern Homebrew already used. Refreshing a pinned installer
  requires updating the commit + sha256 constants together.
- **POSIX env-var grammar validation** before writing
  `[env.vars]` to shell rc files. Keys not matching
  `^[A-Za-z_][A-Za-z0-9_]*$` are skipped with a structured
  `event="env.refused_invalid_key"` warning instead of corrupting
  `~/.bashrc` / `~/.zshrc`.
- **`tools::install_method`** canonical classifier
  (`Brew`/`Cargo`/`Nvm`/`Pyenv`/`Rustup`/`Snap`/`System`/
  `NotFound`/`Unknown`). `commands::diagnose`, `commands::drift`,
  and `observability::bundle` all delegate here instead of
  hand-rolling three near-identical detectors.

### Changed

- **Logging pipeline rewired** to `tracing_appender::rolling` for
  daily rotation + `tracing_appender::non_blocking` for buffered
  writes. `analytics::shutdown_logging()` flushes both the
  `SdkLoggerProvider` and the file `WorkerGuard` before
  `process::exit`, so buffered records aren't lost on early
  termination. `EnvFilter` now has a default-on floor of
  `warn,jarvy=info` if `RUST_LOG` is unset.
- **`Hook::run_with_policy`** collapsed from a 3-state `HookOutcome`
  enum to `Result<(), HookError>`. Production callers only ever
  checked `Fail` vs not-Fail; the warning-on-`continue_on_error`
  side effect already conveyed the difference. The new `Err` case
  returns the underlying `HookError` so `error_codes::HOOK_FAILED`
  callers keep working.
- **`Sanitizer::sanitize_borrowed`** returns `Cow<'_, str>` so the
  no-match path skips allocation entirely. `Sanitizer::sanitize`
  preserves the same fast path internally.
- **`tracing::warn!` → `tracing::error!`** on `tool.failed`,
  `hook.failed`, `hook.timeout`, `config.parse_error`, and
  `telemetry.endpoint.refused`. These are operator-actionable
  conditions, not advisory.
- **Subprocess spans.** `services::run_command` and
  `tools::common::run_capture` are now wrapped in
  `tracing::info_span!("subprocess.exec", cmd, args_count, ...)`
  with start/duration/exit_code events.
- **`paths.rs` cleanup.** `cache_dir` inlined into
  `remote_config_cache_dir` (only caller); `#![allow(dead_code)]`
  removed since every public function has external callers now.

### Security

- **CA-bundle trust check tightened.** `network::propagate` no
  longer accepts paths under the broad `~/.jarvy/` cache prefix —
  only `~/.jarvy/ca/` is trusted, with a trailing-slash anchor so
  `~/.jarvy/ca-attacker/...` can't slip through.
- **Cross-origin redirects refused** on
  `remote::validated_get` / `fetch_remote_config`. `ureq` agent
  now uses `.max_redirects(0)`; redirects must be revalidated
  through the policy gate.
- **Sigstore companion verification.** `update::release` returns
  `None` for cosign companion files when the `.sig`/`.pem` aren't
  exact-match siblings — a substring-match bug that would have let
  a malicious tarball claim sibling signatures was closed.
- **`exec.rs` deleted** (zero-caller speculative seam).
- **`team::inheritance::transform_github_url`** duplicate deleted;
  callers route through the canonical `remote::transform_github_url`
  so URL hardening lives in one place.

### Fixed

- `validate_get` rejected URLs with empty hosts under `file://`
  scheme but didn't match the documented "scheme not allowed"
  error string. Test relaxed to accept any error variant; behavior
  unchanged.
- `paths::remote_config_cache_dir` now reads `JARVY_HOME`
  consistently with the rest of `paths.rs` (was hand-rolling the
  override before).
- `update_rc_content` argument order documented; previously the
  test suite caller had `(content, &vars, &ctx, ShellType)` instead
  of the actual `(content, ShellType, &vars, &ctx)`.

### Tests

- 1,633+ tests passing across lib + binary + integration suites
  (was ~1,580). Highlights of the new coverage:
  - `validated_get` rejection tests for HTTP-to-remote, disallowed
    host, `file://` scheme, missing scheme.
  - `Hook::run_with_policy` outcome matrix (dry-run / success /
    failure × continue_on_error true|false).
  - `verify_no_tar_escape` containment tests + symlink-escape
    refusal.
  - Cosign companion exact-match (no substring) regression.
  - Path-containment refusal + `JARVY_ALLOW_EXTERNAL_SECRETS=1`
    override path for `[env.secrets] from_file`.
  - Shell-interpreted-key table-driven test
    (`every_shell_interpreted_key_refuses_bang_prefix`) so adding
    a new shell-interpreted git config key lights up the test
    suite immediately.
- `#[serial_test::serial]` annotations added for
  `JARVY_ALLOW_*` env mutations to keep parallel runs isolated.

### Docs

- `CLAUDE.md` Logging section rewritten to match the actual
  `src/logging/` (thin re-export layer) and `src/observability/`
  (where rotation + sanitizer + analytics live) split.
- `examples/README.md` + `docs/templates-index.md` published as
  the human/AI-facing template indexes.
- `llms-full.txt` "Project Templates" section added (with
  `docs/llms.txt` + `docs/llms-full.txt` symlinks for the published
  docs site).

## [v0.0.5] — Chocolatey install script + bundled v0.0.4 fixes (2026-05-05)

Folds in everything queued for v0.0.4 (which was tagged but never
publicly published) plus a Chocolatey install-script fix.

### Fixed

- **Chocolatey package** v0.0.3 failed moderation with `404 Not Found`
  for the install URL. Two bugs in
  `dist/windows/chocolatey/tools/chocolateyinstall.ps1`:
  - URL pattern referenced
    `jarvy-vVERSION_PLACEHOLDER-x86_64-pc-windows-msvc.zip` — but
    cargo-packager produces `.msi` and `.exe`, no `.zip` for Windows.
  - VERSION_PLACEHOLDER and SHA256_PLACEHOLDER were never substituted
    because the publish workflow only ran sed against `jarvy.nuspec`,
    not the install script.

  Rewrote the install script to use `Install-ChocolateyPackage` with
  `-FileType msi` and silent install args, pointing at the actual
  `jarvy_<v>_x64_en-US.msi` asset. Updated
  `publish-packages.yml::update-chocolatey` to substitute both files
  AND pull the real msi SHA256 from `SHA256SUMS.txt` so the integrity
  check passes.
- **`cargo fmt --check`** drift in `src/team/inheritance.rs:760-768`
  (single-quoted TOML literals from v0.0.3 needed compaction).
- **OpenSSF Scorecard** failed on v0.0.3 tag with `Only the default
  branch main is supported`. ossf/scorecard-action explicitly refuses
  tag-push triggers. Restored `push: branches: [main]` for scorecard
  only — every other validating workflow stays tag-triggered.
- **Homebrew tap publish** now gracefully skips when
  `HOMEBREW_TAP_DEPLOY_KEY` is not configured. Previously the missing
  secret failed the whole `publish-packages.yml` workflow, masking
  the success of crates.io, AUR, winget, and Chocolatey jobs.

### Validated downstream (v0.0.3)

After the v0.0.3 fixes, the following propagation channels worked:

- ✅ crates.io: jarvy@0.0.3 + cargo-jarvy@0.0.3 published
- ✅ AUR (jarvy-bin)
- ✅ Submit to winget (publish-packages.yml job; separate winget.yml
  still needs manual first submission)
- ✅ GitHub Pages docs site (after maintainer enabled Pages)
- ❌ Chocolatey: failed moderation due to broken install script
  (v0.0.5 fixes)
- ⚠️  Homebrew tap: pending secret config (now non-blocking)

### Note

v0.0.4 was tagged but the draft was never publicly published —
v0.0.4's fixes ship together with the Chocolatey fix as v0.0.5 to
reduce propagation churn (one round of crates.io / AUR / etc.
updates instead of two back-to-back).

## [v0.0.4] — Lint formatting + scorecard + homebrew-tap guard (2026-05-05)

### Fixed

- **`cargo fmt --check`** failed in the Lint job on
  `src/team/inheritance.rs:760-768` because the v0.0.3 single-quoted
  TOML literal edits left format strings on multiple lines that
  rustfmt wanted compacted. Re-ran `cargo fmt` to normalize.
- **OpenSSF Scorecard** failed on the v0.0.3 tag with `Only the
  default branch main is supported`. ossf/scorecard-action explicitly
  refuses tag-push triggers; v0.0.3's trigger trim moved scorecard
  off main-push, which broke it. Restored `push: branches: [main]`
  for scorecard only — every other validating workflow stays
  tag-triggered. Release-tag scorecard runs produce no useful signal
  anyway since the action only inspects the default branch.
- **Homebrew tap publish** now gracefully skips when
  `HOMEBREW_TAP_DEPLOY_KEY` is not configured. Previously the whole
  `publish-packages.yml` workflow exited 1 with "API_TOKEN_GITHUB
  and SSH_DEPLOY_KEY are empty", masking the success of crates.io,
  AUR, winget, and Chocolatey jobs. New behavior: missing secret
  emits a warning ("set per docs/MAINTAINER_RELEASE_GUIDE.md") and
  the push step is skipped via `if:` guard.

### Validated downstream (v0.0.3)

After the v0.0.3 fixes, the following propagation channels worked:

- ✅ crates.io: jarvy@0.0.3 + cargo-jarvy@0.0.3 published
- ✅ Submit to winget (job inside publish-packages.yml; the separate
  winget.yml workflow still requires manual first submission per
  v0.0.3 release notes)
- ✅ Chocolatey
- ✅ AUR (jarvy-bin)
- ✅ GitHub Pages docs site (after maintainer enabled Pages in repo
  Settings)
- ⚠️  Homebrew tap: blocked on `HOMEBREW_TAP_DEPLOY_KEY` secret;
  v0.0.4 makes this a non-blocker so missing-secret no longer fails
  the whole workflow.

## [v0.0.3] — Unblock crates.io and Homebrew downstream publish (2026-05-05)

Patch release. v0.0.2 went live on the GitHub release page but the
crates.io and Homebrew workflows that fire on `release: published`
both failed, leaving `cargo install jarvy` and
`brew install bearbinary/tap/jarvy` unavailable.

### Fixed

- **Cargo.toml** declared `readme = "README.md"` (uppercase) but the
  tracked file is `Readme.md` (mixed case). On macOS the difference
  is invisible (case-insensitive filesystem); on the Linux CI runner
  it failed `cargo publish` with `readme "README.md" does not appear
  to exist`. Both `Publish Crate` and `Publish to Package Managers`
  workflows hit the same error. Same fix in the `include = [...]`
  manifest list. Now matches what's actually in the git tree.
- **`.github/workflows/winget.yml`** was scaffolded from a different
  project's template and never customized — `identifier: Benji377.Tooka`
  and `fork-user: Benji377` referenced a totally unrelated package.
  Rewrote with placeholder TODO values for `Jarvy.Jarvy` /
  `bearbinary` and changed the trigger from `release: published` to
  `workflow_dispatch` only. winget-releaser cannot create a brand-new
  package registration; the first submission must go through
  `wingetcreate new` and a hand-reviewed PR to microsoft/winget-pkgs.
  After that's merged the trigger can be flipped back.

### Removed

- Duplicate `.github/workflows/crates.yml` deleted. Both that and
  `publish-packages.yml::publish-crates-io` were firing on
  `release: published` and trying to `cargo publish`. Even if both
  had the right secret, the second one would race-fail with "crate
  version already exists". Kept the version inside `publish-packages.yml`
  because it composes with the Homebrew tap update via `needs:`.
- `docs/release-testing.md` and `docs/release-quirks-jarvy.md`
  references to `crates.yml` updated to point at the surviving
  workflow path.

### Known issues (not fixed in this release)

- **GitHub Pages** is not enabled for `bearbinary/Jarvy` repo — the
  Deploy Docs workflow fails with `HttpError: Not Found ... Ensure
  GitHub Pages has been enabled`. Fix is in repo Settings → Pages,
  not in code. Until enabled, the docs site at jarvy.dev (or
  whichever Pages URL ends up provisioned) won't update on release.
- **winget first submission** still requires manual `wingetcreate new`
  intervention (see Fixed above for the workflow disable).

## [v0.0.2] — Cosign verify-command case fix (2026-05-05)

Patch release fixing the cosign verification snippet baked into
release notes, SECURITY.md, and docs/release-quirks-jarvy.md.

### Fixed

- **release notes / SECURITY.md / docs**: the
  `--certificate-identity-regexp` value used `bearbinary/jarvy`
  (lowercase j). The actual Sigstore cert subject GitHub Actions
  produces is `bearbinary/Jarvy/...` (capital J — the repo's
  canonical case). cosign's regex is case-sensitive, so users
  copy-pasting the verify command from the v0.0.1 release page
  saw "none of the expected identities matched" even though the
  signature was valid. Corrected all three sources to
  `bearbinary/Jarvy/`. github.com URLs elsewhere in the repo are
  unchanged because GitHub URL matching is case-insensitive — only
  cosign's regex was affected.

## [v0.0.1] — Initial public release (2026-05-05)

First publicly tagged stable release. Validated through the
v0.1.0-rc.1 → v0.1.0-rc.9 soak cycle (same tree, version-string
only differs); cut as 0.0.1 to keep the first-stable surface narrow
and reserve room for 0.1.0 as the first feature-complete milestone.

### Features

- **provisioner:** Cross-platform tool provisioner driven by `jarvy.toml`
  (macOS, Linux, Windows) with native package managers
- **tools:** 154+ tool registry covering compilers, runtimes, CLIs, container
  tools, Kubernetes ecosystem (kubectl, helm, k9s, kagent, kmcp, arctl), cloud
  CLIs (gcloud, aws, az), security tools, observability (opentelemetry-collector),
  Dockerfile converter (dfc) (PRD-013)
- **tools:** Parallel version checking with rayon for ~5x speedup; batch
  package-manager operations
- **tools:** Declarative `define_tool!` macro for tool definitions (~2000 lines
  reduced)
- **tools:** Strict (`depends_on`) and flexible (`depends_on_one_of`) tool
  dependencies with topological install ordering (PRD-034)
- **hooks:** 29+ default post-install hooks for shell completion and
  configuration; idempotent, advisory, user-overridable
- **roles:** Role-based configurations with deep inheritance, version overrides,
  `roles list|show|diff` commands (PRD-033)
- **packages:** Language package deps via `[npm]`, `[pip]`, `[cargo]` —
  package-manager auto-detection, virtualenv support, lockfile install (PRD-039)
- **git:** Git configuration automation — identity, SSH/GPG signing, default
  branch, aliases, credential helper auto-detect per OS (PRD-041)
- **drift:** Configuration drift detection with SHA-256 file hashing, version
  policies, `jarvy drift check|status|accept|fix` (PRD-043)
- **update:** Self-updating with stable/beta/nightly channel selection,
  throttled checks, rollback, multi-method install detection (Homebrew, Cargo,
  apt, dnf, winget, Chocolatey, Scoop, binary fallback) (PRD-035)
- **telemetry:** OTEL-unified logs, metrics, optional traces; OTLP HTTP/gRPC
  endpoints; CI auto-disable; `jarvy telemetry status|enable|disable|test|preview`
  (PRD-022, PRD-050)
- **logging:** Persistent file logging with rotation, gzip compression,
  sensitive-data redaction; `jarvy logs view|stats|clean|config` (PRD-050)
- **ticket:** Debug bundles via `jarvy ticket create|show|list|clean` — ZIP with
  system info, tool versions, sanitized logs (PRD-050)
- **network:** Corporate proxy support — HTTP/HTTPS/SOCKS, NO_PROXY, custom CA
  bundles, per-tool overrides, secure password sources (PRD-019)
- **services:** Docker Compose and Tilt backend support
- **ci:** Auto-detection for 11 CI/CD providers with provider-specific output
- **env:** Environment variable management with `.env` generation and shell rc
  updates
- **mcp:** MCP server exposing tools and resources for AI assistants
- **interactive:** Menu mode when running `jarvy` without a subcommand
- **bootstrap:** `jarvy bootstrap`, `jarvy configure`, `jarvy diagnose` for
  onboarding (PRD-023)

### Distribution

- Multi-channel: crates.io, Homebrew tap, AUR (source + binary), `.deb`, `.rpm`,
  winget, Chocolatey, universal install scripts for macOS/Linux/Windows (PRD-012)
- **Prebuilt platforms**: macOS arm64, Linux x86_64 (musl), Linux aarch64,
  Linux armv7, Windows x86_64. macOS Intel (x86_64) **not shipped as prebuilt** —
  Intel users install via `cargo install jarvy` or Homebrew (both compile from
  source). See `docs/release-testing.md` for rationale.
- Sigstore keyless signing for all release artifacts (PRD-020)
- SBOM generation in SPDX 2.3 and CycloneDX 1.4 formats per release (PRD-020)
- GitHub build provenance attestation per release (PRD-020)
- Opt-in early-release channel: `JARVY_CHANNEL=beta` env var on install
  scripts; `[update] channel = "beta"` in `~/.jarvy/config.toml`;
  `jarvy update --channel beta`

### Quality & Security

- Clippy gate, mutation testing, fuzzing, coverage, benchmarks, OpenSSF
  Scorecard (PRD-018)
- Hybrid cross-platform E2E testing harness (PRD-038)
- Tag-signing enforcement (SSH or GPG) on release workflow
- Cosign keyless signing via GitHub OIDC for all release artifacts

### Infrastructure

- Semantic version checking with proper semver operators
- Cross-platform shell detection and hook execution
- Workspace lint configuration; Rust 2024 edition; MSRV 1.85

[Unreleased]: https://github.com/bearbinary/jarvy/compare/v0.1.0...HEAD
[v0.1.0]: https://github.com/bearbinary/jarvy/releases/tag/v0.1.0
[v0.0.5]: https://github.com/bearbinary/jarvy/releases/tag/v0.0.5
[v0.0.4]: https://github.com/bearbinary/jarvy/releases/tag/v0.0.4
[v0.0.3]: https://github.com/bearbinary/jarvy/releases/tag/v0.0.3
[v0.0.2]: https://github.com/bearbinary/jarvy/releases/tag/v0.0.2
[v0.0.1]: https://github.com/bearbinary/jarvy/releases/tag/v0.0.1
