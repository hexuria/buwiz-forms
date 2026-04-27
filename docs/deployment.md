# eBIRForms Deployment Guide

> **Last updated:** 2026-04-27 · **Current version:** 0.1.0

Everything you need to know to build, sign, package, and distribute eBIRForms
for macOS, Windows, and Linux.

---

## Table of Contents

1. [Overview — How Releases Work](#1-overview--how-releases-work)
2. [Quick Start — Your First Release](#2-quick-start--your-first-release)
3. [Version Management](#3-version-management)
4. [Distribution Channels — What's Possible, What's Not](#4-distribution-channels)
5. [macOS Setup (Developer ID + Notarization)](#5-macos-setup)
6. [Windows Setup (Direct + Microsoft Store)](#6-windows-setup)
7. [Linux Setup (.deb + AUR)](#7-linux-setup)
8. [GitHub Secrets — Complete Reference](#8-github-secrets)
9. [Local Makefile Commands](#9-local-makefile-commands)
10. [Release Artifacts — What Gets Built](#10-release-artifacts)
11. [Troubleshooting](#11-troubleshooting)

---

## 1. Overview — How Releases Work

```
┌──────────────────────────────────────────────────────────────────┐
│  Developer Machine                                               │
│                                                                  │
│  make publish             ← auto-bumps version 0.1.0 → 0.1.1   │
│    └─ scripts/version.sh  ← updates Cargo.toml                  │
│    └─ git tag v0.1.1      ← creates annotated tag               │
│    └─ git push origin v0.1.1  ← pushes tag                      │
│                                                                  │
└─────────────────────────────┬────────────────────────────────────┘
                              │
                              ▼
┌──────────────────────────────────────────────────────────────────┐
│  GitHub Actions (.github/workflows/release.yml)                  │
│                                                                  │
│  Triggered by: push tag matching v*                              │
│                                                                  │
│  ┌─────────────┐  ┌─────────────┐  ┌──────────────┐            │
│  │  macOS M1   │  │  Windows    │  │  Ubuntu      │            │
│  │  (macos-14) │  │  (latest)   │  │  (22.04)     │            │
│  ├─────────────┤  ├─────────────┤  ├──────────────┤            │
│  │ arm64 build │  │ x64 build   │  │ x64 build    │            │
│  │ x86 build   │  │ zip package │  │ .deb package │            │
│  │ lipo merge  │  │             │  │ .tar.gz      │            │
│  │ .app bundle │  │             │  │              │            │
│  │ codesign    │  │             │  │              │            │
│  │ notarize    │  │             │  │              │            │
│  │ DMG         │  │             │  │              │            │
│  └──────┬──────┘  └──────┬──────┘  └──────┬───────┘            │
│         └────────────────┼────────────────┘                     │
│                          ▼                                       │
│              GitHub Release created                              │
│              with all artifacts attached                          │
└──────────────────────────────────────────────────────────────────┘
```

**Two workflows exist:**

| File | Trigger | What it does |
|---|---|---|
| `.github/workflows/ci.yml` | Push to `main`, PRs | Compile check + clippy + tests on all 3 platforms |
| `.github/workflows/release.yml` | Push tag `v*` | Build + package + sign + publish GitHub Release |

---

## 2. Quick Start — Your First Release

**Even without any signing secrets configured**, you can do a release right now.
The workflow will skip signing steps and produce unsigned artifacts.

```bash
# 1. Make sure you're on main with clean working tree
git checkout main
git pull

# 2. Bump version and push tag
make publish
# This does: 0.1.0 → 0.1.1, commits, tags v0.1.1, pushes

# 3. Watch it build
open https://github.com/goldcoders/bir/actions
```

Within ~15-20 minutes, a GitHub Release will appear at
`https://github.com/goldcoders/bir/releases/tag/v0.1.1` with downloadable
artifacts for all three platforms.

**To force a specific version:**

```bash
make publish VERSION_OVERRIDE=0.2.0
```

---

## 3. Version Management

### Where the version lives

```
Cargo.toml (workspace root)
└── [workspace.package]
    └── version = "0.1.0"   ← single source of truth
```

All crates inherit via `version.workspace = true` except `bir-print` which has
its own version field — `scripts/version.sh` updates both.

### Version bump rules

| What changed | Command | Example |
|---|---|---|
| Bug fix | `make publish` | 0.1.0 → 0.1.1 |
| New feature | `make publish VERSION_OVERRIDE=0.2.0` | 0.1.1 → 0.2.0 |
| Breaking change | `make publish VERSION_OVERRIDE=1.0.0` | 0.2.0 → 1.0.0 |

### What `make publish` does under the hood

```bash
# 1. Bump patch version in Cargo.toml
./scripts/version.sh bump     # 0.1.0 → 0.1.1

# 2. Commit + tag + push
./scripts/version.sh tag       # git commit, git tag v0.1.1, git push
```

### Manual version operations

```bash
./scripts/version.sh           # Print current version
./scripts/version.sh bump      # Auto-increment patch
./scripts/version.sh set 1.0.0 # Set exact version
./scripts/version.sh tag       # Commit + tag + push
```

---

## 4. Distribution Channels

### What works and what doesn't

| Channel | Status | Why |
|---|---|---|
| **GitHub Releases** (all platforms) | ✅ Works now | Direct download — DMG, ZIP, .deb, tarball |
| **macOS Developer ID** (DMG) | ✅ Recommended | Signed + notarized, passes Gatekeeper |
| **Microsoft Store** (MSIX) | ✅ Feasible | No blockers; free auto-signing by Microsoft |
| **Debian .deb** | ✅ Automated | cargo-deb metadata configured |
| **Arch Linux AUR** | ✅ Feasible | Simple PKGBUILD needed |
| **Mac App Store** | ❌ Not recommended | See below |
| **Flatpak / Snap** | 🔜 Future | Not configured yet |

### Why NOT the Mac App Store

The Mac App Store requires **App Sandbox**, which breaks our app in three ways:

1. **Swift subprocess for printing** — We spawn `swift` to run a PDFKit print
   script. Sandboxed apps cannot execute arbitrary subprocesses.
2. **`native-tls` for IMAP** — The `native-tls` crate uses `Security.framework`
   to verify TLS certificates. Inside the sandbox, this can fail because the
   framework needs keychain/root CA access that the sandbox restricts.
3. **LaunchAgent daemon** — Already handled (gated behind `mas_build` feature),
   but the first two blockers remain.

**To make MAS work in the future**, you would need to:
- Replace the Swift print subprocess with direct `objc2` FFI (the code exists
  in `bir-print` but is commented out)
- Replace `native-tls` with `rustls` + bundled root certificates
- This is a significant engineering effort — not worth it when Developer ID
  distribution works perfectly

---

## 5. macOS Setup

### What you need

| Item | Cost | Time |
|---|---|---|
| Apple Developer Program | $99/year | 24-48h approval |
| Developer ID Application certificate | Free (included) | 10 min |
| App-specific password | Free | 2 min |

### Step-by-step

#### 5.1 Enroll in Apple Developer Program

1. Go to [developer.apple.com/programs](https://developer.apple.com/programs/)
2. Click **Enroll**
3. Sign in with your Apple ID (or create one)
4. Choose **Individual** or **Organization**
   - Individual: just your Apple ID + payment
   - Organization: requires D-U-N-S number
5. Pay $99 USD
6. Wait for approval (usually 24-48 hours, sometimes instant)

#### 5.2 Create Developer ID Certificate

**Option A — Xcode (easiest):**

1. Open Xcode → **Settings** → **Accounts**
2. Select your Apple ID → click your team
3. Click **Manage Certificates**
4. Click **+** → **Developer ID Application**
5. Done — certificate is now in your Keychain

**Option B — Command line:**

```bash
# Generate CSR
openssl req -new -newkey rsa:2048 -nodes \
  -keyout ~/developer_id.key \
  -out ~/developer_id.csr \
  -subj "/CN=Goldcoders Corp/O=Goldcoders Corp"

# Go to developer.apple.com/account/resources/certificates
# → Click "+" → "Developer ID Application"
# → Upload the .csr file
# → Download the .cer file
# → Double-click to install in Keychain Access
```

#### 5.3 Export certificate as .p12

1. Open **Keychain Access**
2. Search for "Developer ID Application"
3. Right-click the certificate → **Export Items...**
4. Choose **Personal Information Exchange (.p12)** format
5. Set a **strong password** (you'll need this for GitHub secrets)
6. Save to `~/developer_id.p12`

#### 5.4 Get your Team ID

```bash
# Option A — from Xcode
grep -A1 TeamID ~/Library/Preferences/com.apple.dt.Xcode.plist

# Option B — website
# Go to developer.apple.com → Account → Membership → Team ID
# It's a 10-character string like "A1B2C3D4E5"
```

#### 5.5 Create app-specific password for notarization

1. Go to [appleid.apple.com](https://appleid.apple.com)
2. Sign in → **Sign-In and Security** → **App-Specific Passwords**
3. Click **Generate an app-specific password**
4. Name it `eBIRForms Notarization`
5. Copy the generated password (format: `xxxx-xxxx-xxxx-xxxx`)

#### 5.6 Set GitHub secrets

Go to your repo: **Settings → Secrets and variables → Actions → New repository secret**

| Secret name | Value |
|---|---|
| `APPLE_CERTIFICATE_P12` | Run: `base64 -i ~/developer_id.p12 \| pbcopy` then paste |
| `APPLE_CERTIFICATE_PASSWORD` | The password you set in step 5.3 |
| `APPLE_TEAM_ID` | Your 10-character Team ID from step 5.4 |
| `APPLE_ID` | Your Apple ID email address |
| `APPLE_APP_PASSWORD` | App-specific password from step 5.5 |

#### 5.7 Test locally (optional)

```bash
# Set env vars
export APPLE_IDENTITY="Developer ID Application: Goldcoders Corp (A1B2C3D4E5)"
export APPLE_ID="your@email.com"
export APPLE_APP_PASSWORD="xxxx-xxxx-xxxx-xxxx"
export APPLE_TEAM_ID="A1B2C3D4E5"

# Build + package + sign
make package-mac
make sign-mac
```

#### 5.8 What happens in CI

The release workflow automatically:
1. Builds ARM64 (`aarch64-apple-darwin`) and x86_64 (`x86_64-apple-darwin`)
2. Merges into a **universal binary** with `lipo`
3. Creates an `.app` bundle with `Info.plist`, assets, formtypes
4. **Codesigns** with your Developer ID cert (if secrets are set)
5. Creates a DMG
6. **Notarizes** with Apple's notary service (if secrets are set)
7. **Staples** the notarization ticket to the DMG

If secrets are NOT set, steps 4/6/7 are skipped — you get an unsigned DMG.

---

## 6. Windows Setup

### 6.1 Direct distribution (ZIP) — no setup needed

The release workflow automatically produces a
`eBIRForms-Windows-x64-X.Y.Z.zip` containing:

```
bir.exe
bir-daemon.exe
assets/
formtypes/
```

Users download, extract, and run `bir.exe`. **That's it.**

The only downside: Windows SmartScreen may show "Windows protected your PC"
because the exe is unsigned. Users click **More info → Run anyway**.

### 6.2 Optional: Code signing (eliminates SmartScreen)

To avoid SmartScreen warnings on direct downloads:

1. Purchase a code signing certificate (~$200-400/year from DigiCert, Sectigo, or SSL.com)
2. Export as `.pfx` file
3. Set GitHub secrets:

| Secret | Value |
|---|---|
| `WINDOWS_SIGNING_CERT` | `base64 -i cert.pfx \| pbcopy` then paste |
| `WINDOWS_SIGNING_PASSWORD` | PFX password |

> **Note:** If you plan to use the Microsoft Store instead, you do NOT need
> to buy a certificate — Microsoft auto-signs your app for free.

### 6.3 Optional: Microsoft Store (MSIX)

#### Prerequisites

1. **Register at Partner Center**
   - Go to [partner.microsoft.com](https://partner.microsoft.com)
   - Click **Sign up** for a developer account
   - **Individual account:** Free (requires ID verification — government-issued
     ID + selfie, takes 1-3 business days)
   - **Company account:** May require D-U-N-S number

2. **Reserve app name**
   - In Partner Center → **Apps and games** → **New product** → **MSIX or PWA app**
   - Reserve the name `eBIRForms`
   - Note the Package Identity values provided

3. **Create MSIX package** — this is a manual step for now:

```powershell
# On a Windows machine after building:
cargo build --release --target x86_64-pc-windows-msvc

# Create package layout
$VERSION = "0.1.0"
$PKG = "msix-staging"
New-Item -ItemType Directory -Force -Path "$PKG"
Copy-Item "target/x86_64-pc-windows-msvc/release/bir.exe" "$PKG/"
Copy-Item "target/x86_64-pc-windows-msvc/release/bir-daemon.exe" "$PKG/"
Copy-Item -Recurse "assets" "$PKG/"
Copy-Item -Recurse "formtypes" "$PKG/"

# You'll also need an AppxManifest.xml — see Microsoft docs
# Then: MakeAppx.exe pack /d "$PKG" /p "eBIRForms.msix"
```

4. **Submit to Store**
   - Partner Center → your app → **New Submission**
   - Fill in: Pricing (Free), Category (Finance), Age Rating, Store Listing
   - Upload `.msix` → Submit for certification
   - Microsoft reviews in 1-3 business days
   - **Microsoft auto-signs your package** — no cert purchase needed

### 6.4 Runtime requirements

Users need:
- Windows 10 version 1903 or later
- DirectX 12 or Vulkan-compatible GPU
- Visual C++ Redistributable 2022 (usually pre-installed)

---

## 7. Linux Setup

### 7.1 .deb package — automated, no setup needed

The release workflow uses `cargo-deb` to produce `.deb` packages automatically.
The metadata is already configured in `crates/bir-desktop/Cargo.toml`:

```toml
[package.metadata.deb]
name = "ebirforms"
depends = "$auto, libvulkan1, libxkbcommon0, libwayland-client0, fontconfig, libfreetype6"
```

Users install with:

```bash
sudo dpkg -i eBIRForms-Linux-x64-0.1.0.deb
sudo apt-get install -f  # pull missing deps
ebirforms                # run
```

### 7.2 Tarball — automated, no setup needed

Also produced automatically. For users who prefer manual installs:

```bash
tar xzf eBIRForms-Linux-x64-0.1.0.tar.gz
cd eBIRForms-Linux-x64-0.1.0/
./bir
```

### 7.3 Optional: Arch Linux AUR

#### Prerequisites

1. Register at [aur.archlinux.org](https://aur.archlinux.org/register)
2. Add your SSH public key to your AUR account profile

#### Create the PKGBUILD

Create a file called `PKGBUILD`:

```bash
# Maintainer: Goldcoders <admin@goldcoders.dev>
pkgname=ebirforms
pkgver=0.1.0
pkgrel=1
pkgdesc="Philippine BIR tax return filing desktop application"
arch=('x86_64')
url="https://github.com/goldcoders/bir"
license=('MIT')
depends=(
    'vulkan-icd-loader'
    'libxkbcommon'
    'wayland'
    'fontconfig'
    'freetype2'
    'openssl'
)
makedepends=('rust' 'cargo' 'pkg-config' 'vulkan-headers'
             'libx11' 'libxcb' 'wayland-protocols')
source=("$pkgname-$pkgver.tar.gz::https://github.com/goldcoders/bir/archive/v$pkgver.tar.gz")
sha256sums=('SKIP')

build() {
    cd "bir-$pkgver"
    cargo build --release
}

package() {
    cd "bir-$pkgver"
    install -Dm755 "target/release/bir" "$pkgdir/usr/bin/ebirforms"
    install -Dm755 "target/release/bir-daemon" "$pkgdir/usr/bin/ebirforms-daemon"
    install -dm755 "$pkgdir/usr/share/ebirforms"
    cp -r assets "$pkgdir/usr/share/ebirforms/"
    cp -r formtypes "$pkgdir/usr/share/ebirforms/"
    install -Dm644 LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
}
```

#### Submit to AUR

```bash
# First time
git clone ssh://aur@aur.archlinux.org/ebirforms.git
cd ebirforms
cp /path/to/PKGBUILD .
makepkg --printsrcinfo > .SRCINFO
git add PKGBUILD .SRCINFO
git commit -m "Initial upload: ebirforms 0.1.0"
git push

# For updates — change pkgver in PKGBUILD, then:
makepkg --printsrcinfo > .SRCINFO
git add -A && git commit -m "Update to 0.1.1" && git push
```

For automated AUR updates via CI, set this GitHub secret:

| Secret | Value |
|---|---|
| `AUR_SSH_KEY` | SSH private key (`~/.ssh/id_ed25519`) registered with your AUR account |

### 7.4 Runtime dependencies by distro

#### Ubuntu / Debian / Pop!_OS / Mint

```bash
sudo apt install libvulkan1 mesa-vulkan-drivers libxkbcommon0 \
  libxkbcommon-x11-0 libwayland-client0 fontconfig libfreetype6
```

#### Arch / Manjaro / EndeavourOS

```bash
sudo pacman -S vulkan-icd-loader libxkbcommon wayland fontconfig freetype2
# Plus your GPU driver:
sudo pacman -S vulkan-radeon    # AMD
sudo pacman -S vulkan-intel     # Intel
sudo pacman -S nvidia-utils     # NVIDIA
```

#### Fedora

```bash
sudo dnf install vulkan-loader mesa-vulkan-drivers libxkbcommon \
  wayland fontconfig freetype
```

---

## 8. GitHub Secrets — Complete Reference

Go to: **Repository → Settings → Secrets and variables → Actions**

### Required for signed macOS builds

| Secret | How to get it |
|---|---|
| `APPLE_CERTIFICATE_P12` | Export Developer ID cert from Keychain as .p12, then `base64 -i cert.p12` |
| `APPLE_CERTIFICATE_PASSWORD` | Password you set when exporting the .p12 |
| `APPLE_TEAM_ID` | [developer.apple.com](https://developer.apple.com) → Account → Membership |
| `APPLE_ID` | Your Apple Developer email |
| `APPLE_APP_PASSWORD` | [appleid.apple.com](https://appleid.apple.com) → App-Specific Passwords |

### Optional

| Secret | What for |
|---|---|
| `WINDOWS_SIGNING_CERT` | Code signing for direct Windows distribution (base64 .pfx) |
| `WINDOWS_SIGNING_PASSWORD` | PFX password |
| `AUR_SSH_KEY` | Automated AUR package updates (SSH private key) |

### What happens without secrets

| Secrets missing | Result |
|---|---|
| All Apple secrets missing | Builds fine, produces **unsigned** DMG → Gatekeeper warns users |
| Windows cert missing | Builds fine, produces **unsigned** ZIP → SmartScreen warns users |
| AUR key missing | No effect on build — AUR updates are manual |

**The workflow NEVER fails due to missing secrets.** Signing steps are skipped
gracefully via `if: env.APPLE_CERTIFICATE_P12 != ''` conditions.

---

## 9. Local Makefile Commands

```bash
# ── Quality ──────────────────────────────
make check            # cargo check --workspace
make test             # cargo test --workspace
make clippy           # clippy with -D warnings

# ── Build ────────────────────────────────
make build            # release build (current platform)
make build-mac-universal  # arm64 + x86_64 → universal binary via lipo
make build-win        # x86_64-pc-windows-msvc
make build-linux      # x86_64-unknown-linux-gnu

# ── Package ──────────────────────────────
make package-mac      # .app bundle + DMG (requires build-mac-universal)
make package-win      # ZIP with exe + assets
make package-linux    # .deb via cargo-deb (or tarball fallback)

# ── Sign (macOS) ─────────────────────────
make sign-mac         # codesign + notarize (requires env vars)
                      # APPLE_IDENTITY, APPLE_ID, APPLE_APP_PASSWORD, APPLE_TEAM_ID

# ── Release ──────────────────────────────
make publish                       # auto-bump patch → tag → push
make publish VERSION_OVERRIDE=0.2.0  # explicit version → tag → push

# ── Cleanup ──────────────────────────────
make clean            # cargo clean + remove artifacts
```

---

## 10. Release Artifacts

Each release produces these downloadable files:

| Platform | Filename | Size (est.) | Contents |
|---|---|---|---|
| macOS | `eBIRForms-macOS-universal-X.Y.Z.dmg` | ~50-80 MB | Signed .app (arm64 + x86_64) |
| Windows | `eBIRForms-Windows-x64-X.Y.Z.zip` | ~40-60 MB | `bir.exe`, `bir-daemon.exe`, assets |
| Linux | `eBIRForms-Linux-x64-X.Y.Z.deb` | ~40-60 MB | Debian package with deps |
| Linux | `eBIRForms-Linux-x64-X.Y.Z.tar.gz` | ~40-60 MB | Portable tarball |

### What's bundled inside every artifact

```
bir (or bir.exe)           ← main desktop app
bir-daemon (or .exe)       ← background email polling daemon
assets/                    ← icons, SVGs, images
formtypes/                 ← BIR form templates (page SVGs, schemas)
```

---

## 11. Troubleshooting

### Release workflow doesn't trigger

**Cause:** Tag wasn't pushed, or tag doesn't match `v*` pattern.

```bash
# Verify tag exists locally
git tag -l 'v*'

# Verify tag exists on remote
git ls-remote --tags origin

# If missing, push it
git push origin v0.1.1
```

### macOS: "eBIRForms can't be opened because Apple cannot check it"

**Cause:** DMG is not notarized (Apple secrets not configured).

**Fix:** Set all 5 Apple secrets (section 5.6), then re-release.

**Workaround for users:** Right-click the app → Open → Open anyway.

### macOS: Codesign fails with "identity not found"

```bash
# List available signing identities
security find-identity -v -p codesigning

# Make sure it shows "Developer ID Application: ..."
# If not, you need to install the certificate (section 5.2-5.3)
```

### macOS: Notarization fails

```bash
# Check the notarization log
xcrun notarytool log <submission-id> \
  --apple-id "$APPLE_ID" \
  --password "$APPLE_APP_PASSWORD" \
  --team-id "$APPLE_TEAM_ID"
```

Common causes:
- Hardened runtime not enabled (check entitlements.plist)
- Unsigned helper binary (both `bir` and `bir-daemon` must be signed)
- Linking against private frameworks

### Windows: SmartScreen "Unknown publisher"

**Cause:** Executable is not code-signed.

**Options:**
1. Purchase code signing cert ($200-400/year) — see section 6.2
2. Submit to Microsoft Store (free signing) — see section 6.3
3. Tell users to click **More info → Run anyway**

### Linux: "Failed to create Vulkan instance"

**Cause:** Missing or misconfigured GPU Vulkan drivers.

```bash
# Test Vulkan
vulkaninfo | head -20

# If it fails, install drivers (section 7.4)
# AMD:   sudo apt install mesa-vulkan-drivers
# Intel: sudo apt install mesa-vulkan-drivers
# NVIDIA: sudo apt install nvidia-driver-xxx
```

### Linux: "libxkbcommon.so: cannot open shared object file"

```bash
sudo apt install libxkbcommon0 libxkbcommon-x11-0 libwayland-client0
```

### CI: Linux build fails with missing headers

The `ci.yml` workflow installs these packages. If it still fails, GPUI may
have added new dependencies. Check:

```bash
# Clone Zed and see what they install
curl -s https://raw.githubusercontent.com/zed-industries/zed/main/script/linux \
  | grep apt
```

Then update the `apt-get install` list in both `ci.yml` and `release.yml`.

### cargo-deb: "no such file" for assets

The `assets` globs in `[package.metadata.deb]` are relative to the workspace
root. If paths change, update `crates/bir-desktop/Cargo.toml`:

```toml
[package.metadata.deb]
assets = [
    ["target/release/bir", "usr/bin/ebirforms", "755"],
    ["target/release/bir-daemon", "usr/bin/ebirforms-daemon", "755"],
    ["../../assets/**/*", "usr/share/ebirforms/assets/", "644"],
    ["../../formtypes/**/*", "usr/share/ebirforms/formtypes/", "644"],
]
```

---

## Checklist — First Production Release

```
□ Apple Developer Program enrolled ($99/year)
□ Developer ID Application certificate created
□ Certificate exported as .p12
□ App-specific password created at appleid.apple.com
□ Team ID noted

□ GitHub secret: APPLE_CERTIFICATE_P12
□ GitHub secret: APPLE_CERTIFICATE_PASSWORD
□ GitHub secret: APPLE_TEAM_ID
□ GitHub secret: APPLE_ID
□ GitHub secret: APPLE_APP_PASSWORD

□ Run: make publish
□ Verify: GitHub Actions → release workflow completes
□ Verify: GitHub Release page shows all 4 artifacts
□ Test: Download DMG on macOS → opens without Gatekeeper warning
□ Test: Download ZIP on Windows → bir.exe runs
□ Test: Download .deb on Ubuntu → installs and runs

□ (Optional) Microsoft Partner Center account registered
□ (Optional) AUR account + PKGBUILD submitted
```
