# Headless CLI crate `bir`: just publish-bir
#   just publish-bir                 bump patch of crates/bir, push main, tag bir-v<version>
#   just publish-bir 0.1.0           tag that exact version (first release)
# CI on bir-v* cargo-publishes `bir` and uploads cargo-binstall archives.
# Desktop releases stay `just publish` / `just publish 0.2.0` (tag v*).
# This recipe does not cargo-publish and does not read a registry token.
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
PYTHON := if os_family() == "windows" { "python" } else { "python3" }
ISCC := "C:\\Program Files (x86)\\Inno Setup 6\\ISCC.exe"
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

# Run the app locally — the gpui-shell JS app (crates/bir-shell, a standalone
# workspace). Frozen HTML print previews are compiled into bir-print.
run *args="": (shell-run args)

# The painted (native GPUI) app, kept buildable until the cutover completes.
run-painted:
    cargo run --locked --bin bir-desktop --features dev-tools

# gpui-shell host — a standalone workspace under crates/bir-shell (its own
# lockfile carries the LLRT/quickjs pins; see its Cargo.toml). `just check`
# covers the root workspace only, so the shell gets its own targets.
# Development runs use GPUI Fast (the `gpui-fast` feature), which drew the
# Form library's rebuilds at a higher rate than upstream GPUI in a debug
# build. `run-upstream` runs the upstream engine the app ships on today, to
# reproduce release behaviour.
shell-run *args="":
    cargo run --locked --manifest-path crates/bir-shell/Cargo.toml --bin bir-shell --features gpui-fast {{args}}

run-upstream *args="":
    cargo run --locked --manifest-path crates/bir-shell/Cargo.toml --bin bir-shell {{args}}

shell-ui-test:
    node --experimental-vm-modules tools/test_form_row_invalidation.mjs
    node --experimental-vm-modules tools/test_form_profile_and_sample.mjs
    node --experimental-vm-modules tools/test_date_field_rows.mjs
    node --experimental-vm-modules tools/test_form_existing_offer.mjs
    node --experimental-vm-modules tools/test_form_sample_save.mjs
    node --experimental-vm-modules tools/test_form_shared_mechanisms.mjs
    node --experimental-vm-modules tools/test_form_q3_withholding.mjs
    node --experimental-vm-modules tools/test_form_monthly_withholding.mjs
    node --experimental-vm-modules tools/test_form_q3_income_tax.mjs
    node --experimental-vm-modules tools/test_form_annual_1604.mjs
    node --experimental-vm-modules tools/test_form_donor_estate.mjs
    node --experimental-vm-modules tools/test_form_as_needed.mjs
    node tools/test_font_advances.mjs
    node --experimental-vm-modules tools/test_form_radio_untick.mjs
    node --experimental-vm-modules tools/test_form_decisions_round_3.mjs
    node --experimental-vm-modules tools/test_home_calendar_as_needed.mjs
    node --experimental-vm-modules tools/test_taxpayer_export.mjs
    node --experimental-vm-modules tools/test_settings_window.mjs

shell-check: shell-ui-test
    cargo fmt --manifest-path crates/bir-shell/Cargo.toml -- --check
    cargo check --locked --manifest-path crates/bir-shell/Cargo.toml
    cargo check --locked --manifest-path crates/bir-shell/Cargo.toml --features gpui-fast
    cargo clippy --locked --manifest-path crates/bir-shell/Cargo.toml --all-targets -- -D warnings
    {{ if os_family() == "windows" { "$env:EBIR_TEST_ENV='1'; $env:BIR_AUTH_BYPASS='1'; cargo test --locked --manifest-path crates/bir-shell/Cargo.toml" } else { "EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 cargo test --locked --manifest-path crates/bir-shell/Cargo.toml" } }}

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

# Build and run the painted Windows runtime smoke gate in isolated phases.
[windows]
smoke-windows profile="release":
    #!pwsh -NoProfile
    $ErrorActionPreference = 'Stop'
    $profileName = "{{profile}}"
    if ($profileName -notin @("debug", "release")) {
        throw "profile must be debug or release"
    }
    $cargoArgs = @("build", "--locked", "-p", "bir-desktop", "--bin", "bir-desktop", "--features", "dev-tools,agent")
    if ($profileName -eq "release") { $cargoArgs += "--release" }
    & cargo @cargoArgs
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    $targetDir = if ($profileName -eq "release") { "target\release" } else { "target\debug" }
    & {{PYTHON}} scripts/alias_desktop_bin.py $targetDir
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    $binary = if ($profileName -eq "release") {
        "target\release\bir.exe"
    } else {
        "target\debug\bir.exe"
    }
    $work = "target\windows-smoke\$profileName"
    & {{PYTHON}} scripts/windows_smoke.py --bin $binary --work $work --json-out "$work\results.json"
    exit $LASTEXITCODE

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

