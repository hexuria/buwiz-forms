set dotenv-load := true

set windows-shell := ["pwsh", "-NoProfile", "-c"]
APP_NAME := "eBIRForms"
BUNDLE_ID := "dev.goldcoders.bir"
MAC_ARM_TARGET := "aarch64-apple-darwin"
MAC_X86_TARGET := "x86_64-apple-darwin"
WIN_TARGET := "x86_64-pc-windows-msvc"
LINUX_TARGET := "x86_64-unknown-linux-gnu"
RELEASE_DIR := "target/release-artifacts"
MAC_APP := RELEASE_DIR + "/" + APP_NAME + ".app"
VERSION := if os_family() == "windows" {
    `(Select-String -Path Cargo.toml -Pattern '^\s*version' | Select-Object -First 1).Line.Trim() -replace '.*"(.*)".*', '$1'`
} else {
    `grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/'`
}
BUILD_NUMBER := env_var_or_default("BUILD_NUMBER", "26")

# Default task: format, lint, and type check
default: check

# Automatically fetch the latest build from App Store Connect and increment the justfile BUILD_NUMBER
[unix]
bump-build:
    @echo "Bumping build number via App Store Connect API..."
    @set -a && source .env && set +a && uv run scripts/bump_build.py

[windows]
bump-build:
    #!pwsh -NoProfile
    $ErrorActionPreference = 'Stop'
    Write-Host "Bumping build number via App Store Connect API..."
    # Load .env vars into the current process
    if (Test-Path .env) {
        Get-Content .env | ForEach-Object {
            if ($_ -match '^\s*([^#][^=]+)=(.*)$') {
                [Environment]::SetEnvironmentVariable($matches[1].Trim(), $matches[2].Trim(), 'Process')
            }
        }
    }
    uv run scripts/bump_build.py

# Show available commands
help:
    @just --list

# Run the app locally with the bundled offline HTML renderer.
run: build-form-renderer
    cargo run --locked --bin bir --features dev-tools

# Run code formatting, linting, and type checking
[unix]
check:
    cargo fmt --all
    cargo check --locked --workspace
    cargo clippy --locked --workspace -- -D warnings

[windows]
check:
    #!pwsh -NoProfile
    cargo fmt --all
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    cargo check --locked --workspace
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    cargo clippy --locked --workspace -- -D warnings
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

# Check for vulnerability advisories (requires: cargo install cargo-audit)
[unix]
audit:
    @if command -v cargo-audit >/dev/null 2>&1; then \
        cargo audit; \
    else \
        echo "⚠️ cargo-audit is not installed. Run 'cargo install cargo-audit' to enable vulnerability scanning."; \
    fi

[windows]
audit:
    #!pwsh -NoProfile
    if (Get-Command cargo-audit -ErrorAction SilentlyContinue) {
        cargo audit
    } else {
        Write-Warning "cargo-audit is not installed. Run 'cargo install cargo-audit' to enable vulnerability scanning."
    }

# Check for outdated dependencies (requires: cargo install cargo-outdated)
[unix]
outdated:
    @if command -v cargo-outdated >/dev/null 2>&1; then \
        cargo outdated; \
    else \
        echo "⚠️ cargo-outdated is not installed. Run 'cargo install cargo-outdated' to enable."; \
    fi

[windows]
outdated:
    #!pwsh -NoProfile
    if (Get-Command cargo-outdated -ErrorAction SilentlyContinue) {
        cargo outdated
    } else {
        Write-Warning "cargo-outdated is not installed. Run 'cargo install cargo-outdated' to enable."
    }

# Find unused dependencies in Cargo.toml (requires: cargo install cargo-machete)
[unix]
unused:
    @if command -v cargo-machete >/dev/null 2>&1; then \
        cargo machete; \
    else \
        echo "⚠️ cargo-machete is not installed. Run 'cargo install cargo-machete' to enable."; \
    fi

[windows]
unused:
    #!pwsh -NoProfile
    if (Get-Command cargo-machete -ErrorAction SilentlyContinue) {
        cargo machete
    } else {
        Write-Warning "cargo-machete is not installed. Run 'cargo install cargo-machete' to enable."
    }

# Run all unit and integration tests
test:
    cargo test --locked --workspace

# Build the tracked-contract, local-only HTML renderer before any package copies
# the assets directory. The generated bundle stays ignored, so clean packages
# cannot accidentally reuse stale developer output.
build-form-renderer:
    npm ci
    npm run contracts:check
    npm run audit:forms:migration
    npm run audit:no-legacy
    npm run build:forms
    npm run verify:forms:offline

