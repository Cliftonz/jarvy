---
title: "Migrate: Vagrant → Jarvy"
description: "Move from a Vagrantfile to a native Jarvy setup. Translate provisioners, synced folders, and forwarded ports for teams that no longer need a full VM."
tags:
  - migrate
  - vm
  - vagrant

---

# Migrating from Vagrant to Jarvy

Vagrant runs your dev environment in a full virtual machine — a complete Linux box for each project. The original problem it solved (the Mac developer needing a Linux to match production) is now mostly handled by Docker, by macOS itself shipping with a Unix-like userland, or by simply running the production target as a container locally.

If your `Vagrantfile` is mostly there to install some packages and forward some ports, Jarvy does that without the VM — installs the same tools on your host, way faster, with way less RAM.

If your `Vagrantfile` is genuinely about **needing a full Linux kernel** (kernel modules, eBPF, custom networking), keep Vagrant. Jarvy doesn't run a kernel.

---

## Conceptual mapping

| `Vagrantfile` | `jarvy.toml` equivalent |
|---|---|
| `config.vm.box` | Not applicable — no VM |
| `config.vm.provision "shell"` | `[hooks] pre_setup` / `post_setup` |
| `config.vm.provision "ansible"` | `pre_setup` hook (run ansible from host) or migrate to native |
| `config.vm.network "forwarded_port"` | Not applicable — services bind to localhost |
| `config.vm.synced_folder` | Not applicable — your repo is your repo |
| `config.vm.provider "virtualbox"` | Not applicable |
| Vagrant plugins | Not applicable |
| Per-project IP / hostname | `[env.vars]` for `HOST=...` style |

---

## Step 1: read your Vagrantfile

A typical Vagrantfile that's mostly a bash provisioner:

```ruby title="Vagrantfile (before)"
Vagrant.configure("2") do |config|
  config.vm.box = "ubuntu/jammy64"
  config.vm.network "forwarded_port", guest: 3000, host: 3000
  config.vm.network "forwarded_port", guest: 5432, host: 5432
  config.vm.synced_folder ".", "/home/vagrant/app"

  config.vm.provision "shell", inline: <<-SHELL
    apt-get update
    apt-get install -y nodejs npm postgresql
    sudo -u vagrant -i bash -c 'cd /home/vagrant/app && npm ci'
  SHELL
end
```

---

## Step 2: write `jarvy.toml`

```toml title="jarvy.toml"
[provisioner]
node       = "20"
psql       = "latest"

[hooks]
post_setup = "npm ci"

[env.vars]
DATABASE_URL = "postgres://localhost:5432/myapp"
```

The forwarded ports go away — your app on `localhost:3000` is `localhost:3000`. The synced folder goes away — the repo is the repo.

---

## Step 3: handle the heavier `Vagrantfile` patterns

**Multi-machine setups** (e.g., one VM for the app, one for the database) — Jarvy doesn't replace this directly. Two options:

1. **Use containers for the data services:** Postgres in Docker via `[services]`, app native via `[provisioner]`. See [Configuration reference](../configuration.md).
2. **Stay on Vagrant for the multi-VM piece**, use Jarvy to provision the host (including Vagrant itself).

**Ansible / Puppet / Chef provisioners** — these were used because Vagrant's shell provisioner was awkward for complex setups. Jarvy's hooks are better-scoped (per-tool) and the `[provisioner]` table replaces a lot of "install package" work that Ansible was doing. For the genuinely complex configuration management you can:

- Run Ansible from a `pre_setup` hook against `localhost`
- Migrate the host-targeted config to Jarvy's primitives
- Keep Ansible for the production-side pieces only

**Custom kernel / sysctl / loadable modules** — Jarvy can't help. Stay on Vagrant if this is your real need.

---

## Step 4: replicate networking expectations

Vagrant's forwarded ports made guest services reachable on the host. Locally, your services bind directly. A few places this surfaces:

