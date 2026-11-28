# Versions and build numbers

One version number names a release everywhere: the downloads, the Mac App Store, the App Store for iPhone and iPad,
the Microsoft Store and Google Play. This page says where that number lives, what each store adds to it, and how to raise
it in one step.

## The two numbers

| Number | Looks like | Where it lives | Who changes it |
| --- | --- | --- | --- |
| Version | `1.0.0` | `nsh/nsh.json` (`version`), the workspace `Cargo.toml`, `nsh/Cargo.toml`, `crates/bir-print/Cargo.toml`, and the lock files | `scripts/set_release_version.py`, never by hand |
| Apple build number | `27` | Nowhere in the repository for a store build: `just app`, `just ios-appstore` and `just play-bundle` ask App Store Connect for the highest build it has and use the next one (`scripts/bump_build.py --print-next`) | The recipe, at packaging time |

`BUILD_NUMBER` in the `justfile` is only the fallback for a package made without asking App Store Connect.

## What each store shows and requires

| Where | What the user sees | What the store compares | Rule |
| --- | --- | --- | --- |
| Downloads (the website and the release page) | `1.0.0` | The tag `v1.0.0`; the in-app updater compares versions | A new tag for every release |
| Mac App Store | `1.0.0` (`CFBundleShortVersionString`) | The build number (`CFBundleVersion`) | Every upload needs a higher build number, even when the version is the same |
| App Store, iPhone and iPad | `1.0.0` | The same build counter as the Mac (one App Store Connect app, one counter) | The same |
| Microsoft Store | `1.0.0` | The package version `1.0.0.0` | Every upload needs a higher package version than the one before |
| Google Play | `1.0.0` (`versionName`) | `versionCode`: the Apple build number | Every upload needs a higher `versionCode` than any Google Play already has, on any track |

The Microsoft Store package has no separate build number. `scripts/package_nsh_windows.ps1` passes the version's
third part to `scripts/package_msix.ps1`, which writes `Major.Minor.Patch.0`; the Store requires the fourth part to
be 0. So for Windows the version itself is the only thing that can go up.

Google Play's `versionCode` is the Apple build number, so that one counter serves every store that has one:
`just play-bundle` takes App Store Connect's next number, as the Apple recipes do. Google Play does not know App Store
Connect, so two Google Play uploads with no Apple upload between them would get the same number, and Google Play
refuses the second. Then pass the number yourself, one higher: `just play-bundle 28`
([platforms/android.md](platforms/android.md)).

## Raising the version

```bash
python3 scripts/set_release_version.py 1.0.0
```

That one command:

1. writes the version into every manifest listed above, or into none of them if one is not as expected;
2. refreshes both lock files;
3. regenerates `crates/bir-rules/src/generated/manifest.json`, which carries the generator's version, and runs the
   rules check (CI's `validation-rules-v2` job fails on a stale manifest);
4. lists the tests that still spell out the old version, so they can be changed in the same commit. The app's own
   tests compare the running version with a written one in a few places.

If any step fails, every file is put back as it was.

It needs the NSH checkout beside this repository at the commit named by `NSH_PINNED_REF` in
`.github/workflows/nsh.yml`, because `nsh/Cargo.lock` is refreshed against it.

## Which number to raise, and when

| Situation | Version | Apple build |
| --- | --- | --- |
| A new release | Raise it, once, for every store | The next one, taken at packaging time |
| A store rejected the release and the code changes | Raise the patch number (`1.0.0` to `1.0.1`) everywhere, because the Microsoft Store has nothing else to raise | The next one |
| Apple needs the same code uploaded again (a packaging or signing mistake) | Unchanged | The next one; nothing changes for Windows |
| A change that is not released | Unchanged | Unchanged |

A release is one version everywhere. Do not let the Mac ship `1.0.1` while Windows is still on `1.0.0`: the next
Windows upload would then have to skip a number, and the downloads' updater would disagree with the stores.

## Order of work for a release

1. `python3 scripts/set_release_version.py X.Y.Z`, fix the tests it lists, open a pull request, merge it.
2. Tag `vX.Y.Z` on `main`. The release workflows build the downloads and, in a job of its own
   (`build-windows-store`), the unsigned `.msix` for the Microsoft Store. That job has the Google OAuth client, and so
   Gmail Connect, only when the repository variable `STORE_GMAIL` is `1`: Connect's restricted Gmail scope needs
   Google's verification. Otherwise it checks that the package holds no client id
   (`scripts/audit_google_client.py`). The direct downloads keep the client.
3. `just app` builds the Mac App Store package and `just ios-appstore` the iPhone and iPad one, each with the next
   build number; `just testflight-upload` and `just ios-appstore-upload` send them.
4. Upload the `.msix` in Partner Center.
5. Press Submit in each store.
