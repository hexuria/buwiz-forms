set dotenv-load := true

APP_NAME := "eBIRForms"
BUNDLE_ID := "dev.goldcoders.bir"
MAC_ARM_TARGET := "aarch64-apple-darwin"
MAC_X86_TARGET := "x86_64-apple-darwin"
WIN_TARGET := "x86_64-pc-windows-msvc"
LINUX_TARGET := "x86_64-unknown-linux-gnu"
RELEASE_DIR := "target/release-artifacts"
MAC_APP := RELEASE_DIR + "/" + APP_NAME + ".app"
VERSION := `grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/'`

# Default task: format, lint, and type check
default: check

# Show available commands
help:
    @just --list

# Run the app locally for development
run:
    cargo run --bin bir

# Run code formatting, linting, and type checking
check:
    cargo fmt --all
    cargo check --workspace
    cargo clippy --workspace -- -D warnings

# Check for vulnerability advisories (requires: cargo install cargo-audit)
audit:
    @if command -v cargo-audit >/dev/null 2>&1; then \
        cargo audit; \
    else \
        echo "⚠️ cargo-audit is not installed. Run 'cargo install cargo-audit' to enable vulnerability scanning."; \
    fi

# Check for outdated dependencies (requires: cargo install cargo-outdated)
outdated:
    @if command -v cargo-outdated >/dev/null 2>&1; then \
        cargo outdated; \
    else \
        echo "⚠️ cargo-outdated is not installed. Run 'cargo install cargo-outdated' to enable."; \
    fi

# Find unused dependencies in Cargo.toml (requires: cargo install cargo-machete)
unused:
    @if command -v cargo-machete >/dev/null 2>&1; then \
        cargo machete; \
    else \
        echo "⚠️ cargo-machete is not installed. Run 'cargo install cargo-machete' to enable."; \
    fi

# Run all unit and integration tests
test:
    cargo test --workspace

# Automatically figure out your OS and build the installer package
install *args="":
    @if [ "{{os()}}" = "macos" ]; then \
        just _package-mac {{args}}; \
    elif [ "{{os()}}" = "windows" ]; then \
        just _package-win {{args}}; \
    else \
        just _package-linux {{args}}; \
    fi

# Publish a new release (tags and pushes to trigger CI)
publish version="":
    #!/usr/bin/env bash
    set -e
    if [ -n "{{version}}" ]; then
        echo "Forcing version to {{version}}"
        sed -i.bak 's/^version = ".*"/version = "{{version}}"/' Cargo.toml
        rm -f Cargo.toml.bak
        if [ -f crates/bir-print/Cargo.toml ]; then
            sed -i.bak 's/^version = ".*"/version = "{{version}}"/' crates/bir-print/Cargo.toml
            rm -f crates/bir-print/Cargo.toml.bak
        fi
        NEW_VER="{{version}}"
    else
        NEW_VER=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')
    fi
    git add -A
    git commit -m "release: v$NEW_VER" --allow-empty
    git tag -a "v$NEW_VER" -m "Release v$NEW_VER"
    git push origin main
    git push origin "v$NEW_VER"
    echo "🚀 Release v$NEW_VER triggered"

# Remove build artifacts
clean:
    cargo clean
    rm -rf {{RELEASE_DIR}}

# --- Hidden OS-specific packaging tasks ---

_package-mac args="":
    @echo "Building for ARM64..."
    cargo build --release --target {{MAC_ARM_TARGET}} $([ "{{args}}" = "--layout-editor" ] && echo "--features layout-editor")
    @echo "Building for x86_64..."
    cargo build --release --target {{MAC_X86_TARGET}} $([ "{{args}}" = "--layout-editor" ] && echo "--features layout-editor")
    @mkdir -p {{RELEASE_DIR}}
    @echo "Creating universal binary (lipo)..."
    lipo -create target/{{MAC_ARM_TARGET}}/release/bir target/{{MAC_X86_TARGET}}/release/bir -output {{RELEASE_DIR}}/bir
    lipo -create target/{{MAC_ARM_TARGET}}/release/bir-daemon target/{{MAC_X86_TARGET}}/release/bir-daemon -output {{RELEASE_DIR}}/bir-daemon
    @echo "Creating .app bundle..."
    @rm -rf "{{MAC_APP}}"
    @mkdir -p "{{MAC_APP}}/Contents/MacOS" "{{MAC_APP}}/Contents/Resources"
    @cp {{RELEASE_DIR}}/bir "{{MAC_APP}}/Contents/MacOS/"
    @cp {{RELEASE_DIR}}/bir-daemon "{{MAC_APP}}/Contents/MacOS/"
    @cp -R assets "{{MAC_APP}}/Contents/Resources/"
    @cp assets/AppIcon.icns "{{MAC_APP}}/Contents/Resources/"
    @cp -R formtypes "{{MAC_APP}}/Contents/Resources/"
    @cp assets/macos/Info.plist "{{MAC_APP}}/Contents/Info.plist"
    @sed -i '' "s/VERSION_PLACEHOLDER/{{VERSION}}/g" "{{MAC_APP}}/Contents/Info.plist"
    @sed -i '' "s/BUNDLE_ID_PLACEHOLDER/{{BUNDLE_ID}}/g" "{{MAC_APP}}/Contents/Info.plist"
    @sed -i '' "s/APP_NAME_PLACEHOLDER/{{APP_NAME}}/g" "{{MAC_APP}}/Contents/Info.plist"
    @touch "{{MAC_APP}}"
    @echo "✅ {{MAC_APP}} created"
    @if command -v create-dmg >/dev/null 2>&1; then \
        create-dmg --volname "{{APP_NAME}}" --window-size 600 400 --icon-size 100 --icon "{{APP_NAME}}.app" 150 190 --app-drop-link 450 190 "{{RELEASE_DIR}}/{{APP_NAME}}-macOS-{{VERSION}}.dmg" "{{MAC_APP}}"; \
        echo "✅ DMG created"; \
    else \
        echo "⚠️ create-dmg not found. Falling back to zip..."; \
        cd {{RELEASE_DIR}} && zip -r "{{APP_NAME}}-macOS-{{VERSION}}.zip" "{{APP_NAME}}.app"; \
    fi

