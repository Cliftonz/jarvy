# jarvy-telemetry-forwarder

Helm chart for the public OTLP forwarder that receives opt-in
telemetry from Jarvy CLIs, anonymizes every PII-shaped attribute with
a salted SHA-256 (rotating salt), and fans out to Grafana Cloud (or
any OTLP backend). Built on Kubernetes Gateway API; bring your own
GatewayClass (Traefik, Envoy/Contour, Cilium, Istio).

Full operational documentation, threat model, PII policy, and
incident playbook live at
<https://jarvy.dev/operations/telemetry-forwarder/>.

## Install

The chart ships as an OCI artifact on the project's GitHub Container
Registry:

```bash
# Replace <version> with the current chart version. Latest release:
# https://github.com/bearbinary/Jarvy/releases?q=helm-v
helm install jarvy-telemetry \
  oci://ghcr.io/bearbinary/charts/jarvy-telemetry-forwarder \
  --version <version> \
  --namespace jarvy-telemetry --create-namespace
```

Verify the signature with the exact identity (not a regex — see
the release notes for the full command):

```bash
cosign verify \
  --certificate-identity "https://github.com/bearbinary/Jarvy/.github/workflows/helm-release.yml@refs/tags/helm-v<version>" \
  --certificate-oidc-issuer "https://token.actions.githubusercontent.com" \
  ghcr.io/bearbinary/charts/jarvy-telemetry-forwarder:<version>
```

For a development install with inline secrets (dev clusters only):

```bash
helm install jarvy-telemetry \
  oci://ghcr.io/bearbinary/charts/jarvy-telemetry-forwarder \
  --version <version> \
  --namespace jarvy-telemetry --create-namespace \
  --set secrets.strategy=inline \
  --set secrets.inline.acceptRisk=true \
  --set secrets.inline.grafanaToken="$(cat ~/grafana-token)" \
  --set secrets.inline.piiSalt="$(openssl rand -hex 32)"
```

## What gets installed

| Resource | Purpose |
|---|---|
| `Deployment` + `Service` | OpenTelemetry Collector (contrib distribution), 2 replicas, port 4318 |
| `ConfigMap` | Collector pipeline — anonymize-not-drop + body redact + tail sampling + batch |
| `ExternalSecret` × 2 | Pulls Grafana write token + PII salt from your secret backend |
| `Gateway` (optional) + `HTTPRoute` | Gateway API ingress; routes POST to `/v1/{logs,metrics,traces}` to the Collector |
| `Middleware` × 2 (Traefik only) | Rate limit + body cap, attached via `HTTPRoute.filters[].extensionRef` |
| `Certificate` | cert-manager issues TLS for the hostname |
| `NetworkPolicy` | Default-deny + ingress from your ingress controller's namespace; egress to DNS + 443. Default mode `wide-except-rfc1918` excludes private IP ranges; switch to `fqdn` (requires Cilium) for hostname-restricted egress |
| `CiliumNetworkPolicy` (optional) | When `networkPolicy.egressMode: fqdn` or legacy `cilium.enabled: true` — restricts egress to the parsed exporter hostname |
| `HorizontalPodAutoscaler` (optional) | Memory 70% + optional queue-depth custom metric; 2–6 replicas. CPU-based scaling is insufficient (memory_limiter trips before CPU climbs) |
| `ServiceMonitor` (optional) | Prometheus Operator scrape of the metrics-only Service |
| `PrometheusRule` (optional) | Recording rules + alerts (refused records, exporter failing/queue-saturated, memory pressure, salt staleness, tail-sampling rate, allowlist drops, pod restarts, reloader missing, debug exporter on, cert lifecycle) |
| `ConfigMap` (Grafana dashboard) | Auto-imported by the Grafana sidecar via the `grafana_dashboard=1` label |
| `PodDisruptionBudget` | `maxUnavailable: 1` during voluntary disruptions (lets node drains proceed) |

## Customizing the ingress

