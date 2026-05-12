---
title: "Telemetry forwarder operations — Jarvy"
description: "How to stand up and operate the public OTLP forwarder that receives opt-in telemetry from Jarvy CLIs and fans out to Grafana Cloud. Deployed on a self-hosted Kubernetes cluster with Traefik ingress. Anonymize-don't-drop PII policy. Threat model, hardening, incident playbook."
---

# Telemetry forwarder operations

The forwarder is the public-internet endpoint Jarvy CLIs send opt-in
telemetry to (`https://telemetry.jarvy.dev`). It accepts OTLP/HTTP from
anyone, **anonymizes** every PII-shaped field with a rotating salted
hash, rate-limits, and fans out to Grafana Cloud (Loki for logs, Mimir
for metrics, Tempo for traces). This document is the operational
source of truth: what it looks like, how to build it, how to operate
it, and how to recover when it breaks.

> **Telemetry is opt-in.** This doc is a *prerequisite* for opt-in
> actually being useful — without a working forwarder, the data has
> nowhere to go and `JARVY_OTLP_ENDPOINT` is just a config knob. The
> user-facing telemetry reference is at
> [Telemetry](../telemetry.md); the data-handling promise made there
> is the contract this doc must implement.

The forwarder is deployed as discrete Kubernetes Services in a
single namespace on a self-hosted cluster. Traefik handles ingress
and TLS. cert-manager provisions certificates. The
OpenTelemetry Collector runs as a Deployment with horizontal scaling.
Secrets are pulled from an external backend via External Secrets
Operator — no raw tokens or salts in git.

---

## Anonymize, don't drop

The data-handling policy is **anonymization, not deletion**.
Previously-PII fields (hostname, username, install path, IP, config
contents) are hashed with a rotating project-wide salt before they
leave the Collector. The resulting hash is high-entropy, unbounded
by rainbow tables, and bounded in linkability by the salt rotation
cadence (quarterly).

Why anonymize rather than drop:

- **Distinct-host / distinct-user counts** stay computable. Dropping
  `host.name` makes "how many distinct hosts hit setup failures this
  week" impossible to answer; hashing makes it trivial.
- **Co-occurrence analytics** stay computable. "Of the hosts that
  installed `kubectl` this month, what percentage also installed
  `helm`?" requires a stable identifier; an anonymized one is fine.
- **Incident correlation** stays possible. A single user reporting a
  bug can be correlated to *their* telemetry without exposing
  anyone else's data, because the user supplies their own
  pseudonymous identifier on request.
- **Schema evolution stays cheap**. If we later decide a field was
  too revealing even hashed, we can stop emitting it without
  invalidating historical aggregates that relied on its hash.

The cost of anonymization vs deletion:

- A **rainbow-table risk** if the salt is weak, leaks, or doesn't
  rotate. Mitigations: 32-byte random salt, sourced from External
  Secrets, rotated quarterly, never logged.
- A **long-term linkability risk** if the salt never rotates.
  Mitigations: quarterly rotation breaks long-term joins; analytics
  queries must operate within a single quarter for distinct counts.
- A **legal posture risk**: anonymization with reversibility (i.e.
  if we kept the salt forever and could recompute) is closer to
  pseudonymization than true anonymization under GDPR. Rotating the
  salt and discarding old salts moves the posture closer to
  irreversible anonymization, but a privacy lawyer should confirm
  before claiming "anonymous" in user-facing copy.

---

## Why a forwarder (not direct-to-Grafana)

The naive design points every Jarvy CLI directly at a Grafana Cloud
OTLP endpoint with a shared write token. We deliberately do not do
that. Reasons:

| Concern | Direct-to-Grafana | Forwarder in front |
|---|---|---|
| Shared write token leaks | Every Jarvy CLI binary ships the token; rotation requires a release | Token never leaves the forwarder; CLIs use no token at all |
| PII handling | Trust every CLI version forever, including older releases that may emit something we later regret | Single chokepoint where we can hash / drop fields independent of client version |
| Schema evolution | Old clients keep emitting old fields directly into Grafana | Old fields can be remapped or dropped at the forwarder before they hit billing |
| Salt management | Impossible — salt would live in every binary | Salt lives in one place, rotates without re-releasing the CLI |
| Rate limiting / abuse | Grafana ingest limits hit during a runaway client → real users lose data | Per-IP rate limit at the edge protects the upstream quota |
| Cost surprises | A bug that suddenly fires 1000× per setup goes straight to Grafana billing | Forwarder drops the spike, alerts us, never bills |
| Multi-backend | Locked to Grafana | Drop in Honeycomb / Datadog / self-hosted alongside or instead of Grafana with config change |

The forwarder is a thin OTel Collector with a hardened receiver
pipeline. Operationally we treat it like CDN edge: small, stateless,
replaceable.

---

## Architecture

```
   Jarvy CLI (opt-in users)
   └─ HTTPS POST /v1/{logs,metrics,traces}
       │
       ▼
   Cloudflare              ── DDoS / WAF / per-IP rate limit
       │
       ▼
   Traefik IngressRoute    ── TLS termination, method+path filter,
       │                       rate-limit middleware, body cap
       │
       ▼
   Service: otelcol        ── ClusterIP, port 4318
       │
       ▼
   Deployment: otelcol     ── OpenTelemetry Collector (contrib)
       │                       • otlphttp receiver
       │                       • transform/anonymize: salted SHA-256
       │                         of every PII-shaped attribute
       │                       • transform/redact_bodies: type-marker
       │                         substitution for inline PII patterns
       │                       • tail_sampling: 1% OK / 100% errors
       │                       • batch
       │
       ▼
   Grafana Cloud OTLP gateway  (bearer token, never seen by clients)
       ├─ Loki     (logs)
       ├─ Mimir    (metrics)
       └─ Tempo    (traces)
```

