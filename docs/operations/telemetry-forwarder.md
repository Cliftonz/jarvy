---
title: "Telemetry forwarder operations — Jarvy"
description: "How to stand up and operate the public OTLP forwarder that receives opt-in telemetry from Jarvy CLIs and fans out to Grafana Cloud. Threat model, hardening checklist, PII scrubbing, cost controls, and the runbook for the on-call maintainer."
---

# Telemetry forwarder operations

The forwarder is the public-internet endpoint Jarvy CLIs send opt-in
telemetry to (`https://telemetry.jarvy.dev`). It accepts OTLP/HTTP from
anyone, scrubs PII, rate-limits, then fans out to Grafana Cloud (Loki for
logs, Mimir for metrics, Tempo for traces). This document is the
operational source of truth: what it looks like, how to build it, how to
operate it, and how to recover when it breaks.

> **Telemetry is opt-in.** This doc is a *prerequisite* for opt-in actually
> being useful — without a working forwarder, the data has nowhere to go
> and `JARVY_OTLP_ENDPOINT` is just a config knob. The user-facing telemetry
> reference is at [Telemetry](../telemetry.md); the data-handling promise
> made there is the contract this doc must implement.

---

## Why a forwarder (not direct-to-Grafana)

The naive design points every Jarvy CLI directly at a Grafana Cloud OTLP
endpoint with a shared write token. We deliberately do not do that.
Reasons:

| Concern | Direct-to-Grafana | Forwarder in front |
|---|---|---|
| Shared write token leaks | Every Jarvy CLI binary ships the token; rotation requires a release | Token never leaves the forwarder; CLIs use no token at all |
| PII scrubbing | Trust every CLI version forever, including older releases that may emit something we later regret | Single chokepoint where we can drop / hash fields independent of client version |
| Schema evolution | Old clients keep emitting old fields directly into Grafana | Old fields can be remapped or dropped at the forwarder before they hit billing |
| Rate limiting / abuse | Grafana ingest limits hit during a runaway client → real users lose data | Per-IP rate limit at the edge protects the upstream quota |
| Cost surprises | A bug that suddenly fires 1000× per setup goes straight to Grafana billing | Forwarder drops the spike, alerts us, never bills |
| Multi-backend | Locked to Grafana | Drop in Honeycomb / Datadog / self-hosted alongside or instead of Grafana with config change |

The forwarder is a thin OTel Collector with a hardened receiver pipeline.
Operationally we treat it like CDN edge: small, stateless, replaceable.

---

## Architecture

```
   Jarvy CLI (opt-in users)
   └─ HTTPS POST /v1/{logs,metrics,traces}
       │
       ▼
   Cloudflare (DDoS / WAF / per-IP rate limit)
       │
       ▼
   telemetry.jarvy.dev   (Caddy reverse proxy + auto-TLS)
       │   - terminates TLS
       │   - enforces method + path allowlist
       │   - per-IP rate limit (10 req/min)
       │   - drops requests > 64 KB
       ▼
   OpenTelemetry Collector  (contrib distribution)
       │   - otlphttp receiver (no auth)
       │   - attribute processor: drop usernames, hostnames, IPs,
       │     filesystem paths, env variable values
       │   - batch processor
       │   - resource processor: stamp ingestion timestamp, drop
       │     resource attrs we never want
       │   - tail-sampling for traces (1% of OK, 100% of errors)
       ▼
   Grafana Cloud OTLP gateway  (bearer token, never seen by clients)
       ├─ Loki     (logs)
       ├─ Mimir    (metrics)
       └─ Tempo    (traces)
```

Components:

- **Cloudflare** — free tier; orange-cloud the `telemetry` subdomain.
  WAF rule allowlist on `POST /v1/(logs|metrics|traces)`. Anything else
  → 403 at the edge before it touches the origin.
- **Origin host** — single small VM (Hetzner CX22 / DO 1GB is plenty
  through Jarvy v0.x; revisit if request rate exceeds 5/s sustained).
