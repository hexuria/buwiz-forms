# bir

Headless eBIRForms agent CLI. It serves the gpui-agent control plane over TCP
with no desktop window.

```bash
cargo install bir
cargo binstall bir
```

Prebuilt binaries are attached to GitHub Releases tagged `bir-vMAJOR.MINOR.PATCH`
(for example `bir-v0.1.0`) in this repository:

- `bir-x86_64-unknown-linux-gnu.tgz`
- `bir-aarch64-apple-darwin.tgz`

Each archive contains the `bir` executable at its root. A `.sha256` file is
uploaded next to each archive.

The desktop app is a different release. Tags `vMAJOR.MINOR.PATCH` still build
the eBIRForms dmg, deb, and exe. That app's cargo target is `bir-desktop`; the
installed application executable stays `bir` inside the app bundle.

`gpui-agent` is not published from this repository.