Each component is its own Kubernetes object — Deployment, Service,
ConfigMap, Secret, IngressRoute, Middleware — applied via GitOps
from a single namespace.

---

## Threat model

What we are defending against, in priority order:

1. **Cost denial-of-wallet.** A malicious actor (or a buggy Jarvy
   build) hammering the endpoint to burn Grafana Cloud free-tier
   quota or generate an unexpected invoice. Mitigations: per-IP rate
   limit (Cloudflare + Traefik middleware), global ingest rate cap,
   body size cap, alert on quota burn rate.
2. **Salt leak.** If the project-wide PII salt is exposed, every
   hash in the historical dataset becomes trivially reversible by an
   attacker who joins it to a known username/hostname distribution.
   Mitigations: salt lives in External Secrets, mounted as env from
   a Kubernetes Secret, never logged, rotated quarterly. After
   rotation, old data is no longer joinable to new — bounding the
   blast radius of a leak to one quarter.
3. **Accidental PII exfiltration through unhashed fields.** A future
   Jarvy code path emits an attribute we haven't seen yet (e.g. a
   new `jarvy.foo` key carrying a path). Mitigations: the
   anonymizer is an **allowlist of safe keys**, not a denylist of
   unsafe keys — anything not on the allowlist is hashed
   automatically. New schema items must pass review to land on the
   allowlist.
4. **Forwarder credential leak.** If the Grafana write token in the
   forwarder is compromised, the attacker can poison the dataset
   (not exfiltrate — read is a separate token). Mitigations: token
   scoped to write-only; rotate quarterly; sourced from External
   Secrets.
5. **Cluster compromise.** If the cluster itself is owned, the
   attacker becomes the trusted anonymizer. Mitigations: namespace
   isolation, NetworkPolicy egress allowlist, restricted Pod
   Security Standard, no inbound except via Traefik, no exec into
   the Collector pod outside of incident response.

Out of scope:

- **Stopping a determined operator-side leak.** If a Jarvy maintainer
  decides to harvest the data they have access to, that's a
  governance problem. Mitigation lives in the privacy policy + the
  audit trail of who has Grafana Cloud read access.

---

## Stack

| Layer | Component | Why |
|---|---|---|
| Edge | Cloudflare | DDoS protection, WAF, geographic / bot blocking, free tier |
| Cluster ingress | Traefik (CRDs: `IngressRoute`, `Middleware`) | Already present in the user's self-hosted cluster; first-class CRD API for rate limit + method filter + buffering as separate Middlewares; better UX than Ingress annotations |
| TLS | cert-manager + Let's Encrypt | Standard; reuses cluster's existing ClusterIssuer |
| Collector | `otel/opentelemetry-collector-contrib` Deployment | Stateless; horizontally scalable; contrib distro has `transform`, `tail_sampling`, `bearertokenauth` |
| Secret store | External Secrets Operator | Pulls Grafana write token + PII salt from existing secret backend (Vault / 1Password / AWS SM); rotation handled by the backend |
| Self-metrics | Prometheus Operator `ServiceMonitor` | Scrape Collector's `:8888/metrics`; alert independently of the Grafana Cloud exporter |
| Backend | Grafana Cloud OTLP gateway | Loki/Mimir/Tempo; free tier covers Jarvy's expected scale |

Each component below is a separate K8s object. The whole stack is
~10 manifests, ~400 lines of YAML, applied as a single Kustomization
or Helm release from your existing GitOps pipeline.

---

## Prerequisites

The cluster must already have these working:

- **Traefik** installed as the ingress controller. The examples
  assume Traefik v2 or v3 with CRDs enabled (`IngressRoute`,
  `Middleware`). If you only have the stock `Ingress` resource
  available, install the Traefik CRDs first or fall back to
  `Ingress` with annotations.
- **cert-manager** with a `ClusterIssuer` configured for
  Let's Encrypt. The examples assume an issuer named
  `letsencrypt-prod`.
- **External Secrets Operator** with a `ClusterSecretStore` named
  `vault-default` (rename in the manifests if yours differs).
  Sealed Secrets is an acceptable substitute; what matters is that
  the raw Grafana token and the PII salt never land in git.
- **Prometheus Operator** (kube-prometheus-stack) is recommended for
  self-monitoring but not strictly required.

---

## Provisioning

### Install via Helm (recommended)

The chart at `dist/helm/jarvy-telemetry-forwarder/` packages every
manifest below — Collector Deployment + Service, anonymize pipeline
ConfigMap, ExternalSecrets, Gateway API HTTPRoute (+ optional
Gateway), Traefik Middleware bridge for rate limit / body cap,
cert-manager Certificate, NetworkPolicy, optional HPA, optional
ServiceMonitor, PodDisruptionBudget. Released independently from the
Jarvy CLI as a signed OCI artifact:

