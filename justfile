# The `bir-v*` workflow builds the `bir` command from crates/bir-cli.
# Desktop releases stay `just publish` / `just publish 0.2.0` (tag v*).
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

# Covers iPhone, iPad, Mac, Windows, Linux, Android, the tray and the menu-row mark, and lists each file it wrote.
# With a path, that SVG is copied in as the new source first: `just icons ~/new-logo.svg`.
# Needs uv (it fetches Pillow) and rsvg-convert (brew install librsvg).
# Rebuild every app icon from assets/brand/buwiz-forms.svg
icons svg="":
    #!/usr/bin/env bash
    set -euo pipefail
    if [ -n "{{svg}}" ]; then
        cp "{{svg}}" assets/brand/buwiz-forms.svg
        echo "new source: {{svg}} -> assets/brand/buwiz-forms.svg"
    fi
    uv run -q --no-project --with 'pillow>=11.1' python assets/brand/make_icons.py

# Local only, not a CI job: another machine's librsvg can draw edges differently. `just icons` fixes a failure.
# Exit 1 when a committed icon is not what assets/brand/buwiz-forms.svg gives
icons-check:
    uv run -q --no-project --with 'pillow>=11.1' --with numpy python assets/brand/make_icons.py --check

# Then `just icons`. Needs uv (it fetches Pillow and NumPy) and rsvg-convert. assets/brand/parts/README.md says what each part is.
# Rebuild assets/brand/buwiz-forms.svg from the logo's eight parts in assets/brand/parts/
logo *args:
    uv run -q --no-project --with 'pillow>=11.1' --with numpy python assets/brand/parts/merge.py assets/brand/buwiz-forms.svg {{args}}

# Counts the forms and reads the eBIRForms package version from the code; the app version is parked in the script.
# `just badges --check` exits 1 when a committed badge is stale. Needs only uv.
# Rewrite the README badges in assets/badges/ (our own SVG files, so GitHub shows them without camo)
badges *args:
    uv run -q --no-project python assets/badges/make_badges.py {{args}}

# Show available commands
help:
    @just --list

# `just run` is the NSH app (nsh-run, a macOS dev window); the old gpui-shell host is gone.
[macos]
run db=env_var_or_default("BIR_DATABASE_PATH", "") *args="": (nsh-run db args)

# Options pass through: `just mock-relay --fail reject`.
# Local mock of the Buwiz relay, loopback only; hands files on to a running
# `just mock-bir` (docs/relay.md).
mock-relay *args="":
    cargo run --locked -p bir-core --features mock-relay --bin mock_relay -- {{args}}

# In-memory development accounts on localhost:3008 (docs/cloud-account.md).
mock-account *args="":
    cargo run --locked -p bir-core --features mock-account --bin mock_account -- {{args}}

# Options pass through: `just mock-bir --fail fail-after-store`.
# Local mock of the BIR SFTP endpoint, loopback only (docs/mock-bir.md).
mock-bir *args="":
    cargo run --locked -p bir-core --features mock-bir --bin mock_bir -- {{args}}

# Refused since accounts-and-lock A1 (I3): a release app ignores BIR_DATABASE_PATH, so it would open the owner's live
# data folder, not the copy, against the mock. Needs rework before it runs again (docs/mock-bir.md).
[macos]
mock-bir-app db app="/Applications/eBIRForms.app":
    #!/usr/bin/env bash
    echo "mock-bir-app refused: since accounts-and-lock A1 a release app ignores BIR_DATABASE_PATH, so {{app}} would open the live data folder instead of {{db}}; the recipe needs rework (docs/mock-bir.md)." >&2
    exit 1

# bir-app-api's agent tests need the `headless` feature (was the shell-check
# recipe's last line); the four safety variables keep it off keychain, a real
# database and the BIR.
headless-agent-test:
    {{ if os_family() == "windows" { "$env:EBIR_TEST_ENV='1'; $env:BIR_AUTH_BYPASS='1'; $env:BIR_SFTP_DRY_RUN='1'; $env:BUWIZ_NO_KEYCHAIN='1'; cargo test --locked -p bir-app-api --features headless --lib agent::" } else { "EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1 BUWIZ_NO_KEYCHAIN=1 cargo test --locked -p bir-app-api --features headless --lib agent::" } }}
    {{ if os_family() == "windows" { "$env:EBIR_TEST_ENV='1'; $env:BIR_AUTH_BYPASS='1'; $env:BIR_SFTP_DRY_RUN='1'; $env:BUWIZ_NO_KEYCHAIN='1'; cargo test --locked -p bir-cli" } else { "EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1 BUWIZ_NO_KEYCHAIN=1 cargo test --locked -p bir-cli" } }}

# bir-core and bir-app-api tests in the plain-SQLite build the NSH app ships (no sqlcipher), with the four safety variables:
# nothing reaches the keychain, a real database or the BIR.
[unix]
core-plain-test:
    #!/usr/bin/env bash
    set -euo pipefail
    export EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1 BUWIZ_NO_KEYCHAIN=1
    unset BIR_SFTP_LIVE BIR_DATABASE_PATH
    cargo test -p bir-core --no-default-features
    cargo test -p bir-app-api --no-default-features

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

# Pull requests run only the quick checks; the test suites run on a push to test/** (docs/CI.md). Pushes HEAD to
# test/<system>/<branch>, so the results show on the pull request of that commit; a newer push replaces the older run.
# No system: Linux and Windows. Remote: $CI_TEST_REMOTE, else origin (it must be allowed to push workflow files).
# Ask GitHub for the test suites at this commit: `just ci-test [linux|windows|macos|all]`
[unix]
ci-test system="":
    #!/usr/bin/env bash
    set -euo pipefail
    case "{{system}}" in
        linux|windows|macos|all) lane="{{system}}" ;;
        "") lane="linux-windows" ;;
        *) echo "ci-test: system is linux, windows, macos or all (none: Linux and Windows)" >&2; exit 1 ;;
    esac
    branch=$(git symbolic-ref --short -q HEAD) || { echo "ci-test: check out the pull request's branch first" >&2; exit 1; }
    name=$(sed -E 's#^(feat|fix|spike)/##' <<<"$branch")
    remote="${CI_TEST_REMOTE:-origin}"
    sha=$(git rev-parse HEAD)
    target="test/$lane/$name"
    git push --force "$remote" "$sha:refs/heads/$target"
    echo "$sha on $target"
    echo "https://github.com/hexuria/buwiz-forms/actions?query=branch%3A${target//\//%2F}"
    echo "When the runs are done: just ci-test-clean"

# Push the exact head GitHub reports for pull request <number> to test/<system>/pr-<number> and print the Actions URL.
# No system: Linux and Windows. The script fetches pull/<n>/head and refuses when that object is not GitHub's head.
# Ask GitHub for the test suites at a pull request's exact head: `just ci-test-pr <n> [linux|windows|macos|all]`
[unix]
ci-test-pr number system="":
    {{PYTHON}} scripts/ci_batch.py test-pr {{quote(number)}} {{quote(system)}}

# Deletes only branches with no queued or running workflow run; their results stay on the commit and its pull request.
# Delete the test/** branches whose runs have all finished
[unix]
ci-test-clean:
    #!/usr/bin/env bash
    set -euo pipefail
    remote="${CI_TEST_REMOTE:-origin}"
    git ls-remote --heads "$remote" 'refs/heads/test/*' | while read -r _ ref; do
        branch="${ref#refs/heads/}"
        busy=$(gh run list --repo hexuria/buwiz-forms --branch "$branch" --limit 100 --json status \
            --jq '[.[] | select(.status != "completed")] | length' </dev/null)
        if [ "$busy" = 0 ]; then
            git push "$remote" --delete "$branch" </dev/null
        else
            echo "keeping $branch: $busy run(s) not finished"
        fi
    done

# Several pull requests proven together: main plus the squash of each, in order, pushed to test/<system>/batch-...
# Every pull request's `ci-ok` must be green at its current head; `--dry-run` pushes nothing, and
# `--allow-unchecked <pr>` (repeatable) is the hand-run escape hatch. `just ci-batch [system] <pr> <pr> ...`
ci-batch *args:
    {{PYTHON}} scripts/ci_batch.py build {{args}}

# A batch's runs: green or red per workflow; for red, the failed jobs and the two halves to re-run
ci-batch-status branch:
    {{PYTHON}} scripts/ci_batch.py status {{quote(branch)}}

# Prints (never runs) a green batch's merge commands, after checking main and every head are unchanged
ci-batch-merge branch:
    {{PYTHON}} scripts/ci_batch.py merge {{quote(branch)}}

# After the merges: is main's tree the tree the batch proved?
ci-batch-verify branch:
    {{PYTHON}} scripts/ci_batch.py verify {{quote(branch)}}

# After a batch of merges: a push to main runs only the quick checks. Also what the nightly run does, on demand.
# Start a complete run of main: every suite of ci.yml and nsh.yml on all three systems
ci-main:
    gh workflow run ci.yml --repo hexuria/buwiz-forms --ref main -f systems=all -f suites=all
    gh workflow run nsh.yml --repo hexuria/buwiz-forms --ref main -f systems=all -f suites=all
    @echo "https://github.com/hexuria/buwiz-forms/actions?query=branch%3Amain+event%3Aworkflow_dispatch"

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
            Write-Host "   Build it first: nsh package --target windows, then scripts/package_nsh_windows.ps1 (CONTRIBUTING.md)."
            exit 1
        }
    } elseif ($artifacts.Count -eq 0) {
        Write-Host "❌ No installers found in $DIR." -ForegroundColor Red
        Write-Host "   Build them first: nsh package --target windows, then scripts/package_nsh_windows.ps1 (CONTRIBUTING.md)."
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
            Write-Host "⚠️  MSIX must be signed before sideloading: a release build is; for a local one use scripts/package_nsh_windows.ps1 -SmokeMsix." -ForegroundColor Yellow
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

# Publish a new release (tags and pushes to trigger CI)
[unix]
publish version="":
    #!/usr/bin/env bash
    set -e
    if [ -n "{{version}}" ]; then
        echo "Forcing version to {{version}}"
        {{PYTHON}} scripts/set_release_version.py "{{version}}"
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
        {{PYTHON}} scripts/set_release_version.py "$ver"
        if ($LASTEXITCODE -ne 0) { throw "set_release_version.py failed with exit code $LASTEXITCODE" }
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

# Packages the shipped NSH app (nsh/) as the universal release does, with the
# same script: `nsh package --target macos --arch universal`, then the .app,
# .dmg, update archives and checksums under target/release-artifacts. Signed
# ad hoc unless SIGNING_IDENTITY names a Developer ID (and notarized when the
# NOTARY_* variables are set); see scripts/package_macos_release.sh.
_package-mac args="": build-frozen-html-identity
    #!/usr/bin/env bash
    set -euo pipefail
    for arg in {{args}}; do
        case "$arg" in
            --native-evidence|--agent)
                echo "⚠️  $arg was a gpui-shell feature; the NSH app ignores it" ;;
        esac
    done
    NSH="${NSH:-${NSH_TOOLS:-$HOME/BuwizNsh/tools}/bin/nsh}" VERSION="{{VERSION}}" OUT_DIR="{{RELEASE_DIR}}" \
        bash scripts/package_macos_release.sh