_package-win args="":
    cargo build --release --target {{WIN_TARGET}} $([ "{{args}}" = "--layout-editor" ] && echo "--features layout-editor")
    @mkdir -p {{RELEASE_DIR}}/{{APP_NAME}}-Windows-{{VERSION}}
    @cp target/{{WIN_TARGET}}/release/bir.exe {{RELEASE_DIR}}/{{APP_NAME}}-Windows-{{VERSION}}/
    @cp target/{{WIN_TARGET}}/release/bir-daemon.exe {{RELEASE_DIR}}/{{APP_NAME}}-Windows-{{VERSION}}/
    @cp -R assets {{RELEASE_DIR}}/{{APP_NAME}}-Windows-{{VERSION}}/
    @cp -R formtypes {{RELEASE_DIR}}/{{APP_NAME}}-Windows-{{VERSION}}/
    @cd {{RELEASE_DIR}} && zip -r "{{APP_NAME}}-Windows-x64-{{VERSION}}.zip" "{{APP_NAME}}-Windows-{{VERSION}}"
    @echo "✅ Windows package: {{RELEASE_DIR}}/{{APP_NAME}}-Windows-x64-{{VERSION}}.zip"

_package-linux args="":
    cargo build --release --target {{LINUX_TARGET}} $([ "{{args}}" = "--layout-editor" ] && echo "--features layout-editor")
    @mkdir -p {{RELEASE_DIR}}
    @if command -v cargo-deb >/dev/null 2>&1; then \
        cargo deb -p bir-desktop --no-build --target {{LINUX_TARGET}} -o {{RELEASE_DIR}}/{{APP_NAME}}-Linux-x64-{{VERSION}}.deb; \
        echo "✅ .deb: {{RELEASE_DIR}}/{{APP_NAME}}-Linux-x64-{{VERSION}}.deb"; \
    else \
        echo "⚠️ cargo-deb not found. Falling back to tarball..."; \
        mkdir -p {{RELEASE_DIR}}/{{APP_NAME}}-Linux-{{VERSION}}; \
        cp target/{{LINUX_TARGET}}/release/bir {{RELEASE_DIR}}/{{APP_NAME}}-Linux-{{VERSION}}/; \
        cp target/{{LINUX_TARGET}}/release/bir-daemon {{RELEASE_DIR}}/{{APP_NAME}}-Linux-{{VERSION}}/; \
        cp -R assets {{RELEASE_DIR}}/{{APP_NAME}}-Linux-{{VERSION}}/; \
        cp -R formtypes {{RELEASE_DIR}}/{{APP_NAME}}-Linux-{{VERSION}}/; \
        cd {{RELEASE_DIR}} && tar czf "{{APP_NAME}}-Linux-x64-{{VERSION}}.tar.gz" "{{APP_NAME}}-Linux-{{VERSION}}"; \
        echo "✅ Tarball: {{RELEASE_DIR}}/{{APP_NAME}}-Linux-x64-{{VERSION}}.tar.gz"; \
    fi

# Generate a new form layout from an official BIR PDF
# Usage: just generate-form ~/Downloads/1601C.pdf 1601Cv2018 "Monthly Remittance Return"
generate-form pdf form_id title="":
    python3 .scripts/generate_formtype.py \
        --input "{{pdf}}" \
        --form-id "{{form_id}}" \
        --title "{{title}}" \
        --detect-fields

# Extract form structure from a BIR PDF for Typst-native form generation
# Usage: just extract-form ~/Downloads/2551Q.pdf 2551Qv2018
extract-form pdf form_id:
    python3 .scripts/extract_form_structure.py \
        --input "{{pdf}}" \
        --form-id "{{form_id}}"
