# PRD-018: Quality and Testing Infrastructure

## Overview

Establish comprehensive testing infrastructure that ensures reliability through high code coverage, property-based testing, fuzz testing, mutation testing, benchmarks, and robust CI integration.

## Problem Statement

While PRD-006 addressed basic testing infrastructure and mocking, Jarvy needs a more comprehensive quality assurance strategy:

1. **Coverage gaps**: Current test coverage is not measured or enforced
2. **No property testing**: Config parsing not tested with randomized inputs
3. **No fuzz testing**: TOML parser edge cases could crash or behave unexpectedly
4. **Test quality unknown**: No mutation testing to verify tests catch real bugs
5. **Performance regressions**: No benchmarks to detect slowdowns
6. **CI gaps**: Coverage not reported, no test containers for integration tests

## Goals

1. **80%+ unit test coverage** across all modules
2. **Property-based testing** for config parsing and version matching
3. **Fuzz testing** for TOML parser edge cases
4. **Mutation testing** to verify test effectiveness
5. **Benchmark suite** to prevent performance regressions
6. **Code coverage reporting** in CI with PR integration
7. **Test containers** for realistic integration testing

## User Stories

### US-001: Unit Test Coverage Target (80%+)

**As a** maintainer,
**I want** 80%+ unit test coverage enforced in CI,
**So that** new code is thoroughly tested before merging.

**Acceptance Criteria:**
- Coverage measured using cargo-llvm-cov or tarpaulin
- Coverage threshold of 80% enforced in CI (PR fails if below)
- Coverage badge displayed in README
- Module-level coverage breakdown available
- Uncovered lines highlighted in PR comments (via codecov/coveralls)

### US-002: Integration Tests with Real Tool Installation

**As a** maintainer,
**I want** integration tests that actually install tools,
**So that** I can verify the full installation flow works correctly.

**Acceptance Criteria:**
- Integration test suite in `tests/integration/`
- Tests install real tools (jq, ripgrep, etc.) in isolated environments
- CI caching for installed tools to speed up subsequent runs
- Platform-specific tests (macOS, Ubuntu, Fedora, Windows)
- Tests run in containers/VMs for isolation
- `#[ignore]` attribute for tests that require real package managers
- CI matrix runs integration tests on all supported platforms
- Test cleanup removes installed tools after test completion

### US-003: Property-Based Testing for Config Parsing

**As a** maintainer,
**I want** property-based tests for config parsing,
**So that** the parser handles arbitrary valid inputs correctly.

**Acceptance Criteria:**
- Use `proptest` or `quickcheck` crate for property testing
- Generate arbitrary valid TOML configurations
- Verify parsing round-trips correctly (parse -> serialize -> parse)
- Test version string parsing with random semver-like inputs
- Test tool spec parsing with random package manager combinations
- Minimum 1000 test cases per property
- Properties documented in test files

### US-004: Fuzz Testing for TOML Parser Edge Cases

**As a** maintainer,
**I want** fuzz testing for the config parser,
**So that** malformed inputs don't crash or cause undefined behavior.

**Acceptance Criteria:**
- Use `cargo-fuzz` with libfuzzer for fuzzing
- Fuzz targets for:
  - `jarvy.toml` parsing (full config)
  - Version string parsing
  - Tool spec parsing
  - Hook script validation
- Corpus of interesting inputs maintained
- CI runs short fuzz sessions (60 seconds) on every PR
- Weekly scheduled fuzz runs (8+ hours)
- Crashes/panics automatically create GitHub issues

### US-005: Mutation Testing to Verify Test Quality

**As a** maintainer,
**I want** mutation testing to verify test effectiveness,
**So that** I know tests actually catch real bugs.

**Acceptance Criteria:**
- Use `cargo-mutants` for mutation testing
- Mutation score target of 60%+ (mutations caught / total mutations)
- Weekly scheduled mutation testing runs in CI
- Report identifies tests that don't catch mutations
- Critical modules (config.rs, common.rs) require 70%+ mutation score
- Mutation testing documentation in CONTRIBUTING.md

### US-006: Benchmark Suite for Performance Regression

**As a** maintainer,
**I want** benchmarks for critical paths,
**So that** I can detect performance regressions before they ship.

**Acceptance Criteria:**
- Use `criterion` for benchmarks
- Benchmarks for:
  - Config file parsing (small, medium, large files)
  - Tool registry lookup
  - Version comparison operations
  - Parallel installation orchestration
- Baseline stored in repository
- CI compares PR benchmarks against baseline
- Alert on >10% performance regression
- Benchmark results posted as PR comments

### US-007: Code Coverage Reporting in CI

