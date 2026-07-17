---
title: "Tool directory — search 260 tools Jarvy can install"
description: "Search every tool Jarvy installs and see the exact install command for macOS (brew), Linux (apt, dnf, pacman, apk), Windows (winget, choco), and BSD."
hide:
  - toc
tags:
  - reference
  - tools
---

# Tool directory

!!! info "Auto-generated"
    This page is generated from `jarvy tools --index` by `scripts/gen-docs.sh`
    and rebuilt on every docs deploy. Do not edit by hand.

Jarvy installs **260 tools** with one `jarvy setup`. Search below, or use
`jarvy search <name>` from the CLI. Prefer a plain table? See the
[tool registry](tools-registry.md).

And that's just the built-ins — Jarvy also installs any
[npm, pip, cargo, nuget, gem, or go package](packages.md), plus custom tools
via the [plugin registry](registry-remote.md).

<noscript>This directory needs JavaScript — use the
<a href="https://jarvy.dev/tools-registry/">static tool registry table</a> instead.</noscript>

<div id="jt-app">
  <div class="jt-controls">
    <input id="jt-search" type="search" placeholder="Search tools, commands, or package names…"
           autocomplete="off" spellcheck="false" aria-label="Search tools">
    <select id="jt-os" aria-label="Filter by operating system">
      <option value="all">Any OS</option>
      <option value="macos">macOS</option>
      <option value="linux">Linux</option>
      <option value="windows">Windows</option>
      <option value="bsd">BSD</option>
    </select>
    <div id="jt-cats" role="group" aria-label="Filter by category"></div>
  </div>
  <p id="jt-count" aria-live="polite"></p>
  <div id="jt-list"></div>
</div>

<style>
#jt-app { margin-top: .5rem; }
.jt-controls { display: flex; flex-wrap: wrap; gap: .5rem; align-items: center; }
#jt-search {
  flex: 1 1 16rem; padding: .55rem .8rem; font-size: .8rem;
  border: 1px solid var(--md-default-fg-color--lighter);
  border-radius: .2rem; background: var(--md-default-bg-color);
  color: var(--md-default-fg-color);
}
#jt-search:focus { outline: none; border-color: var(--md-accent-fg-color); }
#jt-os {
  padding: .5rem .6rem; font-size: .75rem; border-radius: .2rem;
  border: 1px solid var(--md-default-fg-color--lighter);
  background: var(--md-default-bg-color); color: var(--md-default-fg-color);
}
#jt-cats { display: flex; flex-wrap: wrap; gap: .3rem; }
#jt-cats button {
  padding: .25rem .6rem; font-size: .7rem; border-radius: 1rem; cursor: pointer;
  border: 1px solid var(--md-default-fg-color--lighter);
  background: transparent; color: var(--md-default-fg-color--light);
}
#jt-cats button.jt-on {
  background: var(--md-accent-fg-color); border-color: var(--md-accent-fg-color);
  color: var(--md-accent-bg-color);
}
#jt-count { font-size: .7rem; color: var(--md-default-fg-color--light); margin: .6rem 0 .4rem; }
/* Neutralize Material's admonition-style <details> theming (icon,
   primary-color border, shadow) — these are plain cards. */
.md-typeset .jt-card {
  border: 1px solid var(--md-default-fg-color--lightest);
  border-radius: .2rem; margin: 0 0 .4rem; overflow: hidden;
  box-shadow: none; background: none; font-size: inherit;
}
.md-typeset .jt-card > summary {
  display: flex; flex-wrap: wrap; gap: .5rem; align-items: center;
  padding: .5rem 2rem .5rem .8rem; cursor: pointer; list-style: none;
  background: none; border: none;
}
.md-typeset .jt-card > summary::before { display: none; }
.md-typeset .jt-card > summary::-webkit-details-marker { display: none; }
.md-typeset .jt-card > summary:hover { background: var(--md-code-bg-color); }
.jt-name { font-family: var(--md-code-font-family, monospace); font-weight: 700; font-size: .8rem; }
.jt-badges { display: flex; gap: .25rem; margin-left: auto; }
.jt-badge, .jt-cat {
  font-size: .6rem; padding: .1rem .45rem; border-radius: 1rem;
  border: 1px solid var(--md-default-fg-color--lighter);
  color: var(--md-default-fg-color--light); white-space: nowrap;
}
.jt-cat { border-style: dashed; }
.jt-body { padding: .2rem .8rem .8rem; border-top: 1px solid var(--md-default-fg-color--lightest); }
.jt-body h5 {
  margin: .8rem 0 .25rem; font-size: .65rem; text-transform: uppercase;
  letter-spacing: .05em; color: var(--md-default-fg-color--light);
}
.jt-cmd {
  display: flex; align-items: center; gap: .5rem;
  background: var(--md-code-bg-color); border-radius: .15rem;
  padding: .35rem .6rem; margin: .2rem 0;
}
.jt-cmd code {
  flex: 1; background: none; padding: 0; font-size: .7rem;
  overflow-x: auto; white-space: pre;
}
.jt-cmd .jt-pm { font-size: .6rem; color: var(--md-default-fg-color--light); min-width: 4.5rem; }
.jt-copy {
  border: none; background: none; cursor: pointer; font-size: .7rem;
  color: var(--md-default-fg-color--light); padding: 0 .2rem;
}
.jt-copy:hover { color: var(--md-accent-fg-color); }
.jt-note { font-size: .7rem; color: var(--md-default-fg-color--light); margin: .3rem 0; }
.jt-empty { padding: 1.5rem; text-align: center; color: var(--md-default-fg-color--light); font-size: .8rem; }
</style>