```bash
helm install jarvy-telemetry \
  oci://ghcr.io/bearbinary/charts/jarvy-telemetry-forwarder \
  --version 0.1.0 \
  --namespace jarvy-telemetry --create-namespace
```

`values.yaml` is the canonical customization surface. Every
ingress concern (Gateway annotations / labels / GatewayClass,
HTTPRoute filters / parentRefs / hostnames, Traefik Middleware
toggles) is overridable. Common patterns:

- **Different ingress controller** (Envoy Gateway, Cilium, Istio,
  Contour): set `gatewayApi.gateway.gatewayClassName` and disable
  `gatewayApi.traefikMiddlewares.enabled`; supply equivalent rate
  limit / body cap via `gatewayApi.httpRoute.extraFilters` as
  ExtensionRef entries pointing at your implementation's CRDs.
- **Existing shared Gateway**: set `gatewayApi.gateway.create:
  false` and fill `gatewayApi.httpRoute.parentRefs` to attach to it.
- **Different secret backend**: change
  `secrets.externalSecrets.secretStoreRef` to your ESO store; the
  two `remoteRef.key` paths are independent.

Chart release pipeline:
[`.github/workflows/helm-release.yml`](../../.github/workflows/helm-release.yml)
fires on `helm-vX.Y.Z` tags, lints + packages + signs (cosign
keyless OIDC) + attests SBOM + pushes to GHCR. Decoupled from the
CLI release so chart-only fixes don't require a CLI release.

The sections below describe the manifests the chart renders, for
readers who want to apply them by hand or fork the chart. They are
**not** an installation path — keep the chart in sync if you go
that route, and contribute back any divergence you find useful.

### 1. DNS + Cloudflare

- Create `telemetry.jarvy.dev` as an A or CNAME record pointing at
  the cluster's Traefik LoadBalancer (or the public IP of your
  ingress entry point). Enable Cloudflare proxy (orange cloud).
- Cloudflare SSL/TLS → "Full (strict)". cert-manager will obtain a
  real Let's Encrypt cert for Traefik to serve, and Cloudflare will
  trust it.
- Cloudflare → Rules → WAF custom rule:
  - **If** `(http.request.method ne "POST")` **or**
    `(not (http.request.uri.path matches "^/v1/(logs|metrics|traces)$"))`
  - **Then** → Block
- Cloudflare → Rules → Rate Limiting:
  - 60 requests / 1 minute / IP, action: block 10 minutes.
- Cloudflare → Security → Bots → "Bot Fight Mode" on.

### 2. Namespace + ServiceAccount

```yaml
apiVersion: v1
kind: Namespace
metadata:
  name: jarvy-telemetry
  labels:
    # Enforce restricted Pod Security Standard. The Collector runs
    # fine with no special capabilities; lock it down.
    pod-security.kubernetes.io/enforce: restricted
    pod-security.kubernetes.io/audit: restricted
    pod-security.kubernetes.io/warn: restricted
---
apiVersion: v1
kind: ServiceAccount
metadata:
  name: otelcol
  namespace: jarvy-telemetry
```

The Collector needs no cluster permissions — no `k8sattributes`
processor, no pod auto-discovery. The ServiceAccount has no
`RoleBinding`. That is intentional.

### 3. Secrets: Grafana token + PII salt via External Secrets

```yaml
# Grafana Cloud OTLP write token. Scope: write-only in Grafana
# Cloud access policy. Rotate quarterly via the secret backend.
apiVersion: external-secrets.io/v1beta1
kind: ExternalSecret
metadata:
  name: grafana-otlp-token
  namespace: jarvy-telemetry
spec:
  refreshInterval: 1h
  secretStoreRef:
    name: vault-default
    kind: ClusterSecretStore
  target:
    name: grafana-otlp-token
    creationPolicy: Owner
  data:
    - secretKey: token
      remoteRef:
        key: jarvy/telemetry/grafana-otlp-write-token
---
# PII anonymization salt. 32 bytes of random, never logged.
# Rotate quarterly; rotation breaks long-term linkability of
# hashes from before vs after rotation. Generate in the backend
# with `openssl rand -hex 32` and never copy through a shell.
apiVersion: external-secrets.io/v1beta1
kind: ExternalSecret
metadata:
  name: pii-salt
  namespace: jarvy-telemetry
spec:
  refreshInterval: 1h
  secretStoreRef:
    name: vault-default
    kind: ClusterSecretStore
  target:
    name: pii-salt
    creationPolicy: Owner
  data:
    - secretKey: salt
      remoteRef:
        key: jarvy/telemetry/pii-salt
```

Adjust `secretStoreRef` and the two `remoteRef.key` paths to your
backend. The two values **must be different secrets** with
**different rotation cadences in your backend**: the Grafana token
rotates when access policy changes; the PII salt rotates on a fixed
quarterly schedule to bound linkability.

### 4. Collector configuration ConfigMap

The Collector configuration is where the anonymize-don't-drop policy
lives. Read this section even if you skip everything else — it is the
data-handling contract.

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: otelcol-config
  namespace: jarvy-telemetry
