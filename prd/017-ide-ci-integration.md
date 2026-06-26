# PRD-017: IDE and CI Integrations

## Overview

Integrate Jarvy with popular development workflows including IDE extensions, CI/CD actions, dev container features, and pre-commit hooks to provide seamless developer experience across the entire development lifecycle.

## Problem Statement

Jarvy currently operates as a standalone CLI tool, requiring developers to manually run commands to provision their environment. This creates friction in modern development workflows:

1. **No IDE awareness**: Developers must switch to terminal to discover missing tools
2. **Manual CI setup**: Teams copy-paste setup scripts instead of using native actions
3. **No dev container support**: Codespaces/devcontainer users must manually configure Jarvy
4. **No pre-commit integration**: Tool availability isn't validated before commits
5. **Disconnected experience**: jarvy.toml exists but isn't integrated into development tools

## Goals

1. **Zero-friction IDE integration**: Detect missing tools and offer one-click installation
2. **Native CI/CD support**: GitHub Action for standardized CI setup
3. **Dev container ready**: Automatic provisioning in containerized environments
4. **Pre-commit validation**: Ensure tool availability before commits
5. **Consistent experience**: Same jarvy.toml works across all integrations

## User Stories

### US-001: VS Code Extension

**As a developer using VS Code**, I want the editor to detect when tools defined in jarvy.toml are missing so that I can install them without leaving my IDE.

#### Acceptance Criteria

1. Extension activates when jarvy.toml is present in workspace
2. Extension reads and parses jarvy.toml file
3. Extension checks which defined tools are missing from the system
4. Missing tools displayed in Problems panel with severity "Warning"
5. Quick Fix action "Install with Jarvy" available for each missing tool
6. Quick Fix action "Install All Missing Tools" available at file level
7. Status bar item shows tool status (e.g., "Jarvy: 5/7 tools ready")
8. Command palette includes "Jarvy: Install All Tools", "Jarvy: Check Tools"
9. Extension watches jarvy.toml for changes and re-validates
10. Terminal integration: "Jarvy: Open Setup Terminal" command
11. Extension respects VS Code's proxy settings for downloads

### US-002: GitHub Action

**As a DevOps engineer**, I want a GitHub Action that provisions the CI environment from jarvy.toml so that my pipelines use the same tool configuration as local development.

#### Acceptance Criteria

1. Action available at `uses: Cliftonz/jarvy-action@v1`
2. Action installs Jarvy CLI if not present
3. Action runs `jarvy setup` with CI mode enabled
4. Action supports input parameters:
   - `config-path`: Path to jarvy.toml (default: `jarvy.toml`)
   - `tools`: Override specific tools to install
   - `skip-tools`: Tools to skip in CI
   - `cache`: Enable/disable caching (default: true)
5. Action sets up tool caching using `@actions/cache`
6. Action outputs installed tool versions as step outputs
7. Action integrates with GitHub's problem matchers for error reporting
8. Action supports matrix builds (different tool versions per job)
9. Action provides composite action for minimal overhead
10. Detailed logging with GitHub Actions grouping

### US-003: Devcontainer Feature

**As a developer using GitHub Codespaces or VS Code Dev Containers**, I want Jarvy to automatically install my tools when the container starts so that my development environment is ready immediately.

#### Acceptance Criteria

1. Feature published to GitHub Container Registry (ghcr.io)
2. Feature installable via devcontainer.json:
   ```json
   "features": {
     "ghcr.io/Cliftonz/jarvy-devcontainer-feature:1": {}
   }
   ```
3. Feature installs Jarvy CLI during container build
4. Feature supports options:
   - `version`: Jarvy version to install (default: latest)
   - `configPath`: Path to jarvy.toml (default: auto-detect)
   - `installOnCreate`: Run setup during container creation (default: true)
   - `installOnAttach`: Run setup when attaching to existing container (default: false)
5. Feature adds lifecycle scripts for `onCreate`, `postCreate`, `postAttach`
6. Feature works with both Debian-based and Alpine-based images
7. Feature caches tool installations between rebuilds when possible
8. Feature logs clearly indicate installation progress

### US-004: JetBrains Plugin (Lower Priority)

**As a developer using IntelliJ IDEA, WebStorm, or other JetBrains IDEs**, I want plugin support for jarvy.toml so that I have a consistent experience across IDEs.

#### Acceptance Criteria

1. Plugin published to JetBrains Marketplace
2. Plugin provides jarvy.toml syntax highlighting and schema validation
3. Plugin shows missing tools as inspections (warnings)
4. Quick-fix actions to install missing tools
5. Tool window showing tool status
6. Run configuration for `jarvy setup`
7. Plugin works across all IntelliJ-based IDEs (IDEA, WebStorm, PyCharm, RustRover, etc.)