# Automated tab-walk over the generated form corpus (tools/formgen/tab_check.py).
# Omit the slug to walk every form in forms/. Artifacts (tab.json, page-<N>.png
# per page, and a review index.html) land in forms/review/ -- see that
# module's own docstring for what green/red mean and, just as importantly,
# what this walk cannot see.
tab-check slug="":
    {{PYTHON}} tools/formgen/tab_check.py {{slug}}

# Clear tab_check's review artifacts. forms/review is tracked-tree-adjacent
# but not gitignored, and the migration audit gate requires a clean tree, so
# these have to be cleared (or committed) before a gate run.
[unix]
review-clean:
    rm -rf forms/review
    @echo "forms/review cleared -- the gate needs a clean tree, so commit or clear this before running it."

[windows]
review-clean:
    #!pwsh -NoProfile
    if (Test-Path forms/review) { Remove-Item forms/review -Recurse -Force }
    Write-Host "forms/review cleared -- the gate needs a clean tree, so commit or clear this before running it."

# Serve forms/ (not forms/review/) so both a generated form's index.html AND
# tab_check's forms/review/<slug>/tab.json sit under ONE server root: the
# ?debug=tab overlay in forms/<slug>/index.html fetches
# ../review/<slug>/tab.json relative to itself, which only resolves if both
# trees share a root. This also still serves tab_check's page-<N>.png
# artifacts and forms/review/index.html without tabbing anything and without
# a file:// path -- from a phone on the same network if need be.
review-serve:
    cd forms && {{PYTHON}} -m http.server 4190 --bind 127.0.0.1

# Confirm committed freeze HTML and write the non-promotional identity
# copied into packaged assets/. Preview sheets are also embedded at compile time.
build-frozen-html-identity:
    {{PYTHON}} scripts/write_frozen_html_identity.py --html-frozen html-frozen --source-revision "$(git rev-parse HEAD)" --out assets/form-renderer-build-identity.json

# React native-evidence recipes were removed with the form-renderer.
# Frozen HTML preview is the print path. These names stay so operators
# do not follow a silent just recipe into a deleted driver.
[macos]
native-evidence-macos:
    @echo "Frozen HTML is the print path. The React native-evidence driver was removed."
    @echo "This recipe stays non-promotional and cannot write form-release-evidence.json."
    @exit 1

[macos]
native-evidence-macos-external:
    @echo "Frozen HTML is the print path. The React native-evidence driver was removed."
    @echo "This recipe stays non-promotional and cannot write form-release-evidence.json."
    @exit 1

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
            # PowerShell 7 cannot load Appx on this Server host; use Windows PowerShell 5.1 (#93).
            $env:EBIRFORMS_MSIX_PATH = $path
            powershell.exe -NoProfile -NonInteractive -Command 'Add-AppxPackage -Path $env:EBIRFORMS_MSIX_PATH'
            $msixExitCode = $LASTEXITCODE
            Remove-Item Env:EBIRFORMS_MSIX_PATH -ErrorAction SilentlyContinue
            if ($msixExitCode -ne 0) {
                [Console]::Error.WriteLine("MSIX installation failed (exit code $msixExitCode).")
                Write-Host 'If a different build with the same version is already installed, remove it first: powershell.exe -NoProfile -Command "Get-AppxPackage GOLDCODERSCORP.ebirforms | Remove-AppxPackage"'
                exit $msixExitCode
            }
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

# Usage: just testflight [--agent]
# Build the App Store package and upload it to App Store Connect for TestFlight
[unix]
testflight *args="": (app args) testflight-upload

# The control plane stays off until launched with GPUI_AGENT=1,
# GPUI_AGENT_ALLOW_RELEASE=1 and GPUI_AGENT_TOKEN. Internal testing only: never
# submit an agent build for App Store review.
# Build with the gpui-agent control plane and upload it for internal TestFlight
[unix]
testflight-agent *args="": (testflight "--agent" args)