- **Database URL:** was `postgres://vagrant@localhost:5432/...` → now `postgres://$USER@localhost:5432/...`
- **Hostnames:** if you used `myapp.local` via a Vagrant plugin, switch to `127.0.0.1` or add an entry to `/etc/hosts`
- **Port conflicts:** two projects can't both bind `:3000`. Use `[env.vars]` to make the port configurable per project

```toml
[env.vars]
PORT         = "3000"
DATABASE_URL = "postgres://localhost:5432/myapp_dev"
```

---

## Step 5: shut off the VM

After `jarvy setup && jarvy doctor` is green:

```bash
vagrant halt
vagrant destroy
rm Vagrantfile
```

Reclaim the disk, reclaim the RAM, ship the change.

---

## What you gain

- **No VM overhead** — get GBs of RAM and a CPU back
- **Faster everything** — file I/O, compilation, test runs all native-speed
- **No `vagrant up` wait** — provisioning is seconds, not minutes
- **Editor talks directly to your code** — no SSH, no NFS sync lag
- **Cross-platform without VMs** — the same `jarvy.toml` works on macOS, Linux, Windows hosts
- **Drift detection** — Vagrant's idempotency story is "re-provision and hope"; Jarvy snapshots and reports

## What you give up

- **Full Linux kernel on macOS / Windows** — if you genuinely need eBPF, kernel modules, or specific network stack behavior, the VM was earning its keep. Stay on Vagrant.
- **Hard isolation** — VM is the strongest guarantee you can have on a laptop; Jarvy is process-level, not VM-level.
- **Multi-machine topology** — one Vagrantfile, two VMs is convenient. Replicating it requires Docker Compose or Jarvy's `[services]` block plus careful port management.

---

## Migrate with AI

Paste this into Claude, ChatGPT, Cursor, or any LLM. Drop your `Vagrantfile` between the `<<<` and `>>>` markers.

````text title="Prompt: Vagrant → Jarvy"
You are a config translator. Convert the Vagrantfile below into a valid
jarvy.toml file.

# Schema (jarvy.toml)
- Required: [provisioner] — table of registered tool names mapped to versions.
- Versions: "latest", "20", "^3.10", "~3.12", "=20.11.0".
- Detailed: tool = { version = "20", version_manager = true }
- Optional: [npm] [pip] [cargo] [hooks] [env.vars] [env.secrets]
  [git] [network] [drift] [services] [telemetry] [commands]

# Tool-name canonicalization
- nodejs → node, python3 → python, aws-cli → awscli, azure-cli → azure_cli
- postgresql / postgres → psql, visual-studio-code → vscode, golang → go

# What does NOT translate
- config.vm.box → no VM
- config.vm.network "forwarded_port" → services bind to localhost natively
- config.vm.synced_folder → filesystem is the filesystem
- config.vm.provider → no VM provider
- Vagrant plugins → not applicable
- Multi-machine setups (multiple config.vm.define) → flag with a TOML comment;
  Jarvy doesn't model multi-VM. Suggest [services] block + Docker for the
  data services and [provisioner] for the app machine.

# Per-source rules
- config.vm.provision "shell", inline: → split:
    apt-get install <tool>  → <tool> under [provisioner] (use canonical name)
    npm/pip/cargo install   → [npm]/[pip]/[cargo] block
    project-setup commands  → [hooks] post_setup
- config.vm.provision "ansible" → [hooks] pre_setup running ansible-playbook
  against localhost, OR a TODO comment to migrate ansible roles to native primitives
- Forwarded ports → [env.vars] PORT = "3000" so app can read it

# Output contract
- Output ONLY the jarvy.toml content. No prose, no fence.
- All hooks idempotent.
- If migration loses real semantics (e.g. multi-machine), preface the TOML
  output with a single TOML comment line beginning with "# WARN:" describing
  what was dropped.

# INPUT
<<<
[paste your Vagrantfile here]
>>>
````

After the model responds, run `jarvy validate` and `jarvy setup --dry-run`.

---

## See also

- [vs Vagrant](../competitors/vs-vagrant.md) — feature comparison
- [vs Docker](../competitors/vs-docker.md) — when you want isolation but not a full VM
- [Configuration reference](../configuration.md)