data:
  config.yaml: |
    receivers:
      otlp:
        protocols:
          http:
            endpoint: 0.0.0.0:4318
            # gRPC intentionally disabled — clients are OTLP/HTTP only.

    processors:
      # --------------------------------------------------------------
      # PII anonymization. Replaces each known PII-shaped attribute
      # with a salted SHA-256 of its value. Keeps analytical value
      # (distinct-count, co-occurrence) while preventing recovery of
      # the original string.
      #
      # Salt is read from env `PII_SALT`, sourced from the
      # `pii-salt` Secret. Rotation of that Secret rotates the salt
      # in this pipeline within `refreshInterval` of the
      # ExternalSecret (default 1h) plus a Collector restart.
      #
      # Allowlist model: only the keys explicitly listed below are
      # passed through unhashed. Anything else (including future
      # attributes we haven't seen yet) is hashed by the catch-all
      # statement at the end.
      # --------------------------------------------------------------
      transform/anonymize:
        error_mode: ignore
        log_statements:
          - context: resource
            statements:
              # Anonymize known PII keys explicitly.
              - set(attributes["host.name"], SHA256(Concat([attributes["host.name"], "${env:PII_SALT}"], ""))) where attributes["host.name"] != nil
              - set(attributes["host.id"],   SHA256(Concat([attributes["host.id"],   "${env:PII_SALT}"], ""))) where attributes["host.id"]   != nil
              - set(attributes["host.ip"],   SHA256(Concat([attributes["host.ip"],   "${env:PII_SALT}"], ""))) where attributes["host.ip"]   != nil
              - set(attributes["user.name"], SHA256(Concat([attributes["user.name"], "${env:PII_SALT}"], ""))) where attributes["user.name"] != nil
              - set(attributes["user.email"],SHA256(Concat([attributes["user.email"],"${env:PII_SALT}"], ""))) where attributes["user.email"]!= nil
              - set(attributes["jarvy.config.path"],    SHA256(Concat([attributes["jarvy.config.path"],    "${env:PII_SALT}"], ""))) where attributes["jarvy.config.path"]    != nil
              - set(attributes["jarvy.toml.contents"],  SHA256(Concat([attributes["jarvy.toml.contents"],  "${env:PII_SALT}"], ""))) where attributes["jarvy.toml.contents"]  != nil
              - set(attributes["jarvy.cwd"],            SHA256(Concat([attributes["jarvy.cwd"],            "${env:PII_SALT}"], ""))) where attributes["jarvy.cwd"]            != nil
              - set(attributes["jarvy.install_id"],     SHA256(Concat([attributes["jarvy.install_id"],     "${env:PII_SALT}"], ""))) where attributes["jarvy.install_id"]     != nil

      # --------------------------------------------------------------
      # Inline body redaction. The anonymization above operates on
      # structured attributes; this catches free-text PII shapes that
      # may slip into log message bodies. Replaces matches with bounded
      # type markers — we keep "an email occurred here" as a signal
      # without preserving the email's distinct identity (which would
      # require per-match hashing, awkward in OTTL replace_pattern).
      # --------------------------------------------------------------
      transform/redact_bodies:
        error_mode: ignore
        log_statements:
          - context: log
            statements:
              - replace_pattern(body, "[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\\.[A-Za-z]{2,}", "<email>")
              - replace_pattern(body, "\\b(?!127\\.0\\.0\\.1)\\d{1,3}\\.\\d{1,3}\\.\\d{1,3}\\.\\d{1,3}\\b", "<ip>")
              - replace_pattern(body, "/Users/[^/\\s]+", "/Users/<user>")
              - replace_pattern(body, "/home/[^/\\s]+", "/home/<user>")

      memory_limiter:
        check_interval: 1s
        limit_mib: 400
        spike_limit_mib: 100

      tail_sampling:
        decision_wait: 10s
        num_traces: 50000
        policies:
          - name: errors
            type: status_code
            status_code: { status_codes: [ERROR] }
          - name: probabilistic
            type: probabilistic
            probabilistic: { sampling_percentage: 1 }

      batch:
        timeout: 10s
        send_batch_size: 1024

    exporters:
      otlphttp/grafana:
        endpoint: ${env:GRAFANA_OTLP_ENDPOINT}
        auth:
          authenticator: bearertokenauth/grafana

    extensions:
      bearertokenauth/grafana:
        scheme: "Basic"
        token: ${env:GRAFANA_OTLP_TOKEN}
      health_check:
        endpoint: 0.0.0.0:13133

    service:
      extensions: [bearertokenauth/grafana, health_check]
      telemetry:
        metrics:
          address: 0.0.0.0:8888
      pipelines:
        logs:
          receivers: [otlp]
          processors: [memory_limiter, transform/anonymize, transform/redact_bodies, batch]
          exporters: [otlphttp/grafana]
        metrics:
          receivers: [otlp]
          processors: [memory_limiter, transform/anonymize, batch]
          exporters: [otlphttp/grafana]
        traces:
          receivers: [otlp]
          processors: [memory_limiter, transform/anonymize, tail_sampling, batch]
          exporters: [otlphttp/grafana]