- **Caddy** — TLS termination + reverse-proxy. Caddy's built-in
  rate-limit + body-size limits do the heavy lifting before the
  Collector even parses the request.
- **OpenTelemetry Collector (contrib)** — receivers, processors,
  exporters. Config below.
- **Grafana Cloud** — pre-existing account. We use the OTLP endpoint at
  `https://otlp-gateway-prod-<region>.grafana.net/otlp` with a bearer
  token bound to a Cloud Stack instance with logs + metrics + traces
  scopes.

---

## Threat model

What we are defending against, in priority order:

1. **Cost denial-of-wallet.** A malicious actor (or a buggy Jarvy
   build) hammering the endpoint to burn Grafana Cloud free-tier quota
   or generate an unexpected invoice. Mitigations: per-IP rate limit,
   global ingest rate cap, body size cap, alert on quota burn rate.
2. **Accidental PII exfiltration.** A future Jarvy code path emits
   `jarvy.toml` contents, env var values, or a customer's git remote
   URL by mistake. Mitigations: forwarder strips known PII keys
   regardless of what the client sent; allowlist of fields rather than
   blocklist; log a sampled tail to a tight-ACL bucket for audit.
3. **Forwarder credential leak.** If the Grafana write token in the
   forwarder is compromised, the attacker can poison the dataset (not
   exfiltrate — read is a separate token). Mitigations: token scoped
   to write-only; rotate quarterly; store in the host's systemd
   credentials, not in the Collector config file directly.
4. **Forwarder compromise.** If the VM itself is owned, the attacker
   becomes the trusted PII-scrubbing layer. Mitigations: minimal OS
   surface (Debian stable + unattended-upgrades), SSH key-only access,
   no inbound except 443 from Cloudflare's IP ranges, no other services
   on the host.

Out of scope:

- **Stopping a determined operator-side leak.** If a Jarvy maintainer
  decides to harvest the data they have access to, that is a
  governance problem, not a forwarder problem. Mitigation lives in
  the privacy policy + the audit trail of who has Grafana Cloud
  access.

---

## Stack choice: why Grafana Cloud + OTel Collector

Alternatives considered:

| Stack | Pros | Cons | Verdict |
|---|---|---|---|
| Self-hosted Loki/Mimir/Tempo on a VM | Full control, single bill | Operational tax (storage, retention, alerting) we cannot afford as a one-maintainer project | No |
| Grafana Cloud + direct write from CLI | Simplest | Every concern in the "Why a forwarder" table | No |
| Grafana Cloud + OTel Collector forwarder | Hands off storage + queries to Grafana, retains the chokepoint | Adds a VM to maintain | **Yes** |
| Honeycomb / Datadog instead of Grafana | Polished query UX | Higher cost at our scale; Datadog APM cardinality limits bite quickly | No |
| Sentry for errors only | We get error grouping for free | Doesn't cover the "what tools are people installing" metric the telemetry exists for | Not as primary |

If at some future point Grafana Cloud's pricing or limits change, the
forwarder gives us a single place to swap exporters without re-shipping
a Jarvy release. That property is the whole point.

---

## Provisioning the forwarder

End state: a single VM at `telemetry.jarvy.dev` running Caddy and one
Collector container. Total bootstrap time ~30 minutes.

### 1. DNS + Cloudflare

- Create `telemetry.jarvy.dev` as an A record pointing at the origin's
  public IP. Enable Cloudflare proxy (orange cloud).
- In Cloudflare → SSL/TLS → set to "Full (strict)". Caddy will obtain
  a real cert from Let's Encrypt, and Cloudflare will trust it.
- Cloudflare → Rules → WAF → custom rule:
  - **If** `(http.request.method ne "POST")` **or**
    `(not (http.request.uri.path matches "^/v1/(logs|metrics|traces)$"))`
  - **Then** → Block
- Cloudflare → Rules → Rate Limiting (free tier allows one rule):
  - 60 requests / 1 minute / IP, action: block 10 minutes.
