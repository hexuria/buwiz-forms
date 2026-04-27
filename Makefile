.PHONY: build build-win build-linux build-mac help
.DEFAULT_GOAL := build

# Target OS definitions
MAC_TARGET = x86_64-apple-darwin
WIN_TARGET = x86_64-pc-windows-msvc
LINUX_TARGET = x86_64-unknown-linux-gnu

# Defaults to macOS
build: build-mac

# Build for macOS
build-mac:
	@echo "Building for macOS..."
	cargo build --release --target $(MAC_TARGET)

# Build for Windows
build-win:
	@echo "Building for Windows..."
	cargo build --release --target $(WIN_TARGET)

# Build for Linux
build-linux:
	@echo "Building for Linux..."
	cargo build --release --target $(LINUX_TARGET)

# Help
help:
	@echo "Available targets:"
	@echo "  make build         - Defaults to building for macOS"
	@echo "  make build-mac     - Builds for macOS (x86_64-apple-darwin)"
	@echo "  make build-win     - Builds for Windows (x86_64-pc-windows-msvc)"
	@echo "  make build-linux   - Builds for Linux (x86_64-unknown-linux-gnu)"
