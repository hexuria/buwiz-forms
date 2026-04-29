set dotenv-load := true

APP_NAME := "eBIRForms"
BUNDLE_ID := "dev.goldcoders.bir"
MAC_ARM_TARGET := "aarch64-apple-darwin"
MAC_X86_TARGET := "x86_64-apple-darwin"
WIN_TARGET := "x86_64-pc-windows-msvc"
LINUX_TARGET := "x86_64-unknown-linux-gnu"
RELEASE_DIR := "target/release-artifacts"
MAC_APP := RELEASE_DIR + "/" + APP_NAME + ".app"
VERSION := `sh ./scripts/version.sh`

# Show available commands
default:
    @just --list

# Run the app locally for development
run:
    cargo run --bin bir

# Run code formatting, linting, and type checking
check:
    cargo fmt --all
    cargo check --workspace
    cargo clippy --workspace -- -D warnings

# Run all unit and integration tests
test:
    cargo test --workspace

# Automatically figure out your OS and build the installer package
install:
    @if [ "{{os()}}" = "macos" ]; then \
        just _package-mac; \
    elif [ "{{os()}}" = "windows" ]; then \
        just _package-win; \
    else \
        just _package-linux; \
    fi

# Publish a new release (auto-increments patch, tags, and pushes to trigger CI)
publish version="":
    @if [ -n "{{version}}" ]; then \
        sh ./scripts/version.sh set {{version}}; \
    else \
        sh ./scripts/version.sh bump; \
    fi
    sh ./scripts/version.sh tag
    @echo "🚀 Release v$(sh ./scripts/version.sh) triggered"

# Remove build artifacts
clean:
    cargo clean
    rm -rf {{RELEASE_DIR}}

# --- Hidden OS-specific packaging tasks ---

_package-mac:
    @echo "Building for ARM64..."
    cargo build --release --target {{MAC_ARM_TARGET}}
    @echo "Building for x86_64..."
    cargo build --release --target {{MAC_X86_TARGET}}
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
    @echo '<?xml version="1.0" encoding="UTF-8"?><!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd"><plist version="1.0"><dict><key>CFBundleExecutable</key><string>bir</string><key>CFBundleIdentifier</key><string>{{BUNDLE_ID}}</string><key>CFBundleName</key><string>{{APP_NAME}}</string><key>CFBundleVersion</key><string>{{VERSION}}</string><key>CFBundleShortVersionString</key><string>{{VERSION}}</string><key>CFBundlePackageType</key><string>APPL</string><key>LSMinimumSystemVersion</key><string>13.0</string><key>NSHighResolutionCapable</key><true/><key>CFBundleIconFile</key><string>AppIcon</string></dict></plist>' > "{{MAC_APP}}/Contents/Info.plist"
    @echo "✅ {{MAC_APP}} created"
    @if command -v create-dmg >/dev/null 2>&1; then \
        create-dmg --volname "{{APP_NAME}}" --window-size 600 400 --icon-size 100 --icon "{{APP_NAME}}.app" 150 190 --app-drop-link 450 190 "{{RELEASE_DIR}}/{{APP_NAME}}-macOS-{{VERSION}}.dmg" "{{MAC_APP}}"; \
        echo "✅ DMG created"; \
    else \
        echo "⚠️ create-dmg not found. Falling back to zip..."; \
        cd {{RELEASE_DIR}} && zip -r "{{APP_NAME}}-macOS-{{VERSION}}.zip" "{{APP_NAME}}.app"; \
    fi

_package-win:
    cargo build --release --target {{WIN_TARGET}}
    @mkdir -p {{RELEASE_DIR}}/{{APP_NAME}}-Windows-{{VERSION}}
    @cp target/{{WIN_TARGET}}/release/bir.exe {{RELEASE_DIR}}/{{APP_NAME}}-Windows-{{VERSION}}/
    @cp target/{{WIN_TARGET}}/release/bir-daemon.exe {{RELEASE_DIR}}/{{APP_NAME}}-Windows-{{VERSION}}/
    @cp -R assets {{RELEASE_DIR}}/{{APP_NAME}}-Windows-{{VERSION}}/
    @cp -R formtypes {{RELEASE_DIR}}/{{APP_NAME}}-Windows-{{VERSION}}/
    @cd {{RELEASE_DIR}} && zip -r "{{APP_NAME}}-Windows-x64-{{VERSION}}.zip" "{{APP_NAME}}-Windows-{{VERSION}}"
    @echo "✅ Windows package: {{RELEASE_DIR}}/{{APP_NAME}}-Windows-x64-{{VERSION}}.zip"

_package-linux:
    cargo build --release --target {{LINUX_TARGET}}
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
