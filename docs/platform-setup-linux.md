# Linux Platform Setup

## Distribution Channels

| Channel | Effort | Reach |
|---|---|---|
| GitHub Releases (tarball) | ✅ Zero setup | Anyone |
| Debian `.deb` package | ✅ Automated in CI | Ubuntu, Debian, Pop!_OS, Mint |
| Arch Linux AUR | Low effort | Arch, Manjaro, EndeavourOS |
| Flatpak / Snap | Medium effort | Broad (future) |

---

## Option 1: GitHub Releases (Automatic)

The release workflow automatically produces:

- `eBIRForms-Linux-x64-X.Y.Z.deb` — Debian package
- `eBIRForms-Linux-x64-X.Y.Z.tar.gz` — Portable tarball

No additional setup required.

### Installing from Tarball

```bash
tar xzf eBIRForms-Linux-x64-0.1.0.tar.gz
cd eBIRForms-Linux-x64-0.1.0/
./bir
```

### Installing from .deb

```bash
sudo dpkg -i eBIRForms-Linux-x64-0.1.0.deb
sudo apt-get install -f  # resolve dependencies

# Run
ebirforms
```

---

## Option 2: Arch Linux AUR

### Prerequisites

1. **AUR Account:** Register at [aur.archlinux.org](https://aur.archlinux.org)
2. **SSH Key:** Add your SSH public key to your AUR account

### Creating the PKGBUILD

Create a file `PKGBUILD`:

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
makedepends=(
    'rust'
    'cargo'
    'vulkan-headers'
    'libx11'
    'libxcb'
    'wayland-protocols'
    'pkg-config'
)
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

    # Assets
    install -dm755 "$pkgdir/usr/share/ebirforms"
    cp -r assets "$pkgdir/usr/share/ebirforms/"
    cp -r formtypes "$pkgdir/usr/share/ebirforms/"

    # License
    install -Dm644 LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
}
```

### Submitting to AUR

```bash
# 1. Clone your AUR package (first time)
git clone ssh://aur@aur.archlinux.org/ebirforms.git
cd ebirforms

# 2. Add PKGBUILD and .SRCINFO
cp /path/to/PKGBUILD .
makepkg --printsrcinfo > .SRCINFO

# 3. Commit and push
git add PKGBUILD .SRCINFO
git commit -m "Initial upload: ebirforms 0.1.0"
git push

# For updates:
# 1. Update pkgver in PKGBUILD
# 2. makepkg --printsrcinfo > .SRCINFO
# 3. git commit + push
```

### GitHub Secret for Automated AUR Updates

| Secret | Value |
|---|---|
| `AUR_SSH_KEY` | SSH private key registered with your AUR account |

---

## Runtime Dependencies

eBIRForms requires a working Vulkan stack on Linux:

### Ubuntu / Debian

```bash
sudo apt-get install \
    libvulkan1 \
    mesa-vulkan-drivers \
    libxkbcommon0 \
    libwayland-client0 \
    fontconfig \
    libfreetype6
```

### Arch Linux

```bash
sudo pacman -S \
    vulkan-icd-loader \
    vulkan-radeon   # or vulkan-intel, nvidia-utils
    libxkbcommon \
    wayland \
    fontconfig \
    freetype2
```

### Fedora

```bash
sudo dnf install \
    vulkan-loader \
    mesa-vulkan-drivers \
    libxkbcommon \
    wayland \
    fontconfig \
    freetype
```

---

## Troubleshooting

### "Failed to create Vulkan instance"

Your GPU drivers don't support Vulkan. Check:
```bash
vulkaninfo | head -20
# If this fails, install proper Vulkan drivers for your GPU
```

### AMD GPU
```bash
sudo apt install mesa-vulkan-drivers  # Ubuntu
sudo pacman -S vulkan-radeon          # Arch
```

### Intel GPU
```bash
sudo apt install mesa-vulkan-drivers  # Ubuntu
sudo pacman -S vulkan-intel           # Arch
```

### NVIDIA GPU
```bash
sudo apt install nvidia-driver-xxx    # Ubuntu (replace xxx with version)
sudo pacman -S nvidia-utils           # Arch
```

### "libxkbcommon.so: cannot open shared object file"

Missing Wayland/X11 libraries:
```bash
sudo apt install libxkbcommon0 libxkbcommon-x11-0 libwayland-client0
```