# Re-check the generated renderer against a clean curated source revision
# before copying it into any distributable package. The resulting identity is
# a non-promotional sibling of assets/form-renderer, so its expected tree hash
# cannot recursively include itself.
build-packaged-form-renderer: build-form-renderer
    npm run verify:forms:offline:package

# Build an ad-hoc-signed macOS app with the development-only native-output
# observer, exercise a real PDF export interactively, and validate every
# emitted observation through the Rust-owned schema. This remains diagnostic:
# it does not create trusted release evidence or change readiness flags.
[macos]
native-evidence-macos:
    #!/usr/bin/env bash
    set -euo pipefail
    python3 scripts/audit_html_form_migration.py --require-clean-source
    mkdir -p target/tmp
    TMPDIR="$PWD/target/tmp" cargo test --locked -p bir-print --features native-output-evidence html_output_evidence
    TMPDIR="$PWD/target/tmp" cargo test --locked -p bir-desktop --features dev-tools macos_capture_pipeline
    just _package-mac --native-evidence
    OBSERVATION_DIR="$PWD/target/native-output-observations"
    rm -rf "$OBSERVATION_DIR"
    mkdir -p "$OBSERVATION_DIR"
    echo "Open 2551Q, export a PDF over an existing destination, then close the app."
    echo "Observations will remain local in $OBSERVATION_DIR."
    EBIR_NATIVE_OUTPUT_EVIDENCE_DIR="$OBSERVATION_DIR" \
        "{{MAC_APP}}/Contents/MacOS/bir"
    shopt -s nullglob
    observations=("$OBSERVATION_DIR"/*.observation.json)
    if [ "${#observations[@]}" -eq 0 ]; then
        echo "No native-output observation was produced; complete a successful direct PDF export before closing the app." >&2
        exit 1
    fi
    npm run verify:native-output:observation -- "${observations[@]}"
    echo "Development observations validated. They are non-promotional and must not be added to form-release-evidence.json."

# Install a built package (auto-detects available artifacts)
# Usage: just install [format]
# Formats: exe, msix (Windows) | app, pkg, dmg (macOS) | deb, tar (Linux)
# If no format given: installs the only available artifact, or lists choices if multiple exist
[windows]
install format="":
    #!pwsh -NoProfile
    $ErrorActionPreference = 'Stop'
    $VERSION = "{{VERSION}}"
    $DIR = "{{RELEASE_DIR}}"
    $format = "{{format}}"

    # Scan for available installers
    $artifacts = [ordered]@{}
    $msix = Get-ChildItem "$DIR\*.msix" -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($msix) { $artifacts["msix"] = $msix.FullName }
    $exe = Get-ChildItem "$DIR\*-Setup.exe" -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($exe) { $artifacts["exe"] = $exe.FullName }

    if ($format) {
        if (-not $artifacts.Contains($format)) {
            Write-Host "❌ No '$format' artifact found in $DIR." -ForegroundColor Red
            Write-Host "   Run 'just $format' to build it first."
            exit 1
        }
    } elseif ($artifacts.Count -eq 0) {
        Write-Host "❌ No installers found in $DIR." -ForegroundColor Red
        Write-Host "   Build one first with: just exe  or  just msix"
        exit 1
    } elseif ($artifacts.Count -eq 1) {
        $format = $artifacts.Keys | Select-Object -First 1
        Write-Host "Found one installer, auto-selecting: $format" -ForegroundColor Cyan
    } else {
        Write-Host "Multiple installers found — please specify which one:" -ForegroundColor Yellow
        Write-Host ""
        foreach ($key in $artifacts.Keys) {
            $path = $artifacts[$key]
            $size = [math]::Round((Get-Item $path).Length / 1MB, 1)
            Write-Host "  just install $key" -ForegroundColor Green -NoNewline
            Write-Host "  →  $([System.IO.Path]::GetFileName($path)) ($($size) MB)"
        }
        Write-Host ""
        exit 0
    }

    $path = $artifacts[$format]
    switch ($format) {
        "msix" {
            Write-Host "Installing MSIX package: $path" -ForegroundColor Cyan
            Write-Host "⚠️  MSIX must be signed before sideloading. Run 'just sign-dev' first if you haven't." -ForegroundColor Yellow
            Add-AppxPackage -Path $path
            Write-Host "✅ MSIX installed successfully!"
        }
        "exe" {
            Write-Host "Launching Setup installer: $path" -ForegroundColor Cyan
            Start-Process -FilePath $path -Wait
            Write-Host "✅ Setup installer completed!"
        }
    }

[unix]
install format="":
    #!/usr/bin/env bash
    set -e
    VERSION="{{VERSION}}"
    DIR="{{RELEASE_DIR}}"
    format="{{format}}"

    declare -A artifacts

    if [ "$(uname)" = "Darwin" ]; then
        # macOS: look for .app, .pkg, .dmg
        if [ -d "$DIR/{{APP_NAME}}.app" ]; then artifacts[app]="$DIR/{{APP_NAME}}.app"; fi
        pkg=$(find "$DIR" -maxdepth 1 -name "*-MAS-*.pkg" 2>/dev/null | head -1)
        [ -n "$pkg" ] && artifacts[pkg]="$pkg"
        dmg=$(find "$DIR" -maxdepth 1 -name "*.dmg" 2>/dev/null | head -1)
        [ -n "$dmg" ] && artifacts[dmg]="$dmg"
        valid_formats="app, pkg, or dmg"
    else
        # Linux: look for .deb, .tar.gz
        deb=$(find "$DIR" -maxdepth 1 -name "*.deb" 2>/dev/null | head -1)
        [ -n "$deb" ] && artifacts[deb]="$deb"
        tarball=$(find "$DIR" -maxdepth 1 -name "*.tar.gz" 2>/dev/null | head -1)
        [ -n "$tarball" ] && artifacts[tar]="$tarball"
        valid_formats="deb or tar"
    fi

    if [ -n "$format" ]; then
        if [ -z "${artifacts[$format]+x}" ]; then
            echo "❌ No '$format' artifact found in $DIR."
            echo "   Build it first, then retry."
            exit 1
        fi
    elif [ ${#artifacts[@]} -eq 0 ]; then
        echo "❌ No installers found in $DIR."
        echo "   Build one first (e.g. just app, just install)."
        exit 1
    elif [ ${#artifacts[@]} -eq 1 ]; then
        format="${!artifacts[@]}"
        echo "Found one installer, auto-selecting: $format"
    else
        echo "Multiple installers found — please specify which one:"
        echo ""
        for key in "${!artifacts[@]}"; do
            size=$(du -h "${artifacts[$key]}" | cut -f1)
            echo "  just install $key  →  $(basename "${artifacts[$key]}") ($size)"
        done
        echo ""
        exit 0
    fi

    path="${artifacts[$format]}"
    case "$format" in
        app)
            echo "Copying {{APP_NAME}}.app to /Applications..."
            cp -R "$path" /Applications/
            echo "✅ Installed to /Applications/{{APP_NAME}}.app"
            ;;
        pkg)
            echo "Installing PKG: $path"
            sudo installer -pkg "$path" -target /
            echo "✅ PKG installed successfully!"
            ;;
        dmg)
            echo "Opening DMG: $path"
            open "$path"
            echo "✅ DMG mounted — drag {{APP_NAME}}.app to Applications."
            ;;
        deb)
            echo "Installing DEB: $path"
            sudo dpkg -i "$path"
            echo "✅ DEB installed successfully!"
            ;;
        tar)
            echo "Extracting tarball: $path"
            tar xzf "$path" -C "$DIR"
            echo "✅ Extracted to $DIR/ — run the binary from there."
            ;;
    esac

# Build and package the app for Mac App Store submission. Resolve the next
# counter into the child process environment so the strict renderer source gate
# runs before any tracked build-number update is needed.
# Set CODESIGN_IDENTITY env var to override the default ad-hoc signing ("-")
app *args="":
    @if [ "{{os()}}" = "macos" ]; then \
        set -a && source .env && set +a; \
        if ! NEXT_BUILD_NUMBER="$(uv run scripts/bump_build.py --print-next)"; then \
            echo "Could not resolve the next App Store build number" >&2; exit 1; \
        fi; \
        case "$NEXT_BUILD_NUMBER" in \
            ''|*[!0-9]*) echo "Invalid App Store build number: $NEXT_BUILD_NUMBER" >&2; exit 1 ;; \
        esac; \
        echo "Building App Store package with build $NEXT_BUILD_NUMBER"; \
        BUILD_NUMBER="$NEXT_BUILD_NUMBER" just _package-mac-appstore "{{args}}"; \
    else \
        echo "⚠️ App Store builds are only supported on macOS."; \
    fi

# Build the Inno Setup executable installer (Windows only)
exe *args="": build-packaged-form-renderer
    #!pwsh -NoProfile
    $ErrorActionPreference = 'Stop'
    
    $features = @()
    foreach ($arg in '{{args}}'.Split(' ', [StringSplitOptions]::RemoveEmptyEntries)) {
        if ($arg -eq '--inspector') { $features += 'inspector' }
    }
    
    if ($features.Count -gt 0) {
        $featureList = $features -join ','
        cargo build --locked --release --target {{WIN_TARGET}} --features $featureList
    } else {
        cargo build --locked --release --target {{WIN_TARGET}}
    }
    if ($LASTEXITCODE -ne 0) {
        Write-Error "❌ cargo build failed (exit code $LASTEXITCODE). Aborting EXE packaging."
        exit $LASTEXITCODE
    }
    
    $VERSION = "{{VERSION}}"
    
    $ISCC = "C:\Program Files (x86)\Inno Setup 6\ISCC.exe"
    if (Test-Path $ISCC) {
        Write-Host "Building installer with Inno Setup..."
        & $ISCC /DMyAppVersion=$VERSION installer.iss
        Write-Host "✅ Setup EXE created: target/release-artifacts/{{APP_NAME}}-Windows-x64-$VERSION-Setup.exe"
    } else {
        Write-Warning "⚠️ ISCC.exe not found at $ISCC. Please install Inno Setup 6 (https://jrsoftware.org/isinfo.php)."
        exit 1
    }

# Build a Store-only MSIX candidate (Windows only).
# This artifact is intentionally excluded from public GitHub releases. Store
# promotion remains blocked until the manifest artwork dimensions and packaged
# MSVC runtime behavior pass their separate Windows certification checks.
msix *args="": build-packaged-form-renderer
    #!pwsh -NoProfile
    $ErrorActionPreference = 'Stop'
    Write-Warning "STORE-ONLY MSIX candidate; not a public GitHub release artifact"
    Write-Warning "BLOCKED: certify manifest artwork and packaged MSVC runtime behavior before Store submission"
    
    $features = @()
    foreach ($arg in '{{args}}'.Split(' ', [StringSplitOptions]::RemoveEmptyEntries)) {
        if ($arg -eq '--inspector') { $features += 'inspector' }
    }
    
    if ($features.Count -gt 0) {
        $featureList = $features -join ','
        cargo build --locked --release --target {{WIN_TARGET}} --features $featureList
    } else {
        cargo build --locked --release --target {{WIN_TARGET}}
    }
    if ($LASTEXITCODE -ne 0) {
        Write-Error "❌ cargo build failed (exit code $LASTEXITCODE). Aborting MSIX packaging."
        exit $LASTEXITCODE
    }
    
    $VERSION = "{{VERSION}}"
    $MSIX_DIR = "target/release-artifacts/msix-staging"
    
    # Stage MSIX layout
    if (Test-Path $MSIX_DIR) { Remove-Item $MSIX_DIR -Recurse -Force }
    New-Item -ItemType Directory -Force -Path $MSIX_DIR | Out-Null
    Copy-Item "target/{{WIN_TARGET}}/release/bir.exe" "$MSIX_DIR\"

    # OpenSSL is statically compiled (vendored), so no OpenSSL DLL is needed.
    # These runner-local MSVC DLL copies are diagnostic only. They do not close
    # the Store runtime-certification blocker documented above.
    $vcr = "$env:WINDIR\System32\vcruntime140.dll"
    if (Test-Path $vcr) { Copy-Item $vcr "$MSIX_DIR\" }
    $msvcp = "$env:WINDIR\System32\msvcp140.dll"
    if (Test-Path $msvcp) { Copy-Item $msvcp "$MSIX_DIR\" }

    Copy-Item "assets" "$MSIX_DIR\assets" -Recurse
    python scripts/audit_no_legacy.py --package-root $MSIX_DIR
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    
    # Use the shared BUILD_NUMBER from the justfile (synced with App Store Connect via bump-build).
    # This ensures macOS and Windows builds share the same build counter.
    $BUILD_NUMBER = "{{BUILD_NUMBER}}"

    # Microsoft Store requires Version format: Major.Minor.Build.0
    # The 4th part (Revision) MUST be 0 — Microsoft reserves it.
    # We use: Major.Minor.BuildNumber.0 (semver patch folds into build number progression).
    $vParts = $VERSION.Split('.')
    $MSIX_VERSION = "$($vParts[0]).$($vParts[1]).$BUILD_NUMBER.0"
    Write-Host "MSIX version: $MSIX_VERSION (from app version $VERSION, build $BUILD_NUMBER)"

    # Copy and stamp manifest
    (Get-Content "assets\windows\AppxManifest.xml") `
        -replace 'MSIX_VERSION_PLACEHOLDER', $MSIX_VERSION | `
        Set-Content "$MSIX_DIR\AppxManifest.xml"
    
    # Build MSIX
    $SDK_DIR = Get-ChildItem "C:\Program Files (x86)\Windows Kits\10\bin\10.*" | Sort-Object Name -Descending | Select-Object -First 1
    $MAKEAPPX = "$($SDK_DIR.FullName)\x64\MakeAppx.exe"
    $MAKEPRI = "$($SDK_DIR.FullName)\x64\MakePri.exe"
    
    if (Test-Path $MAKEPRI) {
        Write-Host "Generating resources.pri for advanced tile icons..."
        & $MAKEPRI createconfig /cf "$MSIX_DIR\priconfig.xml" /dq en-US /pv 10.0.0 /o
        & $MAKEPRI new /pr "$MSIX_DIR" /cf "$MSIX_DIR\priconfig.xml" /in "GOLDCODERSCORP.ebirforms" /of "$MSIX_DIR\resources.pri" /o
        Remove-Item "$MSIX_DIR\priconfig.xml" -Force
    }
    
    if (Test-Path $MAKEAPPX) {
        & $MAKEAPPX pack /d "$MSIX_DIR" /p "target/release-artifacts/{{APP_NAME}}-Windows-$VERSION.msix" /o
        Write-Host "✅ MSIX package created: target/release-artifacts/{{APP_NAME}}-Windows-$VERSION.msix"
    } else {
        Write-Warning "⚠️ MakeAppx.exe not found. Install Windows SDK to build MSIX."
    }

# Sign the Store-identity MSIX with a local development certificate for
# sideload testing only. This does not make it a public release artifact.
sign-dev:
    #!pwsh -NoProfile
    $ErrorActionPreference = 'Stop'
    $VERSION = "{{VERSION}}"
    $MSIX_PATH = "target/release-artifacts/{{APP_NAME}}-Windows-$VERSION.msix"
    $CERT_PATH = "target/dev_cert.pfx"
    $CERT_PASSWORD = "devpassword"
    # This must perfectly match the Publisher in AppxManifest.xml
    $PUBLISHER = "CN=04F86D81-D5D3-4477-A363-0CAE79356A84"

    if (-not (Test-Path $MSIX_PATH)) {
        Write-Error "MSIX file not found! Run 'just msix' first."
        exit 1
    }

    if (-not (Test-Path $CERT_PATH)) {
        Write-Host "Generating new local development certificate..."
        $cert = New-SelfSignedCertificate -Type Custom -Subject $PUBLISHER -KeyUsage DigitalSignature -FriendlyName "eBIRForms Local Dev" -CertStoreLocation "Cert:\CurrentUser\My" -TextExtension @("2.5.29.37={text}1.3.6.1.5.5.7.3.3", "2.5.29.19={text}")
        $pwd = ConvertTo-SecureString -String $CERT_PASSWORD -Force -AsPlainText
        Export-PfxCertificate -cert $cert -FilePath $CERT_PATH -Password $pwd | Out-Null
        
        Write-Host "Prompting for Administrator privileges to trust the local development certificate..."
        $certPathAbs = (Resolve-Path $CERT_PATH).Path
        $psCommand = "Import-PfxCertificate -FilePath '$certPathAbs' -Password (ConvertTo-SecureString -String '$CERT_PASSWORD' -Force -AsPlainText) -CertStoreLocation 'Cert:\LocalMachine\TrustedPeople'"
        Start-Process pwsh -ArgumentList "-Command", $psCommand -Verb RunAs -Wait
    }

    $SDK_DIR = Get-ChildItem "C:\Program Files (x86)\Windows Kits\10\bin\10.*" | Sort-Object Name -Descending | Select-Object -First 1
    $SIGNTOOL = "$($SDK_DIR.FullName)\x64\signtool.exe"

    if (Test-Path $SIGNTOOL) {
        Write-Host "Signing the MSIX package..."
        & $SIGNTOOL sign /fd SHA256 /a /f $CERT_PATH /p $CERT_PASSWORD $MSIX_PATH
        Write-Host "✅ MSIX package signed! You can now double-click the .msix file to install it locally."
    } else {
        Write-Warning "⚠️ signtool.exe not found."
    }

# Publish a new release (tags and pushes to trigger CI)
[unix]
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

[windows]
publish version="":
    #!pwsh -NoProfile
    $ErrorActionPreference = 'Stop'
    $ver = '{{version}}'
    if ($ver) {
        Write-Host "Forcing version to $ver"
        (Get-Content Cargo.toml) -replace '^version = ".*"', "version = `"$ver`"" | Set-Content Cargo.toml
        if (Test-Path crates/bir-print/Cargo.toml) {
            (Get-Content crates/bir-print/Cargo.toml) -replace '^version = ".*"', "version = `"$ver`"" | Set-Content crates/bir-print/Cargo.toml
        }
        $NEW_VER = $ver
    } else {
        $NEW_VER = "{{VERSION}}"
    }
    git add -A
    git commit -m "release: v$NEW_VER" --allow-empty
    git tag -a "v$NEW_VER" -m "Release v$NEW_VER"
    git push origin main
    git push origin "v$NEW_VER"
    Write-Host "🚀 Release v$NEW_VER triggered"

# Remove build artifacts
[unix]
clean:
    cargo clean
    rm -rf {{RELEASE_DIR}}

[windows]
clean:
    #!pwsh -NoProfile
    cargo clean
    if (Test-Path "{{RELEASE_DIR}}") { Remove-Item "{{RELEASE_DIR}}" -Recurse -Force }

# --- Hidden OS-specific packaging tasks ---

_package-mac args="": build-packaged-form-renderer
    #!/usr/bin/env bash
    set -e
    FEATURES=""
    for arg in {{args}}; do
        case "$arg" in
            --inspector)     FEATURES="${FEATURES:+$FEATURES,}inspector" ;;
            --native-evidence) FEATURES="${FEATURES:+$FEATURES,}dev-tools" ;;
        esac
    done
    FEATURES_FLAG=""
    if [ -n "$FEATURES" ]; then FEATURES_FLAG="--features $FEATURES"; fi
    echo "Building for ARM64..."
    cargo build --locked --release --target {{MAC_ARM_TARGET}} $FEATURES_FLAG
    echo "Building for x86_64..."
    cargo build --locked --release --target {{MAC_X86_TARGET}} $FEATURES_FLAG
    mkdir -p {{RELEASE_DIR}}
    echo "Creating universal binary (lipo)..."
    lipo -create target/{{MAC_ARM_TARGET}}/release/bir target/{{MAC_X86_TARGET}}/release/bir -output {{RELEASE_DIR}}/bir
    echo "Creating .app bundle..."
    rm -rf "{{MAC_APP}}"
    mkdir -p "{{MAC_APP}}/Contents/MacOS" "{{MAC_APP}}/Contents/Resources"
    cp {{RELEASE_DIR}}/bir "{{MAC_APP}}/Contents/MacOS/"
    cp -R assets "{{MAC_APP}}/Contents/Resources/"
    rm -rf "{{MAC_APP}}/Contents/Resources/assets/macos"
    cp assets/AppIcon.icns "{{MAC_APP}}/Contents/Resources/"

    cp assets/macos/Info.plist "{{MAC_APP}}/Contents/Info.plist"
    sed -i '' "s/VERSION_PLACEHOLDER/{{VERSION}}/g" "{{MAC_APP}}/Contents/Info.plist"
    sed -i '' "s/BUILD_NUMBER_PLACEHOLDER/{{BUILD_NUMBER}}/g" "{{MAC_APP}}/Contents/Info.plist"
    sed -i '' "s/BUNDLE_ID_PLACEHOLDER/{{BUNDLE_ID}}/g" "{{MAC_APP}}/Contents/Info.plist"
    sed -i '' "s/APP_NAME_PLACEHOLDER/{{APP_NAME}}/g" "{{MAC_APP}}/Contents/Info.plist"
    if [ "$DEV_MODE" = "true" ] || [ "$DEVELOPER_MODE" = "true" ]; then
        echo "Injecting DEVELOPER_MODE into Info.plist..."
        /usr/libexec/PlistBuddy -c "Add :LSEnvironment dict" "{{MAC_APP}}/Contents/Info.plist" || true
        /usr/libexec/PlistBuddy -c "Add :LSEnvironment:DEVELOPER_MODE string true" "{{MAC_APP}}/Contents/Info.plist" || true
    fi
    python3 scripts/audit_no_legacy.py --package-root "{{MAC_APP}}"
    touch "{{MAC_APP}}"
    
    echo "Ad-hoc codesigning application..."
    codesign --force --options runtime --entitlements entitlements.dev.plist --sign "-" "{{MAC_APP}}"
    codesign --verify --strict --verbose=2 "{{MAC_APP}}"

    echo "✅ {{MAC_APP}} created and codesigned"
    
    if command -v create-dmg >/dev/null 2>&1; then \
        rm -f "{{RELEASE_DIR}}/{{APP_NAME}}-macOS-{{VERSION}}.dmg"; \
        create-dmg --volname "{{APP_NAME}}" --window-size 600 400 --icon-size 100 --icon "{{APP_NAME}}.app" 150 190 --app-drop-link 450 190 "{{RELEASE_DIR}}/{{APP_NAME}}-macOS-{{VERSION}}.dmg" "{{MAC_APP}}"; \
        echo "✅ DMG created"; \
    else \
        echo "⚠️ create-dmg not found. Falling back to zip..."; \
        cd {{RELEASE_DIR}} && zip -r "{{APP_NAME}}-macOS-{{VERSION}}.zip" "{{APP_NAME}}.app"; \
    fi

_package-mac-appstore args="": build-packaged-form-renderer
    #!/usr/bin/env bash
    set -e
    CERT="${CODESIGN_IDENTITY:--}"
    echo "Building for Mac App Store..."
    # Always include mas_build feature
    FEATURES="mas_build"
    for arg in {{args}}; do
        case "$arg" in
            --inspector)     FEATURES="${FEATURES},inspector" ;;
        esac
    done
    FEATURES_FLAG="--features $FEATURES"
    
    echo "Building for ARM64..."
    cargo build --locked --release --target {{MAC_ARM_TARGET}} $FEATURES_FLAG
    echo "Building for x86_64..."
    cargo build --locked --release --target {{MAC_X86_TARGET}} $FEATURES_FLAG
    
    mkdir -p {{RELEASE_DIR}}
    echo "Creating universal binary (lipo)..."
    lipo -create target/{{MAC_ARM_TARGET}}/release/bir target/{{MAC_X86_TARGET}}/release/bir -output {{RELEASE_DIR}}/bir
    
    echo "Creating sandboxed .app bundle..."
    rm -rf "{{MAC_APP}}"
    mkdir -p "{{MAC_APP}}/Contents/MacOS" "{{MAC_APP}}/Contents/Resources"
    cp {{RELEASE_DIR}}/bir "{{MAC_APP}}/Contents/MacOS/"
    cp -R assets "{{MAC_APP}}/Contents/Resources/"
    rm -rf "{{MAC_APP}}/Contents/Resources/assets/macos"
    cp assets/AppIcon.icns "{{MAC_APP}}/Contents/Resources/"

    
    cp assets/macos/Info.plist "{{MAC_APP}}/Contents/Info.plist"
    sed -i '' "s/VERSION_PLACEHOLDER/{{VERSION}}/g" "{{MAC_APP}}/Contents/Info.plist"
    sed -i '' "s/BUILD_NUMBER_PLACEHOLDER/{{BUILD_NUMBER}}/g" "{{MAC_APP}}/Contents/Info.plist"
    sed -i '' "s/BUNDLE_ID_PLACEHOLDER/{{BUNDLE_ID}}/g" "{{MAC_APP}}/Contents/Info.plist"
    sed -i '' "s/APP_NAME_PLACEHOLDER/{{APP_NAME}}/g" "{{MAC_APP}}/Contents/Info.plist"
    
    # Add Sandbox requirement to Info.plist
    /usr/libexec/PlistBuddy -c "Add :LSApplicationCategoryType string public.app-category.finance" "{{MAC_APP}}/Contents/Info.plist" || true
    
    # Automatically declare export compliance exemption to skip the Encryption popup
    /usr/libexec/PlistBuddy -c "Add :ITSAppUsesNonExemptEncryption bool false" "{{MAC_APP}}/Contents/Info.plist" || true
    
    if [ "$DEV_MODE" = "true" ] || [ "$DEVELOPER_MODE" = "true" ]; then
        echo "Injecting DEVELOPER_MODE into Info.plist..."
        /usr/libexec/PlistBuddy -c "Add :LSEnvironment dict" "{{MAC_APP}}/Contents/Info.plist" || true
        /usr/libexec/PlistBuddy -c "Add :LSEnvironment:DEVELOPER_MODE string true" "{{MAC_APP}}/Contents/Info.plist" || true
    fi
    
    if [ -f "assets/macos/embedded.provisionprofile" ]; then
        echo "Embedding provisioning profile for TestFlight..."
        cp "assets/macos/embedded.provisionprofile" "{{MAC_APP}}/Contents/embedded.provisionprofile"
    else
        echo "⚠️  No embedded.provisionprofile found in assets/macos/. TestFlight builds require this."
    fi
    python3 scripts/audit_no_legacy.py --package-root "{{MAC_APP}}"
    
    touch "{{MAC_APP}}"
    
    echo "Stripping extended attributes (quarantine)..."
    xattr -cr "{{MAC_APP}}"
    
    echo "Codesigning application with identity: $CERT..."
    codesign --force --options runtime --entitlements entitlements.plist --sign "$CERT" "{{MAC_APP}}"

    echo "✅ {{MAC_APP}} created and codesigned"
    
    echo "Creating unsigned .pkg for App Store submission..."
    PKG_PATH="{{RELEASE_DIR}}/{{APP_NAME}}-macOS-MAS-{{VERSION}}.pkg"
    rm -f "$PKG_PATH"
    
    if [ -n "${INSTALLER_IDENTITY:-}" ]; then
        echo "Creating signed .pkg for App Store submission using identity: $INSTALLER_IDENTITY..."
        productbuild --sign "$INSTALLER_IDENTITY" --component "{{MAC_APP}}" /Applications "$PKG_PATH"
        echo "✅ Signed PKG created: $PKG_PATH"
    else
        echo "Creating unsigned .pkg for App Store submission..."
        productbuild --component "{{MAC_APP}}" /Applications "$PKG_PATH"
        echo "✅ Unsigned PKG created: $PKG_PATH"
        echo "⚠️  Note: You must also codesign the .pkg with an 'Apple Distribution' certificate before submitting."
    fi

# NOTE: _package-win was removed — use 'just exe' or 'just msix' instead.

_package-linux args="": build-packaged-form-renderer
    #!/usr/bin/env bash
    set -e
    FEATURES=""
    for arg in {{args}}; do
        case "$arg" in
            --inspector)     FEATURES="${FEATURES:+$FEATURES,}inspector" ;;
        esac
    done
    FEATURES_FLAG=""
    if [ -n "$FEATURES" ]; then FEATURES_FLAG="--features $FEATURES"; fi
    cargo build --locked --release --target {{LINUX_TARGET}} $FEATURES_FLAG
    mkdir -p {{RELEASE_DIR}}
    if command -v cargo-deb >/dev/null 2>&1; then
        DEB="{{RELEASE_DIR}}/{{APP_NAME}}-Linux-x64-{{VERSION}}.deb"
        cargo deb --locked -p bir-desktop --no-build --target {{LINUX_TARGET}} -o "$DEB"
        AUDIT_ROOT="$(mktemp -d)"
        trap 'rm -rf "$AUDIT_ROOT"' EXIT
        dpkg-deb --extract "$DEB" "$AUDIT_ROOT"
        python3 scripts/audit_no_legacy.py --package-root "$AUDIT_ROOT"
        rm -rf "$AUDIT_ROOT"
        trap - EXIT
        echo "✅ .deb: $DEB"
    else
        echo "⚠️ cargo-deb not found. Falling back to tarball..."
        PKG="{{RELEASE_DIR}}/{{APP_NAME}}-Linux-{{VERSION}}"
        TARBALL="{{RELEASE_DIR}}/{{APP_NAME}}-Linux-x64-{{VERSION}}.tar.gz"
        mkdir -p "$PKG"
        cp target/{{LINUX_TARGET}}/release/bir "$PKG/"
        cp -R assets "$PKG/"
        python3 scripts/audit_no_legacy.py --package-root "$PKG"
        if [ "$DEV_MODE" = "true" ] || [ "$DEVELOPER_MODE" = "true" ]; then
            echo "DEVELOPER_MODE=true" > "$PKG/.env"
        fi
        tar czf "$TARBALL" -C "{{RELEASE_DIR}}" "{{APP_NAME}}-Linux-{{VERSION}}"
        AUDIT_ROOT="$(mktemp -d)"
        trap 'rm -rf "$AUDIT_ROOT"' EXIT
        tar xzf "$TARBALL" -C "$AUDIT_ROOT"
        python3 scripts/audit_no_legacy.py --package-root "$AUDIT_ROOT"
        rm -rf "$AUDIT_ROOT"
        trap - EXIT
        echo "✅ Tarball: $TARBALL"
    fi