```

When you add a new PII-shaped attribute to the Jarvy schema, add a
matching `set(...)` line here in the same pull request. The reviewer
should treat any schema PR that doesn't touch this file as
incomplete.

### 5. Deployment + Service

The Collector runs with two replicas minimum so rolling updates do
not drop in-flight batches. The container is read-only-rootfs, drops
all capabilities, runs as a non-root UID.

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: otelcol
  namespace: jarvy-telemetry
  labels:
    app.kubernetes.io/name: otelcol
spec:
  replicas: 2
  strategy:
    type: RollingUpdate
    rollingUpdate:
      maxUnavailable: 0
      maxSurge: 1
  selector:
    matchLabels:
      app.kubernetes.io/name: otelcol
  template:
    metadata:
      labels:
        app.kubernetes.io/name: otelcol
      annotations:
        # Trigger a rolling restart when the ConfigMap changes.
        # Automated by stakater/reloader if installed; otherwise
        # update this annotation manually with the SHA of the
        # ConfigMap on every apply.
        config-hash: "REPLACE_WITH_SHA256_OF_CONFIGMAP"
    spec:
      serviceAccountName: otelcol
      automountServiceAccountToken: false
      securityContext:
        runAsNonRoot: true
        runAsUser: 10001
        runAsGroup: 10001
        fsGroup: 10001
        seccompProfile:
          type: RuntimeDefault
      containers:
        - name: otelcol
          image: otel/opentelemetry-collector-contrib:0.107.0  # pin a digest in prod
          args: ["--config=/etc/otelcol/config.yaml"]
          ports:
            - { name: otlp-http, containerPort: 4318 }
            - { name: health, containerPort: 13133 }
            - { name: self-metrics, containerPort: 8888 }
          env:
            - name: GRAFANA_OTLP_ENDPOINT
              value: https://otlp-gateway-prod-us-east-0.grafana.net/otlp
            - name: GRAFANA_OTLP_TOKEN
              valueFrom:
                secretKeyRef:
                  name: grafana-otlp-token
                  key: token
            - name: PII_SALT
              valueFrom:
                secretKeyRef:
                  name: pii-salt
                  key: salt
          resources:
            requests:
              cpu: 100m
              memory: 256Mi
            limits:
              cpu: 1000m
              memory: 512Mi
          livenessProbe:
            httpGet: { path: /, port: health }
            initialDelaySeconds: 10
            periodSeconds: 30
          readinessProbe:
            httpGet: { path: /, port: health }
            initialDelaySeconds: 5
            periodSeconds: 10
          securityContext:
            allowPrivilegeEscalation: false
            capabilities:
              drop: ["ALL"]
            readOnlyRootFilesystem: true
          volumeMounts:
            - { name: config, mountPath: /etc/otelcol }
            - { name: tmp, mountPath: /tmp }
      volumes:
        - name: config
          configMap:
            name: otelcol-config
        - name: tmp
          emptyDir: {}
      topologySpreadConstraints:
        - maxSkew: 1
          topologyKey: kubernetes.io/hostname
          whenUnsatisfiable: ScheduleAnyway
          labelSelector:
            matchLabels:
              app.kubernetes.io/name: otelcol
---
apiVersion: v1
kind: Service
metadata:
  name: otelcol
  namespace: jarvy-telemetry
  labels:
    app.kubernetes.io/name: otelcol
spec:
  type: ClusterIP
  selector:
    app.kubernetes.io/name: otelcol
  ports:
    - { name: otlp-http, port: 4318, targetPort: otlp-http }
    - { name: self-metrics, port: 8888, targetPort: self-metrics }
```

### 6. Gateway API ingress (HTTPRoute + optional Gateway)

Routing is via Kubernetes Gateway API, not Traefik's native
`IngressRoute`. Gateway API is portable across implementations —
the same manifests work on Traefik, Envoy Gateway, Cilium, Istio,
Contour. Gateway API does not yet have first-class rate limit or
body cap, so when running on Traefik those concerns ride on
Traefik `Middleware` CRDs attached to the `HTTPRoute` via
`ExtensionRef` filters. Other GatewayClasses substitute their own
equivalents in the same filter slot (see the Helm chart's
`gatewayApi.httpRoute.extraFilters` for the documented extension
points per implementation).

