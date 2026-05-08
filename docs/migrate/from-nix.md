---
title: "Migrate: Nix → Jarvy"
description: "Practical migration from Nix shells / flakes to jarvy.toml. Trade hermetic builds for native package managers and a much faster onboarding ramp."
tags:
  - migrate
  - reproducibility
  - nix

---

# Migrating from Nix to Jarvy

Nix and Jarvy solve overlapping problems with different philosophies. Nix is **purely functional** — every input is hashed, every output is reproducible, and the cost is a steep learning curve and a slow onboarding ramp. Jarvy is **declarative but not hermetic** — it leans on each OS's native package manager, so installs are fast and familiar, but you don't get bit-for-bit reproducibility.

If your team values reproducibility above all else, stay on Nix. If the Nix learning curve is blocking contributors and you'd trade some hermeticity for a config that any developer can read and modify on day one, this guide is for you.

---

## Conceptual mapping

| Nix concept | `jarvy.toml` equivalent |
|---|---|
| `flake.nix` `inputs` | Jarvy's tool registry — built-in versioning, no hand-pinning |
| `pkgs.nodejs_20` | `node = "20"` under `[provisioner]` |
| `mkShell { buildInputs = [ ... ]; }` | `[provisioner]` block |
| `shellHook` | `[hooks] post_setup` |
| `direnv` + `use flake` | `jarvy setup` plus your shell's normal PATH |
| `nix profile install` | `jarvy setup` (not user-profile-scoped — always project-scoped) |
| Override / override package versions | `[provisioner]` detailed form, version operators |

---

## Step 1: read off your `flake.nix` or `shell.nix`

A typical `flake.nix` dev shell:

```nix
{
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-23.11";

  outputs = { self, nixpkgs, ... }:
    let pkgs = import nixpkgs { system = "x86_64-linux"; };
    in {
      devShells.default = pkgs.mkShell {
        buildInputs = with pkgs; [
          nodejs_20
          python312
          docker-client
          terraform
          kubectl
        ];
        shellHook = ''
          export NODE_ENV=development
          echo "shell ready"
        '';
      };
    };
}
```

---

## Step 2: write the equivalent `jarvy.toml`

```toml title="jarvy.toml"
[provisioner]
node      = "20"
python    = "3.12"
docker    = "latest"
terraform = "latest"
kubectl   = "latest"

[env.vars]
NODE_ENV = "development"

[hooks]
post_setup = "echo 'shell ready'"
```

That's the whole migration for a typical dev shell.

---

## Step 3: pin tighter if reproducibility matters

Nix gives you exact-version reproducibility for free. With Jarvy, you opt in:

```toml
[provisioner]
node      = "=20.11.0"
python    = "=3.12.1"
terraform = "=1.6.6"

[drift]
enabled        = true
version_policy = "exact"
```

Combined with `jarvy drift accept` committing `.jarvy/state.json`, you get *team-level* version pinning. It's not bit-for-bit hermetic — Homebrew's `node@20.11.0` may have a different libc dependency than apt's — but it's far more reproducible than "install Node 20."

---

## Step 4: handle direnv users

If your team uses `direnv` + `use flake`, the workflow was: `cd` into the directory, the shell auto-updates. Jarvy replaces this with:

```bash
jarvy setup    # one-time provisioning per machine
```

After setup, the tools are on your normal PATH — no shell shim needed. If your team likes the auto-activation feel, you can keep direnv and have it call `jarvy setup --check`:

```bash title=".envrc"
jarvy doctor || jarvy setup
```

---

## Step 5: move flake-specific extras

Things in `flake.nix` that don't have direct Jarvy equivalents:

- **`nix run` scripts** → `[commands]` block in `jarvy.toml`
- **NixOS modules / system config** → out of scope for Jarvy; Jarvy is a dev tool, not a system config tool
- **Custom-built derivations** → use `pre_setup` hooks to clone and build, or contribute the tool to Jarvy's registry

---

## What you gain

- **Onboarding speed** — `jarvy setup` runs in seconds; first-time Nix on a new machine is minutes-to-hours
- **Familiar primitives** — TOML, not Nix expression language
- **Cross-platform** — Nix on Windows is WSL-only; Jarvy is first-class on Windows
- **No `/nix/store`** — you keep your normal PATH and your normal package managers
- **Easy contributions** — every developer on your team can already read `jarvy.toml`

## What you give up

- **Bit-for-bit reproducibility** — Jarvy gives version-pinned, drift-detected, but not hermetic
- **Atomic rollback** — Nix lets you revert a system; Jarvy doesn't sandbox installs
- **Cross-language packaging** — Nix can package anything; Jarvy is consumer-side, not packaging-side
- **The Nix ecosystem** — flakes, overlays, NixOS modules are not in scope

---

## A reasonable hybrid

If you want Nix's rigor for production and Jarvy's ergonomics for dev, run both:

- `flake.nix` for CI builds and production images
- `jarvy.toml` for laptop onboarding

The two configs target the same versions; teammates get the fast path, releases get the reproducible path.

---

## Migrate with AI

Paste this into Claude, ChatGPT, Cursor, or any LLM. Drop your `flake.nix` (or `shell.nix`) between the `<<<` and `>>>` markers.

````text title="Prompt: Nix → Jarvy"
You are a config translator. Convert the Nix flake.nix or shell.nix below
into a valid jarvy.toml file.

# Schema (jarvy.toml)
- Required: [provisioner] — table of registered tool names mapped to versions.
- Versions: "latest", "20", "^3.10", "~3.12", "=20.11.0".
- Detailed: tool = { version = "20", version_manager = true }
- Optional: [npm] [pip] [cargo] [hooks] [env.vars] [env.secrets]
  [git] [network] [drift] [services] [telemetry] [commands]

# Nix package → Jarvy tool name canonicalization
- nodejs / nodejs_20 / nodejs-20_x → node = "20"
- python / python3 / python312 / python3_12 → python = "3.12"
- python311 → python = "3.11"
- go / go_1_22 → go = "1.22"
- rustup / rustc → rust
- docker-client / docker → docker
- terraform / terraform_1 → terraform
- kubectl / kubernetes / kubernetes-helm → kubectl / helm
- postgresql / postgresql_16 → psql (Jarvy registers psql, not server)
- aws / awscli2 → awscli
- google-cloud-sdk → gcloud
- azure-cli → azure_cli

# What does NOT translate
- inputs (flake metadata, nixpkgs URL) → omit
- system targets (x86_64-linux) → omit
- pkgs imports → omit
- Custom-built derivations / overlays → flag with TOML comment as TODO
- nix run scripts → [commands] entry

# Per-source rules
- buildInputs / packages / nativeBuildInputs → [provisioner] entries
- shellHook → [hooks] post_setup. Convert any shell-builtin "export X=Y" lines
  inside shellHook into [env.vars] entries instead, and leave the rest in the hook.
- If the user wanted reproducibility, suggest [drift] version_policy = "exact"
  by adding it to the output (Nix → Jarvy is a strict-to-loose move; flag this)

# Output contract
- Output ONLY the jarvy.toml content. No prose, no fence.

# INPUT
<<<
[paste your flake.nix or shell.nix here]
>>>
````

After the model responds, run `jarvy validate` and `jarvy setup --dry-run`.

---

## See also

- [vs Nix](../competitors/vs-nix.md) — feature comparison
- [Configuration reference](../configuration.md)
- [Drift detection](../drift.md)
