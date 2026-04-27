# Release Guide

## Quick Start

```bash
# Auto-increment patch version (0.1.0 → 0.1.1), create tag, push
make publish

# Force a specific version
make publish VERSION_OVERRIDE=0.2.0
```

This triggers the GitHub Actions release workflow, which automatically:
1. Builds for macOS (Universal arm64+x86_64), Windows (x64), and Linux (x64)
2. Creates distributable packages (DMG, ZIP, .deb, tarball)
3. Publishes a GitHub Release with all artifacts

---

## Version Policy

- **Format:** Semantic Versioning — `MAJOR.MINOR.PATCH`
- **Source of truth:** `Cargo.toml` → `[workspace.package] version`
- **All crates** use `version.workspace = true` except `bir-print` (updated by `scripts/version.sh`)

### Version Bump Rules

| Change Type | Command | Example |
|---|---|---|
| Bug fix / patch | `make publish` | 0.1.0 → 0.1.1 |
| New feature | `make publish VERSION_OVERRIDE=0.2.0` | 0.1.1 → 0.2.0 |
| Breaking change | `make publish VERSION_OVERRIDE=1.0.0` | 0.2.0 → 1.0.0 |

---

## Local Build

```bash
# Build for current platform (release mode)
make build

# Build macOS universal binary (arm64 + x86_64)
make build-mac-universal

# Package into .app + DMG (macOS only)
make package-mac

# Sign + notarize (requires env vars, see platform-setup-macos.md)
make sign-mac
```

---

## GitHub Workflows

| Workflow | Trigger | Purpose |
|---|---|---|
| `ci.yml` | Push to `main`, PRs | Compile check, clippy, tests on all 3 platforms |
| `release.yml` | Push tag `v*` | Build + package + upload release artifacts |

---

## Release Artifacts

Each release produces:

| Platform | Artifact | Contents |
|---|---|---|
| macOS | `eBIRForms-macOS-universal-X.Y.Z.dmg` | Signed .app bundle (arm64+x86_64) |
| Windows | `eBIRForms-Windows-x64-X.Y.Z.zip` | `bir.exe` + `bir-daemon.exe` + assets |
| Linux | `eBIRForms-Linux-x64-X.Y.Z.deb` | Debian package with systemd service |
| Linux | `eBIRForms-Linux-x64-X.Y.Z.tar.gz` | Portable tarball |

---

## Changelog

Update the GitHub Release notes manually or let `generate_release_notes: true` auto-generate from PR titles and commit messages.
