# GitHub Secrets Reference

All secrets are set in: **Repository → Settings → Secrets and variables → Actions**

---

## macOS Code Signing & Notarization

| Secret | Required? | How to Get | Description |
|---|---|---|---|
| `APPLE_CERTIFICATE_P12` | Yes (for signed builds) | Export from Keychain Access as .p12, then: `base64 -i cert.p12 \| pbcopy` | Base64-encoded Developer ID Application certificate |
| `APPLE_CERTIFICATE_PASSWORD` | Yes | Set when exporting .p12 | Password protecting the P12 file |
| `APPLE_TEAM_ID` | Yes | [developer.apple.com](https://developer.apple.com) → Account → Membership | 10-character Team ID (e.g., `XXXXXXXXXX`) |
| `APPLE_ID` | Yes | Your Apple Developer email | Used for notarytool authentication |
| `APPLE_APP_PASSWORD` | Yes | [appleid.apple.com](https://appleid.apple.com) → App-Specific Passwords → Generate | App-specific password for notarization |

**If these are NOT set:** The workflow still builds and packages the app, but
skips codesign and notarization. The resulting DMG will trigger Gatekeeper
warnings on macOS.

---

## Windows Code Signing (Optional)

| Secret | Required? | How to Get | Description |
|---|---|---|---|
| `WINDOWS_SIGNING_CERT` | No (SmartScreen only) | Purchase from DigiCert/Sectigo, export as .pfx, then base64 | Code signing certificate |
| `WINDOWS_SIGNING_PASSWORD` | No | Set when exporting .pfx | PFX password |

**If NOT set:** The ZIP download works fine, but Windows SmartScreen shows a
"Publisher unknown" warning. If distributing via Microsoft Store, this is
unnecessary (Microsoft auto-signs).

---

## Linux AUR (Optional)

| Secret | Required? | How to Get | Description |
|---|---|---|---|
| `AUR_SSH_KEY` | No (manual AUR updates work) | `ssh-keygen -t ed25519`, add public key to [aur.archlinux.org](https://aur.archlinux.org) | SSH private key for automated AUR pushes |

---

## Verification

To verify secrets are properly configured:

```bash
# Check the Actions tab after pushing a tag
git tag -a v0.1.0 -m "test release"
git push origin v0.1.0

# Watch the release workflow in GitHub → Actions
# The codesign/notarize steps will be SKIPPED (not failed) if secrets are missing
```

---

## Security Notes

- **Never commit secrets** to the repository
- Rotate `APPLE_APP_PASSWORD` if compromised
- The P12 certificate password and the certificate itself should use different
  passwords
- GitHub encrypts secrets at rest and masks them in logs