The chart's **supported and tested** ingress configuration is Traefik
(via the Traefik Middleware bridge). Every `Gateway` and `HTTPRoute`
field is also exposed in `values.yaml` for operators on other
GatewayClasses, but the chart's CI only renders + validates against
Traefik — non-Traefik users are responsible for verifying their own
filter equivalents work end-to-end.

### Traefik (default, supported)

```yaml
gatewayApi:
  gateway:
    gatewayClassName: traefik
  traefikMiddlewares:
    enabled: true   # body cap + rate limit via Middleware CRDs
```

### Existing shared Gateway

If your cluster already runs a shared Gateway (typical multi-tenant
pattern), attach the HTTPRoute instead of creating your own Gateway:

```yaml
gatewayApi:
  gateway:
    create: false
  httpRoute:
    parentRefs:
      - name: shared-public-gateway
        namespace: networking
        sectionName: https
    # Cross-namespace attach requires a ReferenceGrant on the target.
    # Setting allowCrossNamespaceParent: true acknowledges that.
    allowCrossNamespaceParent: true
```

### Other GatewayClasses (Envoy / Cilium / Istio / Contour)

Disable `traefikMiddlewares` and supply equivalent rate-limit + body-
cap filters via `gatewayApi.httpRoute.extraFilters`. The exact
ExtensionRef shape varies per implementation — consult your
controller's docs for the right `group`/`kind`. **Not exercised by
chart CI**; treat as community-contributed integration.

## PII policy

The Collector pipeline anonymizes; it does not drop. Every key in
`pii.hashedAttributes` is replaced with `SHA256(value || PII_SALT)`,
preserving distinct-count + co-occurrence analytics without exposing
the original string. Adding a new hashed attribute is a one-line
values change.

Allowlist-shaped: the schema doc at
<https://jarvy.dev/telemetry/> and the chart's `pii.hashedAttributes`
list are the same contract. A telemetry PR touching one without the
other is incomplete.

**Salt rotation**: rotate the value of the secret backed by
`secrets.externalSecrets.piiSalt.remoteRef` quarterly. The
ExternalSecret's `refreshInterval` (1h) propagates the new salt to
the Kubernetes Secret; the Collector picks it up on next pod
restart. The chart defaults to stakater/reloader (off-by-default
upstream — see `JarvyForwarderReloaderMissing` alert) for automatic
rolling. Without Reloader, add any annotation under
`collector.podAnnotations` (e.g. `--set
collector.podAnnotations.salt-version=2026-05-13`) to force a roll
on the next `helm upgrade`.

## Parent-config correlation

The chart's default `pii.hashedAttributes` list includes
`jarvy.parent_config_hash` — a client-side SHA-256 of the resolved
parent jarvy.toml when a project's config uses `extends = "..."` or
inherits from an organization-wide config. Because the salted re-hash
in the forwarder is deterministic, users of the same parent config
produce the same final hash, enabling queries like:

```logql
{service_name="jarvy"} |~ "jarvy.parent_config_hash=\"<hash>\"" | json
```

…to answer "how many distinct hosts use org X's config" without
revealing what's *in* that config.

## Release

The chart releases independently from the Jarvy CLI. Tags shaped
`helm-vX.Y.Z` trigger
[`.github/workflows/helm-release.yml`](../../../.github/workflows/helm-release.yml),
which lints, packages, signs with cosign, and pushes the OCI artifact
to `ghcr.io/bearbinary/charts/jarvy-telemetry-forwarder`.

The `version` field in `Chart.yaml` must equal the tag minus the
`helm-v` prefix; the workflow enforces this. The `appVersion` field
tracks the Jarvy CLI version the chart was tested against — keep
manually in sync.

## Values reference

See [`values.yaml`](values.yaml) for the full, commented value tree.
Every default is overridable; the shape of that file is the chart's
public API.

## Validating values from other systems

The chart ships a JSON Schema describing its values:

- In-tree: [`values.schema.json`](values.schema.json)
- Published: <https://jarvy.dev/schema/jarvy-telemetry-forwarder.values.schema.json>

