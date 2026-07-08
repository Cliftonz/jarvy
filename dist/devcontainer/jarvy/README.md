# Jarvy CLI (devcontainer feature)

Installs the [Jarvy](https://github.com/Cliftonz/jarvy) dev-environment
provisioning CLI into a devcontainer.

## Usage

Reference the feature in your `devcontainer.json`:

```jsonc
{
  "features": {
    "ghcr.io/cliftonz/jarvy/jarvy:0": {
      "version": "latest",
      "channel": "stable",
      "runSetup": false
    }
  }
}
```

> The `ghcr.io/cliftonz/jarvy/jarvy` reference is available once the
> maintainer publishes this feature to GHCR (`devcontainer features
> publish`). Until then, reference it locally with
> `"./dist/devcontainer/jarvy": {}`.

## Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `version` | string | `latest` | Release tag (e.g. `0.5.2`) or `latest`. |
| `channel` | enum | `stable` | `stable` \| `beta` \| `nightly`, passed to `install.sh`. |
| `runSetup` | boolean | `false` | Install a postCreate script that runs `jarvy setup` against `./jarvy.toml`. |

When `runSetup` is `true`, wire the emitted script into your
`devcontainer.json`:

```jsonc
"postCreateCommand": "/usr/local/share/jarvy-postcreate.sh"
```

The install delegates to the canonical `dist/scripts/install.sh`, so the
binary is verified against the release `SHA256SUMS.txt` — the same
integrity check as every other Jarvy install channel.
