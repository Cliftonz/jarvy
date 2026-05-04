---
title: "Roles Guide - Jarvy"
description: "Configure role-based tool sets for different team members with inheritance, overrides, and the jarvy roles CLI."
---

# Roles Guide

Roles let you define different tool sets for different team members. A frontend developer doesn't need kubectl; a DevOps engineer doesn't need bun. Roles solve this without maintaining separate config files.

## Quick Start

```toml
# Assign a role to this developer
role = "frontend"

# Define what each role needs
[roles.base]
description = "Tools every developer needs"
tools = ["git", "docker", "shellcheck"]

[roles.frontend]
extends = "base"
description = "Frontend development"
tools = ["node", "bun", "python"]

[roles.backend]
extends = "base"
description = "Backend development"
tools = ["go", "python", "kubectl", "helm"]

[roles.devops]
extends = "backend"
description = "DevOps and infrastructure"
tools = ["terraform", "awscli", "k9s", "argocd"]
```

When a frontend developer runs `jarvy setup`, they get: git, docker, shellcheck (from base) + node, bun, python (from frontend).

## How Roles Work

### Assignment

Assign one or multiple roles in your config:

```toml
# Single role
role = "frontend"

# Multiple roles (tools are merged, last wins for version conflicts)
role = ["frontend", "devops"]
```

### Inheritance

Roles can extend a parent role. The child inherits all tools from the parent and adds its own.

```toml
[roles.base]
tools = ["git", "docker"]

[roles.frontend]
extends = "base"              # Inherits git, docker
tools = ["node", "bun"]       # Adds node, bun
# Effective tools: git, docker, node, bun

[roles.senior-frontend]
extends = "frontend"          # Inherits git, docker, node, bun
tools = ["kubectl"]           # Adds kubectl
# Effective tools: git, docker, node, bun, kubectl
```

Inheritance supports up to 5 levels deep to prevent circular references.

### Version Overrides

Roles can specify versions for their tools:

```toml
[roles.frontend]
extends = "base"
tools = ["node", "bun"]

[roles.frontend.tools]
node = "20"
bun = "latest"
```

### Direct Tools Always Win

Tools in the `[provisioner]` section always override role-provided tools:

```toml
role = "frontend"

[provisioner]
node = "22"     # This version wins over the role's "20"

[roles.frontend]
tools = ["node"]

[roles.frontend.tools]
node = "20"     # Overridden by [provisioner]
```

## CLI Commands

### List Roles

```bash
# List all defined roles
jarvy roles list

# Verbose output with tool counts
jarvy roles list -v
```

### Show Role Details

```bash
# Show a role's direct tools
jarvy roles show frontend

# Show resolved tools (including inherited)
jarvy roles show frontend --resolved

# Show inheritance chain
jarvy roles show frontend --inheritance
```

### Compare Roles

```bash
# See differences between two roles
jarvy roles diff frontend backend
```

Output shows tools unique to each role and shared tools.

### Override at Runtime

```bash
# Use a different role for a single run
jarvy setup --role backend
```

This doesn't modify your config file.

## Patterns

### Team Structure

```toml
[roles.base]
tools = ["git", "docker", "pre-commit", "shellcheck"]

[roles.frontend]
extends = "base"
tools = ["node", "bun", "python"]

[roles.backend]
extends = "base"
tools = ["go", "python", "kubectl", "helm"]

[roles.mobile]
extends = "base"
tools = ["node", "java", "kotlin"]

[roles.devops]
extends = "backend"
tools = ["terraform", "awscli", "argocd", "k9s", "trivy"]

[roles.data]
extends = "base"
tools = ["python", "duckdb", "julia"]
```

### Per-Role Onboarding

Different roles can have different hooks:

```toml
[hooks.config]
continue_on_error = true

[hooks.node]
post_install = "npm install -g typescript"

[hooks.go]
post_install = "go install golang.org/x/tools/gopls@latest"
```

Hooks run only when their tool is installed — so role-specific tools get role-specific hooks automatically.