<script id="jarvy-tools-data" type="application/json">[{"n":"act","c":"act","brew":"act","lbrew":"act","winget":"nektos.act","choco":"act-cli","pkg":"act"},{"n":"actionlint","c":"actionlint","brew":"actionlint","lbrew":"actionlint","winget":"rhysd.actionlint","pkg":"actionlint"},{"n":"age","c":"age","brew":"age","apt":"age","dnf":"age","pacman":"age","apk":"age","winget":"FiloSottile.age","pkg":"age"},{"n":"air","c":"air","brew":"air","apt":"air","dnf":"air","pacman":"air","apk":"air","winget":"cosmtrek.air","pkg":"air"},{"n":"allure","c":"allure","cat":"testing","brew":"allure","lbrew":"allure"},{"n":"ansible","c":"ansible","brew":"ansible","apt":"ansible","dnf":"ansible","pacman":"ansible","apk":"ansible","choco":"ansible","pkg":"py39-ansible"},{"n":"arctl","c":"arctl","custom":true},{"n":"argo","c":"argo","cat":"workflow","brew":"argo","lbrew":"argo","flex":["kubectl"]},{"n":"argocd","c":"argocd","brew":"argocd","apk":"argocd","lbrew":"argocd","winget":"Argoproj.ArgoCD","pkg":"argocd","flex":["kubectl"],"hook":"Install argocd shell completions for bash and zsh"},{"n":"aria2","c":"aria2c","brew":"aria2","apt":"aria2","dnf":"aria2","pacman":"aria2","apk":"aria2","winget":"aria2.aria2","pkg":"aria2"},{"n":"atlas","c":"atlas","brew":"ariga/tap/atlas","apt":"atlas","dnf":"atlas","pacman":"atlas","apk":"atlas","winget":"Ariga.Atlas","pkg":"atlas"},{"n":"atuin","c":"atuin","brew":"atuin","apt":"atuin","dnf":"atuin","pacman":"atuin","apk":"atuin","winget":"atuinsh.atuin","pkg":"atuin","hook":"Add atuin shell initialization to .bashrc and .zshrc"},{"n":"aws_sam_cli","c":"sam","brew":"aws-sam-cli","lbrew":"aws-sam-cli","winget":"Amazon.SAM-CLI"},{"n":"awscli","c":"aws","brew":"awscli","apt":"awscli","dnf":"awscli","pacman":"aws-cli-v2","apk":"aws-cli","winget":"Amazon.AWSCLI","pkg":"awscli","hook":"Configure AWS CLI shell completion"},{"n":"azd","c":"azd","brew":"azure-dev","apt":"azure-dev","dnf":"azure-dev","pacman":"azure-dev","apk":"azure-dev","winget":"Microsoft.Azd"},{"n":"azure_cli","c":"az","brew":"azure-cli","apt":"azure-cli","dnf":"azure-cli","pacman":"azure-cli","apk":"azure-cli","winget":"Microsoft.AzureCLI","pkg":"py39-azure-cli"},{"n":"bacon","c":"bacon","custom":true,"deps":["rust"]},{"n":"bat","c":"bat","brew":"bat","apt":"bat","dnf":"bat","pacman":"bat","apk":"bat","winget":"sharkdp.bat","pkg":"bat","hook":"Configure bat as MANPAGER for colored man pages"},{"n":"bazelisk","c":"bazelisk","brew":"bazelisk","lbrew":"bazelisk","winget":"Bazel.Bazelisk"},{"n":"betterleaks","c":"betterleaks","brew":"betterleaks","lbrew":"betterleaks","deps":["git"],"hook":"Install git pre-push hook to scan for secrets before each push"},{"n":"bicep","c":"bicep","brew":"bicep","apt":"bicep","dnf":"bicep","pacman":"bicep","apk":"bicep","winget":"Microsoft.Bicep"},{"n":"bottom","c":"btm","brew":"bottom","apt":"bottom","dnf":"bottom","pacman":"bottom","apk":"bottom","winget":"Clement.bottom","pkg":"bottom"},{"n":"brew","c":"brew","custom":true},{"n":"broot","c":"broot","brew":"broot","apt":"broot","dnf":"broot","pacman":"broot","apk":"broot","winget":"Dystroy.broot","pkg":"broot","hook":"Add broot shell function to .bashrc and .zshrc"},{"n":"btop","c":"btop","brew":"btop","apt":"btop","dnf":"btop","pacman":"btop","apk":"btop","pkg":"btop"},{"n":"buf","c":"buf","brew":"bufbuild/buf/buf","apt":"buf","dnf":"buf","pacman":"buf","apk":"buf","winget":"Bufbuild.Buf","pkg":"buf"},{"n":"buildah","c":"buildah","brew":"buildah","apt":"buildah","dnf":"buildah","pacman":"buildah","apk":"buildah","pkg":"buildah"},{"n":"bun","c":"bun","brew":"oven-sh/bun/bun","lbrew":"oven-sh/bun/bun","winget":"Oven-sh.Bun","pkg":"bun"},{"n":"cargo_nextest","c":"cargo-nextest","custom":true,"deps":["rust"]},{"n":"cfn_lint","c":"cfn-lint","brew":"cfn-lint","lbrew":"cfn-lint"},{"n":"checkov","c":"checkov","brew":"checkov","lbrew":"checkov","pkg":"py39-checkov"},{"n":"choose","c":"choose","brew":"choose-rust","apt":"choose","dnf":"choose","pacman":"choose","apk":"choose","winget":"choose.choose","pkg":"choose"},{"n":"cloudflared","c":"cloudflared","cat":"networking","brew":"cloudflared","lbrew":"cloudflared","winget":"Cloudflare.cloudflared"},{"n":"cmake","c":"cmake","brew":"cmake","apt":"cmake","dnf":"cmake","pacman":"cmake","apk":"cmake","winget":"Kitware.CMake","pkg":"cmake"},{"n":"composer","c":"composer","brew":"composer","apt":"composer","dnf":"composer","pacman":"composer","apk":"composer"},{"n":"cosign","c":"cosign","brew":"cosign","apk":"cosign","lbrew":"cosign","winget":"sigstore.cosign","pkg":"cosign"},{"n":"crane","c":"crane","brew":"crane","apt":"crane","dnf":"crane","pacman":"crane","apk":"crane"},{"n":"crystal","c":"crystal","brew":"crystal","apt":"crystal","dnf":"crystal","pacman":"crystal","apk":"crystal","pkg":"crystal"},{"n":"cue","c":"cue","brew":"cue","apt":"cue","dnf":"cue","pacman":"cue","apk":"cue","pkg":"cue"},{"n":"curl","c":"curl","brew":"curl","apt":"curl","dnf":"curl","pacman":"curl","apk":"curl","winget":"cURL.cURL","pkg":"curl"},{"n":"cursor","c":"cursor","cask":"cursor","winget":"Cursor.Cursor","custom":true},{"n":"cypress","c":"cypress","cat":"testing","custom":true,"flex":["node","nvm"]},{"n":"dagger","c":"dagger","brew":"dagger/tap/dagger","apt":"dagger","dnf":"dagger","pacman":"dagger","apk":"dagger","winget":"Dagger.Dagger","pkg":"dagger","flex":["docker","podman"]},{"n":"dapr","c":"dapr","brew":"dapr/tap/dapr-cli","apt":"dapr","dnf":"dapr","pacman":"dapr","apk":"dapr","winget":"Dapr.CLI","flex":["docker","podman"]},{"n":"dbmate","c":"dbmate","brew":"dbmate","lbrew":"dbmate","choco":"dbmate","pkg":"dbmate"},{"n":"delta","c":"delta","brew":"git-delta","apt":"git-delta","dnf":"git-delta","pacman":"git-delta","apk":"git-delta","winget":"dandavison.delta","pkg":"git-delta","hook":"Configure delta as git pager for beautiful diffs"},{"n":"delve","c":"dlv","brew":"delve","apt":"delve","dnf":"delve","pacman":"delve","apk":"delve","winget":"Go.Delve","pkg":"delve","deps":["go"]},{"n":"deno","c":"deno","brew":"deno","lbrew":"deno","winget":"DenoLand.Deno","pkg":"deno"},{"n":"detect_secrets","c":"detect-secrets","brew":"detect-secrets","lbrew":"detect-secrets"},{"n":"dfc","c":"dfc","brew":"chainguard-dev/tap/dfc","lbrew":"chainguard-dev/tap/dfc"},{"n":"direnv","c":"direnv","brew":"direnv","apt":"direnv","dnf":"direnv","pacman":"direnv","apk":"direnv","winget":"direnv.direnv","pkg":"direnv","hook":"Add direnv shell initialization to .bashrc and .zshrc"},{"n":"dive","c":"dive","brew":"dive","apk":"dive","lbrew":"dive","pkg":"dive","flex":["docker","podman"]},{"n":"docker","c":"docker","cask":"docker","apt":"docker.io","dnf":"docker","pacman":"docker","apk":"docker","winget":"Docker.DockerDesktop","pkg":"docker","hook":"Add user to docker group (Linux) for rootless access"},{"n":"docker_desktop","c":"docker","cask":"docker","apt":"docker-desktop","dnf":"docker-desktop","pacman":"docker-desktop","apk":"docker-desktop","winget":"Docker.DockerDesktop","hook":"Add user to docker group (Linux) for rootless access"},{"n":"dog","c":"dog","brew":"dog","apt":"dog","dnf":"dog","pacman":"dog","apk":"dog","pkg":"dog"},{"n":"dotnet","c":"dotnet","cask":"dotnet-sdk","apt":"dotnet-sdk-8.0","dnf":"dotnet-sdk","pacman":"dotnet-sdk","apk":"dotnet-sdk","winget":"Microsoft.DotNet.SDK.8","pkg":"dotnet","hook":"Configure DOTNET_ROOT and add .NET tools to PATH"},{"n":"duckdb","c":"duckdb","brew":"duckdb","apt":"duckdb","dnf":"duckdb","pacman":"duckdb","apk":"duckdb","winget":"DuckDB.cli","choco":"duckdb","pkg":"duckdb"},{"n":"duf","c":"duf","brew":"duf","apt":"duf","dnf":"duf","pacman":"duf","apk":"duf","winget":"muesli.duf","pkg":"duf"},{"n":"dust","c":"dust","brew":"dust","apt":"du-dust","dnf":"dust","pacman":"dust","apk":"dust","winget":"bootandy.dust","choco":"dust","pkg":"dust"},{"n":"earthly","c":"earthly","brew":"earthly/earthly/earthly","apt":"earthly","dnf":"earthly","pacman":"earthly","apk":"earthly","winget":"Earthly.Earthly","pkg":"earthly","flex":["docker","podman"]},{"n":"eksctl","c":"eksctl","brew":"eksctl","apt":"eksctl","dnf":"eksctl","pacman":"eksctl","apk":"eksctl","winget":"weaveworks.eksctl","pkg":"eksctl","hook":"Install eksctl shell completions for bash and zsh"},{"n":"elixir","c":"elixir","brew":"elixir","apt":"elixir","dnf":"elixir","pacman":"elixir","apk":"elixir","winget":"Elixir.Elixir","pkg":"elixir","deps":["erlang"]},{"n":"emacs","c":"emacs","cask":"emacs","apt":"emacs","dnf":"emacs","pacman":"emacs","apk":"emacs","winget":"GNU.Emacs","pkg":"emacs"},{"n":"emqx","c":"emqx","cat":"messaging","brew":"emqx","apt":"emqx","dnf":"emqx","pacman":"emqx","apk":"emqx"},{"n":"erlang","c":"erl","brew":"erlang","apt":"erlang","dnf":"erlang","pacman":"erlang","apk":"erlang","winget":"Erlang.ErlangOTP","choco":"erlang","pkg":"erlang"},{"n":"eza","c":"eza","brew":"eza","apt":"eza","dnf":"eza","pacman":"eza","apk":"eza","winget":"eza-community.eza","pkg":"eza","hook":"Add eza aliases for ls replacement"},{"n":"fd","c":"fd","brew":"fd","apt":"fd-find","dnf":"fd-find","pacman":"fd","apk":"fd","winget":"sharkdp.fd","pkg":"fd-find","hook":"Add fd alias for Debian/Ubuntu (fd-find package)"},{"n":"flux","c":"flux","brew":"fluxcd/tap/flux","lbrew":"fluxcd/tap/flux","winget":"Fluxcd.Flux","pkg":"flux","hook":"Install flux shell completions for bash and zsh"},{"n":"freelens","c":"freelens","cask":"freelens","lbrew":"freelens","winget":"freelensapp.Freelens"},{"n":"fzf","c":"fzf","brew":"fzf","apt":"fzf","dnf":"fzf","pacman":"fzf","apk":"fzf","winget":"junegunn.fzf","pkg":"fzf","hook":"Configure fzf shell integration (keybindings and completions)"},{"n":"gcloud","c":"gcloud","cask":"gcloud-cli","apt":"google-cloud-cli","dnf":"google-cloud-cli","pacman":"google-cloud-cli","apk":"google-cloud-sdk","winget":"Google.CloudSDK","hook":"Add gcloud shell completion and PATH for components"},{"n":"gh","c":"gh","brew":"gh","apt":"gh","dnf":"gh","pacman":"github-cli","apk":"github-cli","winget":"GitHub.cli","pkg":"gh","hook":"Configure GitHub CLI shell completion"},{"n":"git","c":"git","brew":"git","apt":"git","dnf":"git","pacman":"git","apk":"git","winget":"Git.Git","pkg":"git","hook":"Configure sensible Git defaults (defaultBranch=main, autocrlf, rebase)"},{"n":"git_lfs","c":"git-lfs","brew":"git-lfs","apt":"git-lfs","dnf":"git-lfs","pacman":"git-lfs","apk":"git-lfs","winget":"GitHub.GitLFS","pkg":"git-lfs"},{"n":"git_secrets","c":"git-secrets","brew":"git-secrets","apt":"git-secrets","dnf":"git-secrets","pacman":"git-secrets","apk":"git-secrets","deps":["git"]},{"n":"gitleaks","c":"gitleaks","brew":"gitleaks","lbrew":"gitleaks","winget":"Gitleaks.Gitleaks","pkg":"gitleaks","deps":["git"]},{"n":"gitversion","c":"gitversion","brew":"gitversion","apt":"gitversion","dnf":"gitversion","pacman":"gitversion","apk":"gitversion","winget":"GitTools.GitVersion","deps":["git"]},{"n":"glab","c":"glab","brew":"glab","apt":"glab","dnf":"glab","pacman":"glab","apk":"glab","winget":"GLab.GLab","pkg":"glab","hook":"Install glab shell completions for bash and zsh"},{"n":"glances","c":"glances","brew":"glances","apt":"glances","dnf":"glances","pacman":"glances","apk":"glances","pkg":"py-glances","hook":"Seed ~/.config/glances/glances.conf with CSV history export to ~/.jarvy/glances/"},{"n":"gleam","c":"gleam","brew":"gleam","apt":"gleam","dnf":"gleam","pacman":"gleam","apk":"gleam","winget":"Gleam.Gleam","pkg":"gleam"},{"n":"go","c":"go","brew":"go","apt":"golang","dnf":"golang","pacman":"go","apk":"go","winget":"GoLang.Go","pkg":"go","hook":"Configure GOPATH and add Go binaries to PATH"},{"n":"goaccess","c":"goaccess","brew":"goaccess","apt":"goaccess","dnf":"goaccess","pacman":"goaccess","apk":"goaccess"},{"n":"gofumpt","c":"gofumpt","brew":"gofumpt","apt":"gofumpt","dnf":"gofumpt","pacman":"gofumpt","apk":"gofumpt","winget":"mvdan.gofumpt","pkg":"gofumpt","deps":["go"]},{"n":"golangci_lint","c":"golangci-lint","brew":"golangci-lint","apt":"golangci-lint","dnf":"golangci-lint","pacman":"golangci-lint","apk":"golangci-lint","winget":"GolangCI.golangci-lint","pkg":"golangci-lint","deps":["go"]},{"n":"gopls","c":"gopls","brew":"gopls","apt":"gopls","dnf":"gopls","pacman":"gopls","apk":"gopls","winget":"Google.Gopls","pkg":"gopls","deps":["go"]},{"n":"goreleaser","c":"goreleaser","brew":"goreleaser","apt":"goreleaser","dnf":"goreleaser","pacman":"goreleaser","apk":"goreleaser","winget":"GoReleaser.GoReleaser","pkg":"goreleaser","deps":["go"]},{"n":"gotestsum","c":"gotestsum","brew":"gotestsum","apt":"gotestsum","dnf":"gotestsum","pacman":"gotestsum","apk":"gotestsum","winget":"gotestyourself.gotestsum","pkg":"gotestsum","deps":["go"]},{"n":"govulncheck","c":"govulncheck","brew":"govulncheck","apt":"govulncheck","dnf":"govulncheck","pacman":"govulncheck","apk":"govulncheck","pkg":"govulncheck","deps":["go"]},{"n":"gping","c":"gping","brew":"gping","apt":"gping","dnf":"gping","pacman":"gping","apk":"gping","winget":"orf.gping","pkg":"gping"},{"n":"grafanactl","c":"grafanactl","brew":"grafanactl","apt":"grafanactl","dnf":"grafanactl","pacman":"grafanactl","apk":"grafanactl"},{"n":"grex","c":"grex","brew":"grex","apt":"grex","dnf":"grex","pacman":"grex","apk":"grex","winget":"pemistahl.grex","pkg":"grex"},{"n":"grpcurl","c":"grpcurl","brew":"grpcurl","apt":"grpcurl","dnf":"grpcurl","pacman":"grpcurl","apk":"grpcurl","winget":"fullstorydev.grpcurl"},{"n":"grype","c":"grype","brew":"grype","apk":"grype","lbrew":"grype","choco":"grype","pkg":"grype"},{"n":"hadolint","c":"hadolint","brew":"hadolint","lbrew":"hadolint","winget":"hadolint.hadolint","pkg":"hadolint"},{"n":"haskell","c":"ghc","brew":"ghc","apt":"ghc","dnf":"ghc","pacman":"ghc","apk":"ghc","winget":"Haskell.GHCup","pkg":"ghc"},{"n":"headscale","c":"headscale","cat":"networking","custom":true},{"n":"helix","c":"hx","brew":"helix","apt":"helix-editor","dnf":"helix","pacman":"helix","apk":"helix","winget":"Helix.Helix","pkg":"helix"},{"n":"helm","c":"helm","brew":"helm","apt":"helm","dnf":"helm","pacman":"helm","apk":"helm","winget":"Helm.Helm","pkg":"helm","flex":["kubectl"],"hook":"Add common Helm chart repositories"},{"n":"htop","c":"htop","brew":"htop","apt":"htop","dnf":"htop","pacman":"htop","apk":"htop","pkg":"htop"},{"n":"httpie","c":"http","brew":"httpie","apt":"httpie","dnf":"httpie","pacman":"httpie","apk":"py3-httpie","winget":"HTTPie.HTTPie","pkg":"py39-httpie"},{"n":"hugo","c":"hugo","brew":"hugo","apt":"hugo","dnf":"hugo","pacman":"hugo","apk":"hugo","winget":"Hugo.Hugo.Extended","pkg":"hugo"},{"n":"hyperfine","c":"hyperfine","brew":"hyperfine","apt":"hyperfine","dnf":"hyperfine","pacman":"hyperfine","apk":"hyperfine","winget":"sharkdp.hyperfine","choco":"hyperfine","pkg":"hyperfine"},{"n":"infisical","c":"infisical","brew":"infisical","winget":"infisical.infisical"},{"n":"infracost","c":"infracost","brew":"infracost","apt":"infracost","dnf":"infracost","pacman":"infracost","apk":"infracost","winget":"Infracost.Infracost","pkg":"infracost"},{"n":"iterm2","c":"iterm2","cask":"iterm2"},{"n":"java","c":"java","brew":"openjdk","apt":"default-jdk","dnf":"java-latest-openjdk","pacman":"jdk-openjdk","apk":"openjdk21","winget":"Microsoft.OpenJDK.21","choco":"openjdk","pkg":"openjdk21","hook":"Configure JAVA_HOME environment variable"},{"n":"jetbrains_toolbox","c":"jetbrains-toolbox","cask":"jetbrains-toolbox","winget":"JetBrains.Toolbox","custom":true},{"n":"jq","c":"jq","brew":"jq","apt":"jq","dnf":"jq","pacman":"jq","apk":"jq","winget":"jqlang.jq","pkg":"jq"},{"n":"julia","c":"julia","cask":"julia","apt":"julia","dnf":"julia","pacman":"julia","apk":"julia","winget":"Julialang.Julia","pkg":"julia"},{"n":"just","c":"just","brew":"just","apt":"just","dnf":"just","pacman":"just","apk":"just","winget":"Casey.Just","pkg":"just","hook":"Install just shell completions for bash and zsh"},{"n":"k3d","c":"k3d","brew":"k3d","lbrew":"k3d","winget":"k3d-io.k3d","pkg":"k3d","deps":["docker"]},{"n":"k3s","c":"k3s","custom":true},{"n":"k6","c":"k6","brew":"k6","apt":"k6","dnf":"k6","pacman":"k6","apk":"k6","winget":"Grafana.k6","pkg":"k6"},{"n":"k9s","c":"k9s","brew":"derailed/k9s/k9s","apt":"k9s","dnf":"k9s","pacman":"k9s","apk":"k9s","winget":"Derailed.k9s","pkg":"k9s","flex":["kubectl"],"hook":"Configure k9s shell completion"},{"n":"kaf","c":"kaf","cat":"messaging","brew":"kaf","lbrew":"kaf"},{"n":"kafka","c":"kafka-topics","cat":"messaging","brew":"kafka","lbrew":"kafka"},{"n":"kafkactl","c":"kafkactl","cat":"messaging","brew":"deviceinsight/packages/kafkactl","lbrew":"deviceinsight/packages/kafkactl"},{"n":"kagent","c":"kagent","brew":"kagent","lbrew":"kagent","deps":["kubectl"]},{"n":"kcat","c":"kcat","cat":"messaging","brew":"kcat","apt":"kafkacat","dnf":"kcat","pacman":"kcat","apk":"kcat"},{"n":"kind","c":"kind","brew":"kind","lbrew":"kind","winget":"Kubernetes.kind","pkg":"kind","deps":["docker"],"hook":"Install kind shell completions for bash and zsh"},{"n":"kmcp","c":"kmcp","custom":true,"deps":["kubectl"]},{"n":"kn","c":"kn","cat":"workflow","brew":"kn","lbrew":"kn","flex":["kubectl"]},{"n":"ko","c":"ko","brew":"ko","apt":"ko","dnf":"ko","pacman":"ko","apk":"ko","winget":"ko-build.ko","pkg":"ko","deps":["go"]},{"n":"kotlin","c":"kotlin","brew":"kotlin","apt":"kotlin","dnf":"kotlin","pacman":"kotlin","apk":"kotlin","winget":"JetBrains.Kotlin.Compiler","choco":"kotlinc","pkg":"kotlin","deps":["java"]},{"n":"krew","c":"kubectl-krew","brew":"krew","apk":"kubectl-krew","lbrew":"krew","pkg":"krew","flex":["kubectl"],"hook":"Add krew to PATH in .bashrc and .zshrc"},{"n":"kubectl","c":"kubectl","brew":"kubectl","apt":"kubectl","dnf":"kubectl","pacman":"kubectl","apk":"kubectl","winget":"Kubernetes.kubectl","pkg":"kubectl","flex":["minikube","kind","k3d","docker","podman"],"hook":"Enable kubectl shell completion and 'k' alias"},{"n":"kubectx","c":"kubectx","brew":"kubectx","lbrew":"kubectx","winget":"ahmetb.kubectx","pkg":"kubectx","hook":"Add kctx/kns aliases for kubectx and kubens"},{"n":"kubens","c":"kubens","brew":"kubectx","lbrew":"kubectx","pkg":"kubectx"},{"n":"kubescape","c":"kubescape","brew":"kubescape","apt":"kubescape","dnf":"kubescape","pacman":"kubescape","apk":"kubescape","winget":"kubescape.kubescape","pkg":"kubescape","deps":["kubectl"]},{"n":"kustomize","c":"kustomize","brew":"kustomize","apt":"kustomize","dnf":"kustomize","pacman":"kustomize","apk":"kustomize","winget":"Kubernetes.kustomize","pkg":"kustomize"},{"n":"lazydocker","c":"lazydocker","brew":"lazydocker","apk":"lazydocker","lbrew":"lazydocker","choco":"lazydocker","pkg":"lazydocker","deps":["docker"]},{"n":"lazygit","c":"lazygit","brew":"lazygit","apt":"lazygit","dnf":"lazygit","pacman":"lazygit","apk":"lazygit","winget":"JesseDuffield.lazygit","pkg":"lazygit","hook":"Create lg alias for lazygit"},{"n":"linkerd","c":"linkerd","brew":"linkerd","lbrew":"linkerd","flex":["kubectl"]},{"n":"litecli","c":"litecli","brew":"litecli","apt":"litecli","dnf":"litecli","pacman":"litecli","apk":"litecli","choco":"litecli","pkg":"py39-litecli"},{"n":"litellm","c":"litellm","custom":true},{"n":"lnav","c":"lnav","brew":"lnav","apt":"lnav","dnf":"lnav","pacman":"lnav","apk":"lnav","pkg":"lnav"},{"n":"localstack","c":"localstack","brew":"localstack","apt":"localstack","dnf":"localstack","pacman":"localstack","apk":"localstack","pkg":"localstack","flex":["docker","podman"]},{"n":"locust","c":"locust","cat":"testing","brew":"locust","lbrew":"locust"},{"n":"lsd","c":"lsd","brew":"lsd","apt":"lsd","dnf":"lsd","pacman":"lsd","apk":"lsd","winget":"lsd-rs.lsd","pkg":"lsd"},{"n":"lua","c":"lua","brew":"lua","apt":"lua5.4","dnf":"lua","pacman":"lua","apk":"lua","winget":"DEVCOM.Lua","choco":"lua","pkg":"lua54"},{"n":"luarocks","c":"luarocks","brew":"luarocks","apt":"luarocks","dnf":"luarocks","pacman":"luarocks","apk":"luarocks","winget":"LuaRocks.LuaRocks","pkg":"luarocks","deps":["lua"]},{"n":"lynis","c":"lynis","brew":"lynis","apt":"lynis","dnf":"lynis","pacman":"lynis","apk":"lynis","pkg":"lynis"},{"n":"make","c":"make","brew":"make","apt":"make","dnf":"make","pacman":"make","apk":"make","winget":"GnuWin32.Make","pkg":"gmake"},{"n":"micro","c":"micro","brew":"micro","apt":"micro","dnf":"micro","pacman":"micro","apk":"micro","winget":"zyedidia.micro","pkg":"micro"},{"n":"microk8s","c":"microk8s","custom":true},{"n":"minikube","c":"minikube","brew":"minikube","apt":"minikube","dnf":"minikube","pacman":"minikube","apk":"minikube","winget":"Kubernetes.minikube","pkg":"minikube","flex":["docker","podman"],"hook":"Install minikube shell completions for bash and zsh"},{"n":"mise","c":"mise","brew":"mise","lbrew":"mise","winget":"jdx.mise","pkg":"mise","hook":"Add mise shell initialization to .bashrc and .zshrc"},{"n":"mockgen","c":"mockgen","brew":"mockery","apt":"mockery","dnf":"mockery","pacman":"mockery","apk":"mockery","winget":"vektra.mockery","pkg":"mockery","deps":["go"]},{"n":"molecule","c":"molecule","brew":"molecule","apt":"molecule","dnf":"molecule","pacman":"molecule","apk":"molecule","pkg":"molecule","deps":["ansible"]},{"n":"mongosh","c":"mongosh","brew":"mongosh","lbrew":"mongosh","winget":"MongoDB.Shell","pkg":"mongosh"},{"n":"mosquitto","c":"mosquitto","cat":"messaging","brew":"mosquitto","apt":"mosquitto","dnf":"mosquitto","pacman":"mosquitto","apk":"mosquitto","winget":"EclipseFoundation.Mosquitto"},{"n":"mssql_cli","c":"mssql-cli","brew":"mssql-cli","apt":"mssql-cli","dnf":"mssql-cli","pacman":"mssql-cli","apk":"mssql-cli","winget":"Microsoft.SqlServer.MssqlCli"},{"n":"mtr","c":"mtr","brew":"mtr","apt":"mtr","dnf":"mtr","pacman":"mtr","apk":"mtr","pkg":"mtr"},{"n":"mycli","c":"mycli","brew":"mycli","apt":"mycli","dnf":"mycli","pacman":"mycli","apk":"mycli","choco":"mycli","pkg":"py39-mycli"},{"n":"mysql","c":"mysql","brew":"mysql-client","apt":"mysql-client","dnf":"mysql","pacman":"mysql","apk":"mysql-client","winget":"Oracle.MySQL","pkg":"mysql80-client"},{"n":"nats","c":"nats","cat":"messaging","brew":"nats-io/nats-tools/nats","lbrew":"nats-io/nats-tools/nats","winget":"NATSAuthors.CLI"},{"n":"nats_server","c":"nats-server","cat":"messaging","brew":"nats-server","apt":"nats-server","dnf":"nats-server","pacman":"nats-server","apk":"nats-server"},{"n":"ncdu","c":"ncdu","brew":"ncdu","apt":"ncdu","dnf":"ncdu","pacman":"ncdu","apk":"ncdu","pkg":"ncdu"},{"n":"nebula","c":"nebula","cat":"networking","brew":"nebula","lbrew":"nebula"},{"n":"nerdctl","c":"nerdctl","brew":"nerdctl","apk":"nerdctl","lbrew":"nerdctl","pkg":"nerdctl"},{"n":"netbird","c":"netbird","cat":"networking","brew":"netbirdio/tap/netbird","lbrew":"netbirdio/tap/netbird","winget":"Netbird.Netbird"},{"n":"ngrok","c":"ngrok","brew":"ngrok","apt":"ngrok","dnf":"ngrok","pacman":"ngrok","apk":"ngrok","winget":"Ngrok.Ngrok","pkg":"ngrok"},{"n":"nim","c":"nim","brew":"nim","apt":"nim","dnf":"nim","pacman":"nim","apk":"nim","winget":"Nim.Nim","pkg":"nim"},{"n":"nmap","c":"nmap","brew":"nmap","apt":"nmap","dnf":"nmap","pacman":"nmap","apk":"nmap","winget":"Insecure.Nmap","pkg":"nmap"},{"n":"node","c":"node","brew":"node","apt":"nodejs","dnf":"nodejs","pacman":"nodejs","apk":"nodejs","winget":"OpenJS.NodeJS.LTS","pkg":"node","custom":true,"deps":["nvm"],"hook":"Configure npm global prefix and add to PATH"},{"n":"noseyparker","c":"noseyparker","brew":"noseyparker","lbrew":"noseyparker"},{"n":"nsc","c":"nsc","cat":"messaging","brew":"nats-io/nats-tools/nsc","lbrew":"nats-io/nats-tools/nsc","winget":"NATSAuthors.nsc"},{"n":"nushell","c":"nu","brew":"nushell","apt":"nushell","dnf":"nushell","pacman":"nushell","apk":"nushell","winget":"Nushell.Nushell","pkg":"nushell"},{"n":"nvim","c":"nvim","brew":"neovim","apt":"neovim","dnf":"neovim","pacman":"neovim","apk":"neovim","winget":"Neovim.Neovim","pkg":"neovim","hook":"Create ~/.config/nvim with a starter init.lua when no config exists"},{"n":"nvm","c":"nvm","custom":true},{"n":"ocaml","c":"ocaml","brew":"ocaml","apt":"ocaml","dnf":"ocaml","pacman":"ocaml","apk":"ocaml","winget":"OCaml.OCaml","pkg":"ocaml"},{"n":"ollama","c":"ollama","brew":"ollama","lbrew":"ollama","winget":"Ollama.Ollama","custom":true},{"n":"omnictl","c":"omnictl","brew":"siderolabs/tap/omnictl","lbrew":"siderolabs/tap/omnictl","winget":"Sidero.omnictl"},{"n":"openclaw","c":"openclaw","brew":"openclaw-cli","apt":"openclaw-cli","dnf":"openclaw-cli","pacman":"openclaw-cli","apk":"openclaw-cli"},{"n":"openssh","c":"ssh","brew":"openssh","apt":"openssh","dnf":"openssh","pacman":"openssh","apk":"openssh","winget":"Microsoft.OpenSSH.Beta","pkg":"openssh-portable"},{"n":"opentofu","c":"tofu","brew":"opentofu","apt":"opentofu","dnf":"opentofu","pacman":"opentofu","apk":"opentofu","winget":"OpenTofu.OpenTofu","pkg":"opentofu"},{"n":"openvpn","c":"openvpn","cat":"networking","brew":"openvpn","apt":"openvpn","dnf":"openvpn","pacman":"openvpn","apk":"openvpn","winget":"OpenVPNTechnologies.OpenVPN"},{"n":"oras","c":"oras","brew":"oras","apt":"oras","dnf":"oras","pacman":"oras","apk":"oras","winget":"oras-project.oras"},{"n":"p7zip","c":"7z","brew":"p7zip","apt":"p7zip","dnf":"p7zip","pacman":"p7zip","apk":"p7zip","winget":"7zip.7zip","pkg":"p7zip"},{"n":"packer","c":"packer","brew":"packer","apt":"packer","dnf":"packer","pacman":"packer","apk":"packer","winget":"HashiCorp.Packer","pkg":"packer"},{"n":"pgcli","c":"pgcli","brew":"pgcli","apt":"pgcli","dnf":"pgcli","pacman":"pgcli","apk":"pgcli","choco":"pgcli","pkg":"py39-pgcli"},{"n":"php","c":"php","brew":"php","apt":"php","dnf":"php","pacman":"php","apk":"php","winget":"PHP.PHP","pkg":"php83"},{"n":"playwright","c":"playwright","cat":"testing","custom":true,"flex":["node","nvm"]},{"n":"pnpm","c":"pnpm","brew":"pnpm","lbrew":"pnpm","winget":"pnpm.pnpm"},{"n":"podman","c":"podman","brew":"podman","apt":"podman","dnf":"podman","pacman":"podman","apk":"podman","winget":"RedHat.Podman","pkg":"podman"},{"n":"podman_desktop","c":"podman-desktop","cask":"podman-desktop","lbrew":"podman-desktop","winget":"RedHat.Podman-Desktop"},{"n":"powershell","c":"pwsh","cask":"powershell","apt":"powershell","dnf":"powershell","pacman":"powershell","apk":"powershell","winget":"Microsoft.PowerShell","pkg":"powershell"},{"n":"pre_commit","c":"pre-commit","brew":"pre-commit","apt":"pre-commit","dnf":"pre-commit","pacman":"pre-commit","apk":"pre-commit","choco":"pre-commit","pkg":"py39-pre-commit"},{"n":"procs","c":"procs","brew":"procs","apt":"procs","dnf":"procs","pacman":"procs","apk":"procs","winget":"dalance.procs","pkg":"procs"},{"n":"psql","c":"psql","brew":"postgresql","apt":"postgresql-client","dnf":"postgresql","pacman":"postgresql","apk":"postgresql-client","winget":"PostgreSQL.PostgreSQL","pkg":"postgresql16-client"},{"n":"pulsar","c":"pulsar","cat":"messaging","brew":"apache-pulsar","lbrew":"apache-pulsar"},{"n":"pulumi","c":"pulumi","brew":"pulumi","apt":"pulumi","dnf":"pulumi","pacman":"pulumi","apk":"pulumi","winget":"Pulumi.Pulumi","pkg":"pulumi"},{"n":"putty","c":"putty","cat":"networking","brew":"putty","apt":"putty","dnf":"putty","pacman":"putty","apk":"putty","winget":"PuTTY.PuTTY"},{"n":"pyenv","c":"pyenv","brew":"pyenv","lbrew":"pyenv","pkg":"pyenv","hook":"Add pyenv shell initialization to .bashrc and .zshrc"},{"n":"python","c":"python3","brew":"python","apt":"python3","dnf":"python3","pacman":"python","apk":"python3","winget":"Python.Python.3","pkg":"python3","deps":["pyenv"],"hook":"Upgrade pip and configure user site-packages PATH"},{"n":"rabbitmq","c":"rabbitmq-server","cat":"messaging","brew":"rabbitmq","apt":"rabbitmq-server","dnf":"rabbitmq-server","pacman":"rabbitmq-server","apk":"rabbitmq-server"},{"n":"rancher_desktop","c":"rdctl","cask":"rancher","lbrew":"rancher","winget":"suse.RancherDesktop"},{"n":"rbenv","c":"rbenv","brew":"rbenv","lbrew":"rbenv","pkg":"rbenv","hook":"Add rbenv shell initialization to .bashrc and .zshrc"},{"n":"rclone","c":"rclone","brew":"rclone","apt":"rclone","dnf":"rclone","pacman":"rclone","apk":"rclone","winget":"Rclone.Rclone","pkg":"rclone"},{"n":"redis","c":"redis-cli","brew":"redis","apt":"redis","dnf":"redis","pacman":"redis","apk":"redis","winget":"Redis.Redis","pkg":"redis"},{"n":"release_plz","c":"release-plz","custom":true,"deps":["rust"]},{"n":"ripgrep","c":"rg","brew":"ripgrep","apt":"ripgrep","dnf":"ripgrep","pacman":"ripgrep","apk":"ripgrep","winget":"BurntSushi.ripgrep.MSVC","pkg":"ripgrep","hook":"Configure ripgrep shell completion"},{"n":"rpk","c":"rpk","cat":"messaging","brew":"redpanda-data/tap/redpanda","lbrew":"redpanda-data/tap/redpanda"},{"n":"ruby","c":"ruby","brew":"ruby","apt":"ruby","dnf":"ruby","pacman":"ruby","apk":"ruby","winget":"RubyInstallerTeam.Ruby","pkg":"ruby","deps":["rbenv"]},{"n":"ruff","c":"ruff","brew":"ruff","apk":"ruff","lbrew":"ruff","winget":"astral-sh.ruff","pkg":"ruff"},{"n":"rust","c":"rustc","custom":true,"hook":"Install clippy + rustfmt components and source cargo env in shell rc"},{"n":"scala","c":"scala","brew":"scala","apt":"scala","dnf":"scala","pacman":"scala","apk":"scala","winget":"Scala.Scala.3","choco":"scala","pkg":"scala","deps":["java"]},{"n":"sd","c":"sd","brew":"sd","lbrew":"sd","choco":"sd-cli","pkg":"sd"},{"n":"sdkman","c":"sdk","brew":"sdkman-cli","apt":"sdkman","dnf":"sdkman","pacman":"sdkman","apk":"sdkman","pkg":"sdkman","hook":"Add SDKMAN shell initialization to .bashrc and .zshrc"},{"n":"semgrep","c":"semgrep","brew":"semgrep","lbrew":"semgrep","pkg":"semgrep"},{"n":"shellcheck","c":"shellcheck","brew":"shellcheck","apt":"shellcheck","dnf":"shellcheck","pacman":"shellcheck","apk":"shellcheck","winget":"koalaman.shellcheck","pkg":"hs-ShellCheck"},{"n":"shfmt","c":"shfmt","brew":"shfmt","apt":"shfmt","dnf":"shfmt","pacman":"shfmt","apk":"shfmt","winget":"mvdan.shfmt","pkg":"shfmt"},{"n":"skaffold","c":"skaffold","brew":"skaffold","lbrew":"skaffold","winget":"Google.ContainerTools.Skaffold"},{"n":"skopeo","c":"skopeo","brew":"skopeo","apt":"skopeo","dnf":"skopeo","pacman":"skopeo","apk":"skopeo","pkg":"skopeo"},{"n":"sonar_scanner","c":"sonar-scanner","brew":"sonar-scanner","apt":"sonar-scanner","dnf":"sonar-scanner","pacman":"sonar-scanner","apk":"sonar-scanner"},{"n":"sops","c":"sops","brew":"sops","lbrew":"sops","winget":"Mozilla.sops","pkg":"sops"},{"n":"sqlite","c":"sqlite3","brew":"sqlite","apt":"sqlite3","dnf":"sqlite3","pacman":"sqlite3","apk":"sqlite3","winget":"SQLite.SQLite","pkg":"sqlite3"},{"n":"starship","c":"starship","brew":"starship","apt":"starship","dnf":"starship","pacman":"starship","apk":"starship","winget":"Starship.Starship","pkg":"starship","hook":"Add starship shell initialization to .bashrc and .zshrc"},{"n":"stern","c":"stern","brew":"stern","apt":"stern","dnf":"stern","pacman":"stern","apk":"stern","winget":"stern.stern","pkg":"stern","deps":["kubectl"]},{"n":"structurizr","c":"structurizr","brew":"structurizr","lbrew":"structurizr"},{"n":"syft","c":"syft","brew":"syft","apk":"syft","lbrew":"syft","choco":"syft","pkg":"syft"},{"n":"tailscale","c":"tailscale","cat":"networking","brew":"tailscale","lbrew":"tailscale","winget":"Tailscale.Tailscale"},{"n":"talisman","c":"talisman","brew":"talisman","lbrew":"talisman","deps":["git"]},{"n":"talosctl","c":"talosctl","brew":"siderolabs/tap/talosctl","apt":"talosctl","dnf":"talosctl","pacman":"talosctl","apk":"talosctl","winget":"SideroLabs.talosctl","pkg":"talosctl"},{"n":"task","c":"task","cat":"workflow","brew":"go-task","lbrew":"go-task","winget":"Task.Task"},{"n":"temporal","c":"temporal","cat":"workflow","brew":"temporal","lbrew":"temporal"},{"n":"terraform","c":"terraform","brew":"terraform","apt":"terraform","dnf":"terraform","pacman":"terraform","apk":"terraform","winget":"HashiCorp.Terraform","pkg":"terraform","hook":"Install Terraform shell autocomplete"},{"n":"terraform_docs","c":"terraform-docs","brew":"terraform-docs","lbrew":"terraform-docs","choco":"terraform-docs","pkg":"terraform-docs"},{"n":"terragrunt","c":"terragrunt","brew":"terragrunt","lbrew":"terragrunt","winget":"Gruntwork.Terragrunt","choco":"terragrunt","pkg":"terragrunt"},{"n":"tfsec","c":"tfsec","brew":"tfsec","apt":"tfsec","dnf":"tfsec","pacman":"tfsec","apk":"tfsec","winget":"aquasecurity.tfsec","pkg":"tfsec"},{"n":"tilt","c":"tilt","brew":"tilt","apt":"tilt","dnf":"tilt","pacman":"tilt","apk":"tilt","pkg":"tilt"},{"n":"tmux","c":"tmux","brew":"tmux","apt":"tmux","dnf":"tmux","pacman":"tmux","apk":"tmux","pkg":"tmux","hook":"Install TPM (tmux plugin manager) and seed its run line in ~/.tmux.conf"},{"n":"todoist","c":"todoist","brew":"todoist-cli-go","lbrew":"todoist-cli-go"},{"n":"tokei","c":"tokei","brew":"tokei","apt":"tokei","dnf":"tokei","pacman":"tokei","apk":"tokei","winget":"XAMPPRocky.tokei","choco":"tokei","pkg":"tokei"},{"n":"tree","c":"tree","brew":"tree","apt":"tree","dnf":"tree","pacman":"tree","apk":"tree","pkg":"tree"},{"n":"trivy","c":"trivy","brew":"trivy","apk":"trivy","lbrew":"trivy","winget":"aquasecurity.trivy","pkg":"trivy"},{"n":"trufflehog","c":"trufflehog","brew":"trufflehog","lbrew":"trufflehog","pkg":"trufflehog","deps":["git"]},{"n":"twingate","c":"twingate","cat":"networking","cask":"twingate","winget":"Twingate.Client"},{"n":"up","c":"up","brew":"upbound/tap/up","lbrew":"upbound/tap/up","pkg":"up"},{"n":"usql","c":"usql","brew":"usql","lbrew":"usql","choco":"usql","pkg":"usql"},{"n":"uv","c":"uv","brew":"uv","apk":"uv","lbrew":"uv","winget":"astral-sh.uv","pkg":"uv"},{"n":"vagrant","c":"vagrant","cask":"vagrant","apt":"vagrant","dnf":"vagrant","pacman":"vagrant","apk":"vagrant","winget":"Hashicorp.Vagrant","choco":"vagrant","pkg":"vagrant"},{"n":"vault","c":"vault","brew":"vault","apt":"vault","dnf":"vault","pacman":"vault","apk":"vault","winget":"HashiCorp.Vault","pkg":"vault"},{"n":"vfox","c":"vfox","brew":"vfox","lbrew":"vfox","winget":"vfox","pkg":"vfox","hook":"Add vfox shell initialization to .bashrc and .zshrc"},{"n":"vim","c":"vim","brew":"vim","apt":"vim","dnf":"vim-enhanced","pacman":"vim","apk":"vim","winget":"vim.vim","pkg":"vim"},{"n":"vllm","c":"vllm","custom":true},{"n":"vscode","c":"code","cask":"visual-studio-code","apt":"code","dnf":"code","pacman":"code","apk":"code","winget":"Microsoft.VisualStudioCode"},{"n":"watchexec","c":"watchexec","brew":"watchexec","apt":"watchexec","dnf":"watchexec","pacman":"watchexec","apk":"watchexec","winget":"watchexec.watchexec","choco":"watchexec","pkg":"watchexec"},{"n":"wget","c":"wget","brew":"wget","apt":"wget","dnf":"wget","pacman":"wget","apk":"wget","winget":"GnuWin32.Wget","pkg":"wget"},{"n":"wireguard_tools","c":"wg","cat":"networking","brew":"wireguard-tools","apt":"wireguard-tools","dnf":"wireguard-tools","pacman":"wireguard-tools","apk":"wireguard-tools","winget":"WireGuard.WireGuard"},{"n":"xz","c":"xz","brew":"xz","apt":"xz","dnf":"xz","pacman":"xz","apk":"xz","winget":"XZUtils.XZ","pkg":"xz"},{"n":"yamllint","c":"yamllint","brew":"yamllint","apt":"yamllint","dnf":"yamllint","pacman":"yamllint","apk":"py3-yamllint","winget":"adrienverge.yamllint","pkg":"py39-yamllint"},{"n":"yarn","c":"yarn","brew":"yarn","lbrew":"yarn","winget":"Yarn.Yarn"},{"n":"yazi","c":"yazi","brew":"yazi","apt":"yazi","dnf":"yazi","pacman":"yazi","apk":"yazi","winget":"sxyazi.yazi","pkg":"yazi"},{"n":"yq","c":"yq","brew":"yq","apt":"yq","dnf":"yq","pacman":"yq","apk":"yq","winget":"mikefarah.yq","pkg":"yq"},{"n":"zed","c":"zed","cask":"zed"},{"n":"zerotier","c":"zerotier-cli","cat":"networking","cask":"zerotier-one","winget":"ZeroTier.ZeroTierOne"},{"n":"zig","c":"zig","brew":"zig","apt":"zig","dnf":"zig","pacman":"zig","apk":"zig","winget":"zig.zig","choco":"zig","pkg":"zig"},{"n":"zoxide","c":"zoxide","brew":"zoxide","apt":"zoxide","dnf":"zoxide","pacman":"zoxide","apk":"zoxide","winget":"ajeetdsouza.zoxide","pkg":"zoxide","hook":"Add zoxide shell initialization to .bashrc and .zshrc"},{"n":"zsh","c":"zsh","brew":"zsh","apt":"zsh","dnf":"zsh","pacman":"zsh","apk":"zsh","pkg":"zsh"}]</script>