**As a** maintainer,
**I want** code coverage integrated with GitHub PRs,
**So that** I can see coverage impact of every change.

**Acceptance Criteria:**
- Coverage uploaded to Codecov or Coveralls
- PR comments show coverage diff
- Coverage trend graphs in dashboard
- Branch coverage in addition to line coverage
- Configurable coverage thresholds per module
- Coverage report excludes test code and generated code

## Test Infrastructure Setup

### Test Containers

```yaml
# tests/docker/ubuntu.dockerfile
FROM ubuntu:22.04
RUN apt-get update && apt-get install -y curl build-essential
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"
WORKDIR /app
COPY . .
RUN cargo build --release
```

### Mock Package Manager Layer

```rust
// src/test_utils/mock_pm.rs
pub struct MockPackageManager {
    installed: HashSet<String>,
    install_results: HashMap<String, Result<(), InstallError>>,
}

impl MockPackageManager {
    pub fn new() -> Self { ... }

    pub fn with_installed(packages: &[&str]) -> Self { ... }

    pub fn expect_install(&mut self, pkg: &str, result: Result<(), InstallError>) { ... }

    pub fn verify_all_expectations(&self) { ... }
}
```

### Test Directory Structure

```
tests/
├── unit/
│   ├── config_parsing.rs
│   ├── version_matching.rs
│   ├── registry.rs
│   └── tool_spec.rs
├── integration/
│   ├── cli_commands.rs
│   ├── tool_installation.rs
│   └── hook_execution.rs
├── property/
│   ├── config_properties.rs
│   └── version_properties.rs
├── fuzz/
│   ├── fuzz_targets/
│   │   ├── config_parser.rs
│   │   └── version_parser.rs
│   └── corpus/
├── fixtures/
│   ├── valid_configs/
│   ├── invalid_configs/
│   └── version_outputs/
└── docker/
    ├── ubuntu.dockerfile
    ├── fedora.dockerfile
    └── alpine.dockerfile

benches/
├── config_parsing.rs
├── registry_lookup.rs
└── version_comparison.rs
```

## CI Configuration

### Coverage Workflow

```yaml
# .github/workflows/coverage.yml
name: Coverage

on:
  push:
    branches: [main]
  pull_request:

jobs:
  coverage:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: llvm-tools-preview
      - uses: taiki-e/install-action@cargo-llvm-cov

      - name: Generate coverage
        run: cargo llvm-cov --all-features --lcov --output-path lcov.info

      - name: Upload to Codecov
        uses: codecov/codecov-action@v4
        with:
          files: lcov.info
          fail_ci_if_error: true

      - name: Check coverage threshold
        run: |
          COVERAGE=$(cargo llvm-cov --all-features --json | jq '.data[0].totals.lines.percent')
          if (( $(echo "$COVERAGE < 80" | bc -l) )); then
            echo "Coverage $COVERAGE% is below 80% threshold"
            exit 1
          fi
```

### Benchmark Workflow

```yaml
# .github/workflows/benchmark.yml
name: Benchmarks

on:
  pull_request:

jobs:
  benchmark:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable

      - name: Run benchmarks
        run: cargo bench --all-features -- --save-baseline pr

      - name: Compare against main
        run: |
          git fetch origin main
          git checkout origin/main -- target/criterion
          cargo bench --all-features -- --baseline main

      - name: Post benchmark results
        uses: benchmark-action/github-action-benchmark@v1
        with:
          tool: 'cargo'
          output-file-path: target/criterion/**/new/estimates.json
          github-token: ${{ secrets.GITHUB_TOKEN }}
          comment-on-alert: true
          alert-threshold: '110%'
```

### Fuzz Testing Workflow

```yaml
# .github/workflows/fuzz.yml
name: Fuzz Testing

on:
  pull_request:
  schedule:
    - cron: '0 0 * * 0'  # Weekly on Sunday

jobs:
  fuzz-short:
    if: github.event_name == 'pull_request'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@nightly
      - run: cargo install cargo-fuzz

      - name: Fuzz config parser (60s)
        run: cargo +nightly fuzz run config_parser -- -max_total_time=60

  fuzz-extended:
    if: github.event_name == 'schedule'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@nightly
      - run: cargo install cargo-fuzz

      - name: Fuzz config parser (8h)
        run: cargo +nightly fuzz run config_parser -- -max_total_time=28800
```

### Mutation Testing Workflow

```yaml
# .github/workflows/mutation.yml
name: Mutation Testing

on:
  schedule:
    - cron: '0 2 * * 0'  # Weekly on Sunday at 2 AM

jobs:
  mutate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo install cargo-mutants

      - name: Run mutation testing
        run: cargo mutants --timeout 300 --jobs 4

      - name: Upload mutation report
        uses: actions/upload-artifact@v4
        with:
          name: mutation-report
          path: mutants.out/
```