# Authenticates with APP_STORE_KEY_ID, APP_STORE_ISSUER_ID and APP_STORE_P8_PATH
# from .env. Set TRANSPORTER to override the iTMSTransporter path.
# Upload the already-built App Store .pkg to App Store Connect (no rebuild)
[unix]
testflight-upload:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ "{{os()}}" != "macos" ]; then
        echo "⚠️ TestFlight uploads are only supported on macOS." >&2; exit 1
    fi
    set -a && source .env && set +a
    for var in APP_STORE_KEY_ID APP_STORE_ISSUER_ID APP_STORE_P8_PATH; do
        if [ -z "${!var:-}" ]; then echo "❌ $var is not set in .env" >&2; exit 1; fi
    done
    if [ ! -f "$APP_STORE_P8_PATH" ]; then
        echo "❌ APP_STORE_P8_PATH does not point to a file: $APP_STORE_P8_PATH" >&2; exit 1
    fi
    TRANSPORTER="${TRANSPORTER:-/Applications/Transporter.app/Contents/itms/bin/iTMSTransporter}"
    if [ ! -x "$TRANSPORTER" ]; then
        echo "❌ iTMSTransporter not found at $TRANSPORTER" >&2
        echo "   Install Transporter from the Mac App Store, or set TRANSPORTER." >&2
        exit 1
    fi
    PKG_PATH="$(pwd)/{{RELEASE_DIR}}/{{APP_NAME}}-macOS-MAS-{{VERSION}}.pkg"
    if [ ! -f "$PKG_PATH" ]; then
        echo "❌ $PKG_PATH not found. Build it first with: just app" >&2; exit 1
    fi
    if ! pkgutil --check-signature "$PKG_PATH" >/dev/null 2>&1; then
        echo "❌ $PKG_PATH is unsigned. Set INSTALLER_IDENTITY in .env and rebuild with: just app" >&2; exit 1
    fi
    BUILD="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleVersion' "{{MAC_APP}}/Contents/Info.plist" 2>/dev/null || echo '?')"
    # iTMSTransporter only reads the key from a fixed set of private_keys
    # directories; stage it in a throwaway one instead of touching $HOME.
    KEY_DIR="$(mktemp -d)"
    trap 'rm -rf "$KEY_DIR"' EXIT
    mkdir "$KEY_DIR/private_keys"
    ln -s "$(cd "$(dirname "$APP_STORE_P8_PATH")" && pwd)/$(basename "$APP_STORE_P8_PATH")" \
        "$KEY_DIR/private_keys/AuthKey_${APP_STORE_KEY_ID}.p8"
    echo "Uploading {{APP_NAME}} {{VERSION}} (build $BUILD) to App Store Connect..."
    (cd "$KEY_DIR" && "$TRANSPORTER" -m upload -assetFile "$PKG_PATH" \
        -apiKey "$APP_STORE_KEY_ID" -apiIssuer "$APP_STORE_ISSUER_ID" -v informational)
    echo "✅ Uploaded. Processing takes ~10–30 min; then add the build to a TestFlight group in App Store Connect."

# Build the Inno Setup executable installer (Windows only)
[private]
[windows]
check-exe-prerequisites:
    #!pwsh -NoProfile
    $python = "{{PYTHON}}"
    $iscc = "{{ISCC}}"
    if (-not (Get-Command $python -ErrorAction SilentlyContinue)) {
        [Console]::Error.WriteLine("Configured Python executable was not found: $python")
        exit 1
    }
    $versionOutput = & $python --version 2>&1
    $pythonExitCode = $LASTEXITCODE
    if ($pythonExitCode -ne 0 -or ($versionOutput -notmatch '^Python \d+\.\d+')) {
        [Console]::Error.WriteLine("Configured Python is not a usable CPython interpreter: $python")
        if ($pythonExitCode -ne 0) { exit $pythonExitCode }
        exit 1
    }
    if (-not (Test-Path -LiteralPath $iscc -PathType Leaf)) {
        [Console]::Error.WriteLine("Inno Setup compiler was not found: $iscc")
        exit 1
    }