### US-005: Pre-commit Hook Integration

**As a developer using pre-commit framework**, I want to validate that required tools are installed before committing so that I catch environment issues early.

#### Acceptance Criteria

1. Pre-commit hook available in pre-commit hooks repository
2. Hook validates tools defined in jarvy.toml are installed
3. Hook configuration in .pre-commit-config.yaml:
   ```yaml
   - repo: https://github.com/Cliftonz/jarvy
     rev: v1.0.0
     hooks:
       - id: jarvy-check
   ```
4. Hook supports arguments:
   - `--config`: Path to jarvy.toml
   - `--fail-on-missing`: Exit with error if tools missing (default: warning)
   - `--tools`: Check only specific tools
5. Hook outputs clear message about which tools are missing
6. Hook provides actionable guidance (e.g., "Run 'jarvy setup' to install")
7. Hook runs quickly (< 2 seconds for typical configs)
8. Hook works on macOS, Linux, and Windows

## Technical Implementation

### VS Code Extension Architecture

```
jarvy-vscode/
├── package.json              # Extension manifest
├── src/
│   ├── extension.ts          # Entry point
│   ├── config/
│   │   ├── parser.ts         # jarvy.toml parsing
│   │   └── watcher.ts        # File system watcher
│   ├── diagnostics/
│   │   ├── provider.ts       # Diagnostic provider
│   │   └── checker.ts        # Tool availability checker
│   ├── commands/
│   │   ├── install.ts        # Install commands
│   │   └── check.ts          # Check commands
│   ├── ui/
│   │   ├── statusBar.ts      # Status bar item
│   │   └── quickFix.ts       # Quick fix actions
│   └── terminal/
│       └── runner.ts         # Terminal command runner
├── test/
│   └── suite/
└── .vscodeignore
```

**Key Dependencies:**
- `@iarna/toml` - TOML parsing
- `vscode` - VS Code API
- `execa` - Command execution

**Extension Points:**
```typescript
// package.json contributes
{
  "activationEvents": [
    "workspaceContains:jarvy.toml"
  ],
  "contributes": {
    "commands": [
      { "command": "jarvy.installAll", "title": "Jarvy: Install All Tools" },
      { "command": "jarvy.checkTools", "title": "Jarvy: Check Tools" },
      { "command": "jarvy.openTerminal", "title": "Jarvy: Open Setup Terminal" }
    ],
    "configuration": {
      "title": "Jarvy",
      "properties": {
        "jarvy.autoCheck": {
          "type": "boolean",
          "default": true,
          "description": "Automatically check tool availability"
        },
        "jarvy.showStatusBar": {
          "type": "boolean",
          "default": true,
          "description": "Show Jarvy status in status bar"
        }
      }
    }
  }
}
```

### GitHub Action Implementation

```
jarvy-action/
├── action.yml               # Action metadata
├── src/
│   ├── main.ts              # Entry point
│   ├── installer.ts         # Jarvy installer
│   ├── runner.ts            # Setup runner
│   ├── cache.ts             # Tool caching
│   └── outputs.ts           # Output handling
├── dist/
│   └── index.js             # Compiled action
├── .github/
│   └── workflows/
│       └── test.yml         # Action tests
└── package.json
```

**action.yml:**
```yaml
name: 'Jarvy Setup'
description: 'Install development tools from jarvy.toml'
author: 'Bear Binary'
branding:
  icon: 'package'
  color: 'orange'

inputs:
  config-path:
    description: 'Path to jarvy.toml configuration file'
    required: false
    default: 'jarvy.toml'
  tools:
    description: 'Space-separated list of tools to install (overrides jarvy.toml)'
    required: false
  skip-tools:
    description: 'Space-separated list of tools to skip'
    required: false
  cache:
    description: 'Enable tool caching'
    required: false
    default: 'true'
  jarvy-version:
    description: 'Jarvy version to install'
    required: false
    default: 'latest'

outputs:
  tools-installed:
    description: 'JSON object of installed tools and versions'
  cache-hit:
    description: 'Whether the cache was hit'

runs:
  using: 'node20'
  main: 'dist/index.js'
```

**Caching Strategy:**
```typescript
// src/cache.ts
import * as cache from '@actions/cache';

const CACHE_PATHS = [
  '~/.jarvy',
  '~/.cargo',
  '~/.nvm',
  '~/.local/share/mise',
];

export async function restoreCache(configHash: string): Promise<boolean> {
  const cacheKey = `jarvy-tools-${process.platform}-${configHash}`;
  const restoreKeys = [`jarvy-tools-${process.platform}-`];

  const hitKey = await cache.restoreCache(CACHE_PATHS, cacheKey, restoreKeys);
  return hitKey !== undefined;
}

export async function saveCache(configHash: string): Promise<void> {
  const cacheKey = `jarvy-tools-${process.platform}-${configHash}`;
  await cache.saveCache(CACHE_PATHS, cacheKey);
}
```