# ---------------------------------------------------------------------------
# NSH (TSX) app in nsh/. Read nsh/AGENTS.md first.
# The CLI is ~/BuwizNsh/tools/bin/nsh (NSH 0.5.1 built from the NSH checkout that
# sits beside this repo's checkout, ../nsh); never the obsolete one in ~/.nsh.
# Every recipe puts that CLI first on PATH and sets CARGO_TARGET_DIR. Extra
# arguments pass through: `just nsh-test --filter ping`.
# ---------------------------------------------------------------------------

# NSH: cargo check of nsh/'s crate, then tsc over the TSX against the generated types.
# MAS/TestFlight packages the NSH app (nsh/) with `mas_build`, the sandbox
# entitlements and the provisioning profile.
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

testflight *args="": (app args) testflight-upload

testflight-agent *args="": (testflight "--agent" args)

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
    BUILD="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleVersion' "{{RELEASE_DIR}}/mas/{{APP_NAME}}.app/Contents/Info.plist" 2>/dev/null || echo '?')"
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

_package-mac-appstore args="": build-frozen-html-identity
    #!/usr/bin/env bash
    set -euo pipefail
    root="$(pwd)"
    CERT="${CODESIGN_IDENTITY:--}"
    # Gmail Connect (a restricted Google scope) is out of the store build unless STORE_GMAIL=1: scripts/store_gmail.py.
    eval "$({{PYTHON}} scripts/store_gmail.py env)"
    # nsh package has no flag for a feature of the app crate: nsh/mas/cargo-with-mas.sh, set as $CARGO, adds `mas_build`
    # (and `agent` for --agent) to the build of the generated macOS host.
    export NSH_MAS_AGENT=0
    for arg in {{args}}; do
        case "$arg" in
            --agent) NSH_MAS_AGENT=1 ;;
        esac
    done
    ARCH="${NSH_MAS_ARCH:-universal}"
    OUT="$root/{{RELEASE_DIR}}/mas"
    APP="$OUT/{{APP_NAME}}.app"
    PKG_PATH="$root/{{RELEASE_DIR}}/{{APP_NAME}}-macOS-MAS-{{VERSION}}.pkg"
    PROFILE="assets/macos/embedded.provisionprofile"
    if [ "$CERT" != "-" ]; then
        # A store build needs a current profile; check it before the long build.
        if [ ! -f "$PROFILE" ]; then
            echo "❌ No $PROFILE: a store build needs the Mac App Store profile for dev.goldcoders.bir." >&2
            exit 1
        fi
        EXPIRES="$(security cms -D -i "$PROFILE" | plutil -extract ExpirationDate raw -)"
        if [[ "$EXPIRES" < "$(date -u +%Y-%m-%dT%H:%M:%SZ)" ]]; then
            echo "❌ $PROFILE expired $EXPIRES. Download a current Mac App Store profile for dev.goldcoders.bir." >&2
            exit 1
        fi
    fi
    # A store package is built in this checkout's own folder, never the shared ~/BuwizNsh/target-nsh: Cargo cannot tell two
    # checkouts of the same crate version apart, so a shared folder can link another worktree's bir-core (its forms) with no
    # error, and the package would carry another branch's form data. A CARGO_TARGET_DIR set by the caller still wins.
    export PATH="${NSH_TOOLS:-$HOME/BuwizNsh/tools}/bin:$PATH" CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$root/target/nsh-store}"
    # A store package comes only from the pinned NSH: the nsh tool built at NSH_PINNED_REF (NSH_TOOLS names its folder,
    # docs/platforms/ios.md) and the ../nsh checkout beside this one at that commit. Anything else is refused.
    pin="$(sed -n 's/^ *NSH_PINNED_REF: *//p' "$root/.github/workflows/nsh.yml" | head -1)"
    tool="$(nsh --version 2>/dev/null || echo missing)"
    tool_ref="$(printf '%s' "$tool" | sed -n 's/.*(\([0-9a-f]\{7,\}\)).*/\1/p')"
    kit="$(git -C "$root/../nsh" rev-parse HEAD 2>/dev/null || true)"
    if [ -z "$pin" ] || [ -z "$tool_ref" ] || [ "${pin#"$tool_ref"}" = "$pin" ]; then
        echo "❌ The nsh tool ($tool) is not built at the NSH pin ${pin:0:8}: set NSH_TOOLS to a tools folder built at the pin (docs/platforms/ios.md)." >&2; exit 1
    fi
    if [ "$kit" != "$pin" ]; then
        echo "❌ ../nsh beside this checkout is at ${kit:-nothing}, not the NSH pin $pin: check it out there first." >&2; exit 1
    fi
    dirty=""
    git -C "$root" diff --quiet HEAD -- || dirty=" (with uncommitted changes)"
    # The pinned toolchain's own cargo, first on PATH (the nsh tool and nsh/mas/cargo-with-mas.sh call `cargo`): it honours
    # CARGO_TARGET_DIR. The owner's cargo fork (huacnlee/cargo) stays everyone's everyday cargo, but it ignores CARGO_TARGET_DIR and
    # shares one build folder between every checkout, so a store package is refused if it is the cargo found here.
    toolchain_cargo="$(rustup which cargo 2>/dev/null || true)"
    if [ -z "$toolchain_cargo" ]; then
        echo "❌ rustup finds no cargo for this checkout's toolchain (rust-toolchain.toml)." >&2; exit 1
    fi
    export PATH="$(dirname "$toolchain_cargo"):$PATH"
    cargo_v="$(cargo --version 2>/dev/null || echo "cargo missing")"
    case "$cargo_v" in
        *huacnlee*) echo "❌ $cargo_v is the cargo fork, which shares one build folder between checkouts: a store package needs the toolchain's own cargo." >&2; exit 1 ;;
    esac
    PROVENANCE="build: app $(git -C "$root" rev-parse --short=8 HEAD)$dirty, nsh ${kit:0:8} beside it, tool $tool, $cargo_v, folder $CARGO_TARGET_DIR"
    echo "$PROVENANCE" >&2
    echo "Building the NSH app for the Mac App Store ($ARCH, build {{BUILD_NUMBER}}, agent=$NSH_MAS_AGENT)..."
    rm -rf "$OUT"
    mkdir -p "$OUT"
    (cd nsh && CARGO="$root/nsh/mas/cargo-with-mas.sh" nsh package --target macos --arch "$ARCH" \
        --build "{{BUILD_NUMBER}}" --app-version "{{VERSION}}" -o "$OUT")
    # The store build does not update itself: drop the update archive, nsh's ad hoc entitlements and the updater
    # configuration nsh packs into the bundle's assets (without it nsh/updater has no update source).
    rm -f "$OUT"/*.tar.gz "$OUT"/.nsh-updater.json "$OUT"/*.entitlements "$APP/Contents/Resources/nsh/assets/nsh-updater.json"
    # The print renderer reads the per-form CSS fixes from Contents/Resources/print-overrides (bir-app-api print_sheet.rs).
    cp -R print-overrides "$APP/Contents/Resources/"
    # The same privacy manifest as the iOS app: the Mac build collects the same data (assets/ios/PrivacyInfo.xcprivacy).
    cp assets/ios/PrivacyInfo.xcprivacy "$APP/Contents/Resources/"
    PLIST="$APP/Contents/Info.plist"
    /usr/libexec/PlistBuddy -c "Add :LSApplicationCategoryType string public.app-category.finance" "$PLIST" || true
    # Export compliance exemption, so App Store Connect skips the encryption question.
    /usr/libexec/PlistBuddy -c "Add :ITSAppUsesNonExemptEncryption bool false" "$PLIST" || true
    # The screenshot text describes the agent. A build without it must not claim that permission.
    if [ "$NSH_MAS_AGENT" != "1" ]; then
        /usr/libexec/PlistBuddy -c "Delete :NSScreenCaptureUsageDescription" "$PLIST" 2>/dev/null || true
    fi
    if [ "${DEV_MODE:-}" = "true" ] || [ "${DEVELOPER_MODE:-}" = "true" ]; then
        echo "Injecting DEVELOPER_MODE into Info.plist..."
        /usr/libexec/PlistBuddy -c "Add :LSEnvironment dict" "$PLIST" || true
        /usr/libexec/PlistBuddy -c "Add :LSEnvironment:DEVELOPER_MODE string true" "$PLIST" || true
    fi
    # The store's entitlements: network.server only with Gmail Connect or the agent (scripts/store_gmail.py entitlements).
    ENTITLEMENTS="$OUT/store.entitlements"
    agent_flag=""
    [ "$NSH_MAS_AGENT" = "1" ] && agent_flag="--agent"
    {{PYTHON}} scripts/store_gmail.py entitlements "$root/entitlements.plist" "$ENTITLEMENTS" $agent_flag
    if [ "$CERT" = "-" ]; then
        # An ad hoc signature cannot carry the store's application and team identifiers (macOS refuses to launch an app
        # that claims them without a matching profile), so the local build keeps only the sandbox and its permissions.
        /usr/libexec/PlistBuddy -c "Delete :com.apple.application-identifier" "$ENTITLEMENTS"
        /usr/libexec/PlistBuddy -c "Delete :com.apple.developer.team-identifier" "$ENTITLEMENTS"
        echo "⚠️  No CODESIGN_IDENTITY: ad hoc signed with the sandbox entitlements, no provisioning profile (local testing only)."
    else
        echo "Embedding provisioning profile for TestFlight (expires $EXPIRES)..."
        cp "$PROFILE" "$APP/Contents/embedded.provisionprofile"
    fi
    {{PYTHON}} scripts/audit_no_legacy.py --package-root "$APP"
    {{PYTHON}} scripts/store_gmail.py check "$APP"
    xattr -cr "$APP"
    # App Store Connect refuses a package whose files only their owner can read, which is what a umask of 077 builds.
    chmod -R u+rwX,go+rX "$APP"
    echo "Codesigning application with identity: $CERT..."
    codesign --force --options runtime --timestamp=none --entitlements "$ENTITLEMENTS" --sign "$CERT" "$APP"
    codesign --verify --strict --verbose=1 "$APP"
    echo "✅ $APP created and codesigned"
    rm -f "$PKG_PATH"
    if [ -n "${INSTALLER_IDENTITY:-}" ]; then
        echo "Creating signed .pkg for App Store submission using identity: $INSTALLER_IDENTITY..."
        productbuild --sign "$INSTALLER_IDENTITY" --component "$APP" /Applications "$PKG_PATH"
        echo "✅ Signed PKG created: $PKG_PATH"
    else
        echo "Creating unsigned .pkg for App Store submission..."
        productbuild --component "$APP" /Applications "$PKG_PATH"
        echo "✅ Unsigned PKG created: $PKG_PATH"
        echo "⚠️  Note: You must also codesign the .pkg with an 'Apple Distribution' certificate before submitting."
    fi
    echo "$PROVENANCE, build {{BUILD_NUMBER}}" > "${PKG_PATH%.pkg}.provenance.txt"
    echo "   $PROVENANCE"

# Google Play bundle (docs/platforms/android.md): an Android App Bundle for phones and tablets with the Play application
# id com.buwiz.ebirforms (nsh.json keeps dev.goldcoders.bir for every other system and for development builds), the
# target API level and backup switch of nsh.json's "android", and versionCode = the build number (App Store Connect's
# next, as `just app` uses, or the number given: `just play-bundle 28`). Needs no secret: the bundle comes out UNSIGNED, and the owner signs it with the
# upload key (`just play-sign`). nsh package always signs, with the debug key here; `android_bundle.py prepare` takes that
# signature off with the updater's source (Google Play updates the app; the updater never starts on a phone).
[macos]
play-bundle build="":
    #!/usr/bin/env bash
    set -euo pipefail
    umask 022
    root="$(pwd)"
    BUILD="{{build}}"
    if [ -z "$BUILD" ]; then
        set -a && source .env && set +a
        BUILD="$(uv run scripts/bump_build.py --print-next)"
    fi
    case "$BUILD" in ''|*[!0-9]*) echo "Invalid build number: $BUILD" >&2; exit 1 ;; esac
    TARGET_SDK="$({{PYTHON}} -c 'import json; print(json.load(open("nsh/nsh.json"))["android"]["targetSdk"])')"
    # The build folder is this checkout's own (target/play-bundle) unless CARGO_TARGET_DIR is set: Cargo cannot tell two
    # checkouts of the same crate version apart in a shared folder, so a bundle could link another worktree's bir-core,
    # form definitions included, with no error. The nsh CLI is $NSH_TOOLS's (default the shared ~/BuwizNsh/tools): a Mac
    # whose shared tool must stay as it is builds with the pinned one beside it (scripts/setup-android-macos.sh). Either way
    # the CLI must be at the NSH pin, or the bundle is not what CI tested.
    TOOLS="${NSH_TOOLS:-$HOME/BuwizNsh/tools}"
    export PATH="$TOOLS/bin:$PATH" CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$root/target/play-bundle}"
    # Evidence and packages are built by the pinned toolchain's own cargo, never one that shares a folder between checkouts.
    PINNED_CARGO="$(scripts/pinned-cargo-path.sh)"; [ -z "$PINNED_CARGO" ] || export PATH="$PINNED_CARGO:$PATH"
    scripts/build-line.sh --strict
    PIN="$(sed -n 's/^ *NSH_PINNED_REF: *//p' .github/workflows/nsh.yml | head -1)"
    HAVE="$(nsh --version 2>/dev/null || true)"
    case "$HAVE" in
        *"(${PIN:0:7})"*) ;;
        *) echo "❌ $TOOLS/bin/nsh is '${HAVE:-missing}', not the NSH pin ${PIN:0:7}: run scripts/setup-android-macos.sh (NSH_TOOLS=<folder> for one beside the shared tool)." >&2; exit 1 ;;
    esac
    # `nsh package` takes the Android RELEASE runtime next to the CLI (or under $NSH_RUNTIME), whatever built it. The setup
    # script records the NSH commit beside it (nsh-ref); a runtime without that record, or from another commit, is refused.
    RUNTIME="${NSH_RUNTIME:-$TOOLS/runtime}/android/release"
    RUNTIME_REF="$(cat "$RUNTIME/nsh-ref" 2>/dev/null || true)"
    if [ ! -f "$RUNTIME/classes.dex" ] || [ "$RUNTIME_REF" != "$PIN" ]; then
        echo "❌ The Android release runtime in $RUNTIME is ${RUNTIME_REF:+from $RUNTIME_REF, }not recorded as built at the NSH pin $PIN: run scripts/setup-android-macos.sh." >&2
        exit 1
    fi
    echo "nsh: $HAVE; Android release runtime at $PIN, classes.dex sha256 $(shasum -a 256 "$RUNTIME/classes.dex" | cut -d' ' -f1)"
    # NSH's CLI sets CC and AR for Android but not RANLIB, which the vendored OpenSSL needs: the NDK has only llvm-ranlib
    # (setup-linux-nsh.sh and setup-android-windows.ps1 set the same).
    SDK="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}}"
    NDK="${ANDROID_NDK_HOME:-$(ls -d "$SDK"/ndk/* 2>/dev/null | sort -V | tail -1)}"
    RANLIB_NDK="$(ls "$NDK"/toolchains/llvm/prebuilt/*/bin/llvm-ranlib 2>/dev/null | head -1)"
    [ -x "$RANLIB_NDK" ] || { echo "❌ No llvm-ranlib in the NDK ($NDK): run scripts/setup-android-macos.sh." >&2; exit 1; }
    export RANLIB_aarch64_linux_android="${RANLIB_aarch64_linux_android:-$RANLIB_NDK}"
    export RANLIB_armv7_linux_androideabi="${RANLIB_armv7_linux_androideabi:-$RANLIB_NDK}"
    # A release keystore never reaches this recipe, and a development build's baked environment never reaches a store one.
    unset NSH_KEYSTORE_PASSWORD BUWIZ_ANDROID_DEV BUWIZ_ANDROID_DEV_ENV
    # nsh builds libnsh.so in its generated workspace, which has no [profile.release]: this repository's own profile never
    # applies there, and Cargo's defaults make a 116 MB library. These make it 84 MB (24 MB to download) on arm64.
    # Not opt-level z and not panic=abort until such a build has been used on a real phone (docs/platforms/android.md).
    export CARGO_PROFILE_RELEASE_LTO=fat CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 CARGO_PROFILE_RELEASE_OPT_LEVEL=s
    OUT="$root/{{RELEASE_DIR}}/android"
    AAB="$root/{{RELEASE_DIR}}/{{APP_NAME}}-Android-{{VERSION}}-$BUILD.aab"
    rm -rf "$OUT" "$AAB"
    mkdir -p "$OUT"
    (cd nsh && nsh package --target android --format aab --abi arm64-v8a,armeabi-v7a --id com.buwiz.ebirforms \
        --build "$BUILD" --app-version "{{VERSION}}" -o "$OUT")
    {{PYTHON}} scripts/android_bundle.py prepare "$OUT/{{APP_NAME}}.aab" "$AAB"
    {{PYTHON}} scripts/android_bundle.py check "$AAB" --package com.buwiz.ebirforms --version-code "$BUILD" --target-sdk "$TARGET_SDK"
    echo "✅ $AAB ({{VERSION}}, versionCode $BUILD, unsigned): the owner signs it with just play-sign"

