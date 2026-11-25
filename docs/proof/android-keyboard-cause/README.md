# Why content stays under the keyboard (job 6)

Phone emulator, API 35, keyboard docked (Gboard default, top edge at y=1517 on the 1080x2400 screen).
No app code and no NSH code changed. One log per step; recordings with taps shown.

## Part A — in the app

Build used: the PR #345 development build at `b51fabe4` (the same APK as job 5, reinstalled).

### A1 Choose forms search (`K-A1-chooser-scroll.mp4`, `A1-dump-*.xml`, `a1-*.png`)

With "2551Q" typed in Choose forms, the result row sat at bounds `[160,1720][339,1771]` — entirely below
the keyboard's top edge (1517). The prescribed swipe `input swipe 540 1450 540 700 400` (starting above the
keyboard) was applied three times: the row's bounds after the swipes are byte-identical — the page does not
move AT ALL and the row never comes into view above the keyboard. (With a single result there is also no
overflow to scroll; the scroll view itself never shrinks above the keyboard.)

### A2 New taxpayer (`K-A2-new-taxpayer.mp4`, `A2-dump-scroll-limit.xml`, `a2-*.png`)

Registered name tapped, keyboard up, swiping WITHOUT closing it: the page DOES scroll by hand — Phone reached
`[87,1427][993,1478]` (barely above the keyboard) — but the scroll hits a hard limit there: swipes 2–6 change
nothing, and Email (`y1647–1698`) and the Create taxpayer button (`y1908–1963`) stay UNDER the keyboard,
unreachable. Then tapping Phone: the page does NOT move by itself — positions are byte-identical after the tap
and the Phone input row stays cut off at the keyboard edge (`a2-phone-tap.png`).

### A3 Sign-in email page (`K-A3-signin-swipe.mp4`, `A3-dump-*.xml`)

Same swipes on the card: nothing moves — the Email field (`[108,1289][972,1340]`) and Continue with email
(`[379,1538][701,1593]`) keep byte-identical bounds. As expected: the card is not inside a scroll view.
(Continue stays under the keyboard; the keyboard's enter arrow is the only way to submit.)

## Part B — the kit by itself at the pin

NSH checkout `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf` (~/repos/nsh-pin), CLI `nsh 0.6.1 (8c78cfd)` (the backup
at `~/BuwizNsh/tools-8c78cfd-bak/bin/nsh` from job 3), Android dev runtime built at the pin with
`cargo xtask android --dev --abi x86_64` in the buwiz-nsh-2404 sidecar (`xtask-pin-dev.log` — "built libnsh.so
for x86_64", "compiled the Java runtime"). Scratch folder `~/kit-keyboard-test` OUTSIDE both repos: `nsh init`
there first (the CLI needs a project: it wrote `nsh.json`, `tsconfig.json` and a template `App.tsx`), then the
spec's file copied over `App.tsx` verbatim — **no fixes were needed; every import and prop name is valid at the
pin** — and `nsh run --target android` (`nsh-run.log`: built, installed, launched on emulator-5554).
The exact file is committed as `kit-test-App.tsx`. Evidence: `K-B-kit-test.mp4`, `kit-*.png`, `B*-dump-*.xml`.

### B1 — WORKS

Field A tapped, keyboard up: one swipe and Row 9–12 AND Field B are all fully visible above the keyboard
(Field B at `y1105–1148`, keyboard top at 1517). The scroll view gets its bottom padding and the content
scrolls freely (`kit-b1-swipe1.png`).

### B2 — WORKS

Scrolled to the end with the keyboard down, then Field C tapped: the page moves BY ITSELF — Field C jumps from
`y2016–2059` to `y1442–1485`, just above the keyboard, focused (`kit-b2-fieldc-focus.png`). The Last button is
then reachable above the keyboard with one small scroll (`y1432–1475`, `kit-b2-lastbutton.png`).

### B3 — hidden under it

The Bottom bar outside the scroll stays at `y2263–2306` the whole time — underneath the keyboard, invisible
while it is up. Nothing lifts it: only the scroll view AROUND THE FOCUSED FIELD gets the keyboard padding.

## The decisive comparison (`hierarchy-evidence.txt`)

The app's fields are real native EditTexts inside the page's one ScrollView — the same shape as the kit —
yet in the app no padding is ever added and revealFocused never fires, while in the kit both work on the
same device, same runtime, same API 35.

## Conclusion (one sentence)

NSH's keyboard mechanism (Mounter.keyboard + revealFocused) works exactly as written on API 35 at the pin —
so the cause is inside the app's own pages: they never give it a live focused input inside the right
ScrollBox (or re-lay the page so the padding is dropped), even though their fields sit inside a scroll view.
