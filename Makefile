.PHONY: build build-mac build-mac-universal build-win build-linux \
       package-mac package-win package-linux \
       publish sign-mac clean check test help

.DEFAULT_GOAL := help

# Load .env file automatically if it exists, exporting all variables
-include .env
export

# ── Configuration ─────────────────────────────────────────────────────────────

APP_NAME        := eBIRForms
BUNDLE_ID       := com.goldcoders.bir
MAC_ARM_TARGET  := aarch64-apple-darwin
MAC_X86_TARGET  := x86_64-apple-darwin
WIN_TARGET      := x86_64-pc-windows-msvc
LINUX_TARGET    := x86_64-unknown-linux-gnu

VERSION         := $(shell ./scripts/version.sh)
RELEASE_DIR     := target/release-artifacts
MAC_APP         := $(RELEASE_DIR)/$(APP_NAME).app

# ── Quality ───────────────────────────────────────────────────────────────────

check: ## Run cargo check on entire workspace
	cargo check --workspace

test: ## Run all tests
	cargo test --workspace

clippy: ## Run clippy with warnings as errors
	cargo clippy --workspace -- -D warnings

# ── Build ─────────────────────────────────────────────────────────────────────

build: ## Build release for current platform
	cargo build --release

build-mac: ## Build release for macOS (current arch)
	cargo build --release

build-mac-universal: ## Build universal macOS binary (arm64 + x86_64)
	@echo "Building for ARM64..."
	cargo build --release --target $(MAC_ARM_TARGET)
	@echo "Building for x86_64..."
	cargo build --release --target $(MAC_X86_TARGET)
	@mkdir -p $(RELEASE_DIR)
	@echo "Creating universal binary (lipo)..."
	lipo -create \
		target/$(MAC_ARM_TARGET)/release/bir \
		target/$(MAC_X86_TARGET)/release/bir \
		-output $(RELEASE_DIR)/bir
	lipo -create \
		target/$(MAC_ARM_TARGET)/release/bir-daemon \
		target/$(MAC_X86_TARGET)/release/bir-daemon \
		-output $(RELEASE_DIR)/bir-daemon
	@echo "Universal binary: $(RELEASE_DIR)/bir"

build-win: ## Build release for Windows x64
	cargo build --release --target $(WIN_TARGET)

build-linux: ## Build release for Linux x64
	cargo build --release --target $(LINUX_TARGET)

# ── Package ───────────────────────────────────────────────────────────────────

package-mac: build-mac-universal ## Create macOS .app bundle + DMG
	@echo "Creating .app bundle..."
	@rm -rf "$(MAC_APP)"
	@mkdir -p "$(MAC_APP)/Contents/MacOS"
	@mkdir -p "$(MAC_APP)/Contents/Resources"
	@# Copy binaries
	@cp $(RELEASE_DIR)/bir "$(MAC_APP)/Contents/MacOS/"
	@cp $(RELEASE_DIR)/bir-daemon "$(MAC_APP)/Contents/MacOS/"
	@# Copy runtime assets
	@cp -R assets "$(MAC_APP)/Contents/Resources/"
	@cp -R formtypes "$(MAC_APP)/Contents/Resources/"
	@# Generate Info.plist
	@cat > "$(MAC_APP)/Contents/Info.plist" <<'PLIST'
	<?xml version="1.0" encoding="UTF-8"?>
	<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
	<plist version="1.0">
	<dict>
		<key>CFBundleExecutable</key>
		<string>bir</string>
		<key>CFBundleIdentifier</key>
		<string>$(BUNDLE_ID)</string>
		<key>CFBundleName</key>
		<string>$(APP_NAME)</string>
		<key>CFBundleVersion</key>
		<string>$(VERSION)</string>
		<key>CFBundleShortVersionString</key>
		<string>$(VERSION)</string>
		<key>CFBundlePackageType</key>
		<string>APPL</string>
		<key>LSMinimumSystemVersion</key>
		<string>13.0</string>
		<key>NSHighResolutionCapable</key>
		<true/>
		<key>CFBundleIconFile</key>
		<string>AppIcon</string>
	</dict>
	</plist>
	PLIST
	@echo "✅ $(MAC_APP) created"
	@# Create DMG (requires create-dmg: brew install create-dmg)
	@if command -v create-dmg >/dev/null 2>&1; then \
		create-dmg \
			--volname "$(APP_NAME)" \
			--window-size 600 400 \
			--icon-size 100 \
			--icon "$(APP_NAME).app" 150 190 \
			--app-drop-link 450 190 \
			"$(RELEASE_DIR)/$(APP_NAME)-macOS-$(VERSION).dmg" \
			"$(MAC_APP)"; \
		echo "✅ DMG created"; \
	else \
		echo "⚠️  create-dmg not found. Install with: brew install create-dmg"; \
		echo "   Falling back to zip..."; \
		cd $(RELEASE_DIR) && zip -r "$(APP_NAME)-macOS-$(VERSION).zip" "$(APP_NAME).app"; \
	fi