[windows]
exe: check-exe-prerequisites build-frozen-html-identity
    #!pwsh -NoProfile
    $ErrorActionPreference = 'Stop'

    $ISCC = "{{ISCC}}"
    
    # The shipped app is the gpui-shell host (crates/bir-shell, a standalone
    # workspace) built with no features; the painted bir-desktop stays
    # buildable for development but is not packaged.
    cargo build --locked --release --manifest-path crates/bir-shell/Cargo.toml --target {{WIN_TARGET}}
    if ($LASTEXITCODE -ne 0) {
        $cargoExitCode = $LASTEXITCODE
        [Console]::Error.WriteLine("cargo build failed (exit code $cargoExitCode). Aborting EXE packaging.")
        exit $cargoExitCode
    }
    {{PYTHON}} scripts/alias_desktop_bin.py "crates/bir-shell/target/{{WIN_TARGET}}/release" bir-shell
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

    $VERSION = "{{VERSION}}"

    $stageDir = "target\installer-stage"
    if (Test-Path $stageDir) { Remove-Item $stageDir -Recurse -Force }
    New-Item -ItemType Directory -Force -Path $stageDir | Out-Null
    Copy-Item "crates\bir-shell\target\{{WIN_TARGET}}\release\bir.exe" "$stageDir\bir.exe"
    Copy-Item "assets" "$stageDir\assets" -Recurse
    Copy-Item "app" "$stageDir\app" -Recurse
    Copy-Item "print-overrides" "$stageDir\print-overrides" -Recurse

    {{PYTHON}} scripts/audit_no_legacy.py --package-root $stageDir
    if ($LASTEXITCODE -ne 0) {
        $auditExitCode = $LASTEXITCODE
        [Console]::Error.WriteLine("Installer staging tree failed the HTML-only/no-legacy audit.")
        exit $auditExitCode
    }

    {{PYTHON}} scripts/audit_installer_sources.py --installer installer.iss --stage-dir $stageDir
    if ($LASTEXITCODE -ne 0) {
        $sourceExitCode = $LASTEXITCODE
        [Console]::Error.WriteLine("Installer [Files] source validation failed.")
        exit $sourceExitCode
    }

    Write-Host "Building installer with Inno Setup..."
    & $ISCC "/DMyAppVersion=$VERSION" "/DStageDir=$stageDir" installer.iss
    if ($LASTEXITCODE -ne 0) {
        $isccExitCode = $LASTEXITCODE
        [Console]::Error.WriteLine("Inno Setup failed (exit code $isccExitCode).")
        exit $isccExitCode
    }
    Write-Host "✅ Setup EXE created: target/release-artifacts/{{APP_NAME}}-Windows-x64-$VERSION-Setup.exe"

# Build a Store-only MSIX candidate (Windows only).
# This artifact is intentionally excluded from public GitHub releases. Store
# promotion remains blocked until the manifest artwork dimensions and packaged
# MSVC runtime behavior pass their separate Windows certification checks.
msix: build-frozen-html-identity
    #!pwsh -NoProfile
    $ErrorActionPreference = 'Stop'
    Write-Warning "STORE-ONLY MSIX candidate; not a public GitHub release artifact"
    Write-Warning "BLOCKED: certify manifest artwork and packaged MSVC runtime behavior before Store submission"
    
    # The shipped app is the gpui-shell host (crates/bir-shell, a standalone
    # workspace) built with no features; the painted bir-desktop stays
    # buildable for development but is not packaged.
    cargo build --locked --release --manifest-path crates/bir-shell/Cargo.toml --target {{WIN_TARGET}}
    if ($LASTEXITCODE -ne 0) {
        $cargoExitCode = $LASTEXITCODE
        [Console]::Error.WriteLine("cargo build failed (exit code $cargoExitCode). Aborting MSIX packaging.")
        exit $cargoExitCode
    }
    {{PYTHON}} scripts/alias_desktop_bin.py "crates/bir-shell/target/{{WIN_TARGET}}/release" bir-shell
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

    $VERSION = "{{VERSION}}"
    $MSIX_PATH = "target/release-artifacts/{{APP_NAME}}-Windows-$VERSION.msix"
    & powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/package_msix.ps1 `
        -ExePath "crates/bir-shell/target/{{WIN_TARGET}}/release/bir.exe" `
        -AssetsDir assets `
        -AppDir app `
        -PrintOverridesDir print-overrides `
        -Version $VERSION `
        -BuildNumber {{BUILD_NUMBER}} `
        -OutFile $MSIX_PATH `
        -Python {{PYTHON}}
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

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
        [Console]::Error.WriteLine("MSIX file not found! Run 'just msix' first.")
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

