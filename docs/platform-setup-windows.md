# Windows Platform Setup

## Distribution Strategy

eBIRForms can be distributed on Windows through:

1. **GitHub Releases** (ZIP download) — immediate, no setup required
2. **Microsoft Store** (MSIX package) — optional, requires Partner Center account

Both are supported by the release pipeline.

---

## Option 1: Direct Distribution (ZIP)

This works out of the box — no additional setup needed. The release workflow
produces a `eBIRForms-Windows-x64-X.Y.Z.zip` containing:

```
eBIRForms-Windows-x64-0.1.0/
  bir.exe
  bir-daemon.exe
  assets/
  formtypes/
```

Users extract and run `bir.exe`. Windows Defender SmartScreen may show a warning
for unsigned executables.

### Optional: Code Signing for Direct Distribution

To eliminate SmartScreen warnings:

1. Purchase a code signing certificate from a CA (DigiCert, Sectigo, etc.)
2. Sign with `signtool`:
   ```powershell
   signtool sign /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 ^
     /f certificate.pfx /p PASSWORD bir.exe
   ```
3. Add `WINDOWS_SIGNING_CERT` (Base64 .pfx) and `WINDOWS_SIGNING_PASSWORD`
   as GitHub secrets

---

## Option 2: Microsoft Store (MSIX)

### Prerequisites

#### 1. Microsoft Partner Center Account

1. Go to [partner.microsoft.com](https://partner.microsoft.com)
2. Sign up for a **Developer Account**
   - **Individual:** Free (requires identity verification — government ID + selfie)
   - **Company:** May require a D-U-N-S number and business verification
3. Complete identity verification (takes 1-3 business days)

#### 2. Reserve Your App Name

1. In Partner Center → **Apps and games** → **New product** → **MSIX or PWA app**
2. Reserve the name `eBIRForms`
3. Note the **Package Identity** values:
   - Package Identity Name
   - Publisher
   - Publisher Display Name

#### 3. Install Windows SDK Tools

```powershell
# Option A: Install via Visual Studio Installer
# Select "Windows SDK" component

# Option B: Standalone
winget install Microsoft.WindowsSDK
```

You need `MakeAppx.exe` and `SignTool.exe`.

#### 4. Install winapp CLI (Optional)

```powershell
# Microsoft's Rust app packaging CLI
cargo install winapp
winapp init  # Creates AppxManifest.xml template
```

### Creating the MSIX Package

```powershell
# 1. Build release
cargo build --release --target x86_64-pc-windows-msvc

# 2. Create package layout
$PKG = "msix-staging"
New-Item -ItemType Directory -Force -Path "$PKG/VFS/ProgramFiles/eBIRForms"
Copy-Item "target/x86_64-pc-windows-msvc/release/bir.exe" "$PKG/VFS/ProgramFiles/eBIRForms/"
Copy-Item "target/x86_64-pc-windows-msvc/release/bir-daemon.exe" "$PKG/VFS/ProgramFiles/eBIRForms/"
Copy-Item -Recurse "assets" "$PKG/VFS/ProgramFiles/eBIRForms/"
Copy-Item -Recurse "formtypes" "$PKG/VFS/ProgramFiles/eBIRForms/"
Copy-Item "AppxManifest.xml" "$PKG/"

# 3. Create .msix
MakeAppx.exe pack /d "$PKG" /p "eBIRForms.msix"

# 4. (Store submission auto-signs — no cert needed)
```

### Submitting to Microsoft Store

1. Log in to [Partner Center](https://partner.microsoft.com)
2. Go to your reserved app
3. Create a new **Submission**
4. Fill in:
   - **Pricing:** Free
   - **Properties:** Category = Business / Finance
   - **Age Rating:** Complete IARC questionnaire
   - **Packages:** Upload `.msix` file
   - **Store Listing:** Description, screenshots, logos
5. Submit for certification (typically 1-3 business days)

**Important:** Microsoft auto-signs your MSIX during certification — you do NOT
need to purchase a separate certificate for Store distribution.

---

## GitHub Secrets (Windows-specific)

| Secret | Required For | Value |
|---|---|---|
| `WINDOWS_SIGNING_CERT` | Direct distribution only | Base64-encoded .pfx certificate |
| `WINDOWS_SIGNING_PASSWORD` | Direct distribution only | PFX password |

---

## Runtime Requirements

eBIRForms requires:
- **DirectX 12** or **Vulkan** compatible GPU (for GPUI rendering)
- **Visual C++ Redistributable 2022** (usually pre-installed on modern Windows)
- Windows 10 version 1903 or later

---

## Troubleshooting

### SmartScreen warning "Windows protected your PC"

The executable is not code-signed. Options:
1. Purchase a code signing certificate (~$200-400/year from DigiCert)
2. Submit to Microsoft Store (free auto-signing)
3. Users can click "More info" → "Run anyway"

### App crashes on launch

Verify DirectX/Vulkan support:
```powershell
dxdiag
# Check "DirectX Version" and GPU capabilities
```