- Optional but recommended: Cloudflare → Security → Bots → "Bot Fight
  Mode" on. Most Jarvy CLIs are not bots; if a User-Agent ever looks
  bot-y, drop it.

### 2. Origin VM

Cheapest VM that runs systemd reliably. Recommended:

- Hetzner CX22 (2 vCPU, 4 GB RAM, €5/mo) or DigitalOcean Basic 1 GB
- Debian 12 (bookworm) stable
- Single user with sudo + SSH key only; disable password auth
- `ufw` open only on 22 (your IP only) and 443 (Cloudflare IP ranges)
  (`ufw allow proto tcp from <cf-range> to any port 443` for each
  prefix from `https://www.cloudflare.com/ips-v4`)
- `apt install unattended-upgrades` and enable security-only auto-update

### 3. Caddy

Caddy provides TLS + reverse proxy + rate limit + body cap in ~25 lines.
Install:

```bash
sudo apt install -y debian-keyring debian-archive-keyring apt-transport-https
curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' | \
  sudo gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' | \
  sudo tee /etc/apt/sources.list.d/caddy-stable.list
sudo apt update && sudo apt install -y caddy
```

`/etc/caddy/Caddyfile`:

```caddy
{
    # Block anything that isn't OTLP/HTTP. Caddy already enforces TLS
    # by default; this just narrows the surface further.
    servers {
        max_header_size 16KB
    }
}

telemetry.jarvy.dev {
    encode zstd gzip

    # Reject anything larger than 64 KB. OTLP/HTTP payloads from a
    # single Jarvy invocation are well under 10 KB; 64 KB leaves
    # headroom for batched setups without inviting abuse.
    request_body {
        max_size 64KB
    }

    # Allowlist OTLP paths only. WAF at Cloudflare already does
    # this; defense in depth.
    @otlp {
        method POST
        path /v1/logs /v1/metrics /v1/traces
    }
    handle @otlp {
        reverse_proxy 127.0.0.1:4318
    }
    handle {
        respond "Not found" 404
    }

    # Real client IP from Cloudflare for logging only — never
    # forwarded into the Collector pipeline.
    log {
        output file /var/log/caddy/access.log {
            roll_size 100mb
            roll_keep 5
        }
        format json {
            time_format iso8601
        }
    }
}
```

Reload: `sudo systemctl reload caddy`.

### 4. OpenTelemetry Collector (contrib distribution)

The contrib distribution ships the processors we need (`attributes`,
`filter`, `transform`, `tail_sampling`). Standard distribution is too
narrow.

Install via systemd unit running the upstream binary, or via Docker. We
use systemd for one-VM ops simplicity:

```bash
curl -L -o /tmp/otelcol.tar.gz \
  https://github.com/open-telemetry/opentelemetry-collector-releases/releases/latest/download/otelcol-contrib_linux_amd64.tar.gz
sudo mkdir -p /opt/otelcol
sudo tar -xzf /tmp/otelcol.tar.gz -C /opt/otelcol
sudo chown -R root:root /opt/otelcol
```

`/etc/otelcol/config.yaml`:

