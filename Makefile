.PHONY: build run clean install test help run-home run-clean generate-cache clean-cache build-cache setup-precommit lint fmt clippy audit

help:
	@echo "Makefile commands:"
	@echo "  build        - Build the project"
	@echo "  run          - Run the project (usage: make run ARGS='path [--clean]')"
	@echo "  run-home     - Run on home directory"
	@echo "  run-clean    - Run with --clean flag on home directory"
	@echo "  clean        - Clean build artifacts"
	@echo "  install      - Install dependencies"
	@echo "  test         - Run tests"
	@echo "  build-cache  - Build the cache generator tool"
	@echo "  generate-cache - Generate fake cache files for testing"
	@echo "  clean-cache  - Clean up generated cache files"
	@echo "  setup-precommit - Install and setup pre-commit hooks"
	@echo "  lint         - Run all linting tools"
	@echo "  fmt          - Format code"
	@echo "  clippy       - Run clippy linter"
	@echo "  audit        - Run security audit"
	@echo "  pre-commit   - Run pre-commit hooks on all files"
	@echo ""
	@echo "Examples:"
	@echo "  make run ARGS='/ --clean'"
	@echo "  make run ARGS='/home/user'"
	@echo "  make run-clean"
	@echo "  make generate-cache"
	@echo "  make clean-cache"
	@echo "  make setup-precommit"
	@echo "  make lint"

build:
	@echo "Building the project..."
	cargo build --release

build-cache:
	@echo "Building cache generator..."
	@cd tools/cache_generator && cargo build --release

generate-cache: build-cache
	@echo "Generating fake cache files..."
	@tools/cache_generator/target/release/cache_generator

clean-cache: build-cache
	@echo "Cleaning generated cache files..."
	@tools/cache_generator/target/release/cache_generator --clean

run:
	@echo "Running the project with args: $(ARGS)"
	sudo env "PATH=$$PATH" "CARGO_HOME=$$CARGO_HOME" "RUSTUP_HOME=$$RUSTUP_HOME" cargo run -- $(ARGS)

run-home:
	@echo "Running on home directory..."
	sudo env "PATH=$$PATH" "CARGO_HOME=$$CARGO_HOME" "RUSTUP_HOME=$$RUSTUP_HOME" cargo run -- $$HOME

run-clean:
	@echo "Running with --clean flag on home directory..."
	sudo env "PATH=$$PATH" "CARGO_HOME=$$CARGO_HOME" "RUSTUP_HOME=$$RUSTUP_HOME" cargo run -- $$HOME --clean

clean:
	@echo "Cleaning build artifacts..."
	cargo clean
	@if [ -d tools/cache_generator ]; then \
		cd tools/cache_generator && cargo clean; \
	fi

install: build
	@echo "Installing to /usr/local/bin..."
	@if [ ! -f /usr/local/bin/cleaner ] || ! cmp -s target/release/cleaner /usr/local/bin/cleaner; then \
		sudo cp target/release/cleaner /usr/local/bin/cleaner; \
		echo "Binary installed successfully."; \
	else \
		echo "Binary is already up to date."; \
	fi

test:
	@echo "Running tests..."
	cargo test

setup-precommit:
	@echo "Setting up pre-commit hooks..."
	@command -v pre-commit >/dev/null 2>&1 || { echo "Installing pre-commit..."; pip install pre-commit; }
	@command -v cargo-audit >/dev/null 2>&1 || { echo "Installing cargo-audit..."; cargo install cargo-audit; }
	@command -v cargo-deny >/dev/null 2>&1 || { echo "Installing cargo-deny..."; cargo install cargo-deny; }
	pre-commit install
	@echo "Pre-commit hooks installed successfully!"

lint: fmt clippy audit
	@echo "Running all linting tools..."
	cargo check --workspace --all-targets --all-features
	@echo "All linting checks passed!"

fmt:
	@echo "Formatting code..."
	cargo fmt --all

clippy:
	@echo "Running clippy..."
	cargo clippy --workspace --all-targets --all-features -- -D warnings

audit:
	@echo "Running security audit..."
	cargo audit
	cargo deny check

# Default target
run-local:
	cargo run -- $(ARGS)

pre-commit:
	pre-commit run --all-files