package-win: build-win ## Create Windows distribution zip
	@mkdir -p $(RELEASE_DIR)/$(APP_NAME)-Windows-$(VERSION)
	@cp target/$(WIN_TARGET)/release/bir.exe $(RELEASE_DIR)/$(APP_NAME)-Windows-$(VERSION)/
	@cp target/$(WIN_TARGET)/release/bir-daemon.exe $(RELEASE_DIR)/$(APP_NAME)-Windows-$(VERSION)/
	@cp -R assets $(RELEASE_DIR)/$(APP_NAME)-Windows-$(VERSION)/
	@cp -R formtypes $(RELEASE_DIR)/$(APP_NAME)-Windows-$(VERSION)/
	@cd $(RELEASE_DIR) && zip -r "$(APP_NAME)-Windows-x64-$(VERSION).zip" "$(APP_NAME)-Windows-$(VERSION)"
	@echo "✅ Windows package: $(RELEASE_DIR)/$(APP_NAME)-Windows-x64-$(VERSION).zip"

package-linux: build-linux ## Create Linux .deb package
	@mkdir -p $(RELEASE_DIR)
	@if command -v cargo-deb >/dev/null 2>&1; then \
		cargo deb -p bir-desktop --no-build --target $(LINUX_TARGET) \
			-o $(RELEASE_DIR)/$(APP_NAME)-Linux-x64-$(VERSION).deb; \
		echo "✅ .deb: $(RELEASE_DIR)/$(APP_NAME)-Linux-x64-$(VERSION).deb"; \
	else \
		echo "⚠️  cargo-deb not found. Install with: cargo install cargo-deb"; \
		echo "   Falling back to tarball..."; \
		mkdir -p $(RELEASE_DIR)/$(APP_NAME)-Linux-$(VERSION); \
		cp target/$(LINUX_TARGET)/release/bir $(RELEASE_DIR)/$(APP_NAME)-Linux-$(VERSION)/; \
		cp target/$(LINUX_TARGET)/release/bir-daemon $(RELEASE_DIR)/$(APP_NAME)-Linux-$(VERSION)/; \
		cp -R assets $(RELEASE_DIR)/$(APP_NAME)-Linux-$(VERSION)/; \
		cp -R formtypes $(RELEASE_DIR)/$(APP_NAME)-Linux-$(VERSION)/; \
		cd $(RELEASE_DIR) && tar czf "$(APP_NAME)-Linux-x64-$(VERSION).tar.gz" "$(APP_NAME)-Linux-$(VERSION)"; \
		echo "✅ Tarball: $(RELEASE_DIR)/$(APP_NAME)-Linux-x64-$(VERSION).tar.gz"; \
	fi

# ── Signing (macOS) ──────────────────────────────────────────────────────────

