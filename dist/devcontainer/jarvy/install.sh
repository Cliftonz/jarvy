#!/usr/bin/env bash
# Devcontainer feature install script for the Jarvy CLI (PRD-017 phase 4).
#
# Runs at image build. Feature options arrive as UPPERCASE env vars
# (VERSION, CHANNEL, RUNSETUP) per the devcontainer-feature spec. Delegates
# to the canonical installer (dist/scripts/install.sh) so the checksum-
# verification path is shared with every other install channel.
set -euo pipefail

VERSION="${VERSION:-latest}"
CHANNEL="${CHANNEL:-stable}"
RUNSETUP="${RUNSETUP:-false}"

echo "Installing Jarvy CLI (version=${VERSION}, channel=${CHANNEL})..."

# Ensure curl exists (slim base images often omit it). Best-effort across
# the package managers a devcontainer base might ship.
if ! command -v curl >/dev/null 2>&1; then
  if command -v apt-get >/dev/null 2>&1; then
    apt-get update -y && apt-get install -y --no-install-recommends curl ca-certificates
  elif command -v apk >/dev/null 2>&1; then
    apk add --no-cache curl ca-certificates
  elif command -v dnf >/dev/null 2>&1; then
    dnf install -y curl ca-certificates
  fi
fi

INSTALL_ARGS=(--channel "${CHANNEL}")
if [ "${VERSION}" != "latest" ]; then
  INSTALL_ARGS+=(--version "${VERSION}")
fi

# The published installer honors --channel/--version and verifies the
# archive against the release SHA256SUMS.txt before extracting.
curl -fsSL "https://raw.githubusercontent.com/Cliftonz/jarvy/main/dist/scripts/install.sh" \
  | bash -s -- "${INSTALL_ARGS[@]}"

# jarvy installs to a user-local bin; surface it on PATH for later
# feature/install steps and the running container.
for dir in "$HOME/.local/bin" "/usr/local/bin"; do
  if [ -x "$dir/jarvy" ]; then
    echo "export PATH=\"$dir:\$PATH\"" >> /etc/profile.d/jarvy.sh 2>/dev/null || true
    export PATH="$dir:$PATH"
    break
  fi
done

if command -v jarvy >/dev/null 2>&1; then
  echo "Jarvy installed: $(jarvy --version 2>/dev/null || echo 'version unavailable')"
else
  echo "warning: jarvy not found on PATH after install" >&2
fi

# Optional: run setup at postCreate rather than build time. We only drop
# a postCreate marker; actual `jarvy setup` runs against the mounted
# workspace, which isn't present during the build phase.
if [ "${RUNSETUP}" = "true" ]; then
  cat > /usr/local/share/jarvy-postcreate.sh <<'EOF'
#!/usr/bin/env bash
set -e
if [ -f "./jarvy.toml" ] && command -v jarvy >/dev/null 2>&1; then
  echo "Running jarvy setup against ./jarvy.toml..."
  jarvy setup
else
  echo "jarvy: no ./jarvy.toml in workspace or jarvy not installed; skipping setup."
fi
EOF
  chmod +x /usr/local/share/jarvy-postcreate.sh
  echo "runSetup enabled — add this to your devcontainer.json:"
  echo '  "postCreateCommand": "/usr/local/share/jarvy-postcreate.sh"'
fi

echo "Done."