# Once, by the owner: the Google Play upload key (docs/platforms/android.md, "The upload key"). keytool asks for the
# password on the keyboard; it goes into the password manager with the file, and nowhere else.
[macos]
play-upload-key:
    #!/usr/bin/env bash
    set -euo pipefail
    KEYSTORE="${PLAY_UPLOAD_KEYSTORE:-$HOME/Keys/ebirforms-android/upload.jks}"
    [ ! -e "$KEYSTORE" ] || { echo "❌ $KEYSTORE exists: the upload key is made once." >&2; exit 1; }
    JDK="$(just _play-jdk)"
    mkdir -p "$(dirname "$KEYSTORE")" && chmod 700 "$(dirname "$KEYSTORE")"
    "$JDK/keytool" -genkeypair -v -keystore "$KEYSTORE" -storetype PKCS12 -alias upload -keyalg RSA -keysize 2048 \
        -validity 10000 -dname "CN=eBIRForms upload, O=Buwiz, C=PH"
    chmod 600 "$KEYSTORE"
    echo "✅ $KEYSTORE: attach it to the password manager entry that holds its password."

# The owner signs the bundle `just play-bundle` made with the upload key; jarsigner asks for the password on the keyboard.
# Writes <bundle>-signed.aab beside it and shows it in Finder, ready to drag into the Play Console.
[macos]
play-sign bundle="":
    #!/usr/bin/env bash
    set -euo pipefail
    KEYSTORE="${PLAY_UPLOAD_KEYSTORE:-$HOME/Keys/ebirforms-android/upload.jks}"
    [ -f "$KEYSTORE" ] || { echo "❌ No upload key at $KEYSTORE (just play-upload-key, once)." >&2; exit 1; }
    AAB="{{bundle}}"
    if [ -z "$AAB" ]; then
        AAB="$(ls -t {{RELEASE_DIR}}/{{APP_NAME}}-Android-*.aab 2>/dev/null | grep -v -- '-signed\.aab$' | head -1 || true)"
    fi
    [ -f "$AAB" ] || { echo "❌ No bundle to sign: run just play-bundle first." >&2; exit 1; }
    if unzip -l "$AAB" | grep -q ' META-INF/'; then
        echo "❌ $AAB is already signed: sign the unsigned bundle play-bundle wrote." >&2; exit 1
    fi
    JDK="$(just _play-jdk)"
    SIGNED="${AAB%.aab}-signed.aab"
    cp "$AAB" "$SIGNED"
    "$JDK/jarsigner" -sigalg SHA256withRSA -digestalg SHA-256 -keystore "$KEYSTORE" "$SIGNED" upload
    "$JDK/jarsigner" -verify "$SIGNED" | grep -q "jar verified" || { echo "❌ $SIGNED does not verify." >&2; exit 1; }
    echo "✅ $SIGNED: drag it into the Play Console (Test and release > the track > Create new release)."
    open -R "$SIGNED"