sign-mac: ## Codesign + notarize (requires RELEASE_SIGNING_IDENTITY, APPLE_ID, APP_PASSWORD, APPLE_TEAM_ID env vars)
	@if [ -z "$(RELEASE_SIGNING_IDENTITY)" ] || [ -z "$(APPLE_TEAM_ID)" ]; then \
		echo "❌ Set RELEASE_SIGNING_IDENTITY and APPLE_TEAM_ID env vars"; \
		exit 1; \
	fi
	@echo "Preparing entitlements..."
	@sed "s/TEAM_ID_PLACEHOLDER/$(APPLE_TEAM_ID)/g" entitlements.plist > app.entitlements.tmp
	@sed "s/TEAM_ID_PLACEHOLDER/$(APPLE_TEAM_ID)/g" daemon.entitlements.plist > daemon.entitlements.tmp
	
	@echo "Signing daemon (inside-out)..."
	codesign --force --options runtime \
		--sign "$(RELEASE_SIGNING_IDENTITY)" \
		--entitlements daemon.entitlements.tmp \
		--identifier "$(BUNDLE_ID).daemon" \
		"$(MAC_APP)/Contents/MacOS/bir-daemon"
		
	@echo "Signing $(MAC_APP)..."
	codesign --force --options runtime \
		--sign "$(RELEASE_SIGNING_IDENTITY)" \
		--entitlements app.entitlements.tmp \
		"$(MAC_APP)"
		
	@rm -f app.entitlements.tmp daemon.entitlements.tmp
	@echo "Notarizing..."
	xcrun notarytool submit "$(RELEASE_DIR)/$(APP_NAME)-macOS-$(VERSION).dmg" \
		--apple-id "$(APPLE_ID)" \
		--password "$(APP_PASSWORD)" \
		--team-id "$(APPLE_TEAM_ID)" \
		--wait
	xcrun stapler staple "$(RELEASE_DIR)/$(APP_NAME)-macOS-$(VERSION).dmg"
	@echo "✅ Signed and notarized"

# ── Publish ───────────────────────────────────────────────────────────────────

publish: ## Auto-increment patch version, tag, and push (triggers release workflow)
ifdef VERSION_OVERRIDE
	./scripts/version.sh set $(VERSION_OVERRIDE)
else
	./scripts/version.sh bump
endif
	./scripts/version.sh tag
	@echo "🚀 Release v$$(./scripts/version.sh) triggered"

# ── Clean ─────────────────────────────────────────────────────────────────────

clean: ## Remove build artifacts
	cargo clean
	rm -rf $(RELEASE_DIR)

# ── Help ──────────────────────────────────────────────────────────────────────

help: ## Show this help
	@echo ""
	@echo "  eBIRForms Build System (v$(VERSION))"
	@echo "  ─────────────────────────────────────"
	@echo ""
	@echo "  Quality:"
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | grep -E 'check|test|clippy' | awk 'BEGIN {FS = ":.*?## "}; {printf "    \033[36m%-24s\033[0m %s\n", $$1, $$2}'
	@echo ""
	@echo "  Build:"
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | grep -E 'build' | awk 'BEGIN {FS = ":.*?## "}; {printf "    \033[36m%-24s\033[0m %s\n", $$1, $$2}'
	@echo ""
	@echo "  Package:"
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | grep -E 'package|sign' | awk 'BEGIN {FS = ":.*?## "}; {printf "    \033[36m%-24s\033[0m %s\n", $$1, $$2}'
	@echo ""
	@echo "  Release:"
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | grep -E 'publish|clean' | awk 'BEGIN {FS = ":.*?## "}; {printf "    \033[36m%-24s\033[0m %s\n", $$1, $$2}'
	@echo ""
	@echo "  Publish:"
	@echo "    make publish                   Auto-increment patch, tag, push"
	@echo "    make publish VERSION_OVERRIDE=0.1.1   Force specific version"
	@echo ""
