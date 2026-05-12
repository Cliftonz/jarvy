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
helm install jarvy-telemetry \
  oci://ghcr.io/bearbinary/charts/jarvy-telemetry-forwarder \
  --version 0.1.0 \
  --namespace jarvy-telemetry --create-namespace
```

For a development install with inline secrets (dev clusters only):

```bash
helm install jarvy-telemetry \
  oci://ghcr.io/bearbinary/charts/jarvy-telemetry-forwarder \
  --version 0.1.0 \
  --namespace jarvy-telemetry --create-namespace \
  --set secrets.strategy=inline \
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
| `NetworkPolicy` | Locks ingress to your ingress controller's namespace, egress to DNS + 443 |
| `HorizontalPodAutoscaler` (optional) | CPU 70%, 2–6 replicas |
| `ServiceMonitor` (optional) | Prometheus Operator scrape of the Collector's self-metrics |
| `PodDisruptionBudget` | minAvailable: 1 during voluntary disruptions |

## Customizing for different GatewayClasses

Every `Gateway` and `HTTPRoute` annotation / label / filter is
exposed in `values.yaml`. Common patterns:

### Traefik (default)

```yaml
gatewayApi:
  gateway:
    gatewayClassName: traefik
  traefikMiddlewares:
    enabled: true   # body cap + rate limit via Middleware CRDs
```

### Envoy Gateway

```yaml
gatewayApi:
  gateway:
    gatewayClassName: envoy
    annotations:
      gateway.envoyproxy.io/inline-policy: "..."
  traefikMiddlewares:
    enabled: false
  httpRoute:
    extraFilters:
      - type: ExtensionRef
        extensionRef:
          group: gateway.envoyproxy.io
          kind: BackendTrafficPolicy
          name: my-rate-limit
      - type: ExtensionRef
        extensionRef:
          group: gateway.envoyproxy.io
          kind: ClientTrafficPolicy
          name: my-body-limit
```

### Cilium

```yaml
gatewayApi:
  gateway:
    gatewayClassName: cilium
    annotations:
      io.cilium/lb-mode: snat
  traefikMiddlewares:
    enabled: false
  httpRoute:
    extraFilters:
      - type: ExtensionRef
        extensionRef:
          group: cilium.io
          kind: CiliumEnvoyConfig
          name: my-policy
```

### Existing shared Gateway

If your cluster already runs a shared Gateway (typical multi-tenant
pattern), don't create another one — just attach the HTTPRoute:

```yaml
gatewayApi:
  gateway:
    create: false
  httpRoute:
    parentRefs:
      - name: shared-public-gateway
        namespace: networking
        sectionName: https
```

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
restart (use stakater/reloader or set `collector.podAnnotations.salt-version`
to force a roll).

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
