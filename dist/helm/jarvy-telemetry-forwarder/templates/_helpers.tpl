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
Applied to EVERY resource — Namespace, ServiceAccount, Deployment,
Service, ConfigMap, Gateway, HTTPRoute, NetworkPolicy, HPA, PDB,
ServiceMonitor, ExternalSecrets, Certificate, Middlewares,
PrometheusRule. If you set commonAnnotations expecting them on one
or two resources, you'll get them on all 14+ — by design.
*/}}
{{- define "jarvy-telemetry-forwarder.annotations" -}}
{{- with .Values.commonAnnotations -}}
{{ toYaml . }}
{{- end -}}
{{- end -}}

{{/*
Compute the container image reference. Prefer `digest` over `tag` for
production supply-chain hygiene; tag is decorative when digest is set.
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
Render the Collector configuration. Used by the ConfigMap template
when `collector.config` is empty. The pipeline shape encodes the
chart's central privacy contract:

  receivers → memory_limiter → transform/anonymize → keep_keys
            → transform/redact_bodies (logs only) → tail_sampling
            (traces only) → batch → exporter

`transform/anonymize` hashes every key in `pii.hashedAttributes`
with a salted SHA-256. The `keep_keys` filter then DROPS every
attribute not in `pii.passThroughAttributes ∪ pii.hashedAttributes`
— this is the allowlist enforcement. Without keep_keys, future
schema additions or attacker-controlled attribute keys land in the
backend plaintext.

`error_mode: propagate` on the transform processors means OTTL
failures (compile errors, type mismatches) surface as
`otelcol_processor_dropped_*` metrics rather than silently bypassing
the anonymization. Required for the salt-staleness and
denylist-not-allowlist alerts in the PrometheusRule to actually
fire.
*/}}
{{- define "jarvy-telemetry-forwarder.collectorConfig" -}}
receivers:
  otlp:
    protocols:
      http:
        endpoint: 0.0.0.0:4318

processors:
  transform/anonymize:
    error_mode: propagate
    log_statements:
      - context: resource
        statements:
{{- range .Values.pii.hashedAttributes }}
          - set(attributes[{{ . | quote }}], SHA256(Concat([attributes[{{ . | quote }}], "${env:PII_SALT}"], ""))) where attributes[{{ . | quote }}] != nil
{{- end }}
    metric_statements:
      - context: resource
        statements:
{{- range .Values.pii.hashedAttributes }}
          - set(attributes[{{ . | quote }}], SHA256(Concat([attributes[{{ . | quote }}], "${env:PII_SALT}"], ""))) where attributes[{{ . | quote }}] != nil
{{- end }}
    trace_statements:
      - context: resource
        statements:
{{- range .Values.pii.hashedAttributes }}
          - set(attributes[{{ . | quote }}], SHA256(Concat([attributes[{{ . | quote }}], "${env:PII_SALT}"], ""))) where attributes[{{ . | quote }}] != nil
{{- end }}

  {{- /* keep_keys allowlist closes the denylist gap. Anything not
        on passThroughAttributes ∪ hashedAttributes is DROPPED. */}}
  filter/keep_allowlist:
    error_mode: propagate
    logs:
      log_record:
        - 'not (
{{- $allowed := concat .Values.pii.passThroughAttributes .Values.pii.hashedAttributes -}}
{{- range $i, $k := $allowed -}}
{{- if $i }} or {{ end -}}
resource.attributes[{{ $k | quote }}] != nil
{{- end -}}
)'
    metrics:
      metric:
        - 'not (
{{- range $i, $k := $allowed -}}
{{- if $i }} or {{ end -}}
resource.attributes[{{ $k | quote }}] != nil
{{- end -}}
)'
    traces:
      span:
        - 'not (
{{- range $i, $k := $allowed -}}
{{- if $i }} or {{ end -}}
resource.attributes[{{ $k | quote }}] != nil
{{- end -}}
)'

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
      processors: [memory_limiter, transform/anonymize, filter/keep_allowlist, transform/redact_bodies, batch]
      exporters:
        - otlphttp/backend
{{- if .Values.collector.debugExporter.enabled }}
        - debug
{{- end }}
    metrics:
      receivers: [otlp]
      processors: [memory_limiter, transform/anonymize, filter/keep_allowlist, batch]
      exporters:
        - otlphttp/backend
{{- if .Values.collector.debugExporter.enabled }}
        - debug
{{- end }}
    traces:
      receivers: [otlp]
      processors: [memory_limiter, transform/anonymize, filter/keep_allowlist, tail_sampling, batch]
      exporters:
        - otlphttp/backend
{{- if .Values.collector.debugExporter.enabled }}
        - debug
{{- end }}
{{- end -}}