# The JDK's bin folder: JAVA_HOME's, Homebrew's or Android Studio's. Never /usr/bin's, which is macOS's stub.
[macos]
_play-jdk:
    #!/usr/bin/env bash
    for bin in "${JAVA_HOME:+$JAVA_HOME/bin}" /opt/homebrew/opt/openjdk/bin "/Applications/Android Studio.app/Contents/jbr/Contents/Home/bin"; do
        if [ -n "$bin" ] && [ -x "$bin/jarsigner" ] && "$bin/jarsigner" -help >/dev/null 2>&1; then echo "$bin"; exit 0; fi
    done
    echo "❌ No JDK: brew install openjdk" >&2
    exit 1

# App Store iOS package (the devices are nsh.json's ios.infoPlist.UIDeviceFamily): nsh signs it with the App Store profile,
# then the store's additions go in and the bundle is signed again with the same entitlements. CODESIGN_IDENTITY is the
# "Apple Distribution: …" identity; the profile is the iOS App Store profile for dev.goldcoders.bir (docs/platforms/ios.md).
# Upload the .ipa with `just ios-appstore-upload`.
[macos]
ios-appstore: build-frozen-html-identity
    #!/usr/bin/env bash
    set -euo pipefail
    # App Store Connect refuses a package whose files only their owner can read (a umask of 077 builds that).
    umask 022
    root="$(pwd)"
    CERT="${CODESIGN_IDENTITY:?set CODESIGN_IDENTITY to the Apple Distribution identity}"
    PROFILE="assets/ios/embedded.mobileprovision"
    if [ ! -f "$PROFILE" ]; then
        echo "❌ No $PROFILE: download the iOS App Store profile for dev.goldcoders.bir." >&2; exit 1
    fi
    EXPIRES="$(security cms -D -i "$PROFILE" | plutil -extract ExpirationDate raw -)"
    if [[ "$EXPIRES" < "$(date -u +%Y-%m-%dT%H:%M:%SZ)" ]]; then
        echo "❌ $PROFILE expired $EXPIRES." >&2; exit 1
    fi
    set -a && source .env && set +a
    # Gmail Connect (a restricted Google scope) is out of the store build unless STORE_GMAIL=1: scripts/store_gmail.py.
    eval "$({{PYTHON}} scripts/store_gmail.py env)"
    BUILD="$(uv run scripts/bump_build.py --print-next)"
    case "$BUILD" in ''|*[!0-9]*) echo "Invalid App Store build number: $BUILD" >&2; exit 1 ;; esac
    # A store package is built in this checkout's own folder, never the shared ~/BuwizNsh/target-nsh: Cargo cannot tell two
    # checkouts of the same crate version apart, so a shared folder can link another worktree's bir-core (its forms) with no
    # error, and the package would carry another branch's form data. A CARGO_TARGET_DIR set by the caller still wins.
    export PATH="${NSH_TOOLS:-$HOME/BuwizNsh/tools}/bin:$PATH" CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$root/target/nsh-store}"
    # A store package comes only from the pinned NSH: the nsh tool built at NSH_PINNED_REF (NSH_TOOLS names its folder,
    # docs/platforms/ios.md) and the ../nsh checkout beside this one at that commit. Anything else is refused.
    pin="$(sed -n 's/^ *NSH_PINNED_REF: *//p' "$root/.github/workflows/nsh.yml" | head -1)"
    tool="$(nsh --version 2>/dev/null || echo missing)"
    tool_ref="$(printf '%s' "$tool" | sed -n 's/.*(\([0-9a-f]\{7,\}\)).*/\1/p')"
    kit="$(git -C "$root/../nsh" rev-parse HEAD 2>/dev/null || true)"
    if [ -z "$pin" ] || [ -z "$tool_ref" ] || [ "${pin#"$tool_ref"}" = "$pin" ]; then
        echo "❌ The nsh tool ($tool) is not built at the NSH pin ${pin:0:8}: set NSH_TOOLS to a tools folder built at the pin (docs/platforms/ios.md)." >&2; exit 1
    fi
    if [ "$kit" != "$pin" ]; then
        echo "❌ ../nsh beside this checkout is at ${kit:-nothing}, not the NSH pin $pin: check it out there first." >&2; exit 1
    fi
    dirty=""
    git -C "$root" diff --quiet HEAD -- || dirty=" (with uncommitted changes)"
    # The pinned toolchain's own cargo, first on PATH (the nsh tool and nsh/mas/cargo-with-mas.sh call `cargo`): it honours
    # CARGO_TARGET_DIR. The owner's cargo fork (huacnlee/cargo) stays everyone's everyday cargo, but it ignores CARGO_TARGET_DIR and
    # shares one build folder between every checkout, so a store package is refused if it is the cargo found here.
    toolchain_cargo="$(rustup which cargo 2>/dev/null || true)"
    if [ -z "$toolchain_cargo" ]; then
        echo "❌ rustup finds no cargo for this checkout's toolchain (rust-toolchain.toml)." >&2; exit 1
    fi
    export PATH="$(dirname "$toolchain_cargo"):$PATH"
    cargo_v="$(cargo --version 2>/dev/null || echo "cargo missing")"
    case "$cargo_v" in
        *huacnlee*) echo "❌ $cargo_v is the cargo fork, which shares one build folder between checkouts: a store package needs the toolchain's own cargo." >&2; exit 1 ;;
    esac
    PROVENANCE="build: app $(git -C "$root" rev-parse --short=8 HEAD)$dirty, nsh ${kit:0:8} beside it, tool $tool, $cargo_v, folder $CARGO_TARGET_DIR"
    echo "$PROVENANCE" >&2
    OUT="$root/{{RELEASE_DIR}}/ios"
    APP="$OUT/{{APP_NAME}}.app"
    IPA="$root/{{RELEASE_DIR}}/{{APP_NAME}}-iOS-{{VERSION}}.ipa"
    rm -rf "$OUT" "$IPA"
    mkdir -p "$OUT"
    (cd nsh && nsh package --target ios --build "$BUILD" --app-version "{{VERSION}}" \
        --sign "$CERT" --profile "$root/$PROFILE" -o "$OUT")
    # The store build does not update itself (the updater never starts on a phone; its source goes too), it reads the
    # per-form print CSS beside its executable (bir-app-api packaged.rs), and it carries Apple's privacy manifest.
    rm -f "$OUT"/.nsh-updater.json "$OUT"/*.ipa "$APP/nsh/assets/nsh-updater.json"
    cp -R print-overrides "$APP/"
    cp assets/ios/PrivacyInfo.xcprivacy "$APP/"
    # The screen-capture text describes the desktop agent; an iOS store build must not claim that permission.
    if plutil -extract NSScreenCaptureUsageDescription raw "$APP/Info.plist" >/dev/null 2>&1; then
        echo "❌ $APP/Info.plist has NSScreenCaptureUsageDescription." >&2; exit 1
    fi
    {{PYTHON}} scripts/audit_no_legacy.py --package-root "$APP"
    {{PYTHON}} scripts/store_gmail.py check "$APP"
    xattr -cr "$APP"
    chmod -R u+rwX,go+rX "$APP"
    codesign --force --sign "$CERT" --preserve-metadata=entitlements,requirements,flags "$APP"
    codesign --verify --strict --verbose=1 "$APP"
    STAGE="$(mktemp -d)"
    mkdir "$STAGE/Payload"
    ditto "$APP" "$STAGE/Payload/{{APP_NAME}}.app"
    (cd "$STAGE" && zip -qry "$IPA" Payload)
    rm -rf "$STAGE"
    echo "$PROVENANCE, build $BUILD" > "${IPA%.ipa}.provenance.txt"
    echo "✅ $IPA ({{VERSION}}, build $BUILD)"
    echo "   $PROVENANCE"

# Uploads the iOS package `just ios-appstore` made, with the App Store Connect key in .env (as testflight-upload does).
[macos]
ios-appstore-upload:
    #!/usr/bin/env bash
    set -euo pipefail
    set -a && source .env && set +a
    IPA="{{RELEASE_DIR}}/{{APP_NAME}}-iOS-{{VERSION}}.ipa"
    [ -f "$IPA" ] || { echo "❌ No $IPA: run just ios-appstore first." >&2; exit 1; }
    "${NSH_TOOLS:-$HOME/BuwizNsh/tools}/bin/nsh" publish ios "$IPA" --app-id 6767850225 \
        --issuer-id "$APP_STORE_ISSUER_ID" --key-id "$APP_STORE_KEY_ID" --private-key "$APP_STORE_P8_PATH"

[unix]
nsh-check *args="":
    #!/usr/bin/env bash
    set -euo pipefail
    export PATH="${NSH_TOOLS:-$HOME/BuwizNsh/tools}/bin:$PATH" CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/BuwizNsh/target-nsh}"
    cd nsh
    exec nsh check {{args}}

# Windows: the same command in PowerShell.
[windows]
nsh-check *args="":
    #!pwsh -NoProfile
    $ErrorActionPreference = 'Stop'
    $tools = if ($env:NSH_TOOLS) { $env:NSH_TOOLS } else { "$HOME\BuwizNsh\tools" }
    $env:PATH = "$tools\bin;$env:PATH"
    if (-not $env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR = "$HOME\BuwizNsh\target-nsh" }
    Set-Location nsh
    nsh check {{args}}
    exit $LASTEXITCODE

# NSH: bundle and compile the app; every warning it prints is a finding.
[unix]
nsh-build *args="":
    #!/usr/bin/env bash
    set -euo pipefail
    export PATH="${NSH_TOOLS:-$HOME/BuwizNsh/tools}/bin:$PATH" CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/BuwizNsh/target-nsh}"
    cd nsh
    exec nsh build {{args}}

# Windows: the same command in PowerShell.
[windows]
nsh-build *args="":
    #!pwsh -NoProfile
    $ErrorActionPreference = 'Stop'
    $tools = if ($env:NSH_TOOLS) { $env:NSH_TOOLS } else { "$HOME\BuwizNsh\tools" }
    $env:PATH = "$tools\bin;$env:PATH"
    if (-not $env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR = "$HOME\BuwizNsh\target-nsh" }
    Set-Location nsh
    nsh build {{args}}
    exit $LASTEXITCODE

# nsh-test also sets the dev environment, so a test that reaches the door can never read the keychain or send. It never sets BIR_DATABASE_PATH.

# NSH: Rust tests, then every *.test.tsx and *.test.ts, headless (390x844 phone viewport unless a test opens another).
# With no path: nsh/verify/ (git-ignored real-window verifier scripts that need the owner's data copy, so they always fail headless)
# is left out. `nsh test` has no exclude option and a folder path must hold the app's App.tsx, so the Rust tests run once with
# cargo and every other test file gets its own `nsh test <file>` run; the recipe fails if any of them did. A path in the
# arguments (`just nsh-test shell/frame.test.tsx`, `just nsh-test verify`) runs exactly that, as before.
[unix]
nsh-test *args="":
    #!/usr/bin/env bash
    set -euo pipefail
    export PATH="${NSH_TOOLS:-$HOME/BuwizNsh/tools}/bin:$PATH" CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/BuwizNsh/target-nsh}"
    # Evidence and packages are built by the pinned toolchain's own cargo, never one that shares a folder between checkouts.
    PINNED_CARGO="$(scripts/pinned-cargo-path.sh)"; [ -z "$PINNED_CARGO" ] || export PATH="$PINNED_CARGO:$PATH"
    scripts/build-line.sh --strict
    export BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1
    unset BIR_SFTP_LIVE BIR_DATABASE_PATH
    cd nsh
    args=({{args}})
    has_path=0 skip=0 filter=()
    for a in ${args[@]+"${args[@]}"}; do
        if [ "$skip" = 1 ]; then skip=0; continue; fi
        case "$a" in
            --filter) skip=1 ;;
            --filter=*) filter=("${a#--filter=}") ;;
            --junit|--target|--browser|--web-host|--display) skip=1 ;;
            -*) ;;
            *) has_path=1 ;;
        esac
    done
    for ((i = 0; i < ${#args[@]}; i++)); do
        if [ "${args[$i]}" = "--filter" ] && [ $((i + 1)) -lt ${#args[@]} ]; then filter=("${args[$((i + 1))]}"); fi
    done
    if [ "$has_path" = 1 ]; then exec nsh test ${args[@]+"${args[@]}"}; fi
    failed=()
    cargo test --manifest-path Cargo.toml --no-fail-fast ${filter[@]+"${filter[@]}"} || failed+=("rust crate")
    while IFS= read -r file; do
        nsh test "$file" ${args[@]+"${args[@]}"} || failed+=("$file")
    done < <(find . -mindepth 1 \( -path ./verify -o -name node_modules -o -name target -o -name '.*' \) -prune -o \( -name '*.test.ts' -o -name '*.test.tsx' \) -print | sort)
    if [ ${#failed[@]} -gt 0 ]; then
        echo "nsh-test: failed in:"
        printf '  %s\n' "${failed[@]}"
        exit 1
    fi
    echo "nsh-test: every test file passed (nsh/verify left out)"

# Windows: as the Unix recipe and CI's Windows job (.github/workflows/nsh.yml): with a path in the arguments, exactly `nsh test <arguments>`;
# with none, the Rust tests once (those a --filter names), then every tracked *.test.ts and *.test.tsx outside nsh/verify, one headless run per file.
[windows]
nsh-test *args="":
    #!pwsh -NoProfile
    $ErrorActionPreference = 'Stop'
    $tools = if ($env:NSH_TOOLS) { $env:NSH_TOOLS } else { "$HOME\BuwizNsh\tools" }
    $env:PATH = "$tools\bin;$env:PATH"
    if (-not $env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR = "$HOME\BuwizNsh\target-nsh" }
    # The provenance line is information, never a reason to stop: without git on the PATH (a source copy, or a
    # caller's own narrow PATH) it says so instead of ending the recipe under ErrorActionPreference Stop (#412).
    $git = Get-Command git -ErrorAction SilentlyContinue
    $app = if ($git) { (git rev-parse --short=8 HEAD) 2>$null } else { 'unknown (git not found)' }
    $kit = if ($git) { (git -C ..\nsh rev-parse --short=8 HEAD) 2>$null } else { 'unknown (git not found)' }
    $tool = (nsh --version) 2>$null; $cargoVersion = (cargo --version) 2>$null
    [Console]::Error.WriteLine("build: app $app, nsh $kit beside it, tool $tool, $cargoVersion, folder $env:CARGO_TARGET_DIR")
    $env:BUWIZ_NO_KEYCHAIN = '1'; $env:EBIR_TEST_ENV = '1'; $env:BIR_AUTH_BYPASS = '1'; $env:BIR_SFTP_DRY_RUN = '1'
    Remove-Item Env:BIR_SFTP_LIVE -ErrorAction SilentlyContinue
    Remove-Item Env:BIR_DATABASE_PATH -ErrorAction SilentlyContinue
    Set-Location nsh
    $given = @'
    {{args}}
    '@
    $hasPath = $false; $skip = $false; $filter = @()
    $words = @($given -split '\s+' | Where-Object { $_ })
    for ($i = 0; $i -lt $words.Count; $i++) {
        $word = $words[$i]
        if ($skip) { $skip = $false; continue }
        if ($word -eq '--filter') { $skip = $true; if ($i + 1 -lt $words.Count) { $filter = @($words[$i + 1].Trim('"', "'")) } }
        elseif ($word -like '--filter=*') { $filter = @($word.Substring(9).Trim('"', "'")) }
        elseif ($word -in '--junit', '--target', '--browser', '--web-host', '--display') { $skip = $true }
        elseif ($word -notlike '-*') { $hasPath = $true }
    }
    if ($hasPath) { nsh test {{args}}; exit $LASTEXITCODE }
    $failed = @()
    cargo test --manifest-path Cargo.toml --no-fail-fast @filter
    if ($LASTEXITCODE -ne 0) { $failed += 'rust crate' }
    [string[]]$files = git ls-files -- '*.test.ts' '*.test.tsx' | Where-Object { $_ -notlike 'verify/*' }
    [Array]::Sort($files, [StringComparer]::Ordinal)
    foreach ($file in $files) {
        nsh test "./$file" {{args}}
        if ($LASTEXITCODE -ne 0) { $failed += $file }
    }
    if ($failed.Count -gt 0) {
        "nsh-test: failed in:"
        $failed | ForEach-Object { "  $_" }
        exit 1
    }
    "nsh-test: every test file passed (nsh/verify left out)"

# Refuses a database path inside the owner's live data, the migrated copy or the migration folder (Hard rules 2 and 3).
# The same checks as the door's guard (nsh/src/guards.rs, which a test compares with this script): the home folder
# is the account's own from the password database, never $HOME; the path is resolved through symlinks and ".."; and
# it is refused by name, by device+inode (macOS reaches one folder as /Users/u/x and as /System/Volumes/Data/Users/u/x,
# and a volume may ignore case: an ancestor that IS a protected folder is refused), and as a hard link to a file
# directly inside a protected folder.
[private]
[unix]
_nsh-db-guard db:
    #!/usr/bin/env bash
    set -euo pipefail
    [ -n {{quote(db)}} ] || exit 0
    python3 -I - {{quote(db)}} <<'PY'
    import os
    import pwd
    import sys

    home = pwd.getpwuid(os.getuid()).pw_dir
    protected = [
        home + "/Library/Group Containers/group.dev.goldcoders.bir",
        home + "/BuwizNsh/data",
        home + "/BuwizNsh/migration",
    ]
    candidate = os.path.realpath(sys.argv[1])


    def identity(path):
        try:
            info = os.stat(path)
        except OSError:
            return None
        return (info.st_dev, info.st_ino)


    def lower(path):
        return path.lower() if sys.platform in ("darwin", "win32") else path


    def refuse(why):
        print("refusing %s: %s, the owner's data. cp a copy to ~/BuwizNsh/agents/<track>.db and use that" % (candidate, why), file=sys.stderr)
        sys.exit(1)


    folders = [os.path.realpath(path) for path in protected]
    for folder in folders:
        if lower(candidate) == lower(folder) or lower(candidate).startswith(lower(folder).rstrip(os.sep) + os.sep):
            refuse("it is inside " + folder)

    known = {}
    for folder in folders:
        found = identity(folder) if os.path.isdir(folder) else None
        if found is not None:
            known[found] = folder

    path = candidate
    while True:
        found = identity(path)
        if found is not None and found in known:
            refuse("it is inside %s (the same folder by device and inode, under another name)" % known[found])
        parent = os.path.dirname(path)
        if parent == path:
            break
        path = parent

    if os.path.isfile(candidate):
        info = os.stat(candidate)
        if info.st_nlink > 1:
            for folder in known.values():
                for name in os.listdir(folder):
                    if identity(os.path.join(folder, name)) == (info.st_dev, info.st_ino):
                        refuse("it is a hard link to a file in " + folder)
    PY

# Windows: the same refusals as the Unix recipe and the door's guard (nsh/src/guards.rs): the profile folder is the account's own from
# its token (GetUserProfileDirectoryW), never USERPROFILE; names compare without case; a folder or file is known by volume and file index.
# In these dev recipes the app's own data folder (AppData\Local\<app id>, and under LOCALAPPDATA) is protected too, as the door does
# in a test environment.
[private]
[windows]
_nsh-db-guard $db:
    #!pwsh -NoProfile
    $ErrorActionPreference = 'Stop'
    $db = $env:db
    if (-not $db) { exit 0 }
    $script = @'
    import ctypes
    import os
    import sys
    from ctypes import wintypes

    advapi32, userenv, kernel32 = ctypes.WinDLL("advapi32"), ctypes.WinDLL("userenv"), ctypes.WinDLL("kernel32")
    kernel32.GetCurrentProcess.restype = wintypes.HANDLE
    advapi32.OpenProcessToken.argtypes = [wintypes.HANDLE, wintypes.DWORD, ctypes.POINTER(wintypes.HANDLE)]
    userenv.GetUserProfileDirectoryW.argtypes = [wintypes.HANDLE, wintypes.LPWSTR, ctypes.POINTER(wintypes.DWORD)]
    token = wintypes.HANDLE()
    if not advapi32.OpenProcessToken(kernel32.GetCurrentProcess(), 0x0008, ctypes.byref(token)):
        sys.exit("refusing: the account's profile folder cannot be found")
    size = wintypes.DWORD(32768)
    buffer = ctypes.create_unicode_buffer(size.value)
    if not userenv.GetUserProfileDirectoryW(token, buffer, ctypes.byref(size)):
        sys.exit("refusing: the account's profile folder cannot be found")
    home = buffer.value
    app_id = sys.argv[2]
    protected = [
        os.path.join(home, "Library", "Group Containers", "group.dev.goldcoders.bir"),
        os.path.join(home, "BuwizNsh", "data"),
        os.path.join(home, "BuwizNsh", "migration"),
        os.path.join(home, "AppData", "Local", app_id),
    ]
    moved = os.environ.get("LOCALAPPDATA", "")
    if os.path.isabs(moved):
        protected.append(os.path.join(moved, app_id))
    candidate = os.path.realpath(sys.argv[1])


    def identity(path):
        try:
            info = os.stat(path)
        except OSError:
            return None
        return (info.st_dev, info.st_ino)


    def refuse(why):
        print("refusing %s: %s, the owner's data. Copy it to %s\\BuwizNsh\\agents\\<track>.db and use that" % (candidate, why, home), file=sys.stderr)
        sys.exit(1)


    folders = [os.path.realpath(path) for path in protected]
    for folder in folders:
        if candidate.lower() == folder.lower() or candidate.lower().startswith(folder.lower().rstrip(os.sep) + os.sep):
            refuse("it is inside " + folder)

    known = {}
    for folder in folders:
        found = identity(folder) if os.path.isdir(folder) else None
        if found is not None:
            known[found] = folder

    path = candidate
    while True:
        found = identity(path)
        if found is not None and found in known:
            refuse("it is inside %s (the same folder by volume and file index, under another name)" % known[found])
        parent = os.path.dirname(path)
        if parent == path:
            break
        path = parent

    if os.path.isfile(candidate):
        info = os.stat(candidate)
        if info.st_nlink > 1:
            for folder in known.values():
                for name in os.listdir(folder):
                    if identity(os.path.join(folder, name)) == (info.st_dev, info.st_ino):
                        refuse("it is a hard link to a file in " + folder)
    '@
    $script | & "{{PYTHON}}" -I - $db "dev.goldcoders.bir"
    exit $LASTEXITCODE

# The database comes from the caller: just nsh-run ~/BuwizNsh/agents/<track>.db, or BIR_DATABASE_PATH in the environment.
# A path inside the owner's data, ~/BuwizNsh/data or ~/BuwizNsh/migration is refused.

# NSH: open the app in a macOS window (hot TSX edits) in dry-run dev mode: no keychain, no sign-in gate, nothing sent.
[macos]
nsh-run db=env_var_or_default("BIR_DATABASE_PATH", "") *args="": (_nsh-db-guard db)
    #!/usr/bin/env bash
    set -euo pipefail
    export PATH="${NSH_TOOLS:-$HOME/BuwizNsh/tools}/bin:$PATH" CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/BuwizNsh/target-nsh}"
    export BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1
    unset BIR_SFTP_LIVE
    if [ -n "{{db}}" ]; then export BIR_DATABASE_PATH="{{db}}"; else unset BIR_DATABASE_PATH; fi
    cd nsh
    exec nsh run --target macos {{args}}

# NSH: the app in a real window with the agent control planes on, for gpui-agent recipes: the window's own (hexuria/nsh-gpui-agent:
# tree, press, type, screenshot) on GPUI_AGENT_ADDR, and the app's data plane (app.info, profile.*, form.*) on the port it reports.
# target is macos, linux, windows, ios (Simulator) or android (emulator; then `adb forward tcp:<port> tcp:<port>`).
# GPUI_AGENT_TOKEN must be set by the caller (never printed, never compiled into the app). Dry-run dev mode as nsh-run: nothing is sent anywhere.
[unix]
nsh-run-agent target db=env_var_or_default("BIR_DATABASE_PATH", "") *args="": (_nsh-db-guard db)
    #!/usr/bin/env bash
    set -euo pipefail
    [ -n "${GPUI_AGENT_TOKEN:-}" ] || { echo "nsh-run-agent: set GPUI_AGENT_TOKEN first"; exit 1; }
    root="$(pwd -P)"
    export PATH="${NSH_TOOLS:-$HOME/BuwizNsh/tools}/bin:$PATH" CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/BuwizNsh/target-nsh}"
    # Evidence and packages are built by the pinned toolchain's own cargo, never one that shares a folder between checkouts.
    PINNED_CARGO="$(scripts/pinned-cargo-path.sh)"; [ -z "$PINNED_CARGO" ] || export PATH="$PINNED_CARGO:$PATH"
    scripts/build-line.sh --strict
    export BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1
    export GPUI_AGENT=1 GPUI_AGENT_ADDR="${GPUI_AGENT_ADDR:-127.0.0.1:17421}"
    # .env (dotenv-load) may hold the live SFTP account and relay: none of it reaches an agent run, dry run or not.
    unset BIR_SFTP_LIVE BIR_SFTP_HOST BIR_SFTP_PORT BIR_SFTP_USERNAME BIR_SFTP_PASSWORD BIR_SFTP_ACCEPT_ANY_HOST_KEY \
        BIR_RELAY_URL BUWIZ_RELAY_SEND
    # An account server that is not on this machine neither: a debug build then talks to the local mock.
    for name in BUWIZ_SERVER_URL BUWIZ_ACCOUNT_URL; do
        if [ -n "${!name:-}" ] && [[ ! "${!name}" =~ ^https?://(localhost|127\.0\.0\.1|\[::1\])(:[0-9]+)?(/|$) ]]; then unset "$name"; fi
    done
    # The guard read a relative path from here, and the app starts in nsh/: hand it the same file, as an absolute path.
    db={{quote(db)}}
    if [ -n "$db" ]; then
        case "$db" in /*) ;; *) db="$root/$db" ;; esac
        export BIR_DATABASE_PATH="$db"
    else
        unset BIR_DATABASE_PATH
    fi
    port="${GPUI_AGENT_ADDR##*:}"
    run_args=()
    if [ "{{target}}" = android ] && [ "$(uname -s)" = Linux ]; then
        # The runtimes `nsh run` (dev) and `nsh package` (release) take, checked against the pin: a runtime that is a
        # link into a checkout's build folder, or was built at another commit, made every test on it a test of another NSH.
        pin="$(sed -n 's/^ *NSH_PINNED_REF: *//p' "$root/.github/workflows/nsh.yml" | head -1)"
        for variant in dev release; do
            dir="${NSH_RUNTIME:-$HOME/BuwizNsh/tools/runtime}/android/$variant"
            if [ -L "$dir" ] || [ "$(cat "$dir/nsh-ref" 2>/dev/null || true)" != "$pin" ]; then
                echo "nsh-run-agent: $dir is not a copy recorded as built at the NSH pin ${pin:0:7}: run scripts/setup-linux-nsh.sh" >&2
                exit 1
            fi
            echo "nsh android $variant runtime: ${pin:0:7}, classes.dex sha256 $(sha256sum "$dir/classes.dex" | cut -d' ' -f1)"
        done
    fi
    if [ "{{target}}" = android ]; then
        # An Android app process has no launch environment: the dry-run settings and the data plane's switch and address are
        # baked into this development build (nsh/src/dev_env.rs). The token is not (a baked one is readable in the built library):
        # it is written into the host's private files with `run-as`, over standard input so it is in no process's arguments, as
        # nsh does for the window plane. On a clean device the host is installed by `nsh run` below, so a background job waits
        # for the package first; the app waits up to 10 s for the file, then reads and deletes it (src/agent.rs, take_token).
        # The data plane listens on the port after the window's (the baked address is the window's; src/agent.rs choose()
        # adds one): forward both. The window plane reads its switch and address from the launch intent's extras, so a
        # second background job relaunches the host with them once the first launch is up (android-handoff.sh plug).
        export BUWIZ_ANDROID_DEV=1
        export BUWIZ_ANDROID_DEV_ENV="EBIR_TEST_ENV=1;BIR_AUTH_BYPASS=1;BUWIZ_NO_KEYCHAIN=1;BIR_SFTP_DRY_RUN=1;GPUI_AGENT=1;GPUI_AGENT_ADDR=127.0.0.1:$port"
        # One adb and one device for the whole launch: the SDK's adb and the device nsh starts the app on, checked before
        # anything is built (nsh/agent/android-handoff.sh). The writer ends with this launcher: a failed or cancelled launch
        # writes no token afterwards.
        { read -r adb; read -r serial; } < <("$root/nsh/agent/android-handoff.sh" device) || true
        [ -n "${adb:-}" ] && [ -n "${serial:-}" ] || exit 1
        "$root/nsh/agent/android-handoff.sh" token "$adb" "$serial" "$$" &
        # The relaunch points the activity back at nsh's dev server, so its port must be the one the run
        # actually uses: --port in {{args}} wins (it is what `nsh run` binds), else NSH_DEV_PORT, else 8347.
        devport="$(printf %s " {{args}} " | sed -n 's/.* --port[ =]\([0-9]*\).*/\1/p')"
        devport="${devport:-${NSH_DEV_PORT:-8347}}"
        case " {{args}} " in
            *" --port "*|*" --port="*) ;;
            *) run_args+=(--port "$devport") ;;
        esac
        "$root/nsh/agent/android-handoff.sh" plug "$adb" "$serial" "$$" "$port" "$devport" &
        echo "nsh-run-agent: device $serial, window plane on 127.0.0.1:$port, data plane on 127.0.0.1:$((port + 1)); run: \"$adb\" -s $serial forward tcp:$port tcp:$port && \"$adb\" -s $serial forward tcp:$((port + 1)) tcp:$((port + 1))"
    fi
    if [ "{{target}}" = ios ]; then
        # simctl gives the app only SIMCTL_CHILD_* variables, and nsh keeps GPUI_AGENT_TOKEN out of them (its window plane reads a
        # private file): the dry-run switches and the database go by name, the data plane's token as BIR_AGENT_TOKEN (src/agent.rs),
        # the window plane's switch and address as GPUI_AGENT / GPUI_AGENT_ADDR and its token into the host's data container
        # (nsh/agent/ios-handoff.sh, in the background until this launcher dies). The data plane listens on the port after
        # the window's (src/agent.rs choose() adds one).
        unset SIMCTL_CHILD_BIR_SFTP_LIVE SIMCTL_CHILD_BIR_DATABASE_PATH
        export SIMCTL_CHILD_BUWIZ_NO_KEYCHAIN=1 SIMCTL_CHILD_EBIR_TEST_ENV=1 SIMCTL_CHILD_BIR_AUTH_BYPASS=1 SIMCTL_CHILD_BIR_SFTP_DRY_RUN=1
        export SIMCTL_CHILD_BIR_AGENT_TOKEN="$GPUI_AGENT_TOKEN"
        export SIMCTL_CHILD_GPUI_AGENT=1 SIMCTL_CHILD_GPUI_AGENT_ADDR="127.0.0.1:$port"
        if [ -n "$db" ]; then export SIMCTL_CHILD_BIR_DATABASE_PATH="$db"; fi
        simdev="booted"
        case " {{args}} " in
            *" --device "*) simdev="$(printf %s " {{args}} " | sed -n 's/.* --device \([^ ]*\).*/\1/p')" ;;
            *" --device="*) simdev="$(printf %s " {{args}} " | sed -n 's/.* --device=\([^ ]*\).*/\1/p')" ;;
        esac
        "$root/nsh/agent/ios-handoff.sh" token "$$" "$simdev" &
        echo "nsh-run-agent: window plane on 127.0.0.1:$port, data plane on 127.0.0.1:$((port + 1))"
    fi
    cd nsh
    CARGO="$root/nsh/agent/cargo-with-agent.sh" exec nsh run --target {{target}} "${run_args[@]}" {{args}}

