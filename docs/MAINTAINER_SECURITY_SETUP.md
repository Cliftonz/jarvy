# Security Infrastructure Manual Setup Guide

This document describes the manual setup required to fully enable the security infrastructure implemented in PRD-020.

## Overview

The following components require manual configuration in GitHub and your development environment:

1. GitHub Repository Settings
2. Branch Protection Rules
3. Secret Scanning Configuration
4. Dependabot Configuration
5. OpenSSF Scorecard Badge
6. Local Development Setup

## 1. GitHub Repository Settings

### Enable GitHub Security Features

Navigate to: **Settings → Security → Code security and analysis**

Enable the following features:

| Feature | Setting | Purpose |
|---------|---------|---------|
| Dependency graph | Enabled | Tracks dependencies |
| Dependabot alerts | Enabled | CVE notifications |
| Dependabot security updates | Enabled | Auto-fix PRs |
| Secret scanning | Enabled | Detect leaked secrets |
| Secret scanning push protection | Enabled | Block secret commits |
| Code scanning | Enabled | SARIF upload from CI |

### Configure Secret Scanning

Navigate to: **Settings → Security → Code security and analysis → Secret scanning**

1. Enable "Push protection"
2. Review and add custom patterns if needed
3. Configure bypass permissions (admins only recommended)

## 2. Branch Protection Rules

Navigate to: **Settings → Branches → Branch protection rules → Add rule**

### Main Branch Protection

Create a rule for `main`:

```
Branch name pattern: main
```

Enable the following settings:

**Protect matching branches:**
- [x] Require a pull request before merging
  - [x] Require approvals: 1 (or more for teams)
  - [x] Dismiss stale pull request approvals when new commits are pushed
  - [x] Require review from Code Owners
- [x] Require status checks to pass before merging
  - [x] Require branches to be up to date before merging
  - Add required status checks:
    - `Security Audit (cargo-audit)`
    - `Dependency Check (cargo-deny)`
    - `Secret Scanning (Gitleaks)`
    - `SAST (Semgrep)`
    - `SAST (CodeQL)`
    - `Security Summary`
- [x] Require conversation resolution before merging
- [x] Require signed commits (optional but recommended)
- [x] Require linear history (optional)
- [x] Do not allow bypassing the above settings

**Rules applied to everyone including administrators:**
- [x] Enabled (recommended for high-security projects)

## 3. Dependabot Configuration

Create `.github/dependabot.yml` if not already present:

```yaml
version: 2
updates:
  # Rust dependencies
  - package-ecosystem: "cargo"
    directory: "/"
    schedule:
      interval: "weekly"
      day: "monday"
    open-pull-requests-limit: 10
    labels:
      - "dependencies"
      - "rust"
    reviewers:
      - "Cliftonz/maintainers"  # Replace with your team
    commit-message:
      prefix: "chore(deps):"

  # GitHub Actions
  - package-ecosystem: "github-actions"
    directory: "/"
    schedule:
      interval: "weekly"
      day: "monday"
    open-pull-requests-limit: 5
    labels:
      - "dependencies"
      - "github-actions"
    commit-message:
      prefix: "chore(ci):"
```

## 4. OpenSSF Scorecard Badge

### Enable Scorecard Results Publishing

The `scorecard.yml` workflow automatically publishes results when:
```yaml
publish_results: true
```

### Add Badge to README

Add the following badge to your `README.md`:

```markdown
[![OpenSSF Scorecard](https://api.securityscorecards.dev/projects/github.com/Cliftonz/jarvy/badge)](https://securityscorecards.dev/viewer/?uri=github.com/Cliftonz/jarvy)
```

### View Scorecard Results

After the workflow runs, view results at:
```
https://securityscorecards.dev/viewer/?uri=github.com/Cliftonz/jarvy
```

## 5. CODEOWNERS File (Recommended)

Create `.github/CODEOWNERS` for security-related files:

```
# Security files require security team review
/deny.toml @Cliftonz
/.gitleaks.toml @Cliftonz
/SECURITY.md @Cliftonz
/.github/workflows/security.yml @Cliftonz
/.github/workflows/scorecard.yml @Cliftonz
/.github/workflows/release.yml @Cliftonz @Cliftonz
/.pre-commit-config.yaml @Cliftonz
/scripts/security/ @Cliftonz
```

