.PHONY: all build test cover lint format check clean help

all: lint format test ## Lint, format, and test

build: ## Build all crates
	cargo build

test: ## Run full test suite
	cargo test

cover: ## Generate code coverage report (one-shot)
	cargo tarpaulin --all-features

lint: ## Lint with autofix
	cargo clippy --all-targets --all-features --fix --allow-dirty -- -D warnings

format: ## Format code
	cargo fmt

check: ## Check compilation without building
	cargo check --all-targets --all-features

clean: ## Remove build artifacts
	cargo clean

help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  %-10s %s\n", $$1, $$2}'
