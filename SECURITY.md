# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 1.x.x   | :white_check_mark: |
| 0.x.x   | :white_check_mark: |
| < 0.1   | :x:                |

## Reporting a Vulnerability

**Please do not report security vulnerabilities through public GitHub issues.**

Instead, please report them via one of the following methods:

### GitHub Private Vulnerability Reporting (Preferred)

Use GitHub's built-in private vulnerability reporting feature:
https://github.com/Cliftonz/jarvy/security/advisories/new

### Email

Email: security@jarvy.dev (or open a private advisory if email is unavailable)

### Response Timeline

- **Initial response**: Within 48 hours
- **Status update**: Within 5 business days
- **Resolution target**: Within 90 days (depending on severity)

If you do not receive a response within 48 hours, please follow up via email or create a new private advisory to ensure we received your original report.

## What to Include

Please provide the following information to help us triage and respond effectively:

- **Type of issue**: (e.g., command injection, path traversal, privilege escalation, etc.)
- **Affected component**: (e.g., tool installation, hook execution, config parsing)
- **Full paths of source file(s)** related to the issue
- **Location**: Tag/branch/commit or direct URL to affected source code
- **Step-by-step reproduction instructions**
- **Proof-of-concept or exploit code** (if available)
- **Impact assessment**: How an attacker might exploit this vulnerability
- **Suggested fix**: (if you have one)

## Security Measures

This project implements comprehensive security practices:

### Static Application Security Testing (SAST)

- **cargo-audit**: RustSec advisory database scanning for known CVEs
- **cargo-deny**: License compliance, advisory scanning, and crate bans
- **Semgrep**: Custom security rules for Rust code patterns
- **CodeQL**: GitHub-native semantic analysis

### Secret Scanning

- **GitHub Secret Scanning**: Native detection of leaked credentials
- **Gitleaks**: Pre-commit and CI scanning for secrets

### Dependency Security

- **Automated vulnerability scanning**: Daily checks against RustSec database
- **License compliance**: All dependencies must have OSS-compatible licenses
- **Source restrictions**: Only crates.io registry allowed (no arbitrary git deps)

### Supply Chain Security

- **Signed releases**: All release artifacts signed with Sigstore (cosign)
- **SBOM generation**: SPDX and CycloneDX Software Bill of Materials
- **Build provenance**: SLSA Level 2+ attestation via GitHub Actions
- **Reproducible builds**: Deterministic build process

### OpenSSF Scorecard

We maintain an OpenSSF Scorecard for public security posture reporting:
- Target score: 8.0+ out of 10
- Badge displayed in README

## Allowed Licenses

Dependencies must use one of the following licenses:

- MIT
- Apache-2.0
- BSD-2-Clause
- BSD-3-Clause
- ISC
- Zlib
- CC0-1.0
- Unlicense
- MPL-2.0 (weak copyleft, file-level only)

GPL, AGPL, SSPL, and proprietary licenses are blocked.

## Verification

### Verifying Release Signatures

All releases are signed with Sigstore. To verify:

```bash
# Install cosign
brew install cosign  # or see https://docs.sigstore.dev/cosign/installation

# Download release artifact and signature
curl -LO https://github.com/Cliftonz/jarvy/releases/download/v1.0.0/jarvy-linux-x86_64.tar.gz
curl -LO https://github.com/Cliftonz/jarvy/releases/download/v1.0.0/jarvy-linux-x86_64.tar.gz.sig
curl -LO https://github.com/Cliftonz/jarvy/releases/download/v1.0.0/jarvy-linux-x86_64.tar.gz.pem

# Verify signature
cosign verify-blob \
  --signature jarvy-linux-x86_64.tar.gz.sig \
  --certificate jarvy-linux-x86_64.tar.gz.pem \
  --certificate-identity-regexp "https://github.com/Cliftonz/jarvy/" \
  --certificate-oidc-issuer "https://token.actions.githubusercontent.com" \
  jarvy-linux-x86_64.tar.gz
```

### Verifying Checksums

```bash
# Download checksums file
curl -LO https://github.com/Cliftonz/jarvy/releases/download/v1.0.0/SHA256SUMS.txt

# Verify
sha256sum -c SHA256SUMS.txt
```

## Security Best Practices for Users

When using Jarvy:

1. **Review jarvy.toml before running**: Especially when using third-party configs
2. **Review hooks before execution**: Use `--dry-run` to preview what will run
3. **Use `--no-hooks`** when running untrusted configurations
4. **Keep Jarvy updated**: Security fixes are released promptly
5. **Verify downloads**: Check signatures and checksums for manual installs

## Security Acknowledgments

We thank the following individuals for responsibly disclosing security issues:

| Reporter | Issue | Disclosed | Fixed |
|----------|-------|-----------|-------|
| (None yet) | | | |

If you've reported a security issue and would like to be acknowledged, please let us know in your report.

## Security Contact

For security-related questions or concerns:

- **Private advisory**: https://github.com/Cliftonz/jarvy/security/advisories/new
- **Email**: security@jarvy.dev

For general questions, please use [GitHub Discussions](https://github.com/Cliftonz/jarvy/discussions).
