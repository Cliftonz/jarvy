# PRD-034: Enhanced Tool Dependency System

## Overview

Expand the tool dependency system to support both strict (required) and flexible (one-of-many) dependencies. This enables more accurate dependency modeling for tools that have multiple valid runtime providers.

## Problem Statement

The current `depends_on` field treats all dependencies as strict requirements. However, real-world tool dependencies often have flexibility:

**Current Limitation:**
```rust
// kubectl depends on BOTH minikube AND kind
// But in reality, kubectl just needs ANY Kubernetes cluster
depends_on: &["minikube", "kind"],
```

**Real-World Scenarios:**

1. **kubectl** - Needs a Kubernetes cluster, which could come from:
   - minikube
   - kind
   - Docker Desktop (has built-in K8s)
   - Rancher Desktop
   - Podman
   - k3d
   - Cloud provider CLI (already configured)

2. **helm** - Needs kubectl/kubeconfig, similar flexibility

3. **JVM languages** - Need a JVM, which could come from:
   - java (OpenJDK)
   - graalvm
   - temurin
   - corretto
   - zulu

4. **Container tools** - Need a container runtime:
   - docker
   - podman
   - nerdctl

## Proposed Solution

### Dependency Types

Introduce two dependency modes:

| Type | Description | Example |
|------|-------------|---------|
| **Strict** (`requires`) | All listed tools must be installed | lazydocker requires docker |
| **Flexible** (`requires_one_of`) | At least one of the listed tools must be installed | kubectl requires_one_of [minikube, kind, docker] |

### Syntax Options

#### Option A: Structured Dependency Object

```rust
define_tool!(KUBECTL, {
    command: "kubectl",
    macos: { brew: "kubectl" },
    dependencies: {
        // Strict: all of these must be present
        requires: &[],
        // Flexible: at least one of these must be present
        requires_one_of: &["minikube", "kind", "docker", "podman", "rancher-desktop"],
    },
});
```

#### Option B: Tagged Dependencies (Recommended)

```rust
define_tool!(KUBECTL, {
    command: "kubectl",
    macos: { brew: "kubectl" },
    // Strict dependencies - must install all
    depends_on: &["some-required-tool"],
    // Flexible dependencies - install first available, or warn if none present
    depends_on_one_of: &["minikube", "kind", "docker", "podman"],
});
```

#### Option C: Enum-Based Dependencies

```rust
define_tool!(KUBECTL, {
    command: "kubectl",
    macos: { brew: "kubectl" },
    dependencies: &[
        Dependency::Strict("required-tool"),
        Dependency::OneOf(&["minikube", "kind", "docker"]),
    ],
});
```

### Recommended Approach: Option B

Option B is recommended because:
- Backward compatible with existing `depends_on` field
- Clear semantic distinction between `depends_on` and `depends_on_one_of`
- Macro syntax remains simple and readable
- No new enum types needed

## Detailed Design

### ToolSpec Changes

```rust
pub struct ToolSpec {
    // ... existing fields ...

    /// Strict dependencies - ALL tools in this list must be installed before this tool.
    /// Used for tools that have a single required dependency (e.g., lazydocker -> docker).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depends_on: Option<&'static [&'static str]>,

    /// Flexible dependencies - AT LEAST ONE tool from this list must be installed.
    /// Used for tools that can work with multiple providers (e.g., kubectl -> any K8s cluster).
    /// Installation behavior:
    /// - If one is already installed: proceed (dependency satisfied)
    /// - If none installed but one in config: install the first one in config
    /// - If none installed and none in config: warn but proceed (user may have external setup)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depends_on_one_of: Option<&'static [&'static str]>,
}
```

### Macro Updates

```rust
#[macro_export]
macro_rules! define_tool {
    ($name:ident, {
        command: $cmd:expr,
        $(macos: { $($macos_key:ident: $macos_val:expr),* $(,)? },)?
        $(linux: { $($linux_key:ident: $linux_val:expr),* $(,)? },)?
        $(windows: { $($windows_key:ident: $windows_val:expr),* $(,)? },)?
        $(custom_install: $custom:expr,)?
        $(default_hook: { description: $hook_desc:expr, script: $hook_script:expr },)?
        $(depends_on: $deps:expr,)?
        $(depends_on_one_of: $flex_deps:expr,)?
    }) => {
        // ... generate ToolSpec with both dependency types
    };
}
```

### Dependency Resolution Algorithm

