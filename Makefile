# Common contributor entrypoints. `make setup` is the only command a new
# developer should need on a clean laptop.

SHELL := /usr/bin/env bash
.DEFAULT_GOAL := help

.PHONY: help setup bootstrap doctor drift fmt lint test build clean

help:  ## Show available targets
	@awk 'BEGIN {FS = ":.*##"} /^[a-zA-Z_-]+:.*##/ {printf "  \033[36m%-12s\033[0m %s\n", $$1, $$2}' $(MAKEFILE_LIST)

setup: bootstrap  ## Install Jarvy if missing, then run jarvy setup (clean-laptop onboarding)

bootstrap:  ## Run the bootstrap script (idempotent)
	@./scripts/bootstrap.sh

doctor:  ## Verify environment health
	@jarvy doctor --extended

drift:  ## Check environment drift against the team baseline
	@jarvy drift check

fmt:  ## Format Rust code
	@cargo fmt --all

lint:  ## Run clippy with the same rules as CI
	@cargo clippy --all-features -- -D warnings

test:  ## Run the test suite
	@cargo test --verbose -- --show-output

build:  ## Release build
	@cargo build --release

clean:  ## Clean build artifacts
	@cargo clean
