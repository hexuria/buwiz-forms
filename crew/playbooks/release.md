# Playbook: pins, tags and releases

Who may move what, and in which order. `docs/VERSIONING.md` covers the app's version and build numbers in full; `crew/REPOS.md` lists where each pin is read.

## Pins

The app names each dependency it is built on by an exact value. Read the current value from the file every time; never from memory or another document.

| What | Where the app pins it | Moved by |
| --- | --- | --- |
| The NSH runtime fork | `NSH_PINNED_REF` in `.github/workflows/nsh.yml` | the `nsh` role, in its own app pull request |
| gpui-agent (the driver) | `gpui-agent` version in `Cargo.toml` and in `nsh/Cargo.toml` | the `driver` role |
| nsh-gpui-agent (the plug) | the plug's `rev` on every platform dependency in `nsh/Cargo.toml` (all the same) | the `driver` role |
| gpui-agent-cli on machines | the version `Cargo.lock` pins; the setup scripts install it | follows the `gpui-agent` move |

A pin move:
1. goes plug first, then the app: the driver releases, the plug moves to the new driver, the app moves to the new plug rev;
2. has its lock files written by cargo on a machine with the sibling checkouts, never by hand;
3. checks every other manifest that names the same crate, so no two pins disagree;
4. is proven with the full suites on every system, then `just ci-main` after it merges so the caches catch up.

## Tags

| Tag | Where | What it does | Who cuts it |
| --- | --- | --- | --- |
| `vX.Y.Z` | hexuria/gpui-agent `main` | publishes the driver's crates (`publish.yml` there) | the conductor, after the release pull request merges |
| `vX.Y.Z` | hexuria/buwiz-forms `main` | builds, signs and publishes the downloads, and builds the unsigned `.msix` | the maintainer |
| `bir-vX.Y.Z` | hexuria/buwiz-forms `main` | archives the headless `bir` command | the maintainer |

A tag on this repository publishes to every user, so it stays with the maintainer. Workers never push tags; the allow-list refuses them.

## The app release, in order

1. Every pull request planned for the release is merged, and `main` has one complete green run.
2. The version pull request (`python3 scripts/set_release_version.py X.Y.Z`, then the tests it lists) lands last.
3. The maintainer tags `vX.Y.Z` on `main`.
4. The Apple packages are built and uploaded on the maintainer's computer with the maintainer's keychain (`just app`, `just ios-appstore`, then their upload recipes). Never on a cloud machine.
5. The `.msix` from the tag's run is uploaded in Partner Center by the maintainer.
6. The maintainer presses Submit in each store.
7. Google Play follows as its own release: `just play-bundle` builds, the maintainer signs and uploads.

The crew prepares every step and checks it. Every upload, signature and Submit is the maintainer's.

## Production

Deploys of the account server to production stay with the maintainer. The crew prepares the change, its tests and a deploy note. No crew member sends anything to the production relay or the real BIR, before or after a release.
