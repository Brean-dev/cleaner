.PHONY: build run clean install test help

help:
	@echo "Makefile commands:"
	@echo "  build   - Build the project"
	@echo "  run     - Run the project"
	@echo "  clean   - Clean build artifacts"
	@echo "  install - Install dependencies"
	@echo "  test    - Run tests"

build:
	@echo "Building the project..."
	cargo build --release
	
run:
	@echo "Running the project..."
	sudo env "PATH=$$PATH" "CARGO_HOME=$$CARGO_HOME" "RUSTUP_HOME=$$RUSTUP_HOME" cargo run -- $(ARGS)

clean:
	@echo "Cleaning build artifacts..."
	cargo clean

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

# Default target
run-local:
	cargo run -- $(ARGS)