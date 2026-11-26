# Icon v2 device proof — PR #380 (redrawn logo)

Built commit **05be9f4d** (feat/ios-store-icon-v2). The branch head moved to 26fc3f67 while building; that move only adds the logo's part files under assets/brand/parts/ and a recipe — every icon file is byte-identical at both commits (verified: `git diff 05be9f4d 26fc3f67 -- assets/ios assets/macos assets/AppIcon.icns assets/brand/buwiz-forms.svg` is empty). This proof branch is based on 26fc3f67. The PR has since merged into main as ab8d7526 with the icon files still byte-identical (verified: same empty diff against 05be9f4d) — **icons identical to main ab8d7526**, so these pictures show main's icons.

Tools: `nsh package` from nsh 0.6.1 with ../nsh at the pin 8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf; cargo 1.99.0 (rustup toolchain 1.99.0); one fresh build folder ~/BuwizNsh/target-iconv2 (deleted afterwards). macOS 26.6.2, iPhone 17 Pro simulator iOS 26.5. Both packages finished without errors (unsigned/ad hoc). Safety variables on throughout, GOOGLE_* unset, no sign-in, nothing queued. The iOS bundle's own AppIcon60x60@2x.png was verified before installing: opaque white corners, the redrawn white-tile master — the icon comes from nsh.json, not the NshDev host.

## Pictures
- iphone-home-light.png — iPhone 17 Pro home screen (page 2, where SpringBoard keeps the icon), light appearance: the new white tile with the redrawn logo; opaque corners, no black/transparent edges, single rounding, "Buwiz Forms" in the artwork.
- mac-dock-light.png — Mac Dock in light appearance at 2x: eBIRForms running (dot underneath) beside Chrome's stock squircle — white tile as drawn, same size as its neighbours, no legacy-icon frame.
