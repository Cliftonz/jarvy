---
title: "Telemetry - Jarvy"
description: "Configure OpenTelemetry logs, metrics, and optional traces for Jarvy. Opt-out by default."
---

# Telemetry

Jarvy emits OpenTelemetry (OTLP) signals — logs, metrics, and optional traces — so teams can monitor adoption, surface common errors, and observe setup performance across a fleet.

**Telemetry is opt-out.** It is on by default after the first-run notice. Disable with `jarvy telemetry disable`, `JARVY_TELEMETRY=0`, or `[telemetry] enabled = false` in `~/.jarvy/config.toml`. CI and unattended sandboxes auto-disable unless explicitly overridden.

When enabled, Jarvy CLIs send to the project's hardened public forwarder
at `https://telemetry.jarvy.dev` by default. The forwarder strips PII,
rate-limits, and fans out to Grafana Cloud. The full architecture,
threat model, scrub policy, and operational runbook live in
[Telemetry forwarder operations](operations/telemetry-forwarder.md) —
that doc is the contract this page's data-handling promise must
implement.

Override the endpoint with `JARVY_OTLP_ENDPOINT=https://your-collector`
or `[telemetry] endpoint = "..."` to send to your own collector instead.

## Quick Disable

```bash
jarvy telemetry disable                # persistent
JARVY_TELEMETRY=0 jarvy <cmd>          # per-invocation
```

## Quick Enable / Configure

```bash
jarvy telemetry enable
jarvy telemetry set-endpoint http://otel-collector.internal:4318
jarvy telemetry test
```

Or edit `~/.jarvy/config.toml` directly:

```toml
[telemetry]
enabled = true
endpoint = "https://telemetry.jarvy.dev"  # project's public forwarder; override for self-hosted
protocol = "http"     # "http" or "grpc"
logs = true
metrics = true
traces = false
sample_rate = 1.0
```

## Configuration Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | bool | `true` | Master switch |
| `endpoint` | string | – | OTLP endpoint URL |
| `protocol` | enum | `http` | `http` (port 4318) or `grpc` (port 4317) |
| `logs` | bool | `true` | Export structured logs |
| `metrics` | bool | `true` | Export counters, histograms, gauges |
| `traces` | bool | `false` | Export spans (heavier; off by default) |
| `sample_rate` | float | `1.0` | Trace sampling 0.0–1.0 |

## Environment Overrides

| Variable | Equivalent |
|----------|-----------|
| `JARVY_TELEMETRY=1` / `JARVY_TELEMETRY=0` | `enabled = true` / `enabled = false` |
| `JARVY_OTLP_ENDPOINT=...` | `endpoint = ...` |
| `JARVY_OTLP_PROTOCOL=grpc` | `protocol = "grpc"` |
| `JARVY_OTLP_LOGS=1` | `logs = true` |
| `JARVY_OTLP_METRICS=1` | `metrics = true` |
| `JARVY_OTLP_TRACES=1` | `traces = true` |
| `JARVY_OTLP_SAMPLE_RATE=0.1` | `sample_rate = 0.1` |

Env always wins over config file.

## CI Behavior

When `CI=true` is set (or Jarvy detects an unattended sandbox), telemetry is **auto-disabled** even though the global default is on. Override with `JARVY_TELEMETRY=1` if you genuinely want CI runs to report.

## What Gets Sent

### Logs
Structured `tracing` events with `info!`/`warn!`/`error!` level. Includes command name, exit code, duration, OS, arch, Jarvy version. **No filesystem paths, secrets, or hostnames.**

### Metrics
| Metric | Type | Description |
|--------|------|-------------|
| `jarvy.command.count` | counter | Commands run, labeled by name + result |
| `jarvy.tool.install.duration` | histogram | Per-tool install time |
| `jarvy.tool.install.count` | counter | Installs by tool name + outcome |
| `jarvy.setup.duration` | histogram | Total `jarvy setup` time |
| `jarvy.config.tool_count` | gauge | Tools declared in `jarvy.toml` |

### Traces (off by default)
Spans cover `setup`, per-tool `install`, hook execution, and remote-config fetch. Useful for diagnosing slow installs.

## CLI Commands

```bash
jarvy telemetry status               # Show current config
jarvy telemetry enable               # Enable
jarvy telemetry disable              # Disable
jarvy telemetry set-endpoint <url>   # Set OTLP endpoint
jarvy telemetry test                 # Send test signals + report success
jarvy telemetry preview              # Print what would be sent — no network
```

## Self-Hosting an OTLP Collector

A minimal Docker setup:

```yaml
# docker-compose.yml
services:
  otel-collector:
    image: otel/opentelemetry-collector-contrib:latest
    command: ["--config=/etc/otel-collector.yaml"]
    ports:
      - "4318:4318"   # HTTP
      - "4317:4317"   # gRPC
    volumes:
      - ./otel-collector.yaml:/etc/otel-collector.yaml
```

```yaml
# otel-collector.yaml
receivers:
  otlp:
    protocols:
      http:
      grpc:

exporters:
  prometheus:
    endpoint: "0.0.0.0:9464"
  loki:
    endpoint: "http://loki:3100/loki/api/v1/push"

service:
  pipelines:
    metrics:
      receivers: [otlp]
      exporters: [prometheus]
    logs:
      receivers: [otlp]
      exporters: [loki]
```

Then point Jarvy at it: `jarvy telemetry set-endpoint http://otel-collector.internal:4318`.

## Privacy

- Opt-in by default
- No secrets, no env vars, no file paths
- A stable per-machine ID (machineid-rs) lets you correlate signals from the same workstation without identifying the user
- See [`PRIVACY.md`](https://github.com/bearbinary/jarvy/blob/main/PRIVACY.md) for the full data-handling policy

## Module

- Source: `src/telemetry.rs`, `src/observability/`
- Stack: `tracing` + `tracing-subscriber` + `opentelemetry`/`opentelemetry-otlp` 0.31