## Implementation Steps

1. **Phase 1: Coverage Infrastructure (Week 1)**
   - Add cargo-llvm-cov to CI
   - Configure Codecov integration
   - Set up coverage thresholds
   - Add coverage badge to README

2. **Phase 2: Property-Based Testing (Week 1)**
   - Add proptest dependency
   - Write config parsing properties
   - Write version matching properties
   - Document property test patterns

3. **Phase 3: Fuzz Testing (Week 2)**
   - Set up cargo-fuzz
   - Create fuzz targets
   - Build initial corpus
   - Add fuzz CI workflows

4. **Phase 4: Benchmarks (Week 2)**
   - Add criterion dependency
   - Write benchmarks for critical paths
   - Set up benchmark CI
   - Configure regression detection

5. **Phase 5: Mutation Testing (Week 3)**
   - Add cargo-mutants integration
   - Configure mutation testing CI
   - Analyze and improve weak tests
   - Document mutation testing process

6. **Phase 6: Test Containers (Week 3)**
   - Create Dockerfiles for test environments
   - Set up integration test isolation
   - Add CI caching for containers
   - Write documentation

## Acceptance Criteria (Summary)

- [ ] Code coverage at 80%+ with CI enforcement
- [ ] Property tests cover config parsing and version matching
- [ ] Fuzz targets run on every PR (60s) and weekly (8h)
- [ ] Mutation score at 60%+ for overall codebase
- [ ] Benchmarks run on PRs with regression alerts at 10%
- [ ] Coverage reports integrated with GitHub PRs
- [ ] Test containers available for Ubuntu, Fedora, Alpine
- [ ] Integration tests run on macOS, Linux, Windows in CI
- [ ] All test infrastructure documented in CONTRIBUTING.md

## Non-Goals

- **100% code coverage**: Diminishing returns beyond 80-85%
- **Testing external package managers**: We test our code, not apt/brew/winget
- **Exhaustive fuzz testing**: Reasonable time limits, not infinite
- **GUI testing**: Jarvy is CLI-only
- **Load testing**: Single-user CLI tool, not a service
- **Testing third-party dependencies**: Rely on upstream testing

## Success Metrics

| Metric | Current | Target |
|--------|---------|--------|
| Line coverage | ~40% | 80%+ |
| Branch coverage | Unknown | 70%+ |
| Mutation score | Unknown | 60%+ |
| Property tests | 0 | 50+ |
| Fuzz targets | 0 | 4+ |
| Benchmark suites | 0 | 4+ |
| CI platforms | 1 | 4 |

## Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Slow CI from extensive testing | High | Medium | Parallelize, use caching, run long tests on schedule |
| Flaky integration tests | Medium | High | Isolated containers, retry logic, deterministic ordering |
| Mutation testing too slow | Medium | Medium | Run weekly, limit to critical modules |
| Fuzz findings overwhelming | Low | Medium | Triage process, prioritize security issues |

## Dependencies

- `cargo-llvm-cov` - Code coverage
- `proptest` - Property-based testing
- `cargo-fuzz` - Fuzz testing (requires nightly)
- `cargo-mutants` - Mutation testing
- `criterion` - Benchmarks
- Docker - Test containers
- Codecov/Coveralls - Coverage reporting

## Effort Estimate

| Phase | Task | Effort |
|-------|------|--------|
| 1 | Coverage infrastructure | 1 day |
| 2 | Property-based testing | 1.5 days |
| 3 | Fuzz testing setup | 1.5 days |
| 4 | Benchmark suite | 1 day |
| 5 | Mutation testing | 1 day |
| 6 | Test containers | 1 day |
| 7 | CI integration | 1 day |
| 8 | Documentation | 1 day |
| **Total** | | **~9 days** |

## Files to Create/Modify

```
# New Files
tests/property/mod.rs
tests/property/config_properties.rs
tests/property/version_properties.rs
fuzz/Cargo.toml
fuzz/fuzz_targets/config_parser.rs
fuzz/fuzz_targets/version_parser.rs
benches/config_parsing.rs
benches/registry_lookup.rs
benches/version_comparison.rs
tests/docker/ubuntu.dockerfile
tests/docker/fedora.dockerfile
tests/docker/alpine.dockerfile
.github/workflows/coverage.yml
.github/workflows/benchmark.yml
.github/workflows/fuzz.yml
.github/workflows/mutation.yml
codecov.yml

# Modified Files
Cargo.toml (add dev-dependencies)
CONTRIBUTING.md (testing documentation)
README.md (coverage badge)
```