```yaml
# Traefik Middleware: rate limit. 60 requests/min/IP. Cloudflare's
# rate limit is still primary defense; this is defense in depth at
# the cluster edge. On non-Traefik GatewayClasses, swap for the
# implementation's equivalent (Envoy Gateway `BackendTrafficPolicy`,
# Cilium `CiliumEnvoyConfig`, etc.) and attach via `extraFilters`
# in the Helm chart.
apiVersion: traefik.io/v1alpha1
kind: Middleware
metadata:
  name: otelcol-ratelimit
  namespace: jarvy-telemetry
spec:
  rateLimit:
    average: 60
    period: 1m
    burst: 30
    sourceCriterion:
      ipStrategy:
        # Trust Cloudflare's CF-Connecting-IP. depth=1 reads the
        # first IP in X-Forwarded-For; depth=0 (no proxy) uses the
        # immediate peer.
        depth: 1
---
# Traefik Middleware: body cap. Reject anything > 64 KiB. OTLP/HTTP
# payloads from a single Jarvy invocation are well under 10 KiB.
apiVersion: traefik.io/v1alpha1
kind: Middleware
metadata:
  name: otelcol-bodylimit
  namespace: jarvy-telemetry
spec:
  buffering:
    maxRequestBodyBytes: 65536
    memRequestBodyBytes: 65536
---
# cert-manager-issued certificate. Gateway listener references this
# Secret directly via `certificateRefs`.
apiVersion: cert-manager.io/v1
kind: Certificate
metadata:
  name: telemetry-jarvy-dev
  namespace: jarvy-telemetry
spec:
  secretName: telemetry-jarvy-dev-tls
  issuerRef:
    name: letsencrypt-prod
    kind: ClusterIssuer
  dnsNames:
    - telemetry.jarvy.dev
---
# Gateway. Create one per public ingress data plane; many clusters
# already have a shared Gateway in a networking namespace, in which
# case skip this resource and just attach the HTTPRoute below to
# it via parentRefs.
apiVersion: gateway.networking.k8s.io/v1
kind: Gateway
metadata:
  name: telemetry
  namespace: jarvy-telemetry
spec:
  # GatewayClass name varies per implementation:
  #   Traefik:        "traefik"
  #   Envoy Gateway:  "envoy"
  #   Cilium:         "cilium"
  #   Istio:          "istio"
  #   Contour:        "contour"
  gatewayClassName: traefik
  listeners:
    - name: https
      port: 443
      protocol: HTTPS
      tls:
        mode: Terminate
        certificateRefs:
          - kind: Secret
            name: telemetry-jarvy-dev-tls
      allowedRoutes:
        namespaces:
          from: Same
---
# HTTPRoute. The actual telemetry routing rule. Always required.
# Filters attach the Traefik Middlewares above via ExtensionRef —
# this is the standard Gateway API extension point.
apiVersion: gateway.networking.k8s.io/v1
kind: HTTPRoute
metadata:
  name: telemetry
  namespace: jarvy-telemetry
spec:
  parentRefs:
    - name: telemetry
      sectionName: https
  hostnames:
    - telemetry.jarvy.dev
  rules:
    - matches:
        - path: { type: PathPrefix, value: /v1/logs }
          method: POST
        - path: { type: PathPrefix, value: /v1/metrics }
          method: POST
        - path: { type: PathPrefix, value: /v1/traces }
          method: POST
      filters:
        - type: ExtensionRef
          extensionRef:
            group: traefik.io
            kind: Middleware
            name: otelcol-bodylimit
        - type: ExtensionRef
          extensionRef:
            group: traefik.io
            kind: Middleware
            name: otelcol-ratelimit
      backendRefs:
        - name: otelcol
          port: 4318
```

### 7. NetworkPolicy: lock the namespace down

The Collector pod should accept inbound only from Traefik (and
optionally Prometheus). Egress should be DNS + HTTPS only.

```yaml
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: otelcol
  namespace: jarvy-telemetry
spec:
  podSelector:
    matchLabels:
      app.kubernetes.io/name: otelcol
  policyTypes: [Ingress, Egress]
  ingress:
    - from:
        # Traefik namespace — adjust to where Traefik is installed.
        - namespaceSelector:
            matchLabels:
              kubernetes.io/metadata.name: traefik
      ports:
        - { protocol: TCP, port: 4318 }
    - from:
        - namespaceSelector:
            matchLabels:
              kubernetes.io/metadata.name: monitoring
      ports:
        - { protocol: TCP, port: 8888 }
  egress:
    - to:
        - namespaceSelector: {}
          podSelector:
            matchLabels:
              k8s-app: kube-dns
      ports:
        - { protocol: UDP, port: 53 }
    - to: []
      ports:
        - { protocol: TCP, port: 443 }
```

If your CNI supports DNS-based egress (Cilium, Calico Enterprise),
restrict the wide `to: []` for 443 to the Grafana Cloud OTLP
hostname.

### 8. HorizontalPodAutoscaler (optional)

```yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: otelcol
  namespace: jarvy-telemetry
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: otelcol
  minReplicas: 2
  maxReplicas: 6
  metrics:
    - type: Resource
      resource:
        name: cpu
        target:
          type: Utilization
          averageUtilization: 70
```

Requires metrics-server. If you don't have it, leave the Deployment
at `replicas: 2`.

### 9. ServiceMonitor for self-monitoring

```yaml
apiVersion: monitoring.coreos.com/v1
kind: ServiceMonitor
metadata:
  name: otelcol
  namespace: jarvy-telemetry
  labels:
    # Match whatever label your Prometheus instance selects on.
    release: kube-prometheus-stack
spec:
  selector:
    matchLabels:
      app.kubernetes.io/name: otelcol
  endpoints:
    - port: self-metrics
      interval: 30s
```

### 10. Verify end-to-end

```bash
# DNS + TLS + method filter
curl -I https://telemetry.jarvy.dev/v1/logs
# Expect: HTTP/2 404 — IngressRoute rejects non-POST at the router.

curl -X GET https://telemetry.jarvy.dev/v1/logs
# Expect: HTTP/2 404 — same reason.

# Real OTLP-shaped POST
curl -X POST -H 'Content-Type: application/json' \
  -d '{"resourceLogs":[]}' \
  https://telemetry.jarvy.dev/v1/logs
# Expect: HTTP/2 200 — Collector accepted the empty batch.

# Synthetic Jarvy event from a development laptop
JARVY_TELEMETRY=1 \
JARVY_OTLP_ENDPOINT=https://telemetry.jarvy.dev \
jarvy --version

# Grafana Cloud → Explore → Loki:
#   {service_name="jarvy"} |= "jarvy.startup"

# Spot check anonymization in the result:
#   resource.host.name should be a 64-char hex string, NOT a
#   human-readable hostname. Same for user.name, jarvy.cwd, etc.
```

If the synthetic event lands but Jarvy's events don't, walk:
`kubectl logs deploy/otelcol -n jarvy-telemetry`, then Traefik
access logs, then the ServiceMonitor-scraped Collector metrics.