## 6. Local Development Setup

### Install Pre-commit Hooks

```bash
# Install pre-commit
pip install pre-commit

# Or with Homebrew
brew install pre-commit

# Install hooks
cd /path/to/jarvy
pre-commit install

# Optionally install commit-msg hook
pre-commit install --hook-type commit-msg
```

### Install Security Tools

```bash
# cargo-audit
cargo install cargo-audit

# cargo-deny
cargo install cargo-deny

# cargo-geiger (optional)
cargo install cargo-geiger

# Gitleaks
brew install gitleaks
# Or: https://github.com/gitleaks/gitleaks#installing

# cosign (for signature verification)
brew install cosign
# Or: https://docs.sigstore.dev/cosign/installation
```

### Run Security Checks Locally

```bash
# Full security audit
cargo audit

# License and dependency checks
cargo deny check all

# Secret scanning
gitleaks detect

# Unsafe code report
cargo geiger --all-features

# All pre-commit hooks
pre-commit run --all-files
```

## 7. Troubleshooting

### cargo-deny Failures

If `cargo-deny` fails with license errors:

1. Check `deny.toml` for the allowed licenses list
2. Add legitimate exceptions with justification:
   ```toml
   [[licenses.clarify]]
   name = "problematic-crate"
   expression = "MIT"
   license-files = [{ path = "LICENSE", hash = 0x12345678 }]
   ```

### Gitleaks False Positives

If Gitleaks flags false positives:

1. Add patterns to `.gitleaks.toml`:
   ```toml
   [allowlist]
   regexes = [
       '''specific-false-positive-pattern'''
   ]
   ```

2. Or add file paths:
   ```toml
   [allowlist]
   paths = [
       '''path/to/file\.ext$'''
   ]
   ```

### Semgrep Timeouts

If Semgrep times out on large files:

1. Add to `.semgrepignore`:
   ```
   # Large generated files
   target/
   **/generated/*.rs
   ```

### CodeQL Build Failures

If CodeQL can't build the project:

1. Ensure all build dependencies are in the workflow
2. Check that the build command matches local builds
3. Consider adding `continue-on-error: true` for non-blocking analysis

## 8. Security Response Process

### When a Security Alert is Triggered

1. **Immediate Assessment** (within 24 hours)
   - Determine severity (Critical/High/Medium/Low)
   - Identify affected versions
   - Assess exploitability

2. **Mitigation**
   - For CVEs: Update dependency or add to ignore list with justification
   - For secrets: Rotate immediately, add to `.gitleaks.toml` allowlist if false positive
   - For SAST findings: Fix code or add exception with documentation

3. **Communication**
   - Critical/High: Private security advisory
   - Medium/Low: Public issue with security label

### CVE Response Timeline

| Severity | Response Time | Fix Time |
|----------|---------------|----------|
| Critical | 4 hours | 24 hours |
| High | 24 hours | 7 days |
| Medium | 48 hours | 30 days |
| Low | 7 days | 90 days |

## 9. Checklist Summary

Before your first release with security infrastructure:

- [ ] GitHub Security features enabled
- [ ] Branch protection rules configured
- [ ] Required status checks added
- [ ] Dependabot configured
- [ ] CODEOWNERS file created
- [ ] OpenSSF Scorecard badge added to README
- [ ] Pre-commit hooks installed locally
- [ ] Security tools installed locally
- [ ] Test run of `cargo deny check all` passes
- [ ] Test run of `gitleaks detect` passes
- [ ] Release workflow tested (creates signed artifacts)

## 10. References

- [GitHub Security Features](https://docs.github.com/en/code-security)
- [OpenSSF Scorecard](https://securityscorecards.dev/)
- [cargo-deny Documentation](https://embarkstudios.github.io/cargo-deny/)
- [Gitleaks Documentation](https://gitleaks.io/)
- [Sigstore Documentation](https://docs.sigstore.dev/)
- [SLSA Framework](https://slsa.dev/)
- [SPDX Specification](https://spdx.dev/)
- [CycloneDX Specification](https://cyclonedx.org/)
