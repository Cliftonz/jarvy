{{/*
Expand the name of the chart.
*/}}
{{- define "jarvy-telemetry-forwarder.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{/*
Fully qualified app name.
*/}}
{{- define "jarvy-telemetry-forwarder.fullname" -}}
{{- if .Values.fullnameOverride -}}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" -}}
{{- else -}}
{{- $name := default .Chart.Name .Values.nameOverride -}}
{{- if contains $name .Release.Name -}}
{{- .Release.Name | trunc 63 | trimSuffix "-" -}}
{{- else -}}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" -}}
{{- end -}}
{{- end -}}
{{- end -}}

{{/*
Chart label.
*/}}
{{- define "jarvy-telemetry-forwarder.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{/*
Common labels.
*/}}
{{- define "jarvy-telemetry-forwarder.labels" -}}
helm.sh/chart: {{ include "jarvy-telemetry-forwarder.chart" . }}
{{ include "jarvy-telemetry-forwarder.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- with .Values.commonLabels }}
{{ toYaml . }}
{{- end }}
{{- end -}}

{{/*
Selector labels (stable subset of common labels).
*/}}
{{- define "jarvy-telemetry-forwarder.selectorLabels" -}}
app.kubernetes.io/name: {{ include "jarvy-telemetry-forwarder.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end -}}

{{/*
Common annotations (rendered into every resource that opts in).
*/}}
{{- define "jarvy-telemetry-forwarder.annotations" -}}
{{- with .Values.commonAnnotations -}}
{{ toYaml . }}
{{- end -}}
{{- end -}}

{{/*
Container image reference. Prefer `digest` over `tag` for production
supply-chain hygiene; tag is decorative when digest is set.
*/}}
{{- define "jarvy-telemetry-forwarder.image" -}}
{{- $img := .Values.collector.image -}}
{{- if $img.digest -}}
{{- printf "%s@%s" $img.repository $img.digest -}}
{{- else -}}
{{- printf "%s:%s" $img.repository $img.tag -}}
{{- end -}}
{{- end -}}

{{/*
Render the OTTL anonymize statements — one `set(...)` per key in
`pii.hashedAttributes`. Used in three OTel contexts
(resource attributes on log_statements / metric_statements /
trace_statements) so this helper is the single source of truth for
the salt construction. The Concat order is value-then-salt to keep
SHA-256 length-extension safe; reversing it would be silently
exploitable. The chart CI greps for the ordering to prevent
regression.
*/}}
{{- define "jarvy-telemetry-forwarder.anonymizeStatements" -}}
{{- range . }}
- set(attributes[{{ . | quote }}], SHA256(Concat([attributes[{{ . | quote }}], "${env:PII_SALT}"], ""))) where attributes[{{ . | quote }}] != nil
{{- end }}
{{- end -}}

{{/*
Render the OTel allowlist as a single OTTL `keep_keys(attributes, [...])`
call that DROPS every attribute not on the (passThrough ∪ hashed)
union. This is the actual enforcement of the privacy contract — the
chart's headline claim "anything not on the allowlist is dropped"
depends on this statement producing a `keep_keys` (which removes
unknown keys from each record), not a `filter` (which only drops
whole records lacking allowlisted keys).
*/}}
{{- define "jarvy-telemetry-forwarder.keepKeysStatement" -}}
{{- $allowed := concat .Values.pii.passThroughAttributes .Values.pii.hashedAttributes | uniq -}}
- keep_keys(attributes, [{{- range $i, $k := $allowed }}{{- if $i }}, {{ end }}{{ $k | quote }}{{- end }}])
{{- end -}}

{{/*
Render the Collector configuration. Used by the ConfigMap template
when `collector.config` is empty. Pipeline shape encodes the chart's
privacy contract:

  receivers → memory_limiter
            → transform/anonymize  (salted SHA-256 of hashed keys)
            → transform/keep_allowlist_attrs  (drop unknown keys)
            → transform/redact_bodies  (logs only)
            → tail_sampling  (traces only)
            → batch → exporter

The keep_allowlist_attrs step is load-bearing: without it,
attacker-controlled or future-emitted attribute keys land in the
backend unhashed.

A render-time guard refuses the install if
`pii.passThroughAttributes` and `pii.hashedAttributes` overlap —
a key cannot simultaneously be passed through unhashed AND hashed.
*/}}
{{- define "jarvy-telemetry-forwarder.collectorConfig" -}}
{{- $overlap := list -}}
{{- $hashed := .Values.pii.hashedAttributes -}}
{{- range .Values.pii.passThroughAttributes -}}
{{- if has . $hashed -}}
{{- $overlap = append $overlap . -}}
{{- end -}}
{{- end -}}
{{- if $overlap -}}
{{- fail (printf "pii.passThroughAttributes and pii.hashedAttributes overlap on keys: %v. A key cannot be both passed through unhashed AND hashed. Move each conflicting key to exactly one list." $overlap) -}}
{{- end -}}
receivers:
  otlp:
    protocols:
      http:
        endpoint: 0.0.0.0:4318
        # Conservative receiver-level body cap (defense in depth in
        # case the ingress-side body cap is misconfigured on a
        # non-Traefik GatewayClass).
        max_request_body_size: 4194304  # 4 MiB

processors:
  transform/anonymize:
    error_mode: propagate
    log_statements:
      - context: resource
        statements:
          {{- include "jarvy-telemetry-forwarder.anonymizeStatements" .Values.pii.hashedAttributes | nindent 10 }}
    metric_statements:
      - context: resource
        statements:
          {{- include "jarvy-telemetry-forwarder.anonymizeStatements" .Values.pii.hashedAttributes | nindent 10 }}
    trace_statements:
      - context: resource
        statements:
          {{- include "jarvy-telemetry-forwarder.anonymizeStatements" .Values.pii.hashedAttributes | nindent 10 }}

  transform/keep_allowlist_attrs:
    error_mode: propagate
    log_statements:
      - context: resource
        statements:
          {{- include "jarvy-telemetry-forwarder.keepKeysStatement" . | nindent 10 }}
    metric_statements:
      - context: resource
        statements:
          {{- include "jarvy-telemetry-forwarder.keepKeysStatement" . | nindent 10 }}
    trace_statements:
      - context: resource
        statements:
          {{- include "jarvy-telemetry-forwarder.keepKeysStatement" . | nindent 10 }}

  transform/redact_bodies:
    error_mode: propagate
    log_statements:
      - context: log
        statements:
{{- range .Values.pii.bodyRedactPatterns }}
          - replace_pattern(body, {{ .pattern | quote }}, {{ .replacement | quote }})
{{- end }}

  memory_limiter:
    check_interval: {{ .Values.collector.pipeline.memoryLimiter.checkInterval }}
    limit_mib: {{ .Values.collector.pipeline.memoryLimiter.limitMib }}
    spike_limit_mib: {{ .Values.collector.pipeline.memoryLimiter.spikeLimitMib }}

  tail_sampling:
    decision_wait: {{ printf "%ds" (int .Values.collector.pipeline.tailSampling.decisionWaitSeconds) }}
    num_traces: {{ .Values.collector.pipeline.tailSampling.numTraces }}
    policies:
      - name: errors
        type: status_code
        status_code: { status_codes: [ERROR] }
      - name: probabilistic
        type: probabilistic
        probabilistic:
          sampling_percentage: {{ .Values.collector.pipeline.tailSampling.probabilisticSamplingPercentage }}

  batch:
    timeout: {{ .Values.collector.pipeline.batch.timeout }}
    send_batch_size: {{ .Values.collector.pipeline.batch.sendBatchSize }}
    send_batch_max_size: {{ .Values.collector.pipeline.batch.sendBatchMaxSize }}

exporters:
  otlphttp/backend:
    endpoint: ${env:BACKEND_OTLP_ENDPOINT}
    auth:
      authenticator: bearertokenauth/backend
  {{- if .Values.collector.debugExporter.enabled }}
  debug:
    verbosity: {{ .Values.collector.debugExporter.verbosity }}
    sampling_initial: {{ .Values.collector.debugExporter.samplingInitial }}
    sampling_thereafter: {{ .Values.collector.debugExporter.samplingThereafter }}
  {{- end }}

extensions:
  bearertokenauth/backend:
    scheme: {{ .Values.exporter.authScheme | quote }}
    token: ${env:BACKEND_OTLP_TOKEN}
  health_check:
    endpoint: 0.0.0.0:13133

service:
  extensions: [bearertokenauth/backend, health_check]
  telemetry:
    logs:
      level: {{ .Values.collector.logLevel | quote }}
      encoding: {{ .Values.collector.logFormat | quote }}
    metrics:
      address: 0.0.0.0:8888
    resource:
      service.name: jarvy-telemetry-forwarder
      service.namespace: {{ .Release.Namespace }}
      service.version: {{ .Chart.AppVersion | quote }}
      deployment.environment: {{ .Release.Name }}
  pipelines:
    logs:
      receivers: [otlp]
      processors: [memory_limiter, transform/anonymize, transform/keep_allowlist_attrs, transform/redact_bodies, batch]
      exporters:
        - otlphttp/backend
{{- if .Values.collector.debugExporter.enabled }}
        - debug
{{- end }}
    metrics:
      receivers: [otlp]
      processors: [memory_limiter, transform/anonymize, transform/keep_allowlist_attrs, batch]
      exporters:
        - otlphttp/backend
{{- if .Values.collector.debugExporter.enabled }}
        - debug
{{- end }}
    traces:
      receivers: [otlp]
      processors: [memory_limiter, transform/anonymize, transform/keep_allowlist_attrs, tail_sampling, batch]
      exporters:
        - otlphttp/backend
{{- if .Values.collector.debugExporter.enabled }}
        - debug
{{- end }}
{{- end -}}
