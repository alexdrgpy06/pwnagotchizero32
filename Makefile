# Makefile for pwnagotchi-zero

.PHONY: help build-32bit build-64bit build-rust test clean docker-build release

# Default target
help:
	@echo "Pwnagotchi Zero - Build targets:"
	@echo "  make build-rust       - Cross-compile Rust binary for both architectures"
	@echo "  make build-32bit      - Build 32-bit ARM image (Pi Zero W / Zero 2W 32-bit)"
	@echo "  make build-64bit      - Build 64-bit kernel / 32-bit userland image (Pi Zero 2W)"
	@echo "  make test             - Run Rust tests"
	@echo "  make clean            - Clean build artifacts"
	@echo "  make docker-build     - Build Docker image for CI"
	@echo "  make release VERSION=v1.0.0  - Create release images"

# Rust cross-compilation
build-rust:
	./scripts/build_oxigotchi.sh --arch 32bit
	./scripts/build_oxigotchi.sh --arch 64bit

# Build 32-bit image
build-32bit:
	./scripts/bake_release.sh --arch 32bit --release $(VERSION)

# Build 64-bit image
build-64bit:
	./scripts/bake_release.sh --arch 64bit --release $(VERSION)

# Run tests
test:
	cd rust && cargo test --all-targets

# Clean build artifacts
clean:
	rm -rf rust/target
	rm -rf build
	rm -rf output/*.img.xz

# Build Docker image for CI
docker-build:
	docker build -t pwnagotchi-zero:build -f docker/Dockerfile.build .

# Create release
release:
	@if [ -z "$(VERSION)" ]; then echo "Usage: make release VERSION=v1.0.0"; exit 1; fi
	./scripts/bake_release.sh --arch 32bit --release $(VERSION)
	./scripts/bake_release.sh --arch 64bit --release $(VERSION)

# Development helpers
dev-shell:
	docker run --rm -it -v $(PWD):/workspace pwnagotchi-zero:build bash

# Format code
fmt:
	cd rust && cargo fmt

# Lint
	cd rust && cargo clippy --all-targets --all-features -- -D warnings

# Check without building
check:
	cd rust && cargo check --all-targets

# Generate documentation
doc:
	cd rust && cargo doc --no-deps --open

# Install pre-commit hooks
install-hooks:
	cp .github/hooks/pre-commit .git/hooks/
	chmod +x .git/hooks/pre-commit

# Full CI pipeline locally
ci: fmt check test build-rust
	@echo "CI pipeline complete"

# Default variables
VERSION ?= dev