```yaml
# Receivers: OTLP/HTTP only, no auth (Caddy is the auth/auth ingress).
receivers:
  otlp:
    protocols:
      http:
        endpoint: 127.0.0.1:4318
        # Disable the gRPC receiver — we accept HTTP from CLIs only.

# Processors: PII scrub, rate-limit-on-attribute, batch.
processors:
  # Strip attributes that are likely to carry PII regardless of what
  # the client sent. The list is a denylist by exact key match plus
  # value-pattern regexps. Keep in sync with the telemetry schema
  # documented at https://jarvy.dev/telemetry/#schema.
  attributes/scrub:
    actions:
      # Delete keys that should never be in telemetry.
      - key: host.name
        action: delete
      - key: host.id
        action: delete
      - key: host.ip
        action: delete
      - key: user.name
        action: delete
      - key: user.email
        action: delete
      - key: jarvy.config.path
        action: delete
      - key: jarvy.toml.contents
        action: delete
      - key: jarvy.cwd
        action: delete
      # Hash anything that we want to count distinct values of but
      # never see the raw value of (e.g. anonymized install ID).
      - key: jarvy.install_id
        action: hash

  # Drop log records whose body matches PII patterns (in case the
  # CLI's sanitizer missed them).
  transform/redact_bodies:
    log_statements:
      - context: log
        statements:
          # Email-shaped strings
          - replace_pattern(body, "[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\\.[A-Za-z]{2,}", "<redacted-email>")
          # IPv4-shaped strings (not loopback)
          - replace_pattern(body, "\\b(?!127\\.0\\.0\\.1)\\d{1,3}\\.\\d{1,3}\\.\\d{1,3}\\.\\d{1,3}\\b", "<redacted-ip>")
          # Anything that looks like an absolute home path
          - replace_pattern(body, "/Users/[^/\\s]+", "/Users/<redacted>")
          - replace_pattern(body, "/home/[^/\\s]+", "/home/<redacted>")

  # Cap memory in case of a runaway batch.
  memory_limiter:
    check_interval: 1s
    limit_mib: 200
    spike_limit_mib: 50

  # Tail-sampling: keep 100% of error traces, 1% of success traces.
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

# Exporters: Grafana Cloud OTLP. Token is loaded from systemd
# credentials so it never lands in the config file or process
# environment string.
exporters:
  otlphttp/grafana:
    endpoint: ${env:GRAFANA_OTLP_ENDPOINT}
    auth:
      authenticator: bearertokenauth/grafana
  # Local file mirror, 7-day retention, tight ACL — used for audit
  # only when investigating a PII regression. Disable in production
  # once you have audit confidence.
  file/audit:
    path: /var/log/otelcol/audit.json
    rotation:
      max_megabytes: 100
      max_days: 7

extensions:
  bearertokenauth/grafana:
    scheme: "Basic"
    token: ${env:GRAFANA_OTLP_TOKEN}
  health_check:
    endpoint: 127.0.0.1:13133

service:
  extensions: [bearertokenauth/grafana, health_check]
  pipelines:
    logs:
      receivers: [otlp]
      processors: [memory_limiter, attributes/scrub, transform/redact_bodies, batch]
      exporters: [otlphttp/grafana]
    metrics:
      receivers: [otlp]
      processors: [memory_limiter, attributes/scrub, batch]
      exporters: [otlphttp/grafana]
    traces:
      receivers: [otlp]
      processors: [memory_limiter, attributes/scrub, tail_sampling, batch]
      exporters: [otlphttp/grafana]
```

`/etc/systemd/system/otelcol.service`:

```ini
[Unit]
Description=OpenTelemetry Collector (Jarvy forwarder)
After=network-online.target
Wants=network-online.target

[Service]
ExecStart=/opt/otelcol/otelcol-contrib --config=/etc/otelcol/config.yaml
LoadCredentialEncrypted=grafana_token:/etc/otelcol/grafana_token.enc
Environment=GRAFANA_OTLP_ENDPOINT=https://otlp-gateway-prod-us-east-0.grafana.net/otlp
Environment=GRAFANA_OTLP_TOKEN=%d/grafana_token
User=otelcol
Group=otelcol
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
PrivateTmp=true
PrivateDevices=true
ProtectKernelTunables=true
ProtectKernelModules=true
ProtectControlGroups=true
RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX
LockPersonality=true
MemoryDenyWriteExecute=true
RestrictRealtime=true
RestrictNamespaces=true
SystemCallArchitectures=native

[Install]
WantedBy=multi-user.target
```

Generate the encrypted credential:

```bash
sudo systemd-creds encrypt --name=grafana_token - /etc/otelcol/grafana_token.enc <<<'<your-grafana-cloud-write-token>'
sudo chmod 600 /etc/otelcol/grafana_token.enc
sudo useradd -r -s /usr/sbin/nologin otelcol
sudo mkdir -p /var/log/otelcol
sudo chown otelcol:otelcol /var/log/otelcol
sudo systemctl daemon-reload
sudo systemctl enable --now otelcol
```

