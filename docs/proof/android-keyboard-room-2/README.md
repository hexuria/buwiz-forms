# Job 16 — keyboard room: job-12 redo on certified isolated builds, overscroll, scroll-chain diagnosis

**Report header**

| piece | BEFORE | AFTER |
|---|---|---|
| app commit | `00a20033` (main) | `36505288` (#351) |
| sibling NSH checkout | `8c78cfd6` (`~/j16-before/nsh`, real worktree) | `8c78cfd6` (`~/j16-after/nsh`, real worktree) |
| `nsh` tool | `nsh 0.6.1 (8c78cfd)` (`tools-8c78cfd-bak`, in docker) | same |
| runtime classes.dex sha256 in the APK | `a53652883f334dbb2844bd2844fa5246097d2f49acddf068341c43702c46943c` (re-hashed from the packaged APK; `strings | grep -c NSHKBD` = 0) | same |
| CARGO_TARGET_DIR | `~/BuwizNsh/target-00a20033` | `~/BuwizNsh/target-36505288` |

Packaged with `nsh package --target android --abi x86_64` (the `--abi` flag is required: without it the x86_64-only runtime lib is silently skipped). Baked dev env: `EBIR_TEST_ENV=1;BUWIZ_NO_KEYCHAIN=1;BIR_SFTP_DRY_RUN=1` — the allowlist at these commits excludes `BUWIZ_SERVER_URL`/`BIR_DATABASE_PATH`, and a first bake with them crashed every screen with `Invalid dev.env line 4` (the mock is reached through the compiled-in fallback `DEV_SERVER_URL http://localhost:3008`, adb-reverse to the mock). Sign-in at this era is the **device flow**; the mock ran with `--auto-approve`. Phone: API 35 emulator, 1080×2400, **gesture mode set with `cmd overlay enable-exclusive com.android.internal.systemui.navbar.gestural`** (the pill is visible in the shots). Part 3 uses the job-15 instrumented build (app `fe1a5f46`, instrumented dex `4c1f4445…`, its own runtime folder) — single-commit diagnosis, per the job-15 ruling.

## 1. Job 12's core table redone (certified builds, keyboard up, gesture, portrait)

**The chooser row is NOT stuck in either build.** This contradicts job 12's BEFORE result, which belonged to the shared-`CARGO_TARGET_DIR` builds — exactly the failure mode the one-folder rule exists for.

| case | BEFORE 00a20033 | AFTER 36505288 |
|---|---|---|
| Choose forms 2551Q row, keyboard up | moves y1598→930 on the first swipe (end of scroll); fully above the keyboard (top 1517) | moves y1598→683→336 over two swipes; fully above the keyboard |
| New taxpayer, Email + Create with keyboard up | initially under the keyboard (Email y1713–1823, Create y1908–1963); after swipes Email y1045–1155, Create y1240–1295 — both fully reachable | same reachability; end state Email y181–272, Create y357–412 |
| Settings last row | **not measured — see gaps** | not measured |

## 2. Overscroll — the owner's padding+spacer hypothesis is CONFIRMED

End-of-scroll state with the keyboard verified up (`mInputShown=true` at measurement time), gentle swipes to the scroll end, then pixel-scan at x=540 from the content end to the keyboard top (~y1512):

| page | BEFORE blank gap | AFTER blank gap |
|---|---|---|
| Choose forms | content ends y1084, white to y1512: **428 px blank** | content ends y488, white to y1512: **1024 px blank** |
| New taxpayer | Create bottom y1295, gap to keyboard **222 px** | Create bottom y412, gap **~1100 px** — the whole form compressed into the top of the viewport |

Content shift between rest and end-state: BEFORE 668 px (exactly NSH's padding), AFTER 1262 px (padding + ~594 px spacer). In AFTER the page can be scrolled a second keyboard-height past its own content end — the regression the owner must not ship is real on the phone, not only the tablet artifact that prompted the job. Evidence: `before/after-chooser-overscroll.png`, `after-chooser-endstate.png/xml`, `before/after-newtp-*endstate*`.

Method note: hard 80 ms flings sometimes **dismiss the keyboard** (the first AFTER attempt ended with no keyboard — `after-chooser-overscroll.png`, which also shows the page snapped back to scroll-zero). All reported end-states use gentle swipes with `mInputShown` verified true at measurement time.

## 3. Scroll-chain diagnosis (instrumented runtime, app fe1a5f46, Choose forms, "2551q", keyboard up)

From `p3-chooser-kbd.xml` and the NSHKBD lines in `p3-logcat.txt`:

- **There is exactly ONE ScrollView in the whole tree**: the child of `FrameLayout[main-scroll]`, bounds [0,181][1080,2122]. The chain of the 2551Q row: row → `tp-form-2551Q` → `tp-forms-list` → `tp-forms-tab` → `tp-page-forms` → `content-column` → two anonymous ViewGroups → **the ScrollView** → `main-scroll`. The search field's chain (`EditText` → `tp-forms-search` → three ViewGroups → `tp-forms-tab` → …) **joins the same ScrollView** — row and search field share it.
- The padding goes to that same ScrollView: `inputFocused input=3254 focused=true scroll=2920`, `setContentPaddingBottom scroll=2920 px=605` — the only scroll container the padding could land on, and it does.
- Yet in this state the row scrolls up fine (measured 1598→993 in three-button mode during job 15's variants; and the certified BEFORE build here scrolls it to 930). The immovability job 11/12 saw is therefore not structural in the normal path — consistent with job 15's intermittent, state-dependent reading. The honest residual: the stuck state exists but is rare/state-dependent, and the chooser's real defect this job establishes is the AFTER overscroll, which is structural and reproducible.

`keyboard-room` testID: **not present** in the Choose forms uiautomator dumps of either build in the states captured (matching job 12's note that it is only sometimes exposed).

## 4. Tablet (Pixel Tablet image, landscape via `user_rotation 1` — the rotation takes)

Rotation worked (2560×1600 landscape, screenshot proof). Keyboard top in landscape ≈ y860. In BOTH builds the 2551Q row initially straddles the keyboard edge (y834–947), matching job 12's tablet photo. But the landscape measurement is **inconclusive and honestly reported as such**: the content panel's scroll container is not a uiautomator `ScrollView` — the only one in the tree is the narrow left sidebar [16,612][480,1392] — and swipes on the panel did not produce a measurable, verifiable scroll in either build (the row leaves the visible tree rather than arriving at a stable end-state). The end-state dumps are byte-identical across BEFORE/AFTER (`forms-tab` [704,472][2336,1038] in both). Whether the overscroll regression reaches the tablet panel needs a deeper look (probably coordinate-driven scrolling inside the panel with screenshot verification per step); the phone evidence above stands on its own. Portrait on the tablet was not reached in the time available. Files: `tab-before/after-chooser-*.png/xml`.

## 5. Gaps and honesty notes

- Settings page overscroll: not measured. At this commit the Settings entry (the avatar's account menu) did not open reliably under scripted taps; documented rather than faked.
- Case "form page" is outside this job's pages per the spec.
- Hard flings can dismiss the keyboard; the first AFTER chooser attempt is kept as `after-chooser-overscroll.png` precisely because it shows the artifact (no keyboard, page snapped back).
- The tablet creation flow mangles the scratch name under fast scripting (`MariaClara to y1RizalStd1000j001`); scratch data, measurement unaffected.
- Video: `before-pass.mp4` covers the BEFORE phone pass (sign-in through overscroll). The AFTER pass ran identically scripted; screenshots + dumps are the per-state evidence.
