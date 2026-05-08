---
title: "Recipe: Behind a corporate proxy — Jarvy"
description: "Configure Jarvy to install through an HTTP/HTTPS proxy with corporate CA certs, with credentials sourced from a vault."
tags:
  - cookbook
  - network
  - corporate
---

# Recipe: behind a corporate proxy

## Problem

Your company routes all outbound HTTP through a forward proxy (Squid, BlueCoat, ZScaler), MITMs TLS with an internal CA, and developers can't `brew install` without setting half a dozen env vars first. You want `jarvy setup` to work for every new hire on day one.

---

## Config

```toml title="jarvy.toml"
[network]
http_proxy  = "http://proxy.corp.com:8080"
https_proxy = "http://proxy.corp.com:8080"
no_proxy    = ["localhost", "127.0.0.1", ".corp.com", ".internal"]

[network.auth]
username = "{{ env.USER }}"
password = { env = "PROXY_PASSWORD" }   # never hardcode

[network.tls]
ca_bundle = "/etc/ssl/certs/corporate-ca.crt"

# Some tools need their own per-tool proxy override (e.g. git over a different
# proxy, or skipping the proxy entirely for a private mirror).
[network.overrides.git]
https_proxy = "http://git-proxy.corp.com:8888"
```

---

## Why it works

| Block | What it does |
|---|---|
| `[network]` | Sets `HTTPS_PROXY`, `HTTP_PROXY`, `NO_PROXY` for every package manager Jarvy invokes. |
| `[network.auth]` | Adds `username:password@` to the proxy URL. Password comes from the env var, never from disk. |
| `[network.tls] ca_bundle` | Exports `CURL_CA_BUNDLE`, `SSL_CERT_FILE`, `NODE_EXTRA_CA_CERTS`, `GIT_SSL_CAINFO` so every downstream tool trusts the corporate CA. |
| `[network.overrides.<tool>]` | Per-tool override when one tool needs a different proxy (e.g. an internal mirror). |

The `{ env = "..." }` syntax means the value is read from the environment, not the file. Developers source `PROXY_PASSWORD` from their vault; the file ships clean to git.

---

## Variations

**Behind two proxies (one for git, one for everything else):**

```toml
[network]
https_proxy = "http://proxy.corp.com:8080"

[network.overrides.git]
https_proxy = "http://git-proxy.corp.com:8888"

[network.overrides.cargo]
https_proxy = "http://artifactory.corp.com:8081"
```

**SOCKS proxy:**

```toml
[network]
https_proxy = "socks5://socks.corp.com:1080"
```

**No auth needed:**

Just omit `[network.auth]`. The proxy is used unauthenticated.

**Conditional on shell env:**

If only some developers are behind the proxy, gate the block with a role:

```toml
role = "remote"

[roles.remote]
tools = ["git", "node"]

[network]
https_proxy = "http://proxy.corp.com:8080"
# ...
```

Then on-network developers skip the role.

---

## Caveats

- **Self-signed certificates:** if the proxy's CA isn't recognized by the system, you may need to also add it to your OS trust store (`update-ca-certificates` on Linux, Keychain on macOS).
- **Some installers ignore env vars:** a few package managers (notably Chocolatey on certain configurations) read proxy from their own config, not env. For those, write a `pre_setup` hook that runs the right configuration command.
- **MITM with self-signed CA breaks `cargo` and `npm` in subtle ways:** if you see `unable to get local issuer certificate`, double-check `NODE_EXTRA_CA_CERTS` and `CARGO_NET_GIT_FETCH_WITH_CLI` are exported.

---

## See also

- [Network & proxy guide](../network.md) — full reference
- [Roles guide](../roles.md) — per-role network overrides