```rust
pub enum DependencyCheckResult {
    /// All dependencies satisfied
    Satisfied,
    /// Missing strict dependencies
    MissingRequired(Vec<String>),
    /// No flexible dependency satisfied, but one is in config (will install)
    WillInstallFlexible(String),
    /// No flexible dependency satisfied or in config (warning)
    MissingFlexible {
        needed_one_of: Vec<String>,
        suggestion: Option<String>,
    },
}

pub fn check_dependencies(
    tool: &str,
    config_tools: &[String],
    installed_tools: &HashSet<String>,
) -> DependencyCheckResult {
    let spec = get_tool_spec(tool)?;

    // Check strict dependencies
    if let Some(required) = spec.depends_on {
        let missing: Vec<_> = required.iter()
            .filter(|dep| !installed_tools.contains(*dep) && !config_tools.contains(&dep.to_string()))
            .collect();
        if !missing.is_empty() {
            return DependencyCheckResult::MissingRequired(missing);
        }
    }

    // Check flexible dependencies
    if let Some(one_of) = spec.depends_on_one_of {
        // First check if any is already installed
        if one_of.iter().any(|dep| installed_tools.contains(*dep)) {
            return DependencyCheckResult::Satisfied;
        }

        // Check if any is in the config (will be installed)
        if let Some(will_install) = one_of.iter()
            .find(|dep| config_tools.contains(&dep.to_string()))
        {
            return DependencyCheckResult::WillInstallFlexible(will_install.to_string());
        }

        // None available - warn but allow (user may have external setup)
        return DependencyCheckResult::MissingFlexible {
            needed_one_of: one_of.iter().map(|s| s.to_string()).collect(),
            suggestion: Some(one_of[0].to_string()), // Suggest first option
        };
    }

    DependencyCheckResult::Satisfied
}
```

### Installation Ordering Updates

Update `order_tools_by_dependencies()` to handle both dependency types:

```rust
pub fn order_tools_by_dependencies<'a, I>(tools: I) -> Vec<(String, String)>
where
    I: Iterator<Item = (&'a str, &'a str)>,
{
    let tool_list: Vec<_> = tools.collect();
    let tool_set: HashSet<&str> = tool_list.iter().map(|(n, _)| *n).collect();

    // Build dependency graph
    let mut in_degree: HashMap<&str, usize> = HashMap::new();
    let mut dependents: HashMap<&str, Vec<&str>> = HashMap::new();

    for (name, _) in &tool_list {
        in_degree.entry(*name).or_insert(0);

        // Add strict dependencies
        for dep in get_tool_dependencies(name) {
            if tool_set.contains(dep) {
                *in_degree.entry(*name).or_insert(0) += 1;
                dependents.entry(*dep).or_default().push(*name);
            }
        }

        // Add flexible dependencies (only the FIRST one in config)
        if let Some(flex_deps) = get_tool_flexible_dependencies(name) {
            if let Some(chosen_dep) = flex_deps.iter().find(|d| tool_set.contains(*d)) {
                *in_degree.entry(*name).or_insert(0) += 1;
                dependents.entry(*chosen_dep).or_default().push(*name);
            }
        }
    }

    // Kahn's algorithm (existing implementation)
    // ...
}
```

## User Stories

### US-001: Strict Dependencies (Existing, Enhanced)

**As a developer**, I want to specify that a tool requires ALL of certain dependencies so that installation fails or warns appropriately if they're missing.

**Acceptance Criteria:**
- [ ] `depends_on: &["tool1", "tool2"]` requires both tool1 AND tool2
- [ ] Missing strict dependency in config triggers a warning/error
- [ ] Installation order respects strict dependencies
- [ ] Tests cover strict dependency scenarios

### US-002: Flexible Dependencies (New)

**As a developer**, I want to specify that a tool requires ONE OF several possible dependencies so that users have flexibility in their setup.

**Acceptance Criteria:**
- [ ] `depends_on_one_of: &["tool1", "tool2", "tool3"]` accepts any one of the options
- [ ] If one option is already installed, dependency is satisfied
- [ ] If no option installed but one in config, install that one first
- [ ] If no option installed or in config, warn but proceed (advisory)
- [ ] Tests cover flexible dependency scenarios

### US-003: Doctor Command Integration

**As a user**, I want `jarvy doctor` to show me which flexible dependencies are satisfied and which options are available.

**Acceptance Criteria:**
- [ ] Doctor shows "kubectl: needs K8s cluster (satisfied by: docker)"
- [ ] Doctor shows "kubectl: needs K8s cluster (MISSING - install one of: minikube, kind, docker)"
- [ ] JSON output includes dependency satisfaction status

### US-004: Diff Command Integration

**As a user**, I want `jarvy diff` to show which flexible dependency will be installed to satisfy a requirement.

**Acceptance Criteria:**
- [ ] Diff shows "kubectl requires K8s cluster - will use: minikube (in config)"
- [ ] Diff warns if no flexible dependency will be satisfied

## Tool Dependency Mappings

### High Priority Tools

| Tool | Strict (`depends_on`) | Flexible (`depends_on_one_of`) |
|------|----------------------|-------------------------------|
| lazydocker | docker | - |
| kind | docker | - |
| minikube | - | docker, podman |
| kubectl | - | minikube, kind, docker, podman, rancher-desktop, k3d |
| helm | - | kubectl (which has its own flex deps) |
| k9s | - | kubectl |
| stern | - | kubectl |
| flux | - | kubectl |
| argocd | - | kubectl |

### Medium Priority Tools