# Bump the patch, or tag an exact version, as bir-v* (not the desktop release).
[unix]
publish-bir version="":
    #!/usr/bin/env bash
    set -euo pipefail
    ver="{{version}}"
    if [ -n "${ver}" ] && ! [[ "${ver}" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
        echo "version must be MAJOR.MINOR.PATCH, got ${ver}" >&2
        exit 1
    fi
    if [ -n "${ver}" ]; then
        {{PYTHON}} scripts/publish_bir.py "${ver}"
    else
        {{PYTHON}} scripts/publish_bir.py
    fi

[windows]
publish-bir version="":
    #!pwsh -NoProfile
    $ErrorActionPreference = 'Stop'
    $ver = '{{version}}'
    if ($ver -and $ver -notmatch '^[0-9]+\.[0-9]+\.[0-9]+$') {
        [Console]::Error.WriteLine("version must be MAJOR.MINOR.PATCH, got $ver")
        exit 1
    }
    if ($ver) {
        {{PYTHON}} scripts/publish_bir.py $ver
    } else {
        {{PYTHON}} scripts/publish_bir.py
    }
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }


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

# Packages the shipped gpui-shell app: crates/bir-shell is a standalone
# workspace built with no features, and the .app stages app/ and
# print-overrides/ under Contents/Resources where the shell resolves them
# (crates/bir-shell/src/packaged.rs). The painted bir-desktop stays buildable
# for development but is not packaged here.
_package-mac args="": build-frozen-html-identity
    #!/usr/bin/env bash
    set -e
    for arg in {{args}}; do
        case "$arg" in
            --native-evidence|--agent)
                echo "⚠️  $arg is a bir-desktop feature; the shipped shell ignores it" ;;
        esac
    done
    echo "Building for ARM64..."
    cargo build --locked --release --manifest-path crates/bir-shell/Cargo.toml --target {{MAC_ARM_TARGET}}
    echo "Building for x86_64..."
    cargo build --locked --release --manifest-path crates/bir-shell/Cargo.toml --target {{MAC_X86_TARGET}}
    {{PYTHON}} scripts/alias_desktop_bin.py crates/bir-shell/target/{{MAC_ARM_TARGET}}/release bir-shell
    {{PYTHON}} scripts/alias_desktop_bin.py crates/bir-shell/target/{{MAC_X86_TARGET}}/release bir-shell
    mkdir -p {{RELEASE_DIR}}
    echo "Creating universal binary (lipo)..."
    lipo -create crates/bir-shell/target/{{MAC_ARM_TARGET}}/release/bir crates/bir-shell/target/{{MAC_X86_TARGET}}/release/bir -output {{RELEASE_DIR}}/bir
    echo "Creating .app bundle..."
    rm -rf "{{MAC_APP}}"
    mkdir -p "{{MAC_APP}}/Contents/MacOS" "{{MAC_APP}}/Contents/Resources"
    cp {{RELEASE_DIR}}/bir "{{MAC_APP}}/Contents/MacOS/"
    cp -R assets "{{MAC_APP}}/Contents/Resources/"
    rm -rf "{{MAC_APP}}/Contents/Resources/assets/macos"
    cp assets/AppIcon.icns "{{MAC_APP}}/Contents/Resources/"
    cp -R app "{{MAC_APP}}/Contents/Resources/"
    cp -R print-overrides "{{MAC_APP}}/Contents/Resources/"

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
    {{PYTHON}} scripts/audit_no_legacy.py --package-root "{{MAC_APP}}"
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

# MAS/TestFlight still packages the painted bir-desktop + mas_build — a
# conservative hold, not the shipped-app story. bir-shell has no mas_build
# equivalent yet, and its QuickJS JIT engine needs the sandboxed
# allow-unsigned-executable-memory/allow-jit entitlement story worked out
# before App Store submission. Non-store releases (DMG, EXE, deb/rpm) ship
# bir-shell; cutting MAS over is follow-up work (see
# docs/plans/gpui-shell-migration.md, S7).
_package-mac-appstore args="": build-frozen-html-identity
    #!/usr/bin/env bash
    set -e
    CERT="${CODESIGN_IDENTITY:--}"
    echo "Building for Mac App Store (painted bir-desktop — see deferral note above)..."
    # Always include mas_build feature
    FEATURES="mas_build"
    for arg in {{args}}; do
        case "$arg" in
            --agent)         FEATURES="${FEATURES},agent" ;;
        esac
    done
    FEATURES_FLAG="--features $FEATURES"
    
    echo "Building for ARM64..."
    cargo build --locked --release --target {{MAC_ARM_TARGET}} $FEATURES_FLAG
    echo "Building for x86_64..."
    cargo build --locked --release --target {{MAC_X86_TARGET}} $FEATURES_FLAG
    
    {{PYTHON}} scripts/alias_desktop_bin.py target/{{MAC_ARM_TARGET}}/release
    {{PYTHON}} scripts/alias_desktop_bin.py target/{{MAC_X86_TARGET}}/release
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
    {{PYTHON}} scripts/audit_no_legacy.py --package-root "{{MAC_APP}}"
    
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

