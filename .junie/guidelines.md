Junie Guidelines for Jarvy

Purpose
- Provide clear, minimal, and actionable guidance for AI assistants (Junie) and contributors working on this repository.
- Ensure changes align with project conventions, CI, and maintain code quality.

Principles
- Prefer minimal, targeted changes that solve the stated issue.
- Preserve existing behavior and public API unless the issue explicitly requires changes.
- Explain your plan and reasoning in updates and PRs.
- Keep changes reversible and small.

Language and Tooling
- Primary language: Rust (edition 2024)
- Build tool: cargo
- Formatting: cargo fmt --all
- Linting: cargo clippy --all-features -- -D warnings
- Checking: cargo check --verbose
- Testing: cargo test --verbose -- --show-output
- Build release: cargo build --release

Coding Standards
- Follow Rust 2024 idioms. Keep code readable and modular.
- Run cargo fmt and cargo clippy locally before committing.
- Add or update tests when fixing bugs or adding features.
- Avoid unnecessary dependencies; prefer standard library and existing crates already in Cargo.toml when possible.

Commit Messages
- Use Conventional Commits:
  - feat: add new feature
  - fix: fix a bug
  - docs: documentation only changes
  - chore: maintenance tasks (no production code changes)
  - refactor: code change that neither fixes a bug nor adds a feature
  - test: adding or improving tests
  - ci: CI/CD changes
  - build: build system or external dependencies changes
  - perf: performance improvements
- Scope is optional but recommended (e.g., feat(cli): ...).

Branch and PR Workflow
- Create feature branches from main.
- Ensure branch is up to date with main before opening PR.
- PR checklist:
  - [ ] Code formatted (cargo fmt)
  - [ ] Lint clean (cargo clippy -- -D warnings)
  - [ ] Tests pass (cargo test)
  - [ ] Tests added/updated when applicable
  - [ ] README/docs updated if behavior or usage changes
  - [ ] Clear Conventional Commit summary in PR title

CI Expectations
- GitHub Actions workflows present:
  - Test: runs cargo check and cargo test on macOS
  - Rust-Clippy-Analyzer: runs clippy and uploads SARIF
  - Release: builds and packages binaries for multiple targets
- Do not break CI. If CI flags issues, fix locally and push updates.

Versioning and Releases
- Follow semantic versioning in Cargo.toml.
- Changes intended for release should have meaningful Conventional Commits to generate release notes.
- Release workflow is managed via GitHub Actions; do not hardcode version-specific logic in code.

Documentation
- Keep README.md accurate. Update usage docs when CLI behavior or configuration changes.
- Include examples and comments for new modules or public functions.

Testing Guidance
- Prefer unit tests near the code under src/ and integration tests under src/tests/.
- Cover error paths and edge cases (e.g., missing config, malformed TOML, unsupported OS/tool).
- Keep tests deterministic and CI-friendly.

Security and Privacy
- Do not commit secrets.
- Validate and sanitize input from configuration files (e.g., jarvy.toml).
- Avoid executing arbitrary commands from untrusted input.

Performance
- Favor efficient algorithms; avoid unnecessary allocations and blocking operations.
- Use tracing judiciously; keep default logging lightweight.

When Unsure
- Open an issue or draft PR describing the approach.
- Prefer asking for clarification rather than making large, assumption-heavy changes.

Quick Commands
- Format: cargo fmt --all
- Lint: cargo clippy --all-features -- -D warnings
- Check: cargo check --verbose
- Test: cargo test --verbose -- --show-output
- Build (release): cargo build --release

Scope of Junie Sessions
- During automated sessions, adhere to minimal-diff changes that fully satisfy the issue.
- Always describe plan, actions, and results in session updates.
- Avoid modifying CI/release configuration unless the issue specifically asks for it.
