#!/usr/bin/env bash
# Smoke test the public jarvy-telemetry-forwarder endpoint end-to-end.
#
# Walks the same three OTLP signals (logs / metrics / traces) the
# in-cluster `helm test` smoke pod exercises, but goes through the
# real public ingress: TLS handshake, gateway routing, traefik rate-
# limit + body-cap middlewares, then the receiver pipeline. A diff
# between this and the in-cluster `helm test` smoke isolates ingress
# as the suspect.
#
# Usage:
#   scripts/smoke-live.sh                       # default host
#   HOST=jarvyotel.clifton.quest \
#     scripts/smoke-live.sh
#   HOST=stage.example.com scripts/smoke-live.sh
#   VERBOSE=1 scripts/smoke-live.sh
#
# Exit codes:
#   0 — all three signals returned 2xx
#   1 — one or more signals failed (status, body printed)
#   2 — TLS/DNS/connectivity broke before HTTP could be evaluated
#   3 — usage error (missing curl, bad HOST)

set -euo pipefail

HOST="${HOST:-jarvyotel.clifton.quest}"
SCHEME="${SCHEME:-https}"
TIMEOUT="${TIMEOUT:-10}"
VERBOSE="${VERBOSE:-0}"

if ! command -v curl >/dev/null 2>&1; then
  echo "fatal: curl not on PATH" >&2
  exit 3
fi
if [[ -z "${HOST}" ]]; then
  echo "fatal: HOST is empty" >&2
  exit 3
fi

ENDPOINT="${SCHEME}://${HOST}"
log() { printf '[smoke-live] %s\n' "$*"; }

# Cyan/green for ok, red for fail when stdout is a tty.
if [[ -t 1 ]]; then
  C_OK=$'\033[32m'; C_FAIL=$'\033[31m'; C_OFF=$'\033[0m'
else
  C_OK=""; C_FAIL=""; C_OFF=""
fi

log "endpoint: ${ENDPOINT}"

# ---------------------------------------------------------------------
# Step 1 — reachability + TLS.
# ---------------------------------------------------------------------
# Use HEAD on `/` so we don't accidentally trip a body-cap middleware.
# Any 4xx/5xx is acceptable here — we only need proof the TLS handshake
# completed and a response came back. Network/DNS errors return curl
# exit codes 6/7/35.
log "step 1/4: TLS handshake + reachability"
tls_out=""
if ! tls_out=$(curl -sS --max-time "${TIMEOUT}" -I "${ENDPOINT}/" 2>&1); then
  printf "%sFAIL%s reachability: %s\n" "${C_FAIL}" "${C_OFF}" "${tls_out}"
  exit 2
fi
tls_status=$(printf '%s\n' "${tls_out}" | awk '/^HTTP\//{print $2; exit}')
log "  ok: HTTP ${tls_status} (TLS up, ingress reachable)"

# ---------------------------------------------------------------------
# Step 2-4 — POST minimal OTLP/HTTP JSON to each signal endpoint.
# ---------------------------------------------------------------------
# Payloads match the in-cluster helm test pod byte-for-byte so any
# divergence in behavior between live and in-cluster narrows to
# ingress (gateway/middleware/TLS) as the only differing variable.
LOG_BODY='{"resourceLogs":[{"resource":{"attributes":[{"key":"service.name","value":{"stringValue":"smoke-live"}}]},"scopeLogs":[{"scope":{"name":"smoke-live"},"logRecords":[{"timeUnixNano":"0","severityNumber":9,"severityText":"INFO","body":{"stringValue":"smoke-live ping"}}]}]}]}'
METRIC_BODY='{"resourceMetrics":[{"resource":{"attributes":[{"key":"service.name","value":{"stringValue":"smoke-live"}}]},"scopeMetrics":[{"scope":{"name":"smoke-live"},"metrics":[{"name":"smoke.live.ping","sum":{"aggregationTemporality":2,"isMonotonic":true,"dataPoints":[{"asInt":"1","timeUnixNano":"0"}]}}]}]}]}'
TRACE_BODY='{"resourceSpans":[{"resource":{"attributes":[{"key":"service.name","value":{"stringValue":"smoke-live"}}]},"scopeSpans":[{"scope":{"name":"smoke-live"},"spans":[{"traceId":"5b8aa5a2d2c872e8321cf37308d69df2","spanId":"051581bf3cb55c13","name":"smoke.live.ping","kind":1,"startTimeUnixNano":"0","endTimeUnixNano":"1"}]}]}]}'

fail=0
step=2
for pair in "logs:${LOG_BODY}" "metrics:${METRIC_BODY}" "traces:${TRACE_BODY}"; do
  signal="${pair%%:*}"
  body="${pair#*:}"
  url="${ENDPOINT}/v1/${signal}"
  log "step ${step}/4: POST /v1/${signal}"
  step=$((step + 1))

  # `-w '\nHTTP_STATUS:%{http_code}\n'` ensures we always see the
  # status even when curl succeeds. `--fail-with-body` makes 4xx/5xx
  # a non-zero exit while still printing the body for diagnostics.
  out=""
  rc=0
  out=$(curl -sS --max-time "${TIMEOUT}" --fail-with-body \
    -H 'Content-Type: application/json' \
    -X POST --data-raw "${body}" \
    -w '\nHTTP_STATUS:%{http_code}\n' \
    "${url}" 2>&1) || rc=$?

  code=$(printf '%s\n' "${out}" | awk -F: '/^HTTP_STATUS:/{print $2}' | tail -1)
  if [[ "${rc}" -ne 0 || -z "${code}" || "${code}" -lt 200 || "${code}" -ge 300 ]]; then
    printf "  %sFAIL%s ${signal}: rc=%s status=%s\n" "${C_FAIL}" "${C_OFF}" "${rc}" "${code:-unknown}"
    if [[ "${VERBOSE}" == "1" || "${rc}" -ne 0 ]]; then
      printf "  body:\n%s\n" "${out}" | sed 's/^/    /'
    fi
    fail=1
  else
    printf "  %sok%s ${signal}: ${code}\n" "${C_OK}" "${C_OFF}"
  fi
done

if [[ "${fail}" -eq 0 ]]; then
  printf "%sall checks passed%s\n" "${C_OK}" "${C_OFF}"
else
  printf "%sone or more checks failed%s\n" "${C_FAIL}" "${C_OFF}"
fi
exit "${fail}"