# Fails the Linux package when the app-menu launcher or its icons are missing,
# so docks can match windows (app id dev.goldcoders.bir) to the installed entry.
_check-linux-launcher apps_dir icons_dir:
    #!/usr/bin/env bash
    set -e
    ENTRY="{{apps_dir}}/{{BUNDLE_ID}}.desktop"
    test -f "$ENTRY" || { echo "❌ missing launcher: $ENTRY"; exit 1; }
    for n in 48 64 128 256 512; do
        ICON="{{icons_dir}}/${n}x${n}/apps/{{BUNDLE_ID}}.png"
        test -f "$ICON" || { echo "❌ missing icon: $ICON"; exit 1; }
    done
    if command -v desktop-file-validate >/dev/null 2>&1; then desktop-file-validate "$ENTRY"; fi
    echo "✅ Linux launcher + icons present"

# Packages the shipped gpui-shell app (crates/bir-shell, standalone workspace,
# no features). deb/rpm metadata lives in crates/bir-shell/Cargo.toml; the
# tarball fallback stages app/ and print-overrides/ beside the binary.
_package-linux: build-frozen-html-identity
    #!/usr/bin/env bash
    set -e
    cargo build --locked --release --manifest-path crates/bir-shell/Cargo.toml --target {{LINUX_TARGET}}
    {{PYTHON}} scripts/alias_desktop_bin.py crates/bir-shell/target/{{LINUX_TARGET}}/release bir-shell
    mkdir -p {{RELEASE_DIR}}
    if command -v cargo-deb >/dev/null 2>&1; then
        DEB="{{RELEASE_DIR}}/{{APP_NAME}}-Linux-x64-{{VERSION}}.deb"
        cargo deb --locked --manifest-path crates/bir-shell/Cargo.toml --no-build --target {{LINUX_TARGET}} -o "$DEB"
        AUDIT_ROOT="$(mktemp -d)"
        trap 'rm -rf "$AUDIT_ROOT"' EXIT
        dpkg-deb --extract "$DEB" "$AUDIT_ROOT"
        {{PYTHON}} scripts/audit_no_legacy.py --package-root "$AUDIT_ROOT"
        {{just_executable()}} _check-linux-launcher "$AUDIT_ROOT/usr/share/applications" "$AUDIT_ROOT/usr/share/icons/hicolor"
        rm -rf "$AUDIT_ROOT"
        trap - EXIT
        echo "✅ .deb: $DEB"
    else
        echo "⚠️ cargo-deb not found. Falling back to tarball..."
        PKG="{{RELEASE_DIR}}/{{APP_NAME}}-Linux-{{VERSION}}"
        TARBALL="{{RELEASE_DIR}}/{{APP_NAME}}-Linux-x64-{{VERSION}}.tar.gz"
        mkdir -p "$PKG"
        cp crates/bir-shell/target/{{LINUX_TARGET}}/release/bir "$PKG/"
        cp -R assets "$PKG/"
        cp -R app "$PKG/"
        cp -R print-overrides "$PKG/"
        cp assets/linux/install-desktop-entry.sh "$PKG/"
        {{PYTHON}} scripts/audit_no_legacy.py --package-root "$PKG"
        if [ "$DEV_MODE" = "true" ] || [ "$DEVELOPER_MODE" = "true" ]; then
            echo "DEVELOPER_MODE=true" > "$PKG/.env"
        fi
        tar czf "$TARBALL" -C "{{RELEASE_DIR}}" "{{APP_NAME}}-Linux-{{VERSION}}"
        AUDIT_ROOT="$(mktemp -d)"
        trap 'rm -rf "$AUDIT_ROOT"' EXIT
        tar xzf "$TARBALL" -C "$AUDIT_ROOT"
        {{PYTHON}} scripts/audit_no_legacy.py --package-root "$AUDIT_ROOT"
        PKG_ROOT="$AUDIT_ROOT/{{APP_NAME}}-Linux-{{VERSION}}"
        test -x "$PKG_ROOT/install-desktop-entry.sh" || { echo "❌ tarball is missing install-desktop-entry.sh"; exit 1; }
        {{just_executable()}} _check-linux-launcher "$PKG_ROOT/assets/linux" "$PKG_ROOT/assets/linux/icons/hicolor"
        rm -rf "$AUDIT_ROOT"
        trap - EXIT
        echo "✅ Tarball: $TARBALL"
    fi