# Windows: the same in PowerShell (target windows, or android with the emulator and adb on this machine). $CARGO is
# nsh/agent/cargo-with-agent.cmd, the counterpart of cargo-with-agent.sh.
[windows]
nsh-run-agent target $db=env_var_or_default("BIR_DATABASE_PATH", "") *args="": (_nsh-db-guard db)
    #!pwsh -NoProfile
    $ErrorActionPreference = 'Stop'
    $tools = if ($env:NSH_TOOLS) { $env:NSH_TOOLS } else { "$HOME\BuwizNsh\tools" }
    $env:PATH = "$tools\bin;$env:PATH"
    if (-not $env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR = "$HOME\BuwizNsh\target-nsh" }
    # The provenance line is information, never a reason to stop: without git on the PATH (a source copy, or a
    # caller's own narrow PATH) it says so instead of ending the recipe under ErrorActionPreference Stop (#412).
    $git = Get-Command git -ErrorAction SilentlyContinue
    $app = if ($git) { (git rev-parse --short=8 HEAD) 2>$null } else { 'unknown (git not found)' }
    $kit = if ($git) { (git -C ..\nsh rev-parse --short=8 HEAD) 2>$null } else { 'unknown (git not found)' }
    $tool = (nsh --version) 2>$null; $cargoVersion = (cargo --version) 2>$null
    [Console]::Error.WriteLine("build: app $app, nsh $kit beside it, tool $tool, $cargoVersion, folder $env:CARGO_TARGET_DIR")
    if (-not $env:GPUI_AGENT_TOKEN) { "nsh-run-agent: set GPUI_AGENT_TOKEN first"; exit 1 }
    $root = (Get-Location).ProviderPath
    $env:BUWIZ_NO_KEYCHAIN = '1'; $env:EBIR_TEST_ENV = '1'; $env:BIR_AUTH_BYPASS = '1'; $env:BIR_SFTP_DRY_RUN = '1'
    # .env (dotenv-load) may hold the live SFTP account and relay: none of it reaches an agent run, dry run or not.
    foreach ($name in 'BIR_SFTP_LIVE', 'BIR_SFTP_HOST', 'BIR_SFTP_PORT', 'BIR_SFTP_USERNAME', 'BIR_SFTP_PASSWORD', 'BIR_SFTP_ACCEPT_ANY_HOST_KEY', 'BIR_RELAY_URL', 'BUWIZ_RELAY_SEND') {
        Remove-Item "Env:$name" -ErrorAction SilentlyContinue
    }
    # An account server that is not on this machine neither: a debug build then talks to the local mock.
    foreach ($name in 'BUWIZ_SERVER_URL', 'BUWIZ_ACCOUNT_URL') {
        $value = [Environment]::GetEnvironmentVariable($name)
        if ($value -and $value -notmatch '^https?://(localhost|127\.0\.0\.1|\[::1\])(:\d+)?(/|$)') { Remove-Item "Env:$name" -ErrorAction SilentlyContinue }
    }
    $env:GPUI_AGENT = '1'
    if (-not $env:GPUI_AGENT_ADDR) { $env:GPUI_AGENT_ADDR = '127.0.0.1:17421' }
    $writer = $null
    $plugger = $null
    # The parameter arrives as an environment variable (PowerShell cannot read just's quoting of an apostrophe); a relative
    # path is the one the guard checked, from here, not from nsh/.
    $db = $env:db
    if ($db) { $env:BIR_DATABASE_PATH = [IO.Path]::GetFullPath($db, $root) } else { Remove-Item Env:BIR_DATABASE_PATH -ErrorAction SilentlyContinue }
    if ('{{target}}' -eq 'android') {
        # Same Android hand-off as the Unix recipe: the dry-run switches and the data plane's switch and address are baked
        # into the development build (nsh/src/dev_env.rs); the token is not — it goes into the host's private files with
        # `run-as` once `nsh run` installs it, over stdin so it is in no process's arguments (src/agent.rs, take_token).
        $port = [int](($env:GPUI_AGENT_ADDR -split ':')[-1])
        $env:BUWIZ_ANDROID_DEV = '1'
        $env:BUWIZ_ANDROID_DEV_ENV = "EBIR_TEST_ENV=1;BIR_AUTH_BYPASS=1;BUWIZ_NO_KEYCHAIN=1;BIR_SFTP_DRY_RUN=1;GPUI_AGENT=1;GPUI_AGENT_ADDR=127.0.0.1:$port"
        # adb is the one nsh itself runs: in the SDK it finds (ANDROID_HOME, ANDROID_SDK_ROOT, then Android Studio's
        # default place), not whichever is on PATH. Checked here, before anything is built or launched.
        $sdkRoot = @($env:ANDROID_HOME, $env:ANDROID_SDK_ROOT) | Where-Object { $_ } | Select-Object -First 1
        if (-not $sdkRoot -and $env:LOCALAPPDATA) { $sdkRoot = Join-Path $env:LOCALAPPDATA 'Android\Sdk' }
        if (-not $sdkRoot -or -not (Test-Path -LiteralPath $sdkRoot -PathType Container)) {
            [Console]::Error.WriteLine('nsh-run-agent: Android SDK not found: install it and set ANDROID_HOME')
            exit 1
        }
        $adb = Join-Path $sdkRoot 'platform-tools\adb.exe'
        if (-not (Test-Path -LiteralPath $adb -PathType Leaf)) {
            [Console]::Error.WriteLine("nsh-run-agent: no adb in $sdkRoot\platform-tools: install it with ``sdkmanager platform-tools``")
            exit 1
        }
        # nsh launches the first device `adb devices` lists as online and names it to every adb command (`-s`); it does not
        # read ANDROID_SERIAL. The token goes to that same device, by name: with two devices online an unnamed
        # `adb shell` is refused as ambiguous, and one named by ANDROID_SERIAL could be another than the one launched.
        $listed = & $adb devices
        if ($LASTEXITCODE -ne 0) { [Console]::Error.WriteLine("nsh-run-agent: ``$adb devices`` failed"); exit 1 }
        $serial = $listed | Select-Object -Skip 1 | ForEach-Object {
            $serialAndState = $_ -split "`t", 2
            if ($serialAndState.Count -eq 2 -and $serialAndState[1] -eq 'device') { $serialAndState[0] }
        } | Select-Object -First 1
        if (-not $serial) { [Console]::Error.WriteLine('nsh-run-agent: no Android device or emulator connected (see `adb devices`)'); exit 1 }
        if ($env:ANDROID_SERIAL -and $env:ANDROID_SERIAL -ne $serial) {
            [Console]::Error.WriteLine("nsh-run-agent: ANDROID_SERIAL is $($env:ANDROID_SERIAL), but nsh launches the first online device, $serial. Disconnect the other devices, or unset ANDROID_SERIAL to use $serial.")
            exit 1
        }
        # The writer lives no longer than the launch: it is stopped when `nsh run` returns or is interrupted (below), and
        # stops itself when this shell is gone (a closed window, a killed process). It keeps the token in memory and
        # writes it to adb's standard input itself, so no file on this machine holds it.
        $agentHost = 'com.longbridge.nsh.host'
        $tokenDir = 'files/nsh/dev.goldcoders.bir'
        $writer = Start-Job -ScriptBlock {
            param($adb, $serial, $token, $pkg, $dir, $launcher, $launcherStarted)
            for ($i = 0; $i -lt 1800; $i++) {
                $alive = Get-Process -Id $launcher -ErrorAction SilentlyContinue
                if (-not $alive -or $alive.StartTime.Ticks -ne $launcherStarted) { return }
                & $adb -s $serial shell run-as $pkg true 2>$null | Out-Null
                if ($LASTEXITCODE -eq 0) {
                    $start = [Diagnostics.ProcessStartInfo]::new($adb)
                    foreach ($argument in '-s', $serial, 'shell', '-T', "run-as $pkg sh -c 'umask 077 && mkdir -p $dir && cat > $dir/agent-token.new && mv $dir/agent-token.new $dir/agent-token'") {
                        $start.ArgumentList.Add($argument)
                    }
                    $start.UseShellExecute = $false
                    $start.RedirectStandardInput = $true
                    $start.StandardInputEncoding = [Text.UTF8Encoding]::new($false)
                    $write = [Diagnostics.Process]::Start($start)
                    $write.StandardInput.Write($token)
                    $write.StandardInput.Close()
                    $write.WaitForExit()
                    if ($write.ExitCode -eq 0) { return }
                }
                Start-Sleep -Milliseconds 500
            }
            "nsh-run-agent: the data plane's token was not stored (is the host installed?)"
        } -ArgumentList $adb, $serial, $env:GPUI_AGENT_TOKEN, $agentHost, $tokenDir, $PID, (Get-Process -Id $PID).StartTime.Ticks
        # The window plane reads its switch and address from the launch intent's extras, which only a hook in
        # `nsh run` could add — there is none for a plain run. This job stores files/gpui-agent-token, forwards
        # both plane ports, then relaunches the activity on the same dev-server url once `nsh run`'s own launch
        # is up (`am start -S` gives it a pid not seen before), rewriting the data plane's consumed token and
        # sending its own `am start -S`. The url needs a known dev-server port, so the run below pins one.
        # The dev-server port the relaunch points the activity back at: --port in {{args}} wins (it is what
        # `nsh run` binds), else NSH_DEV_PORT, else 8347 — and no second --port is added to the run.
        $devport = if (' {{args}} ' -match ' --port[= ](\d+) ') { [int]$Matches[1] }
            elseif ($env:NSH_DEV_PORT) { [int]$env:NSH_DEV_PORT }
            else { 8347 }
        $plugger = Start-Job -ScriptBlock {
            param($adb, $serial, $token, $pkg, $dir, $port, $devport, $launcher, $launcherStarted)
            $seenPid = $null
            $seenAny = $false
            $forwarded = $false
            $storeToken = {
                param($deviceScript)
                $writeToken = [Diagnostics.ProcessStartInfo]::new($adb)
                foreach ($argument in '-s', $serial, 'shell', '-T', $deviceScript) {
                    $writeToken.ArgumentList.Add($argument)
                }
                $writeToken.UseShellExecute = $false
                $writeToken.RedirectStandardInput = $true
                $writeToken.StandardInputEncoding = [Text.UTF8Encoding]::new($false)
                $w = [Diagnostics.Process]::Start($writeToken)
                $w.StandardInput.Write($token)
                $w.StandardInput.Close()
                $w.WaitForExit()
            }
            for ($i = 0; $i -lt 1800; $i++) {
                $alive = Get-Process -Id $launcher -ErrorAction SilentlyContinue
                if (-not $alive -or $alive.StartTime.Ticks -ne $launcherStarted) {
                    # The run ended before the relaunch: only forwards this watcher made are removed — a stale
                    # one may belong to another run.
                    if ($forwarded) {
                        & $adb -s $serial forward --remove "tcp:$port" 2>$null | Out-Null
                        & $adb -s $serial forward --remove "tcp:$($port + 1)" 2>$null | Out-Null
                    }
                    return
                }
                & $adb -s $serial shell run-as $pkg true 2>$null | Out-Null
                if ($LASTEXITCODE -eq 0) {
                    & $adb -s $serial forward "tcp:$port" "tcp:$port" 2>$null | Out-Null
                    & $adb -s $serial forward "tcp:$($port + 1)" "tcp:$($port + 1)" 2>$null | Out-Null
                    $forwarded = $true
                    & $storeToken "run-as $pkg sh -c 'umask 077 && [ -f files/gpui-agent-token ] || { cat > files/gpui-agent-token.new && mv files/gpui-agent-token.new files/gpui-agent-token; }'"
                    # `nsh run` launches with `am start -S`: the process it starts has a pid not yet seen —
                    # whether none ran or a stale host's was force-stopped (a transient down is shorter than a
                    # poll). The first pid observed could still be a stale host; a new one is nsh's own launch.
                    $hostPid = (& $adb -s $serial shell pidof $pkg 2>$null) -replace '\s', ''
                    if (-not $seenAny) { $seenAny = $true; $seenPid = $hostPid }
                    elseif ($hostPid -and $hostPid -ne $seenPid) {
                        & $storeToken "run-as $pkg sh -c 'umask 077 && mkdir -p $dir && cat > $dir/agent-token.new && mv $dir/agent-token.new $dir/agent-token'"
                        & $adb -s $serial shell am start -S -n "$pkg/com.longbridge.nsh.DevActivity" --es url "ws://127.0.0.1:$devport/ws" --es GPUI_AGENT 1 --es GPUI_AGENT_ADDR "127.0.0.1:$port" 2>$null | Out-Null
                        return
                    }
                }
                Start-Sleep -Milliseconds 500
            }
        } -ArgumentList $adb, $serial, $env:GPUI_AGENT_TOKEN, $agentHost, $tokenDir, $port, $devport, $PID, (Get-Process -Id $PID).StartTime.Ticks
        "nsh-run-agent: window plane on 127.0.0.1:$port, data plane on 127.0.0.1:$($port + 1) on $serial; run: adb -s $serial forward tcp:$port tcp:$port && adb -s $serial forward tcp:$($port + 1) tcp:$($port + 1)"
    }
    $env:CARGO = "$root\nsh\agent\cargo-with-agent.cmd"
    Set-Location nsh
    $code = 1
    try {
        if ('{{target}}' -eq 'android' -and ' {{args}} ' -notmatch ' --port[= ]') { nsh run --target {{target}} --port $devport {{args}} }
        else { nsh run --target {{target}} {{args}} }
        $code = $LASTEXITCODE
    } finally {
        if ($writer) {
            # The launch is over, however it ended (a failed build, a failed install, Ctrl+C): the writers must not go
            # on polling the device, and a token the app never read must not stay in the host's files.
            Stop-Job $writer
            Receive-Job $writer | ForEach-Object { [Console]::Error.WriteLine($_) }
            Remove-Job $writer -Force
            if ($plugger) {
                Stop-Job $plugger
                Receive-Job $plugger | ForEach-Object { [Console]::Error.WriteLine($_) }
                Remove-Job $plugger -Force
            }
            & $adb -s $serial shell "run-as $agentHost rm -f $tokenDir/agent-token $tokenDir/agent-token.new files/gpui-agent-token files/gpui-agent-token.new" 2>$null | Out-Null
        }
    }
    exit $code