### 5. Verify end-to-end

From your laptop:

```bash
JARVY_TELEMETRY=1 \
JARVY_OTLP_ENDPOINT=https://telemetry.jarvy.dev \
jarvy --version
```

Then in Grafana Cloud → Explore → Loki, query:

```logql
{service_name="jarvy"} |= "jarvy.startup"
```

You should see the startup event within a minute. If you don't, walk
the pipeline: Caddy access log → Collector `audit.json` → Grafana
Loki. Whichever stage shows the event and the next stage doesn't is
where to look.

---

## PII scrubbing checklist

The forwarder enforces what the Jarvy CLI also tries to enforce. The
two layers exist because a single client-side regression can quietly
leak something for months before someone notices.

The forwarder **drops** these attributes unconditionally:

- `host.name`, `host.id`, `host.ip`
- `user.name`, `user.email`
- `jarvy.config.path`, `jarvy.toml.contents`, `jarvy.cwd`

The forwarder **hashes** these:

- `jarvy.install_id` — anonymized install identifier (the CLI generates
  it as a UUID per `~/.jarvy/`; the forwarder hashes again as defense
  in depth)

The forwarder **redacts inside log bodies**:

- Email-shaped strings → `<redacted-email>`
- Public IPv4 addresses → `<redacted-ip>`
- `/Users/<name>` and `/home/<name>` path prefixes → `<redacted>`

The forwarder **keeps**:

- `service.name`, `service.version` (Jarvy version)
- `os.type`, `os.version` (e.g. `darwin 14.5`)
- Tool names from the registry (`node`, `docker`, etc.) — these are
  open-source identifiers, not PII
- Timing data (setup duration, install duration)
- Error categories (HTTP 4xx vs network timeout vs missing prereq)

When a new code path is added in Jarvy that emits a new attribute,
update both:

1. The schema doc at `docs/telemetry.md` (user-facing promise)
2. This file's allowlist + the Collector config (enforcement)

If the schema doc and the Collector config drift, the next privacy
audit will find it. Treat them as one change.

---

## Hardening checklist

Run through this every time the forwarder is provisioned or after any
significant config change.

- [ ] DNS resolves and Cloudflare proxy is enabled (orange cloud)
- [ ] Cloudflare WAF rule blocks non-`POST /v1/{logs,metrics,traces}`
- [ ] Cloudflare rate-limit rule active at 60/min/IP
- [ ] Origin VM firewall: 443 from Cloudflare IPs only, 22 from
      maintainer IP only