| Tool | Strict (`depends_on`) | Flexible (`depends_on_one_of`) |
|------|----------------------|-------------------------------|
| kotlin | - | java, graalvm, temurin |
| scala | - | java, graalvm, temurin |
| gradle | - | java, graalvm, temurin |
| maven | - | java, graalvm, temurin |
| elixir | erlang | - |

### Container Ecosystem

| Tool | Strict (`depends_on`) | Flexible (`depends_on_one_of`) |
|------|----------------------|-------------------------------|
| docker-compose | - | docker, podman |
| buildah | - | (standalone, no deps) |
| skopeo | - | (standalone, no deps) |
| dive | - | docker, podman |
| ctop | - | docker, podman |

## Implementation Plan

### Phase 1: Core Infrastructure (P0)

1. Update `ToolSpec` struct with `depends_on_one_of` field
2. Update `define_tool!` macro to support new field
3. Implement `get_tool_flexible_dependencies()` function
4. Update `check_dependencies()` with new algorithm
5. Update `order_tools_by_dependencies()` for flexible deps
6. Add unit tests for dependency resolution

### Phase 2: Tool Updates (P1)

1. Update kubectl with flexible K8s cluster dependencies
2. Update minikube with flexible container runtime dependency
3. Update helm, k9s, stern, flux, argocd with kubectl dependency
4. Add java/JVM tools if they exist, with flexible JVM dependency

### Phase 3: CLI Integration (P2)

1. Update `jarvy doctor` to show dependency satisfaction
2. Update `jarvy diff` to show flexible dependency resolution
3. Update `jarvy validate` to check dependency configuration
4. Add `--ignore-missing-deps` flag for advanced users

### Phase 4: Documentation (P3)

1. Update CLAUDE.md with new dependency syntax
2. Add examples to tool implementation guide
3. Document dependency resolution algorithm
4. Add troubleshooting section for dependency issues

## Success Metrics

| Metric | Target |
|--------|--------|
| Tools with accurate dependency modeling | 100% of container/K8s tools |
| False dependency warnings | < 5% of setups |
| User confusion about dependency resolution | Minimal (clear messaging) |

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Complex dependency chains | Medium | Medium | Limit to 2 levels of indirection |
| User confusion about flexible deps | Medium | Low | Clear messaging in doctor/diff |
| Performance impact on large configs | Low | Low | Dependencies are static, cached |
| Backward compatibility | Low | Medium | New field is additive, existing `depends_on` unchanged |

## Files to Modify

- `src/tools/spec.rs` - Add `depends_on_one_of` field, update ordering functions
- `src/tools/common.rs` - Add dependency checking utilities
- `src/main.rs` - Update installation flow for flexible deps
- `src/commands/doctor.rs` - Show dependency satisfaction
- `src/commands/diff.rs` - Show dependency resolution
- Tool files: kubectl, minikube, helm, k9s, etc.
- `tests/` - Add dependency resolution tests

## Effort Estimate

| Phase | Effort | Priority |
|-------|--------|----------|
| Phase 1: Core Infrastructure | 2-3 days | P0 |
| Phase 2: Tool Updates | 1 day | P1 |
| Phase 3: CLI Integration | 1-2 days | P2 |
| Phase 4: Documentation | 0.5 days | P3 |
| **Total** | **5-7 days** | |

## Open Questions

1. **Should flexible deps be advisory or blocking?**
   - Recommendation: Advisory with clear warning (user may have external K8s cluster)

2. **How to handle nested flexible deps (kubectl -> K8s cluster, K8s cluster -> container runtime)?**
   - Recommendation: Flatten to single level; don't chain flexible deps

3. **Should we add a "provided_by" field for external tools?**
   - Example: `provided_by_external: &["Docker Desktop", "Rancher Desktop"]`
   - Recommendation: Defer to future PRD; keep scope focused

## Appendix: Dependency Graph Examples

### Example 1: K8s Development Stack

```
User config: [kubectl, helm, k9s, minikube, docker]

Dependency resolution:
1. docker (no deps)
2. minikube (flex dep on docker - satisfied)
3. kubectl (flex dep on minikube - satisfied)
4. helm (flex dep on kubectl - satisfied)
5. k9s (flex dep on kubectl - satisfied)
```

### Example 2: Missing Flexible Dependency

```
User config: [kubectl, helm]

Dependency resolution:
1. kubectl (flex dep on [minikube, kind, docker] - NONE in config)
   -> WARNING: kubectl needs a Kubernetes cluster. Consider adding: minikube, kind, or docker
2. helm (flex dep on kubectl - satisfied by kubectl in config)

Result: Installs proceed with warning
```

### Example 3: External Cluster

```
User config: [kubectl, helm]
User has: Docker Desktop with K8s enabled (external to Jarvy)

Dependency resolution:
1. kubectl (flex dep check - NONE in Jarvy config)
   -> WARNING: kubectl needs a Kubernetes cluster...
2. User runs: kubectl get nodes -> Works! (Docker Desktop provides cluster)

Result: Warning was advisory; external setup works fine
```