# Starts the app with nsh-run-agent on a new scratch database (a debug build, dry run), waits for gpui-agent, runs a
# suite of recipes (agent-recipes/e2e/<suite>.json) and writes report.md, report.json and run.log to
# ~/BuwizNsh/evidence/e2e/<target>-<time>/. target is macos, linux, windows, ios or android; `just e2e check smoke`
# checks a suite with no app. Everything goes through gpui-agent: no OS input, no fixed coordinates. A recipe marked
# send (Queue) runs only with --allow-send under EBIR_TEST_ENV=1 BIR_SFTP_DRY_RUN=1. See docs/proof/E2E.md.
# End to end: run a suite of gpui-agent recipes against the real app (just e2e <target> [suite] [--allow-send])
e2e target="check" suite="smoke" *args="":
    {{PYTHON}} scripts/e2e {{quote(target)}} {{quote(suite)}} {{args}}

# NSH: the Android device choice and token hand-off of nsh-run-agent, against a stand-in adb (no device needed).
[unix]
nsh-agent-handoff-test:
    bash nsh/agent/android-handoff.test.sh

# Opens real macOS windows for a minute or so. BIR_DATABASE_PATH, if set in the environment, is passed on (and guarded).

# Starts the app in a real window with the `agent` feature built into its host (a scratch database, dry run, no keychain; the token is made here and
# never printed), then runs nsh/tests/window_plane.rs against both control planes: the window's (tree, press, type, screenshot) and the data plane's
# (app.info, profile.*, form.*). Nothing is sent anywhere. See docs/agent.md. nsh's CLI builds its hosts without app features;
# nsh/agent/cargo-with-agent.sh, set as $CARGO for that one command, adds the feature to the host build.