### Devcontainer Feature Implementation

```
jarvy-devcontainer-feature/
├── src/
│   └── jarvy/
│       ├── devcontainer-feature.json
│       ├── install.sh
│       └── library_scripts.sh
├── test/
│   └── jarvy/
│       ├── test.sh
│       └── scenarios.json
└── README.md
```

**devcontainer-feature.json:**
```json
{
  "id": "jarvy",
  "version": "1.0.0",
  "name": "Jarvy Development Environment",
  "description": "Install development tools from jarvy.toml",
  "documentationURL": "https://jarvy.dev/docs/devcontainer",
  "options": {
    "version": {
      "type": "string",
      "default": "latest",
      "description": "Jarvy version to install"
    },
    "configPath": {
      "type": "string",
      "default": "",
      "description": "Path to jarvy.toml (auto-detected if empty)"
    },
    "installOnCreate": {
      "type": "boolean",
      "default": true,
      "description": "Run jarvy setup during container creation"
    }
  },
  "installsAfter": ["ghcr.io/devcontainers/features/common-utils"]
}
```

**install.sh:**
```bash
#!/bin/bash
set -e

VERSION="${VERSION:-latest}"
CONFIG_PATH="${CONFIGPATH:-}"
INSTALL_ON_CREATE="${INSTALLONCREATE:-true}"

# Install Jarvy
curl -fsSL https://jarvy.dev/install.sh | JARVY_VERSION="$VERSION" bash

# Set up lifecycle script if needed
if [ "$INSTALL_ON_CREATE" = "true" ]; then
  mkdir -p /usr/local/share/jarvy
  cat > /usr/local/share/jarvy/on-create.sh << 'EOF'
#!/bin/bash
if [ -f "${JARVY_CONFIG:-jarvy.toml}" ]; then
  jarvy setup --ci
fi
EOF
  chmod +x /usr/local/share/jarvy/on-create.sh

  # Register lifecycle command
  echo '/usr/local/share/jarvy/on-create.sh' >> /usr/local/share/jarvy-lifecycle-commands
fi
```

### Pre-commit Hook Implementation

```
# In Jarvy main repository
hooks/
├── .pre-commit-hooks.yaml
└── jarvy-check.sh
```

**.pre-commit-hooks.yaml:**
```yaml
- id: jarvy-check
  name: Check Jarvy Tools
  description: Verify tools from jarvy.toml are installed
  entry: jarvy get --check
  language: system
  files: ''
  pass_filenames: false
  always_run: true
  stages: [pre-commit, pre-push]
```

**Jarvy CLI Enhancement:**
```rust
// src/commands/get.rs
#[derive(Args)]
pub struct GetArgs {
    /// Check mode: exit with error if tools are missing
    #[arg(long)]
    pub check: bool,

    /// Tools to check (default: all from jarvy.toml)
    #[arg(long)]
    pub tools: Option<Vec<String>>,
}

pub fn run_check_mode(args: &GetArgs) -> Result<(), ExitCode> {
    let config = Config::load()?;
    let missing = check_missing_tools(&config)?;

    if missing.is_empty() {
        println!("All {} tools installed", config.tools.len());
        Ok(())
    } else {
        eprintln!("Missing tools: {}", missing.join(", "));
        eprintln!("Run 'jarvy setup' to install");
        Err(ExitCode::from(1))
    }
}
```

### JetBrains Plugin Architecture

```
jarvy-jetbrains/
├── src/main/
│   ├── kotlin/
│   │   └── dev/jarvy/plugin/
│   │       ├── JarvyBundle.kt           # Plugin bundle
│   │       ├── config/
│   │       │   └── JarvyConfigParser.kt  # TOML parsing
│   │       ├── inspection/
│   │       │   └── MissingToolInspection.kt
│   │       ├── quickfix/
│   │       │   └── InstallToolQuickFix.kt
│   │       ├── toolwindow/
│   │       │   └── JarvyToolWindow.kt
│   │       └── run/
│   │           └── JarvyRunConfiguration.kt
│   └── resources/
│       ├── META-INF/
│       │   └── plugin.xml
│       └── messages/
│           └── JarvyBundle.properties
├── build.gradle.kts
└── gradle.properties
```

