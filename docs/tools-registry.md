---
title: "Tool registry (auto-generated) — Jarvy"
description: "Every tool Jarvy knows how to install — 260 entries spanning runtimes, build tools, cloud SDKs, container tools, security scanners, and editors."
tags:
  - reference
  - tools
---

# Tool registry

!!! info "Auto-generated"
    This page is generated from `jarvy tools --index` by `scripts/gen-docs.sh`. 
    Run that script after registering new tools.

Jarvy currently ships **260 tools**. Reference one in your `jarvy.toml` by its **name**.

| Name | Command | macOS | Linux | Windows | Default hook | Depends on |
|---|---|---|---|---|---|---|
| `act` | `act` | `brew: act` | — | `winget: nektos.act`<br>`choco: act-cli` |  | — |
| `actionlint` | `actionlint` | `brew: actionlint` | — | `winget: rhysd.actionlint` |  | — |
| `age` | `age` | `brew: age` | apt: `age`<br>dnf: `age`<br>pacman: `age`<br>apk: `age` | `winget: FiloSottile.age` |  | — |
| `air` | `air` | `brew: air` | apt: `air`<br>dnf: `air`<br>pacman: `air`<br>apk: `air` | `winget: cosmtrek.air` |  | — |
| `allure` | `allure` | `brew: allure` | — | — |  | — |
| `ansible` | `ansible` | `brew: ansible` | apt: `ansible`<br>dnf: `ansible`<br>pacman: `ansible`<br>apk: `ansible` | `choco: ansible` |  | — |
| `arctl` | `arctl` | — | — | — |  | — |
| `argo` | `argo` | `brew: argo` | — | — |  | `kubectl` |
| `argocd` | `argocd` | `brew: argocd` | apk: `argocd` | `winget: Argoproj.ArgoCD` | ✓ | `kubectl` |
| `aria2` | `aria2c` | `brew: aria2` | apt: `aria2`<br>dnf: `aria2`<br>pacman: `aria2`<br>apk: `aria2` | `winget: aria2.aria2` |  | — |
| `atlas` | `atlas` | `brew: ariga/tap/atlas` | apt: `atlas`<br>dnf: `atlas`<br>pacman: `atlas`<br>apk: `atlas` | `winget: Ariga.Atlas` |  | — |
| `atuin` | `atuin` | `brew: atuin` | apt: `atuin`<br>dnf: `atuin`<br>pacman: `atuin`<br>apk: `atuin` | `winget: atuinsh.atuin` | ✓ | — |
| `aws_sam_cli` | `sam` | `brew: aws-sam-cli` | — | `winget: Amazon.SAM-CLI` |  | — |
| `awscli` | `aws` | `brew: awscli` | apt: `awscli`<br>dnf: `awscli`<br>pacman: `aws-cli-v2`<br>apk: `aws-cli` | `winget: Amazon.AWSCLI` | ✓ | — |
| `azd` | `azd` | `brew: azure-dev` | apt: `azure-dev`<br>dnf: `azure-dev`<br>pacman: `azure-dev`<br>apk: `azure-dev` | `winget: Microsoft.Azd` |  | — |
| `azure_cli` | `az` | `brew: azure-cli` | apt: `azure-cli`<br>dnf: `azure-cli`<br>pacman: `azure-cli`<br>apk: `azure-cli` | `winget: Microsoft.AzureCLI` |  | — |
| `bacon` | `bacon` | — | — | — |  | `rust` |
| `bat` | `bat` | `brew: bat` | apt: `bat`<br>dnf: `bat`<br>pacman: `bat`<br>apk: `bat` | `winget: sharkdp.bat` | ✓ | — |
| `bazelisk` | `bazelisk` | `brew: bazelisk` | — | `winget: Bazel.Bazelisk` |  | — |
| `betterleaks` | `betterleaks` | `brew: betterleaks` | — | — | ✓ | `git` |
| `bicep` | `bicep` | `brew: bicep` | apt: `bicep`<br>dnf: `bicep`<br>pacman: `bicep`<br>apk: `bicep` | `winget: Microsoft.Bicep` |  | — |
| `bottom` | `btm` | `brew: bottom` | apt: `bottom`<br>dnf: `bottom`<br>pacman: `bottom`<br>apk: `bottom` | `winget: Clement.bottom` |  | — |
| `brew` | `brew` | — | — | — |  | — |
| `broot` | `broot` | `brew: broot` | apt: `broot`<br>dnf: `broot`<br>pacman: `broot`<br>apk: `broot` | `winget: Dystroy.broot` | ✓ | — |
| `btop` | `btop` | `brew: btop` | apt: `btop`<br>dnf: `btop`<br>pacman: `btop`<br>apk: `btop` | — |  | — |
| `buf` | `buf` | `brew: bufbuild/buf/buf` | apt: `buf`<br>dnf: `buf`<br>pacman: `buf`<br>apk: `buf` | `winget: Bufbuild.Buf` |  | — |
| `buildah` | `buildah` | `brew: buildah` | apt: `buildah`<br>dnf: `buildah`<br>pacman: `buildah`<br>apk: `buildah` | — |  | — |
| `bun` | `bun` | `brew: oven-sh/bun/bun` | — | `winget: Oven-sh.Bun` |  | — |
| `cargo_nextest` | `cargo-nextest` | — | — | — |  | `rust` |
| `cfn_lint` | `cfn-lint` | `brew: cfn-lint` | — | — |  | — |
| `checkov` | `checkov` | `brew: checkov` | — | — |  | — |
| `choose` | `choose` | `brew: choose-rust` | apt: `choose`<br>dnf: `choose`<br>pacman: `choose`<br>apk: `choose` | `winget: choose.choose` |  | — |
| `cloudflared` | `cloudflared` | `brew: cloudflared` | — | `winget: Cloudflare.cloudflared` |  | — |
| `cmake` | `cmake` | `brew: cmake` | apt: `cmake`<br>dnf: `cmake`<br>pacman: `cmake`<br>apk: `cmake` | `winget: Kitware.CMake` |  | — |
| `composer` | `composer` | `brew: composer` | apt: `composer`<br>dnf: `composer`<br>pacman: `composer`<br>apk: `composer` | — |  | — |
| `cosign` | `cosign` | `brew: cosign` | apk: `cosign` | `winget: sigstore.cosign` |  | — |
| `crane` | `crane` | `brew: crane` | apt: `crane`<br>dnf: `crane`<br>pacman: `crane`<br>apk: `crane` | — |  | — |
| `crystal` | `crystal` | `brew: crystal` | apt: `crystal`<br>dnf: `crystal`<br>pacman: `crystal`<br>apk: `crystal` | — |  | — |
| `cue` | `cue` | `brew: cue` | apt: `cue`<br>dnf: `cue`<br>pacman: `cue`<br>apk: `cue` | — |  | — |
| `curl` | `curl` | `brew: curl` | apt: `curl`<br>dnf: `curl`<br>pacman: `curl`<br>apk: `curl` | `winget: cURL.cURL` |  | — |
| `cursor` | `cursor` | `cask: cursor` | — | `winget: Cursor.Cursor` |  | — |
| `cypress` | `cypress` | — | — | — |  | `node`, `nvm` |
| `dagger` | `dagger` | `brew: dagger/tap/dagger` | apt: `dagger`<br>dnf: `dagger`<br>pacman: `dagger`<br>apk: `dagger` | `winget: Dagger.Dagger` |  | `docker`, `podman` |
| `dapr` | `dapr` | `brew: dapr/tap/dapr-cli` | apt: `dapr`<br>dnf: `dapr`<br>pacman: `dapr`<br>apk: `dapr` | `winget: Dapr.CLI` |  | `docker`, `podman` |
| `dbmate` | `dbmate` | `brew: dbmate` | — | `choco: dbmate` |  | — |
| `delta` | `delta` | `brew: git-delta` | apt: `git-delta`<br>dnf: `git-delta`<br>pacman: `git-delta`<br>apk: `git-delta` | `winget: dandavison.delta` | ✓ | — |
| `delve` | `dlv` | `brew: delve` | apt: `delve`<br>dnf: `delve`<br>pacman: `delve`<br>apk: `delve` | `winget: Go.Delve` |  | `go` |
| `deno` | `deno` | `brew: deno` | — | `winget: DenoLand.Deno` |  | — |
| `detect_secrets` | `detect-secrets` | `brew: detect-secrets` | — | — |  | — |
| `dfc` | `dfc` | `brew: chainguard-dev/tap/dfc` | — | — |  | — |
| `direnv` | `direnv` | `brew: direnv` | apt: `direnv`<br>dnf: `direnv`<br>pacman: `direnv`<br>apk: `direnv` | `winget: direnv.direnv` | ✓ | — |
| `dive` | `dive` | `brew: dive` | apk: `dive` | — |  | `docker`, `podman` |
| `docker` | `docker` | `cask: docker` | apt: `docker.io`<br>dnf: `docker`<br>pacman: `docker`<br>apk: `docker` | `winget: Docker.DockerDesktop` | ✓ | — |
| `docker_desktop` | `docker` | `cask: docker` | apt: `docker-desktop`<br>dnf: `docker-desktop`<br>pacman: `docker-desktop`<br>apk: `docker-desktop` | `winget: Docker.DockerDesktop` | ✓ | — |
| `dog` | `dog` | `brew: dog` | apt: `dog`<br>dnf: `dog`<br>pacman: `dog`<br>apk: `dog` | — |  | — |
| `dotnet` | `dotnet` | `cask: dotnet-sdk` | apt: `dotnet-sdk-8.0`<br>dnf: `dotnet-sdk`<br>pacman: `dotnet-sdk`<br>apk: `dotnet-sdk` | `winget: Microsoft.DotNet.SDK.8` | ✓ | — |
| `duckdb` | `duckdb` | `brew: duckdb` | apt: `duckdb`<br>dnf: `duckdb`<br>pacman: `duckdb`<br>apk: `duckdb` | `winget: DuckDB.cli`<br>`choco: duckdb` |  | — |
| `duf` | `duf` | `brew: duf` | apt: `duf`<br>dnf: `duf`<br>pacman: `duf`<br>apk: `duf` | `winget: muesli.duf` |  | — |
| `dust` | `dust` | `brew: dust` | apt: `du-dust`<br>dnf: `dust`<br>pacman: `dust`<br>apk: `dust` | `winget: bootandy.dust`<br>`choco: dust` |  | — |
| `earthly` | `earthly` | `brew: earthly/earthly/earthly` | apt: `earthly`<br>dnf: `earthly`<br>pacman: `earthly`<br>apk: `earthly` | `winget: Earthly.Earthly` |  | `docker`, `podman` |
| `eksctl` | `eksctl` | `brew: eksctl` | apt: `eksctl`<br>dnf: `eksctl`<br>pacman: `eksctl`<br>apk: `eksctl` | `winget: weaveworks.eksctl` | ✓ | — |
| `elixir` | `elixir` | `brew: elixir` | apt: `elixir`<br>dnf: `elixir`<br>pacman: `elixir`<br>apk: `elixir` | `winget: Elixir.Elixir` |  | `erlang` |
| `emacs` | `emacs` | `cask: emacs` | apt: `emacs`<br>dnf: `emacs`<br>pacman: `emacs`<br>apk: `emacs` | `winget: GNU.Emacs` |  | — |
| `emqx` | `emqx` | `brew: emqx` | apt: `emqx`<br>dnf: `emqx`<br>pacman: `emqx`<br>apk: `emqx` | — |  | — |
| `erlang` | `erl` | `brew: erlang` | apt: `erlang`<br>dnf: `erlang`<br>pacman: `erlang`<br>apk: `erlang` | `winget: Erlang.ErlangOTP`<br>`choco: erlang` |  | — |
| `eza` | `eza` | `brew: eza` | apt: `eza`<br>dnf: `eza`<br>pacman: `eza`<br>apk: `eza` | `winget: eza-community.eza` | ✓ | — |
| `fd` | `fd` | `brew: fd` | apt: `fd-find`<br>dnf: `fd-find`<br>pacman: `fd`<br>apk: `fd` | `winget: sharkdp.fd` | ✓ | — |
| `flux` | `flux` | `brew: fluxcd/tap/flux` | — | `winget: Fluxcd.Flux` | ✓ | — |
| `freelens` | `freelens` | `cask: freelens` | — | `winget: freelensapp.Freelens` |  | — |
| `fzf` | `fzf` | `brew: fzf` | apt: `fzf`<br>dnf: `fzf`<br>pacman: `fzf`<br>apk: `fzf` | `winget: junegunn.fzf` | ✓ | — |
| `gcloud` | `gcloud` | `cask: gcloud-cli` | apt: `google-cloud-cli`<br>dnf: `google-cloud-cli`<br>pacman: `google-cloud-cli`<br>apk: `google-cloud-sdk` | `winget: Google.CloudSDK` | ✓ | — |
| `gh` | `gh` | `brew: gh` | apt: `gh`<br>dnf: `gh`<br>pacman: `github-cli`<br>apk: `github-cli` | `winget: GitHub.cli` | ✓ | — |
| `git` | `git` | `brew: git` | apt: `git`<br>dnf: `git`<br>pacman: `git`<br>apk: `git` | `winget: Git.Git` | ✓ | — |
| `git_lfs` | `git-lfs` | `brew: git-lfs` | apt: `git-lfs`<br>dnf: `git-lfs`<br>pacman: `git-lfs`<br>apk: `git-lfs` | `winget: GitHub.GitLFS` |  | — |
| `git_secrets` | `git-secrets` | `brew: git-secrets` | apt: `git-secrets`<br>dnf: `git-secrets`<br>pacman: `git-secrets`<br>apk: `git-secrets` | — |  | `git` |
| `gitleaks` | `gitleaks` | `brew: gitleaks` | — | `winget: Gitleaks.Gitleaks` |  | `git` |
| `gitversion` | `gitversion` | `brew: gitversion` | apt: `gitversion`<br>dnf: `gitversion`<br>pacman: `gitversion`<br>apk: `gitversion` | `winget: GitTools.GitVersion` |  | `git` |
| `glab` | `glab` | `brew: glab` | apt: `glab`<br>dnf: `glab`<br>pacman: `glab`<br>apk: `glab` | `winget: GLab.GLab` | ✓ | — |
| `glances` | `glances` | `brew: glances` | apt: `glances`<br>dnf: `glances`<br>pacman: `glances`<br>apk: `glances` | — | ✓ | — |
| `gleam` | `gleam` | `brew: gleam` | apt: `gleam`<br>dnf: `gleam`<br>pacman: `gleam`<br>apk: `gleam` | `winget: Gleam.Gleam` |  | — |
| `go` | `go` | `brew: go` | apt: `golang`<br>dnf: `golang`<br>pacman: `go`<br>apk: `go` | `winget: GoLang.Go` | ✓ | — |
| `goaccess` | `goaccess` | `brew: goaccess` | apt: `goaccess`<br>dnf: `goaccess`<br>pacman: `goaccess`<br>apk: `goaccess` | — |  | — |
| `gofumpt` | `gofumpt` | `brew: gofumpt` | apt: `gofumpt`<br>dnf: `gofumpt`<br>pacman: `gofumpt`<br>apk: `gofumpt` | `winget: mvdan.gofumpt` |  | `go` |
| `golangci_lint` | `golangci-lint` | `brew: golangci-lint` | apt: `golangci-lint`<br>dnf: `golangci-lint`<br>pacman: `golangci-lint`<br>apk: `golangci-lint` | `winget: GolangCI.golangci-lint` |  | `go` |
| `gopls` | `gopls` | `brew: gopls` | apt: `gopls`<br>dnf: `gopls`<br>pacman: `gopls`<br>apk: `gopls` | `winget: Google.Gopls` |  | `go` |
| `goreleaser` | `goreleaser` | `brew: goreleaser` | apt: `goreleaser`<br>dnf: `goreleaser`<br>pacman: `goreleaser`<br>apk: `goreleaser` | `winget: GoReleaser.GoReleaser` |  | `go` |
| `gotestsum` | `gotestsum` | `brew: gotestsum` | apt: `gotestsum`<br>dnf: `gotestsum`<br>pacman: `gotestsum`<br>apk: `gotestsum` | `winget: gotestyourself.gotestsum` |  | `go` |
| `govulncheck` | `govulncheck` | `brew: govulncheck` | apt: `govulncheck`<br>dnf: `govulncheck`<br>pacman: `govulncheck`<br>apk: `govulncheck` | — |  | `go` |
| `gping` | `gping` | `brew: gping` | apt: `gping`<br>dnf: `gping`<br>pacman: `gping`<br>apk: `gping` | `winget: orf.gping` |  | — |
| `grafanactl` | `grafanactl` | `brew: grafanactl` | apt: `grafanactl`<br>dnf: `grafanactl`<br>pacman: `grafanactl`<br>apk: `grafanactl` | — |  | — |
| `grex` | `grex` | `brew: grex` | apt: `grex`<br>dnf: `grex`<br>pacman: `grex`<br>apk: `grex` | `winget: pemistahl.grex` |  | — |
| `grpcurl` | `grpcurl` | `brew: grpcurl` | apt: `grpcurl`<br>dnf: `grpcurl`<br>pacman: `grpcurl`<br>apk: `grpcurl` | `winget: fullstorydev.grpcurl` |  | — |
| `grype` | `grype` | `brew: grype` | apk: `grype` | `choco: grype` |  | — |
| `hadolint` | `hadolint` | `brew: hadolint` | — | `winget: hadolint.hadolint` |  | — |
| `haskell` | `ghc` | `brew: ghc` | apt: `ghc`<br>dnf: `ghc`<br>pacman: `ghc`<br>apk: `ghc` | `winget: Haskell.GHCup` |  | — |
| `headscale` | `headscale` | — | — | — |  | — |
| `helix` | `hx` | `brew: helix` | apt: `helix-editor`<br>dnf: `helix`<br>pacman: `helix`<br>apk: `helix` | `winget: Helix.Helix` |  | — |
| `helm` | `helm` | `brew: helm` | apt: `helm`<br>dnf: `helm`<br>pacman: `helm`<br>apk: `helm` | `winget: Helm.Helm` | ✓ | `kubectl` |
| `htop` | `htop` | `brew: htop` | apt: `htop`<br>dnf: `htop`<br>pacman: `htop`<br>apk: `htop` | — |  | — |
| `httpie` | `http` | `brew: httpie` | apt: `httpie`<br>dnf: `httpie`<br>pacman: `httpie`<br>apk: `py3-httpie` | `winget: HTTPie.HTTPie` |  | — |
| `hugo` | `hugo` | `brew: hugo` | apt: `hugo`<br>dnf: `hugo`<br>pacman: `hugo`<br>apk: `hugo` | `winget: Hugo.Hugo.Extended` |  | — |
| `hyperfine` | `hyperfine` | `brew: hyperfine` | apt: `hyperfine`<br>dnf: `hyperfine`<br>pacman: `hyperfine`<br>apk: `hyperfine` | `winget: sharkdp.hyperfine`<br>`choco: hyperfine` |  | — |
| `infisical` | `infisical` | `brew: infisical` | — | `winget: infisical.infisical` |  | — |
| `infracost` | `infracost` | `brew: infracost` | apt: `infracost`<br>dnf: `infracost`<br>pacman: `infracost`<br>apk: `infracost` | `winget: Infracost.Infracost` |  | — |
| `iterm2` | `iterm2` | `cask: iterm2` | — | — |  | — |
| `java` | `java` | `brew: openjdk` | apt: `default-jdk`<br>dnf: `java-latest-openjdk`<br>pacman: `jdk-openjdk`<br>apk: `openjdk21` | `winget: Microsoft.OpenJDK.21`<br>`choco: openjdk` | ✓ | — |
| `jetbrains_toolbox` | `jetbrains-toolbox` | `cask: jetbrains-toolbox` | — | `winget: JetBrains.Toolbox` |  | — |
| `jq` | `jq` | `brew: jq` | apt: `jq`<br>dnf: `jq`<br>pacman: `jq`<br>apk: `jq` | `winget: jqlang.jq` |  | — |
| `julia` | `julia` | `cask: julia` | apt: `julia`<br>dnf: `julia`<br>pacman: `julia`<br>apk: `julia` | `winget: Julialang.Julia` |  | — |
| `just` | `just` | `brew: just` | apt: `just`<br>dnf: `just`<br>pacman: `just`<br>apk: `just` | `winget: Casey.Just` | ✓ | — |
| `k3d` | `k3d` | `brew: k3d` | — | `winget: k3d-io.k3d` |  | `docker` |
| `k3s` | `k3s` | — | — | — |  | — |
| `k6` | `k6` | `brew: k6` | apt: `k6`<br>dnf: `k6`<br>pacman: `k6`<br>apk: `k6` | `winget: Grafana.k6` |  | — |
| `k9s` | `k9s` | `brew: derailed/k9s/k9s` | apt: `k9s`<br>dnf: `k9s`<br>pacman: `k9s`<br>apk: `k9s` | `winget: Derailed.k9s` | ✓ | `kubectl` |
| `kaf` | `kaf` | `brew: kaf` | — | — |  | — |
| `kafka` | `kafka-topics` | `brew: kafka` | — | — |  | — |
| `kafkactl` | `kafkactl` | `brew: deviceinsight/packages/kafkactl` | — | — |  | — |
| `kagent` | `kagent` | `brew: kagent` | — | — |  | `kubectl` |
| `kcat` | `kcat` | `brew: kcat` | apt: `kafkacat`<br>dnf: `kcat`<br>pacman: `kcat`<br>apk: `kcat` | — |  | — |
| `kind` | `kind` | `brew: kind` | — | `winget: Kubernetes.kind` | ✓ | `docker` |
| `kmcp` | `kmcp` | — | — | — |  | `kubectl` |
| `kn` | `kn` | `brew: kn` | — | — |  | `kubectl` |
| `ko` | `ko` | `brew: ko` | apt: `ko`<br>dnf: `ko`<br>pacman: `ko`<br>apk: `ko` | `winget: ko-build.ko` |  | `go` |
| `kotlin` | `kotlin` | `brew: kotlin` | apt: `kotlin`<br>dnf: `kotlin`<br>pacman: `kotlin`<br>apk: `kotlin` | `winget: JetBrains.Kotlin.Compiler`<br>`choco: kotlinc` |  | `java` |
| `krew` | `kubectl-krew` | `brew: krew` | apk: `kubectl-krew` | — | ✓ | `kubectl` |
| `kubectl` | `kubectl` | `brew: kubectl` | apt: `kubectl`<br>dnf: `kubectl`<br>pacman: `kubectl`<br>apk: `kubectl` | `winget: Kubernetes.kubectl` | ✓ | `minikube`, `kind`, `k3d`… |
| `kubectx` | `kubectx` | `brew: kubectx` | — | `winget: ahmetb.kubectx` | ✓ | — |
| `kubens` | `kubens` | `brew: kubectx` | — | — |  | — |
| `kubescape` | `kubescape` | `brew: kubescape` | apt: `kubescape`<br>dnf: `kubescape`<br>pacman: `kubescape`<br>apk: `kubescape` | `winget: kubescape.kubescape` |  | `kubectl` |
| `kustomize` | `kustomize` | `brew: kustomize` | apt: `kustomize`<br>dnf: `kustomize`<br>pacman: `kustomize`<br>apk: `kustomize` | `winget: Kubernetes.kustomize` |  | — |
| `lazydocker` | `lazydocker` | `brew: lazydocker` | apk: `lazydocker` | `choco: lazydocker` |  | `docker` |
| `lazygit` | `lazygit` | `brew: lazygit` | apt: `lazygit`<br>dnf: `lazygit`<br>pacman: `lazygit`<br>apk: `lazygit` | `winget: JesseDuffield.lazygit` | ✓ | — |
| `linkerd` | `linkerd` | `brew: linkerd` | — | — |  | `kubectl` |
| `litecli` | `litecli` | `brew: litecli` | apt: `litecli`<br>dnf: `litecli`<br>pacman: `litecli`<br>apk: `litecli` | `choco: litecli` |  | — |
| `litellm` | `litellm` | — | — | — |  | — |
| `lnav` | `lnav` | `brew: lnav` | apt: `lnav`<br>dnf: `lnav`<br>pacman: `lnav`<br>apk: `lnav` | — |  | — |
| `localstack` | `localstack` | `brew: localstack` | apt: `localstack`<br>dnf: `localstack`<br>pacman: `localstack`<br>apk: `localstack` | — |  | `docker`, `podman` |
| `locust` | `locust` | `brew: locust` | — | — |  | — |
| `lsd` | `lsd` | `brew: lsd` | apt: `lsd`<br>dnf: `lsd`<br>pacman: `lsd`<br>apk: `lsd` | `winget: lsd-rs.lsd` |  | — |
| `lua` | `lua` | `brew: lua` | apt: `lua5.4`<br>dnf: `lua`<br>pacman: `lua`<br>apk: `lua` | `winget: DEVCOM.Lua`<br>`choco: lua` |  | — |
| `luarocks` | `luarocks` | `brew: luarocks` | apt: `luarocks`<br>dnf: `luarocks`<br>pacman: `luarocks`<br>apk: `luarocks` | `winget: LuaRocks.LuaRocks` |  | `lua` |
| `lynis` | `lynis` | `brew: lynis` | apt: `lynis`<br>dnf: `lynis`<br>pacman: `lynis`<br>apk: `lynis` | — |  | — |
| `make` | `make` | `brew: make` | apt: `make`<br>dnf: `make`<br>pacman: `make`<br>apk: `make` | `winget: GnuWin32.Make` |  | — |
| `micro` | `micro` | `brew: micro` | apt: `micro`<br>dnf: `micro`<br>pacman: `micro`<br>apk: `micro` | `winget: zyedidia.micro` |  | — |
| `microk8s` | `microk8s` | — | — | — |  | — |
| `minikube` | `minikube` | `brew: minikube` | apt: `minikube`<br>dnf: `minikube`<br>pacman: `minikube`<br>apk: `minikube` | `winget: Kubernetes.minikube` | ✓ | `docker`, `podman` |
| `mise` | `mise` | `brew: mise` | — | `winget: jdx.mise` | ✓ | — |
| `mockgen` | `mockgen` | `brew: mockery` | apt: `mockery`<br>dnf: `mockery`<br>pacman: `mockery`<br>apk: `mockery` | `winget: vektra.mockery` |  | `go` |
| `molecule` | `molecule` | `brew: molecule` | apt: `molecule`<br>dnf: `molecule`<br>pacman: `molecule`<br>apk: `molecule` | — |  | `ansible` |
| `mongosh` | `mongosh` | `brew: mongosh` | — | `winget: MongoDB.Shell` |  | — |
| `mosquitto` | `mosquitto` | `brew: mosquitto` | apt: `mosquitto`<br>dnf: `mosquitto`<br>pacman: `mosquitto`<br>apk: `mosquitto` | `winget: EclipseFoundation.Mosquitto` |  | — |
| `mssql_cli` | `mssql-cli` | `brew: mssql-cli` | apt: `mssql-cli`<br>dnf: `mssql-cli`<br>pacman: `mssql-cli`<br>apk: `mssql-cli` | `winget: Microsoft.SqlServer.MssqlCli` |  | — |
| `mtr` | `mtr` | `brew: mtr` | apt: `mtr`<br>dnf: `mtr`<br>pacman: `mtr`<br>apk: `mtr` | — |  | — |
| `mycli` | `mycli` | `brew: mycli` | apt: `mycli`<br>dnf: `mycli`<br>pacman: `mycli`<br>apk: `mycli` | `choco: mycli` |  | — |
| `mysql` | `mysql` | `brew: mysql-client` | apt: `mysql-client`<br>dnf: `mysql`<br>pacman: `mysql`<br>apk: `mysql-client` | `winget: Oracle.MySQL` |  | — |
| `nats` | `nats` | `brew: nats-io/nats-tools/nats` | — | `winget: NATSAuthors.CLI` |  | — |
| `nats_server` | `nats-server` | `brew: nats-server` | apt: `nats-server`<br>dnf: `nats-server`<br>pacman: `nats-server`<br>apk: `nats-server` | — |  | — |
| `ncdu` | `ncdu` | `brew: ncdu` | apt: `ncdu`<br>dnf: `ncdu`<br>pacman: `ncdu`<br>apk: `ncdu` | — |  | — |
| `nebula` | `nebula` | `brew: nebula` | — | — |  | — |
| `nerdctl` | `nerdctl` | `brew: nerdctl` | apk: `nerdctl` | — |  | — |
| `netbird` | `netbird` | `brew: netbirdio/tap/netbird` | — | `winget: Netbird.Netbird` |  | — |
| `ngrok` | `ngrok` | `brew: ngrok` | apt: `ngrok`<br>dnf: `ngrok`<br>pacman: `ngrok`<br>apk: `ngrok` | `winget: Ngrok.Ngrok` |  | — |
| `nim` | `nim` | `brew: nim` | apt: `nim`<br>dnf: `nim`<br>pacman: `nim`<br>apk: `nim` | `winget: Nim.Nim` |  | — |
| `nmap` | `nmap` | `brew: nmap` | apt: `nmap`<br>dnf: `nmap`<br>pacman: `nmap`<br>apk: `nmap` | `winget: Insecure.Nmap` |  | — |
| `node` | `node` | `brew: node` | apt: `nodejs`<br>dnf: `nodejs`<br>pacman: `nodejs`<br>apk: `nodejs` | `winget: OpenJS.NodeJS.LTS` | ✓ | `nvm` |
| `noseyparker` | `noseyparker` | `brew: noseyparker` | — | — |  | — |
| `nsc` | `nsc` | `brew: nats-io/nats-tools/nsc` | — | `winget: NATSAuthors.nsc` |  | — |
| `nushell` | `nu` | `brew: nushell` | apt: `nushell`<br>dnf: `nushell`<br>pacman: `nushell`<br>apk: `nushell` | `winget: Nushell.Nushell` |  | — |
| `nvim` | `nvim` | `brew: neovim` | apt: `neovim`<br>dnf: `neovim`<br>pacman: `neovim`<br>apk: `neovim` | `winget: Neovim.Neovim` | ✓ | — |
| `nvm` | `nvm` | — | — | — |  | — |
| `ocaml` | `ocaml` | `brew: ocaml` | apt: `ocaml`<br>dnf: `ocaml`<br>pacman: `ocaml`<br>apk: `ocaml` | `winget: OCaml.OCaml` |  | — |
| `ollama` | `ollama` | `brew: ollama` | — | `winget: Ollama.Ollama` |  | — |
| `omnictl` | `omnictl` | `brew: siderolabs/tap/omnictl` | — | `winget: Sidero.omnictl` |  | — |
| `openclaw` | `openclaw` | `brew: openclaw-cli` | apt: `openclaw-cli`<br>dnf: `openclaw-cli`<br>pacman: `openclaw-cli`<br>apk: `openclaw-cli` | — |  | — |
| `openssh` | `ssh` | `brew: openssh` | apt: `openssh`<br>dnf: `openssh`<br>pacman: `openssh`<br>apk: `openssh` | `winget: Microsoft.OpenSSH.Beta` |  | — |
| `opentofu` | `tofu` | `brew: opentofu` | apt: `opentofu`<br>dnf: `opentofu`<br>pacman: `opentofu`<br>apk: `opentofu` | `winget: OpenTofu.OpenTofu` |  | — |
| `openvpn` | `openvpn` | `brew: openvpn` | apt: `openvpn`<br>dnf: `openvpn`<br>pacman: `openvpn`<br>apk: `openvpn` | `winget: OpenVPNTechnologies.OpenVPN` |  | — |
| `oras` | `oras` | `brew: oras` | apt: `oras`<br>dnf: `oras`<br>pacman: `oras`<br>apk: `oras` | `winget: oras-project.oras` |  | — |
| `p7zip` | `7z` | `brew: p7zip` | apt: `p7zip`<br>dnf: `p7zip`<br>pacman: `p7zip`<br>apk: `p7zip` | `winget: 7zip.7zip` |  | — |
| `packer` | `packer` | `brew: packer` | apt: `packer`<br>dnf: `packer`<br>pacman: `packer`<br>apk: `packer` | `winget: HashiCorp.Packer` |  | — |
| `pgcli` | `pgcli` | `brew: pgcli` | apt: `pgcli`<br>dnf: `pgcli`<br>pacman: `pgcli`<br>apk: `pgcli` | `choco: pgcli` |  | — |
| `php` | `php` | `brew: php` | apt: `php`<br>dnf: `php`<br>pacman: `php`<br>apk: `php` | `winget: PHP.PHP` |  | — |
| `playwright` | `playwright` | — | — | — |  | `node`, `nvm` |
| `pnpm` | `pnpm` | `brew: pnpm` | — | `winget: pnpm.pnpm` |  | — |
| `podman` | `podman` | `brew: podman` | apt: `podman`<br>dnf: `podman`<br>pacman: `podman`<br>apk: `podman` | `winget: RedHat.Podman` |  | — |
| `podman_desktop` | `podman-desktop` | `cask: podman-desktop` | — | `winget: RedHat.Podman-Desktop` |  | — |
| `powershell` | `pwsh` | `cask: powershell` | apt: `powershell`<br>dnf: `powershell`<br>pacman: `powershell`<br>apk: `powershell` | `winget: Microsoft.PowerShell` |  | — |
| `pre_commit` | `pre-commit` | `brew: pre-commit` | apt: `pre-commit`<br>dnf: `pre-commit`<br>pacman: `pre-commit`<br>apk: `pre-commit` | `choco: pre-commit` |  | — |
| `procs` | `procs` | `brew: procs` | apt: `procs`<br>dnf: `procs`<br>pacman: `procs`<br>apk: `procs` | `winget: dalance.procs` |  | — |
| `psql` | `psql` | `brew: postgresql` | apt: `postgresql-client`<br>dnf: `postgresql`<br>pacman: `postgresql`<br>apk: `postgresql-client` | `winget: PostgreSQL.PostgreSQL` |  | — |
| `pulsar` | `pulsar` | `brew: apache-pulsar` | — | — |  | — |
| `pulumi` | `pulumi` | `brew: pulumi` | apt: `pulumi`<br>dnf: `pulumi`<br>pacman: `pulumi`<br>apk: `pulumi` | `winget: Pulumi.Pulumi` |  | — |
| `putty` | `putty` | `brew: putty` | apt: `putty`<br>dnf: `putty`<br>pacman: `putty`<br>apk: `putty` | `winget: PuTTY.PuTTY` |  | — |
| `pyenv` | `pyenv` | `brew: pyenv` | — | — | ✓ | — |
| `python` | `python3` | `brew: python` | apt: `python3`<br>dnf: `python3`<br>pacman: `python`<br>apk: `python3` | `winget: Python.Python.3` | ✓ | `pyenv` |
| `rabbitmq` | `rabbitmq-server` | `brew: rabbitmq` | apt: `rabbitmq-server`<br>dnf: `rabbitmq-server`<br>pacman: `rabbitmq-server`<br>apk: `rabbitmq-server` | — |  | — |
| `rancher_desktop` | `rdctl` | `cask: rancher` | — | `winget: suse.RancherDesktop` |  | — |
| `rbenv` | `rbenv` | `brew: rbenv` | — | — | ✓ | — |
| `rclone` | `rclone` | `brew: rclone` | apt: `rclone`<br>dnf: `rclone`<br>pacman: `rclone`<br>apk: `rclone` | `winget: Rclone.Rclone` |  | — |
| `redis` | `redis-cli` | `brew: redis` | apt: `redis`<br>dnf: `redis`<br>pacman: `redis`<br>apk: `redis` | `winget: Redis.Redis` |  | — |
| `release_plz` | `release-plz` | — | — | — |  | `rust` |
| `ripgrep` | `rg` | `brew: ripgrep` | apt: `ripgrep`<br>dnf: `ripgrep`<br>pacman: `ripgrep`<br>apk: `ripgrep` | `winget: BurntSushi.ripgrep.MSVC` | ✓ | — |
| `rpk` | `rpk` | `brew: redpanda-data/tap/redpanda` | — | — |  | — |
| `ruby` | `ruby` | `brew: ruby` | apt: `ruby`<br>dnf: `ruby`<br>pacman: `ruby`<br>apk: `ruby` | `winget: RubyInstallerTeam.Ruby` |  | `rbenv` |
| `ruff` | `ruff` | `brew: ruff` | apk: `ruff` | `winget: astral-sh.ruff` |  | — |
| `rust` | `rustc` | — | — | — | ✓ | — |
| `scala` | `scala` | `brew: scala` | apt: `scala`<br>dnf: `scala`<br>pacman: `scala`<br>apk: `scala` | `winget: Scala.Scala.3`<br>`choco: scala` |  | `java` |
| `sd` | `sd` | `brew: sd` | — | `choco: sd-cli` |  | — |
| `sdkman` | `sdk` | `brew: sdkman-cli` | apt: `sdkman`<br>dnf: `sdkman`<br>pacman: `sdkman`<br>apk: `sdkman` | — | ✓ | — |
| `semgrep` | `semgrep` | `brew: semgrep` | — | — |  | — |
| `shellcheck` | `shellcheck` | `brew: shellcheck` | apt: `shellcheck`<br>dnf: `shellcheck`<br>pacman: `shellcheck`<br>apk: `shellcheck` | `winget: koalaman.shellcheck` |  | — |
| `shfmt` | `shfmt` | `brew: shfmt` | apt: `shfmt`<br>dnf: `shfmt`<br>pacman: `shfmt`<br>apk: `shfmt` | `winget: mvdan.shfmt` |  | — |
| `skaffold` | `skaffold` | `brew: skaffold` | — | `winget: Google.ContainerTools.Skaffold` |  | — |
| `skopeo` | `skopeo` | `brew: skopeo` | apt: `skopeo`<br>dnf: `skopeo`<br>pacman: `skopeo`<br>apk: `skopeo` | — |  | — |
| `sonar_scanner` | `sonar-scanner` | `brew: sonar-scanner` | apt: `sonar-scanner`<br>dnf: `sonar-scanner`<br>pacman: `sonar-scanner`<br>apk: `sonar-scanner` | — |  | — |
| `sops` | `sops` | `brew: sops` | — | `winget: Mozilla.sops` |  | — |
| `sqlite` | `sqlite3` | `brew: sqlite` | apt: `sqlite3`<br>dnf: `sqlite3`<br>pacman: `sqlite3`<br>apk: `sqlite3` | `winget: SQLite.SQLite` |  | — |
| `starship` | `starship` | `brew: starship` | apt: `starship`<br>dnf: `starship`<br>pacman: `starship`<br>apk: `starship` | `winget: Starship.Starship` | ✓ | — |
| `stern` | `stern` | `brew: stern` | apt: `stern`<br>dnf: `stern`<br>pacman: `stern`<br>apk: `stern` | `winget: stern.stern` |  | `kubectl` |
| `structurizr` | `structurizr` | `brew: structurizr` | — | — |  | — |
| `syft` | `syft` | `brew: syft` | apk: `syft` | `choco: syft` |  | — |
| `tailscale` | `tailscale` | `brew: tailscale` | — | `winget: Tailscale.Tailscale` |  | — |
| `talisman` | `talisman` | `brew: talisman` | — | — |  | `git` |
| `talosctl` | `talosctl` | `brew: siderolabs/tap/talosctl` | apt: `talosctl`<br>dnf: `talosctl`<br>pacman: `talosctl`<br>apk: `talosctl` | `winget: SideroLabs.talosctl` |  | — |
| `task` | `task` | `brew: go-task` | — | `winget: Task.Task` |  | — |
| `temporal` | `temporal` | `brew: temporal` | — | — |  | — |
| `terraform` | `terraform` | `brew: terraform` | apt: `terraform`<br>dnf: `terraform`<br>pacman: `terraform`<br>apk: `terraform` | `winget: HashiCorp.Terraform` | ✓ | — |
| `terraform_docs` | `terraform-docs` | `brew: terraform-docs` | — | `choco: terraform-docs` |  | — |
| `terragrunt` | `terragrunt` | `brew: terragrunt` | — | `winget: Gruntwork.Terragrunt`<br>`choco: terragrunt` |  | — |
| `tfsec` | `tfsec` | `brew: tfsec` | apt: `tfsec`<br>dnf: `tfsec`<br>pacman: `tfsec`<br>apk: `tfsec` | `winget: aquasecurity.tfsec` |  | — |
| `tilt` | `tilt` | `brew: tilt` | apt: `tilt`<br>dnf: `tilt`<br>pacman: `tilt`<br>apk: `tilt` | — |  | — |
| `tmux` | `tmux` | `brew: tmux` | apt: `tmux`<br>dnf: `tmux`<br>pacman: `tmux`<br>apk: `tmux` | — | ✓ | — |
| `todoist` | `todoist` | `brew: todoist-cli-go` | — | — |  | — |
| `tokei` | `tokei` | `brew: tokei` | apt: `tokei`<br>dnf: `tokei`<br>pacman: `tokei`<br>apk: `tokei` | `winget: XAMPPRocky.tokei`<br>`choco: tokei` |  | — |
| `tree` | `tree` | `brew: tree` | apt: `tree`<br>dnf: `tree`<br>pacman: `tree`<br>apk: `tree` | — |  | — |
| `trivy` | `trivy` | `brew: trivy` | apk: `trivy` | `winget: aquasecurity.trivy` |  | — |
| `trufflehog` | `trufflehog` | `brew: trufflehog` | — | — |  | `git` |
| `twingate` | `twingate` | `cask: twingate` | — | `winget: Twingate.Client` |  | — |
| `up` | `up` | `brew: upbound/tap/up` | — | — |  | — |
| `usql` | `usql` | `brew: usql` | — | `choco: usql` |  | — |
| `uv` | `uv` | `brew: uv` | apk: `uv` | `winget: astral-sh.uv` |  | — |
| `vagrant` | `vagrant` | `cask: vagrant` | apt: `vagrant`<br>dnf: `vagrant`<br>pacman: `vagrant`<br>apk: `vagrant` | `winget: Hashicorp.Vagrant`<br>`choco: vagrant` |  | — |
| `vault` | `vault` | `brew: vault` | apt: `vault`<br>dnf: `vault`<br>pacman: `vault`<br>apk: `vault` | `winget: HashiCorp.Vault` |  | — |
| `vfox` | `vfox` | `brew: vfox` | — | `winget: vfox` | ✓ | — |
| `vim` | `vim` | `brew: vim` | apt: `vim`<br>dnf: `vim-enhanced`<br>pacman: `vim`<br>apk: `vim` | `winget: vim.vim` |  | — |
| `vllm` | `vllm` | — | — | — |  | — |
| `vscode` | `code` | `cask: visual-studio-code` | apt: `code`<br>dnf: `code`<br>pacman: `code`<br>apk: `code` | `winget: Microsoft.VisualStudioCode` |  | — |
| `watchexec` | `watchexec` | `brew: watchexec` | apt: `watchexec`<br>dnf: `watchexec`<br>pacman: `watchexec`<br>apk: `watchexec` | `winget: watchexec.watchexec`<br>`choco: watchexec` |  | — |
| `wget` | `wget` | `brew: wget` | apt: `wget`<br>dnf: `wget`<br>pacman: `wget`<br>apk: `wget` | `winget: GnuWin32.Wget` |  | — |
| `wireguard_tools` | `wg` | `brew: wireguard-tools` | apt: `wireguard-tools`<br>dnf: `wireguard-tools`<br>pacman: `wireguard-tools`<br>apk: `wireguard-tools` | `winget: WireGuard.WireGuard` |  | — |
| `xz` | `xz` | `brew: xz` | apt: `xz`<br>dnf: `xz`<br>pacman: `xz`<br>apk: `xz` | `winget: XZUtils.XZ` |  | — |
| `yamllint` | `yamllint` | `brew: yamllint` | apt: `yamllint`<br>dnf: `yamllint`<br>pacman: `yamllint`<br>apk: `py3-yamllint` | `winget: adrienverge.yamllint` |  | — |
| `yarn` | `yarn` | `brew: yarn` | — | `winget: Yarn.Yarn` |  | — |
| `yazi` | `yazi` | `brew: yazi` | apt: `yazi`<br>dnf: `yazi`<br>pacman: `yazi`<br>apk: `yazi` | `winget: sxyazi.yazi` |  | — |
| `yq` | `yq` | `brew: yq` | apt: `yq`<br>dnf: `yq`<br>pacman: `yq`<br>apk: `yq` | `winget: mikefarah.yq` |  | — |
| `zed` | `zed` | `cask: zed` | — | — |  | — |
| `zerotier` | `zerotier-cli` | `cask: zerotier-one` | — | `winget: ZeroTier.ZeroTierOne` |  | — |
| `zig` | `zig` | `brew: zig` | apt: `zig`<br>dnf: `zig`<br>pacman: `zig`<br>apk: `zig` | `winget: zig.zig`<br>`choco: zig` |  | — |
| `zoxide` | `zoxide` | `brew: zoxide` | apt: `zoxide`<br>dnf: `zoxide`<br>pacman: `zoxide`<br>apk: `zoxide` | `winget: ajeetdsouza.zoxide` | ✓ | — |
| `zsh` | `zsh` | `brew: zsh` | apt: `zsh`<br>dnf: `zsh`<br>pacman: `zsh`<br>apk: `zsh` | — |  | — |

---

## Don't see what you need?

- Try a fuzzy search: `jarvy search <name>`
- Add a new tool — see [Adding tools](adding-tools.md) for the macro and PR flow.