<script>
(function () {
  "use strict";
  const tools = JSON.parse(document.getElementById("jarvy-tools-data").textContent);
  const list = document.getElementById("jt-list");
  const countEl = document.getElementById("jt-count");
  const searchEl = document.getElementById("jt-search");
  const osEl = document.getElementById("jt-os");
  const catsEl = document.getElementById("jt-cats");

  const esc = (s) => String(s).replace(/[&<>"']/g, (c) =>
    ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));

  const supports = (t, os) => {
    if (os === "all" || t.custom) return true;
    if (os === "macos") return !!(t.brew || t.cask);
    if (os === "linux") return !!(t.apt || t.dnf || t.pacman || t.apk || t.lbrew);
    if (os === "windows") return !!(t.winget || t.choco);
    if (os === "bsd") return !!t.pkg;
    return true;
  };

  const cmdRow = (pm, cmd) =>
    '<div class="jt-cmd"><span class="jt-pm">' + esc(pm) + "</span><code>" + esc(cmd) +
    '</code><button class="jt-copy" data-cmd="' + esc(cmd) + '" title="Copy" aria-label="Copy command">⧉</button></div>';

  const section = (title, rows) => rows.length ? "<h5>" + esc(title) + "</h5>" + rows.join("") : "";

  function body(t) {
    let h = section("With Jarvy — any OS", [
      cmdRow("jarvy.toml", '[provisioner]\n' + t.n + ' = "latest"'),
      cmdRow("then run", "jarvy setup"),
    ]);
    const mac = [];
    if (t.cask) mac.push(cmdRow("brew cask", "brew install --cask " + t.cask));
    if (t.brew) mac.push(cmdRow("brew", "brew install " + t.brew));
    h += section("macOS", mac);
    const lin = [];
    if (t.apt) lin.push(cmdRow("apt", "sudo apt install " + t.apt));
    if (t.dnf) lin.push(cmdRow("dnf", "sudo dnf install " + t.dnf));
    if (t.pacman) lin.push(cmdRow("pacman", "sudo pacman -S " + t.pacman));
    if (t.apk) lin.push(cmdRow("apk", "sudo apk add " + t.apk));
    if (t.lbrew) lin.push(cmdRow("linuxbrew", "brew install " + t.lbrew));
    h += section("Linux", lin);
    const win = [];
    if (t.winget) win.push(cmdRow("winget", "winget install -e --id " + t.winget));
    if (t.choco) win.push(cmdRow("choco", "choco install -y " + t.choco));
    h += section("Windows", win);
    if (t.pkg) h += section("BSD", [cmdRow("pkg", "sudo pkg install " + t.pkg)]);
    if (t.custom)
      h += '<p class="jt-note">⚙ Uses a custom installer — Jarvy runs the official install script for you during <code>jarvy setup</code>.</p>';
    if (t.deps)
      h += '<p class="jt-note">Requires: ' + t.deps.map(esc).join(", ") + " (Jarvy installs dependencies first)</p>";
    if (t.flex)
      h += '<p class="jt-note">Works with one of: ' + t.flex.map(esc).join(", ") + "</p>";
    if (t.hook)
      h += '<p class="jt-note">Post-install hook: ' + esc(t.hook) + "</p>";
    return h;
  }

  const osBadges = (t) => {
    const b = [];
    if (t.custom) b.push("custom");
    if (t.brew || t.cask) b.push("macOS");
    if (t.apt || t.dnf || t.pacman || t.apk || t.lbrew) b.push("Linux");
    if (t.winget || t.choco) b.push("Windows");
    if (t.pkg) b.push("BSD");
    return b.map((x) => '<span class="jt-badge">' + x + "</span>").join("");
  };

  // Build all cards once; filtering toggles visibility.
  const frag = document.createDocumentFragment();
  const cards = tools.map((t) => {
    const d = document.createElement("details");
    d.className = "jt-card";
    d.innerHTML =
      "<summary><span class=\"jt-name\">" + esc(t.n) + "</span>" +
      (t.cat ? '<span class="jt-cat">' + esc(t.cat) + "</span>" : "") +
      '<span class="jt-badges">' + osBadges(t) + "</span></summary>" +
      '<div class="jt-body">' + body(t) + "</div>";
    frag.appendChild(d);
    const hay = [t.n, t.c, t.cat, t.brew, t.cask, t.apt, t.dnf, t.pacman, t.apk,
                 t.lbrew, t.winget, t.choco, t.pkg].filter(Boolean).join(" ").toLowerCase();
    return { t, el: d, hay };
  });
  list.appendChild(frag);
  const empty = document.createElement("p");
  empty.className = "jt-empty";
  empty.hidden = true;
  empty.innerHTML = "No tools match. Try <code>jarvy search</code> or " +
    '<a href="https://github.com/Cliftonz/jarvy/issues">request a tool</a>.';
  list.appendChild(empty);

  // Category chips (only categories that exist in the data).
  const cats = [...new Set(tools.map((t) => t.cat).filter(Boolean))].sort();
  let activeCat = "all";
  const chips = ["all", ...cats].map((c) => {
    const btn = document.createElement("button");
    btn.type = "button";
    btn.textContent = c === "all" ? "All" : c;
    btn.className = c === "all" ? "jt-on" : "";
    btn.addEventListener("click", () => {
      activeCat = c;
      chips.forEach((x) => x.classList.remove("jt-on"));
      btn.classList.add("jt-on");
      apply();
    });
    catsEl.appendChild(btn);
    return btn;
  });

  function apply() {
    const q = searchEl.value.trim().toLowerCase();
    const os = osEl.value;
    let shown = 0;
    for (const { t, el, hay } of cards) {
      const ok =
        (!q || hay.includes(q)) &&
        supports(t, os) &&
        (activeCat === "all" || t.cat === activeCat);
      el.style.display = ok ? "" : "none";
      if (ok) shown++;
    }
    empty.hidden = shown !== 0;
    countEl.textContent = "Showing " + shown + " of " + tools.length + " tools";
  }

  searchEl.addEventListener("input", apply);
  osEl.addEventListener("change", apply);
  list.addEventListener("click", (ev) => {
    const btn = ev.target.closest(".jt-copy");
    if (!btn) return;
    ev.preventDefault();
    navigator.clipboard.writeText(btn.dataset.cmd).then(() => {
      const old = btn.textContent;
      btn.textContent = "✓";
      setTimeout(() => { btn.textContent = old; }, 1200);
    });
  });
  apply();
})();
</script>