---

## PII anonymization checklist

The anonymization pipeline is **allowlist-shaped**: every attribute
key in the schema either appears on the explicit allowlist (passed
through unhashed) or is hashed automatically. New schema items must
land on one of those two lists in the same PR that adds them.

**Passed through unhashed (the allowlist):**

- `service.name`, `service.version` — Jarvy version
- `os.type`, `os.version` — e.g. `darwin 14.5`
- Tool names from the registry — `node`, `docker`, etc. — these are
  public open-source identifiers
- Timing data — setup duration, install duration, hook duration
- Error category enumerations — `http_4xx`, `network_timeout`,
  `missing_prereq`, etc.
- Sampling / batching metadata — span kind, status code, etc.

**Hashed with salted SHA-256:**

- `host.name`, `host.id`, `host.ip`
- `user.name`, `user.email`
- `jarvy.config.path`, `jarvy.toml.contents`, `jarvy.cwd`
- `jarvy.install_id`
- `jarvy.parent_config_hash` (see correlation note below)

**Replaced inline (in log bodies) with type markers:**

- Email-shaped strings → `<email>`
- Public IPv4 strings → `<ip>`
- `/Users/<name>` / `/home/<name>` prefixes → `/Users/<user>` /
  `/home/<user>`

**Salt management:**

- 32 bytes of cryptographic randomness, generated with
  `openssl rand -hex 32` in the secret backend
- Rotated **quarterly** on a fixed schedule
- Rotation is performed entirely in the secret backend; the
  ExternalSecret's `refreshInterval` (1h) plus a Collector restart
  picks up the new value
- The previous quarter's salt is **discarded**, not archived —
  retaining it would defeat the linkability bound

**Correlation by parent config (`jarvy.parent_config_hash`)**