# NSH: an external agent against the real app window (cargo feature `agent`, both control planes).
[macos]
nsh-agent-check:
    #!/usr/bin/env bash
    set -euo pipefail
    root="$(pwd -P)"
    export PATH="${NSH_TOOLS:-$HOME/BuwizNsh/tools}/bin:$PATH" CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/BuwizNsh/target-nsh}"
    # Evidence and packages are built by the pinned toolchain's own cargo, never one that shares a folder between checkouts.
    PINNED_CARGO="$(scripts/pinned-cargo-path.sh)"; [ -z "$PINNED_CARGO" ] || export PATH="$PINNED_CARGO:$PATH"
    scripts/build-line.sh --strict
    export BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1
    unset BIR_SFTP_LIVE BIR_DATABASE_PATH
    work="$(mktemp -d)"
    out="$HOME/BuwizNsh/evidence/agent"
    mkdir -p "$out"
    port="$(python3 -I -c 'import socket; s = socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1])')"
    export GPUI_AGENT=1 GPUI_AGENT_TOKEN="$(openssl rand -hex 16)" GPUI_AGENT_ADDR="127.0.0.1:$port" GPUI_AGENT_RUNTIME_DIR="$work/runtime"
    cargo test --manifest-path nsh/Cargo.toml --features agent --test window_plane --no-run
    holder=""
    trap 'kill -INT "$holder" 2>/dev/null || true; wait "$holder" 2>/dev/null || true; rm -rf "$work"' EXIT
    # A host binary that was just linked can take longer than nsh's 60 s to start the first time (the system checks it); the next launch is quick.
    up=0
    for attempt in 1 2 3; do
        (cd "$out" && CARGO="$root/nsh/agent/cargo-with-agent.sh" exec nsh test "$root/nsh/shots/agent.test.tsx" --target macos) &
        holder=$!
        for _ in $(seq 1 90); do
            if nc -z 127.0.0.1 "$port" 2>/dev/null; then up=1; break; fi
            kill -0 "$holder" 2>/dev/null || break
            sleep 2
        done
        [ "$up" = 1 ] && break
        wait "$holder" 2>/dev/null || true
        echo "attempt $attempt: the app did not come up"
    done
    [ "$up" = 1 ] || { echo "the window's control plane never came up on port $port"; exit 1; }
    sleep 3
    cargo test --manifest-path nsh/Cargo.toml --features agent --test window_plane -- --ignored --nocapture

