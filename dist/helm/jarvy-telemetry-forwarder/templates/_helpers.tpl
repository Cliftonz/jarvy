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
Render the Collector configuration. Used by the ConfigMap template
when `collector.config` is empty. Iterates over `pii.hashedAttributes`
to produce one OTTL `set(...)` statement per entry, so adding a new
PII attribute is a one-line values change rather than a template
fork.
*/}}
{{- define "jarvy-telemetry-forwarder.collectorConfig" -}}
receivers:
  otlp:
    protocols:
      http:
        endpoint: 0.0.0.0:4318

processors:
  transform/anonymize:
    error_mode: ignore
    log_statements:
      - context: resource
        statements:
{{- range .Values.pii.hashedAttributes }}
          - set(attributes[{{ . | quote }}], SHA256(Concat([attributes[{{ . | quote }}], "${env:PII_SALT}"], ""))) where attributes[{{ . | quote }}] != nil
{{- end }}

  transform/redact_bodies:
    error_mode: ignore
    log_statements:
      - context: log
        statements:
{{- range .Values.pii.bodyRedactPatterns }}
          - replace_pattern(body, {{ .pattern | quote }}, {{ .replacement | quote }})
{{- end }}

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
  otlphttp/backend:
    endpoint: ${env:BACKEND_OTLP_ENDPOINT}
    auth:
      authenticator: bearertokenauth/backend

extensions:
  bearertokenauth/backend:
    scheme: {{ .Values.exporter.authScheme | quote }}
    token: ${env:BACKEND_OTLP_TOKEN}
  health_check:
    endpoint: 0.0.0.0:13133

service:
  extensions: [bearertokenauth/backend, health_check]
  telemetry:
    metrics:
      address: 0.0.0.0:8888
  pipelines:
    logs:
      receivers: [otlp]
      processors: [memory_limiter, transform/anonymize, transform/redact_bodies, batch]
      exporters: [otlphttp/backend]
    metrics:
      receivers: [otlp]
      processors: [memory_limiter, transform/anonymize, batch]
      exporters: [otlphttp/backend]
    traces:
      receivers: [otlp]
      processors: [memory_limiter, transform/anonymize, tail_sampling, batch]
      exporters: [otlphttp/backend]
{{- end -}}
