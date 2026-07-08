# syntax=docker/dockerfile:1
#
# Jarvy MCP server image.
#
# Distinct from tests/docker/*.dockerfile (install-test fixtures): this is
# the shippable runtime image. It builds jarvy from source and defaults to
# the MCP server entrypoint (`jarvy mcp`, JSON-RPC over stdio), so it plugs
# straight into MCP clients that speak stdio to a container:
#
#   docker run -i --rm ghcr.io/cliftonz/jarvy:latest
#
# Override the CMD for plain CLI use:
#
#   docker run --rm ghcr.io/cliftonz/jarvy:latest --version
#
# Build (from the repo root):
#
#   docker build -f dist/docker/Dockerfile -t ghcr.io/cliftonz/jarvy:dev .
#
# Published by .github/workflows/docker-publish.yml on release tags /
# manual dispatch only.

FROM rust:1-alpine3.22 AS builder

# build-base = gcc + make + musl-dev; enough for the same dependency tree
# the release workflow compiles for x86_64-unknown-linux-musl with only
# musl-tools installed.
RUN apk add --no-cache build-base

WORKDIR /src
COPY . .

# Cargo.lock is intentionally untracked in this repo (CI floats to
# latest-compatible), so no --locked here.
RUN cargo build --release -p jarvy \
    && strip target/release/jarvy

FROM alpine:3.22 AS runtime

ARG VERSION=dev

# io.modelcontextprotocol.server.name is the MCP registry's OCI ownership
# proof: the registry refuses to list an OCI package unless this label
# matches the `name` in dist/mcp-registry/server.json.
LABEL org.opencontainers.image.title="jarvy" \
      org.opencontainers.image.description="Jarvy MCP server - safe, auditable developer-tool discovery and installation for AI agents" \
      org.opencontainers.image.url="https://jarvy.dev" \
      org.opencontainers.image.source="https://github.com/Cliftonz/jarvy" \
      org.opencontainers.image.documentation="https://github.com/Cliftonz/jarvy/blob/main/docs/mcp-server.md" \
      org.opencontainers.image.version="${VERSION}" \
      org.opencontainers.image.licenses="MIT OR Apache-2.0" \
      io.modelcontextprotocol.server.name="io.github.cliftonz/jarvy"

# ca-certificates: HTTPS fetches (registry sync, library sources, update
# checks). git: git-based skill library sources (PRD-055).
RUN apk add --no-cache ca-certificates git \
    && addgroup -S jarvy \
    && adduser -S -G jarvy -h /home/jarvy jarvy \
    && mkdir -p /workspace \
    && chown jarvy:jarvy /workspace

COPY --from=builder /src/target/release/jarvy /usr/local/bin/jarvy

USER jarvy
ENV HOME=/home/jarvy \
    # Pin the MCP workspace-containment root to the mount point so
    # caller-supplied paths can't escape into the container filesystem.
    JARVY_MCP_WORKSPACE=/workspace
WORKDIR /workspace

# MCP server on stdio by default; `docker run ... <subcommand>` overrides.
ENTRYPOINT ["jarvy"]
CMD ["mcp"]