Helm validates against this schema automatically on every
`helm install`, `helm upgrade`, and `helm template` — invalid
values fail early with a clear "does not match pattern" / "is not
one of expected values" error.

External systems that need to validate inputs **before** they reach
Helm (ArgoCD ApplicationSets, OPA Gatekeeper policies, in-house CI,
admission webhooks, GitOps PR-time checks) can import the schema
directly:

```bash
# CLI validation with ajv
curl -fsSL https://jarvy.dev/schema/jarvy-telemetry-forwarder.values.schema.json \
  -o telemetry-forwarder.schema.json
yq -o=json . my-overrides.yaml > my-overrides.json
ajv validate -s telemetry-forwarder.schema.json -d my-overrides.json \
  --spec=draft7 -c ajv-formats
```

```yaml
# ArgoCD ApplicationSet — fail the generator if a tenant supplies
# an invalid override before the rendered Application is committed.
spec:
  template:
    spec:
      sources:
        - chart: jarvy-telemetry-forwarder
          repoURL: oci://ghcr.io/bearbinary/charts
          helm:
            valuesObject: { /* tenant overrides */ }
            # ApplicationSet generator validates this against the
            # published schema before commit. See your ArgoCD docs
            # for the generator's schema-validation hook syntax.
```

```rego
# OPA Gatekeeper — refuse Helm releases whose values do not satisfy
# the chart's schema. Pull the schema in the policy bundle build.
package helm.jarvy_telemetry_forwarder
import future.keywords
deny[msg] {
  input.chart == "jarvy-telemetry-forwarder"
  errs := json.verify_schema(input.values, data.schemas.jarvy_telemetry_forwarder)
  count(errs) > 0
  msg := sprintf("values violate schema: %v", [errs])
}
```

Schema invariants enforced beyond plain JSON-Schema (checked at
chart CI time, see `.github/workflows/helm-chart-ci.yml`):

- `pii.passThroughAttributes` and `pii.hashedAttributes` are disjoint
- `collector.image.digest` must be `sha256:` + 64 hex chars or empty
- `exporter.endpoint` matches `^(|https://[a-z0-9.-]+(?::[0-9]+)?(?:/[^\s]*)?)$`
  — empty (chart composes from `grafanaCloud.region`) OR **https only**
  (URL must start with `^https://`; http rejected), no userinfo
  (`user@host` rejected), no uppercase host (rejected). The strictness
  is deliberate: the CiliumNetworkPolicy FQDN parser would silently
  strip userinfo, and uppercase hostnames defeat lowercase-only FQDN
  matching.
- `grafanaCloud.region` matches `^[a-z0-9]+(?:-[a-z0-9]+)+$` (e.g.
  `prod-us-east-3`). API keys in `grafana-otlp-token` are bound to one
  stack which lives in one region — wrong-region presentation returns
  HTTP 401 and silently drops every export. Default `prod-us-east-3`.
- `collector.containerSecurityContext` rejects flipping
  `privileged`, `allowPrivilegeEscalation`, `readOnlyRootFilesystem`,
  or dropping `capabilities.drop: ALL` — the chart's Restricted-PSS
  posture is enforced at schema time, not just rendered.
- `collector.logLevel ∈ {debug,info,warn,error}`,
  `collector.logFormat ∈ {json,text,console}`
- `secrets.strategy ∈ {externalSecrets,existing,inline}`
- `networkPolicy.egressMode ∈ {wide, wide-except-rfc1918, fqdn}`
- `tls.certManager.enabled=true` and `tls.existingSecretName` are
  mutually exclusive (template-time `fail()`).
- `pdb.minAvailable` and `pdb.maxUnavailable` are mutually exclusive.
- On non-Traefik GatewayClasses, install fails when neither
  `httpRoute.extraFilters` nor `dosProtection.acceptUnprotected:true`
  is set — public OTLP receiver requires explicit DoS protection.
- All required sections present; no unknown top-level keys
  (`additionalProperties: false` at every level)

When you change `values.yaml`, update `values.schema.json` in the
same PR. Chart CI fails on drift between them.