**plugin.xml:**
```xml
<idea-plugin>
  <id>dev.jarvy.plugin</id>
  <name>Jarvy</name>
  <vendor>Bear Binary</vendor>

  <depends>com.intellij.modules.platform</depends>
  <depends>org.toml.lang</depends>

  <extensions defaultExtensionNs="com.intellij">
    <localInspection
      language="TOML"
      implementationClass="dev.jarvy.plugin.inspection.MissingToolInspection"
      displayName="Missing Jarvy tool"
      groupName="Jarvy"
      enabledByDefault="true"
      level="WARNING"/>

    <toolWindow
      id="Jarvy"
      factoryClass="dev.jarvy.plugin.toolwindow.JarvyToolWindowFactory"
      anchor="bottom"/>

    <configurationType
      implementation="dev.jarvy.plugin.run.JarvyConfigurationType"/>
  </extensions>

  <actions>
    <action id="Jarvy.InstallAll"
            class="dev.jarvy.plugin.actions.InstallAllAction"
            text="Install All Jarvy Tools"/>
  </actions>
</idea-plugin>
```

## Non-Goals

- **Vim/Neovim plugin**: Users can shell out to `jarvy` commands; ecosystem has sufficient tooling
- **Emacs integration**: Same rationale as Vim
- **Sublime Text plugin**: Lower market share, users can use terminal
- **Eclipse plugin**: Legacy IDE, minimal demand
- **Browser-based IDE support** (other than Codespaces): Out of scope for initial release

## Implementation Phases

### Phase 1: Core CLI Enhancements (1 week)
1. Add `jarvy get --check` command for pre-commit hook
2. Add `--format json` output for machine parsing
3. Ensure CI mode works correctly for all integrations

### Phase 2: GitHub Action (1 week)
1. Create action repository structure
2. Implement core action logic
3. Add caching support
4. Write comprehensive tests
5. Publish to GitHub Marketplace

### Phase 3: VS Code Extension (2 weeks)
1. Set up extension scaffolding
2. Implement jarvy.toml parser
3. Create diagnostic provider
4. Implement quick fixes
5. Add status bar and commands
6. Publish to VS Code Marketplace

### Phase 4: Devcontainer Feature (1 week)
1. Create feature structure
2. Implement install script
3. Add lifecycle hooks
4. Test with Codespaces
5. Publish to ghcr.io

### Phase 5: Pre-commit Hook (0.5 weeks)
1. Add .pre-commit-hooks.yaml to main repo
2. Document usage
3. Test with pre-commit framework

### Phase 6: JetBrains Plugin (2 weeks, lower priority)
1. Set up Gradle project
2. Implement TOML inspection
3. Add tool window
4. Implement run configuration
5. Publish to JetBrains Marketplace

## Success Metrics

| Metric | Target |
|--------|--------|
| VS Code extension installs | 1,000 in first month |
| GitHub Action usage | 500 workflows in first month |
| Devcontainer feature pulls | 200 in first month |
| Pre-commit hook adopters | 100 repos in first month |
| Issue reports (integration bugs) | < 10 critical in first month |

## Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| VS Code API changes | Low | Medium | Pin VS Code version, monitor changelog |
| GitHub Action rate limits | Medium | Medium | Implement caching, use composite action |
| Devcontainer feature compatibility | Medium | Medium | Test across multiple base images |
| JetBrains platform fragmentation | Medium | Low | Use common API across IDEs |
| Pre-commit performance issues | Low | Medium | Optimize check command |

## Dependencies

- VS Code Extension: Node.js, VS Code Extension API
- GitHub Action: @actions/core, @actions/cache, @actions/exec
- Devcontainer Feature: Devcontainer CLI, GitHub Container Registry
- JetBrains Plugin: IntelliJ Platform SDK, Kotlin
- Pre-commit Hook: pre-commit framework

## Effort Estimate

| Component | Effort | Priority |
|-----------|--------|----------|
| CLI enhancements (--check) | 2 days | P0 |
| GitHub Action | 5 days | P0 |
| VS Code Extension | 8 days | P0 |
| Devcontainer Feature | 4 days | P1 |
| Pre-commit Hook | 2 days | P1 |
| JetBrains Plugin | 8 days | P2 |
| Documentation | 3 days | P0 |
| **Total** | **~32 days** | |

## Files to Create

### Repositories
```
github.com/Cliftonz/jarvy-action/
github.com/Cliftonz/jarvy-vscode/
github.com/Cliftonz/jarvy-devcontainer-feature/
github.com/Cliftonz/jarvy-jetbrains/
```

### Main Repository
```
hooks/
├── .pre-commit-hooks.yaml
└── README.md
```

### Documentation
```
docs/integrations/
├── vscode.md
├── github-action.md
├── devcontainer.md
├── jetbrains.md
└── pre-commit.md
```
