# macOS Platform Setup

## Distribution Strategy

> **⚠️ Mac App Store is NOT recommended** for this application due to sandbox
> restrictions that conflict with Swift subprocess printing and `native-tls`
> IMAP connectivity. We use **Developer ID distribution** instead.

With Developer ID distribution, users download the DMG directly from GitHub
Releases and install it by dragging to `/Applications`. macOS Gatekeeper
accepts the app because it is code-signed and notarized.

---

## Prerequisites

### 1. Apple Developer Program ($99/year)

1. Go to [developer.apple.com](https://developer.apple.com)
2. Enroll in the **Apple Developer Program** (individual or organization)
3. Complete payment ($99 USD/year)
4. Wait for enrollment approval (usually 24-48 hours)

### 2. Create a Developer ID Certificate

1. Open **Xcode → Settings → Accounts → Manage Certificates**
2. Click **+** → **Developer ID Application**
3. This creates a certificate in your Keychain

**Or via CLI:**
```bash
# Generate a Certificate Signing Request
openssl req -new -newkey rsa:2048 -nodes \
  -keyout developer_id.key -out developer_id.csr \
  -subj "/CN=Your Name/O=Goldcoders Corp"

# Upload CSR to developer.apple.com/account/resources/certificates
# Download the resulting .cer file
# Double-click to install in Keychain Access
```

### 3. Export the Certificate as .p12

```bash
# In Keychain Access:
# 1. Find "Developer ID Application: Goldcoders Corp (TEAMID)"
# 2. Right-click → Export Items → .p12 format
# 3. Set a strong password
```

### 4. Create an App-Specific Password

1. Go to [appleid.apple.com](https://appleid.apple.com)
2. Sign in → **App-Specific Passwords** → Generate
3. Label it "eBIRForms Notarization"
4. Save the generated password

### 5. Find Your Team ID

```bash
# If you have Xcode:
grep -A1 TeamID ~/Library/Preferences/com.apple.dt.Xcode.plist

# Or check developer.apple.com → Account → Membership → Team ID
```

---

## Environment Variables (Local)

For local signing with `make sign-mac`:

```bash
export APPLE_IDENTITY="Developer ID Application: Goldcoders Corp (XXXXXXXXXX)"
export APPLE_ID="your@email.com"
export APPLE_APP_PASSWORD="xxxx-xxxx-xxxx-xxxx"
export APPLE_TEAM_ID="XXXXXXXXXX"
```

---

## GitHub Secrets (CI/CD)

Set these in your repo: **Settings → Secrets → Actions**

| Secret | Value |
|---|---|
| `APPLE_CERTIFICATE_P12` | Base64-encoded `.p12` file: `base64 -i cert.p12 \| pbcopy` |
| `APPLE_CERTIFICATE_PASSWORD` | Password you set when exporting the .p12 |
| `APPLE_TEAM_ID` | Your 10-character Team ID |
| `APPLE_ID` | Your Apple ID email |
| `APPLE_APP_PASSWORD` | App-specific password from step 4 |

---

## What Happens During Release

1. **Build:** ARM64 + x86_64 binaries compiled on an M1 runner
2. **Lipo:** Universal binary created from both architectures
3. **Bundle:** `.app` structure assembled with assets + formtypes + Info.plist
4. **Sign:** `codesign --deep --force --options runtime` with Developer ID cert
5. **Package:** DMG created with `create-dmg` or `hdiutil`
6. **Notarize:** DMG submitted to Apple's notary service via `xcrun notarytool`
7. **Staple:** Notarization ticket attached to DMG via `xcrun stapler`

If signing secrets are not configured, the workflow still produces an **unsigned**
DMG/ZIP that works but shows a Gatekeeper warning.

---

## Mac App Store (Future)

If you ever need MAS distribution:

1. The `mas_build` Cargo feature is already implemented — it disables LaunchAgent daemon
2. You would need to:
   - Replace `native-tls` with `rustls` for IMAP
   - Replace Swift subprocess printing with NSPrintOperation via FFI or the `bir-print` objc2 path
   - Add full App Sandbox entitlements
   - Submit via App Store Connect / Xcode
3. This is a significant engineering effort and is **not recommended** at this time

---

## Troubleshooting

### "eBIRForms can't be opened because Apple cannot check it for malicious software"

The DMG was not notarized. Run:
```bash
make sign-mac
```

### "The identity can't be found in the keychain"

Ensure the Developer ID certificate is installed:
```bash
security find-identity -v -p codesigning
```

### Notarization fails with "Package Invalid"

Check entitlements:
```bash
codesign -d --entitlements :- "target/release-artifacts/eBIRForms.app"
```