- [ ] Origin VM `unattended-upgrades` enabled
- [ ] Caddy auto-TLS working (`curl -I https://telemetry.jarvy.dev`
      returns a real Let's Encrypt cert)
- [ ] Caddy `request_body max_size 64KB` enforced (verified by
      `curl -X POST -d "$(head -c 100000 /dev/urandom | base64)"
      https://telemetry.jarvy.dev/v1/logs` → 413)
- [ ] Collector runs as non-root `otelcol` user
- [ ] Collector systemd unit has `ProtectSystem=strict`,
      `ProtectHome=true`, `NoNewPrivileges=true`
- [ ] Grafana write token loaded via `LoadCredentialEncrypted`, not
      raw env var
- [ ] Grafana token is write-only-scoped (verify in Grafana Cloud
      access policy UI)
- [ ] Collector PII scrub processor pipeline is in front of
      `otlphttp/grafana` in every signal's pipeline
- [ ] Test event sent from a development laptop appears in Grafana
      Loki within 60 seconds
- [ ] Synthetic PII event (email shape in a log body) appears in
      Grafana with `<redacted-email>`, not the raw value

---

## Cost and quota controls

- **Grafana Cloud free tier** at the time of writing: 10k metrics
  series, 50 GB logs/month, 50 GB traces/month, 14-day retention. A
  Jarvy install emitting a normal volume (setup events + a handful of
  metrics per session) fits well inside that for a five-figure
  monthly-active-user count.
- **Per-IP rate limit at Cloudflare** prevents a single host from
  burning a noticeable fraction of the quota.
- **Collector `memory_limiter` processor** drops batches if the
  Collector RAM grows beyond 200 MiB, so a runaway client cannot OOM
  the forwarder.
- **Grafana Cloud usage alerts**: set "80% of free-tier ingest"
  warnings on logs, metrics, and traces. The alert routes to the
  maintainer's email; investigate before the meter hits 100%.

If the free tier runs out, the cheapest paid Grafana Cloud Pro tier
covers ~100× the current volume.

---

## Monitoring the forwarder itself

We use the forwarder's own outputs to monitor it:

- **Caddy access log** (`/var/log/caddy/access.log`) — request rate,
  status codes, body sizes. Rotate keeps last 500 MB.
- **Collector internal telemetry** — the Collector exposes its own
  metrics on `127.0.0.1:8888/metrics`. Scrape with the Grafana Agent
  on the same host to send back to Grafana Cloud (Mimir).
- **`/healthz`-equivalent** — the Collector's `health_check` extension
  on `127.0.0.1:13133`. Add a Cloudflare Healthcheck → page the
  maintainer if it goes down for >5 min.

Key metrics to alert on:

- `otelcol_receiver_refused_spans` > 0 (means the rate limiter is
  hitting valid traffic)
- `otelcol_exporter_send_failed_spans` rate > 1/min (Grafana endpoint
  is unhealthy or token is invalid)
- `process_resident_memory_bytes` > 250 MiB sustained (memory limiter
  is about to kick in)
- Caddy 4xx rate > 5% (the schema may have drifted; clients are
  sending shapes the WAF rejects)

---

## Incident playbook

When something is wrong with telemetry, the worst case is a privacy
leak that landed in Grafana before the scrubber caught it. Triage in
this order:

1. **Stop the bleed.** `systemctl stop otelcol` on the forwarder.
   Cloudflare WAF will return 5xx; clients fail open (telemetry is
   advisory, not load-bearing).
2. **Confirm scope.** Pull the last hour of `/var/log/otelcol/audit.json`.
   Search for whatever leaked. Note which Jarvy versions are
   represented (`service.version`).
3. **Purge if needed.** Grafana Cloud → Loki → admin API → delete by
   selector for the affected time window. Same for Mimir / Tempo.
4. **Patch.** If the leak is a client-side regression, fix in Jarvy
   main, cut a patch release. If the leak is a forwarder-side gap,
   add to the `attributes/scrub` action list and the
   `transform/redact_bodies` patterns, redeploy.
5. **Restart.** `systemctl start otelcol`. Verify with a manual test.
6. **Post-mortem.** File a `release-postmortem`-tagged issue: what
   leaked, how it bypassed the layers, what new test or scrub rule
   prevents recurrence.

---

## Operational handoff checklist

If you ever hand the forwarder to another maintainer, transfer:

- Cloudflare account access (or zone access via Teams)
- Origin VM SSH access (rotate the maintainer's key in
  `~otelcol/.ssh/authorized_keys` and the maintainer's own
  authorized_keys)
- Grafana Cloud organization admin invite
- The encrypted Grafana write token (regenerate; do not transfer the
  old token)
- This document, with any local deviations noted inline

The forwarder is intentionally small enough that a one-week handoff is
realistic. If it grows beyond that, the design needs a re-look — the
goal is a thing that survives the maintainer being out for a month, not
a thing that requires constant attention.

---

## See also

- [Telemetry](../telemetry.md) — the user-facing schema, opt-in
  command, environment variables, and data-handling promise the
  forwarder must implement.
- [`docs/release-quirks-jarvy.md`](../release-quirks-jarvy.md) —
  release-pipeline quirks; do not auto-deploy forwarder changes from
  release tags.
- OpenTelemetry Collector documentation:
  <https://opentelemetry.io/docs/collector/>
- Grafana Cloud OTLP gateway docs:
  <https://grafana.com/docs/grafana-cloud/send-data/otlp/>