Jarvy supports config inheritance via `extends = "<url-or-path>"` —
projects can layer on top of an organization-wide or
team-maintained parent config. The CLI computes a SHA-256 of the
*resolved parent config* (after templating, before merging with the
project's own config) and emits it as the
`jarvy.parent_config_hash` resource attribute on every telemetry
batch.

The forwarder treats this attribute like any other PII-shaped key:
salted SHA-256 with the project-wide rotating salt. Because the
salt is deterministic across the cohort and the parent hash is
deterministic across users on the same parent, **two users on the
same org config produce the same final hash**. This enables
analytics queries like:

```logql
{service_name="jarvy"} | json
  | jarvy_parent_config_hash="<hash>"
  | line_format "{{.jarvy_event}}"
```

— answering "how many distinct hosts use org X's parent config" or
"what's the install-failure rate for tools required by org X" without
exposing what the config contains or who is on it. Distinct-count
analytics on the hash are bounded by the salt rotation window
(quarterly): cross-quarter joins are not possible.

If a project does not use `extends`, the attribute is absent and
the rule above is a no-op.

**On schema change:**

The schema doc at `docs/telemetry.md` (user-facing promise) and the
Collector ConfigMap above (enforcement) are the same contract. A PR
that touches one without the other is incomplete. Privacy audits
walk both in lockstep.

---

## Hardening checklist

Run through this every time the forwarder is provisioned or after
any significant config change.

- [ ] DNS resolves and Cloudflare proxy is enabled (orange cloud)
- [ ] Cloudflare WAF rule blocks non-`POST /v1/{logs,metrics,traces}`
- [ ] Cloudflare rate-limit rule active at 60/min/IP
- [ ] Traefik IngressRoute matches the same method + path triplet
      (defense in depth at the cluster edge)
- [ ] `otelcol-bodylimit` Middleware enforces `maxRequestBodyBytes`
      (verified by `curl -X POST -d "$(head -c 100000 /dev/urandom |
      base64)" https://telemetry.jarvy.dev/v1/logs` → 413 or 502)
- [ ] `otelcol-ratelimit` Middleware enforces 60/min/IP (verified
      with a quick loop; Cloudflare may intercept first — both
      layers should hold)
- [ ] cert-manager `Certificate` resource is in Ready state and
      Traefik serves a real Let's Encrypt cert
- [ ] Collector Deployment runs as non-root (UID 10001) with
      `readOnlyRootFilesystem: true`, `capabilities.drop: [ALL]`,
      `allowPrivilegeEscalation: false`, seccomp RuntimeDefault
- [ ] Collector pod has `automountServiceAccountToken: false`
- [ ] `grafana-otlp-token` Secret exists, mounted as env from
      ExternalSecret, not visible in `kubectl describe pod`
- [ ] `pii-salt` Secret exists, mounted as env from ExternalSecret,
      32-byte hex string, **different from any prior quarter's salt**
- [ ] NetworkPolicy applied; ingress restricted to Traefik
      namespace; egress restricted to DNS + 443
- [ ] Test event sent from a development laptop appears in Grafana
      Loki within 60 seconds
- [ ] **Anonymization spot check**: the event in Grafana shows
      `host.name`, `user.name`, `jarvy.cwd` etc. as 64-char hex
      hashes, **NOT** as human-readable strings
- [ ] Synthetic PII event with an email in a log body shows
      `<email>` in Grafana, not the raw email
- [ ] ServiceMonitor scraping the Collector's :8888 successfully
      (verified by querying Prometheus for
      `otelcol_receiver_accepted_log_records{namespace="jarvy-telemetry"}`)

---

## Cost and quota controls

- **Grafana Cloud free tier** at time of writing: 10k metrics
  series, 50 GB logs/month, 50 GB traces/month, 14-day retention. A
  Jarvy install emitting a normal volume fits well inside that for
  a five-figure MAU count.
- **Per-IP rate limit** at Cloudflare and at the Traefik
  Middleware prevents single-host abuse.
- **Collector `memory_limiter` processor** drops batches if the
  Collector RAM grows beyond 400 MiB — a runaway client can't OOM
  the pod.
- **Grafana Cloud usage alerts**: set "80% of free-tier ingest"
  warnings on logs, metrics, and traces. Alert routes to the
  maintainer's email; investigate before the meter hits 100%.

If the free tier runs out, the cheapest Grafana Cloud Pro plan
covers ~100× the current volume. The forwarder design is exporter-
agnostic, so swapping to a different backend (Honeycomb, Datadog,
self-hosted) is a config change, not a code change.

---

## Monitoring the forwarder itself

Two independent observability lanes:

- **Traefik access logs** — request rate, status codes, body sizes,
  per-route latency. Available via Traefik's standard log output.
- **Collector self-metrics** — exposed on `:8888/metrics`. Scraped
  by Prometheus Operator via the `ServiceMonitor` above. Lives in
  the cluster, not in Grafana Cloud — so it stays available even
  if the Grafana Cloud exporter is failing.

Key alerts:

- `otelcol_receiver_refused_log_records` > 0 (rate limiter is
  hitting valid traffic — investigate or raise the limit)
- `otelcol_exporter_send_failed_log_records` rate > 1/min (Grafana
  endpoint unhealthy or token invalid)
- `process_resident_memory_bytes{namespace="jarvy-telemetry"}` > 350
  MiB sustained (memory limiter is about to kick in; investigate
  for a runaway client)
- Traefik 4xx rate > 5% (schema may have drifted; clients are
  sending shapes the IngressRoute rejects)
- ExternalSecret reconciliation failures (Vault/backend is down or
  the path is wrong — the Collector keeps running on the cached
  Secret, but the next salt rotation won't take effect)

---

## Incident playbook

When something is wrong with telemetry, the worst case is a privacy
leak that landed in Grafana before the anonymizer caught it. Triage
in this order:

1. **Stop the bleed.** `kubectl scale deploy/otelcol -n
   jarvy-telemetry --replicas=0`. Traefik returns 502; clients fail
   open (telemetry is advisory, not load-bearing).
2. **Confirm scope.** Pull the last hour of Collector logs:
   `kubectl logs -n jarvy-telemetry -l app.kubernetes.io/name=otelcol
   --previous --since=1h`. Search Grafana Loki for whatever the
   suspected leak shape is. Note which Jarvy versions are
   represented in the affected records.
3. **Purge if needed.** Grafana Cloud → Loki / Mimir / Tempo admin
   APIs → delete by selector for the affected time window.
4. **Patch.** If the leak is a client-side regression in Jarvy, fix
   in main and cut a patch release. If the leak is a
   forwarder-side gap (a new attribute slipped through the
   allowlist), add a matching `set(...)` line in the `transform/
   anonymize` processor and re-apply the ConfigMap.
5. **Restart.** `kubectl scale deploy/otelcol -n jarvy-telemetry
   --replicas=2`. Verify with a manual test.
6. **Rotate the salt.** If the leak revealed plaintext values that
   were *supposed* to be hashed but weren't (i.e. the breach is
   that hashing didn't apply), rotate the salt anyway —
   conservative blast-radius minimization.
7. **Post-mortem.** File a `release-postmortem`-tagged issue: what
   leaked, how it bypassed the layers, what new test or rule
   prevents recurrence. Update this document.

---

## Operational handoff checklist

If you hand the forwarder to another maintainer, transfer:

- Cloudflare account or zone access
- Grafana Cloud organization admin invite (and rotate the write
  token at handoff — do not transfer the old one)
- Cluster credentials scoped to at least the `jarvy-telemetry`
  namespace
- Access to the secret backend feeding the two ExternalSecrets,
  including the rotation schedule for the PII salt
- Access to the GitOps pipeline (Argo CD / Flux / Helmfile) that
  applies the manifests — if the manifests aren't in git, fix that
  before handoff
- This document with any local deviations noted inline

The forwarder is intentionally small enough that a one-week handoff
is realistic. If it grows beyond that, the design needs a re-look —
the goal is a thing that survives the maintainer being out for a
month, not a thing that requires constant attention.

---

## See also

- [Telemetry](../telemetry.md) — user-facing schema, opt-in command,
  environment variables, and the data-handling promise this
  document implements.
- [`docs/release-quirks-jarvy.md`](../release-quirks-jarvy.md) —
  release-pipeline quirks; do not auto-deploy forwarder changes
  from release tags.
- OpenTelemetry Collector documentation:
  <https://opentelemetry.io/docs/collector/>
- Traefik IngressRoute reference:
  <https://doc.traefik.io/traefik/routing/providers/kubernetes-crd/>
- Grafana Cloud OTLP gateway docs:
  <https://grafana.com/docs/grafana-cloud/send-data/otlp/>
- OTTL (OpenTelemetry Transformation Language) reference:
  <https://github.com/open-telemetry/opentelemetry-collector-contrib/blob/main/pkg/ottl/README.md>