# NSH: real-window screenshots from nsh/shots/<name>.test.tsx into ~/BuwizNsh/evidence/<name>/ (never the worktree).
[macos]
nsh-shots name="base" *args="":
    #!/usr/bin/env bash
    set -euo pipefail
    export PATH="${NSH_TOOLS:-$HOME/BuwizNsh/tools}/bin:$PATH" CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/BuwizNsh/target-nsh}"
    # Evidence and packages are built by the pinned toolchain's own cargo, never one that shares a folder between checkouts.
    PINNED_CARGO="$(scripts/pinned-cargo-path.sh)"; [ -z "$PINNED_CARGO" ] || export PATH="$PINNED_CARGO:$PATH"
    scripts/build-line.sh --strict
    export BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1
    unset BIR_SFTP_LIVE
    {{just_executable()}} _nsh-db-guard "${BIR_DATABASE_PATH:-}"
    test_file="$(pwd -P)/nsh/shots/{{name}}.test.tsx"
    [ -f "$test_file" ] || { echo "no $test_file: shots tests live in nsh/shots/<name>.test.tsx"; exit 1; }
    out="$HOME/BuwizNsh/evidence/{{name}}"
    mkdir -p "$out"
    # The PNGs are written relative to the current directory, so run from the evidence folder (never the worktree).
    cd "$out"
    nsh test "$test_file" --target macos {{args}}
    echo "screenshots in $out"
