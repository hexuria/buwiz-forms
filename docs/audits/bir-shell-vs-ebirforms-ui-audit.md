# bir-shell vs hexuria/ebirforms — headed visual/UX audit

Audit date: **2026-11-01 UTC**. This replaces the provisional shell-only report. The user-confirmed comparator is **hexuria/ebirforms**, not the official BIR offline package and not the old Rust desktop app.

**The paired audit procedure was executed; this is not an all-code-correctness or merge-readiness guarantee.** All 16 requested modules are classified below. Genuine fresh-state/onboarding, empty and populated pages, a representative October 2026 draft, matched default geometry, narrow behavior, scrolling and themes were observed. Some modules have no reference equivalent. Fully valid returns, populated submitted history, secret-bearing security transitions and updater banners/install remain untested.

## 1. Executive results

**Matched-size supplement:** reused existing isolated synthetic shell/reference stores, **not another fresh run**. Added exact **1440×900 content PNGs** for shell library All/filter, period chooser, taxpayer Profile/Forms and empty Filings. Cancel dismissed the period chooser without a new save. Added genuine **2560×1856 reference form evidence**, matching original shell12. Existing 22-second recording unchanged; supplemental flows are screenshot evidence.

**Heavy-form follow-up:** [section 9](#9-heavy-annual-forms--1700-family-full-scrolling) adds headed 1700/1701/1702RT/1702MX scrolling and UI draft saves on shell `d65a473c`, with unchanged app code. All four reference annual variants explicitly reject editing; their scrolling remains blocked. A **medium shell form-card clipping issue** is visible with the right panel open. This is not a clean bill of health or a matched annual-editor comparison.

- **Right-panel width is equal, not larger in bir-shell:** both render a fixed **320px** panel. Both left sidebars render **248px**.
- **The genuine padding delta is 4px per side:** shell taxpayer-panel branch control is **296px** wide with **12px** body padding; reference is **288px** with **16px**. Thus shell controls occupy 8px more width without a wider panel.
- The reference has an **8px same-background gutter** immediately before its 320px panel. Measuring the whole apparent right-colored region produces 328px; that is not the panel width. Nested group-label inset is also not body padding.
- At matched **800×900** content size the shell uses a dismissible right overlay; the reference retains docked side regions and visibly clips its Calendar content. Manual sidebar collapse improves reference usable space. This is a reference limitation/difference, not a shell responsive regression.
- The shell includes cloud auth, device approval, computer setup, security and updater surfaces that the local-store reference does not implement as equivalent pages. Do not manufacture before/after pairs for them.
- Small light-theme chrome labels use a token pair calculating **4.400:1** contrast, below 4.5:1 for normal text. This is a shared palette risk, with actual small gray shell labels observed; the ratio is token-derived, not sampled glyph contrast.
- Reference form selection first hit an explicitly unsupported catalog variant. Selecting the supported `1601-C` variant opened the editor. Draft save later produced a visible **“Saved. (Template update prompt arrives with guided mode.)”** toast. Validation still reports incomplete synthetic profile/form data; a fully valid return was not established.

## 2. Revisions, authentic state, safety and setup

| Component | Audited revision |
|---|---|
| `hexuria/buwiz-forms` | `b833bdccccb1f792974ae7d481aae6023a9cf05f` — initial HEAD and fetched main matched |
| Local `buwiz-com/buwiz-com` backend | `625ae30b4b05b7ec5bbef4703fc9910eeda43568` |
| `hexuria/ebirforms` latest main fetched for this audit | `14fe8efb29e171cac4e794d0d1ee9afaf1069310` |
| Reference `vendor/buwiz-forms-html` | `0e2691e528ad9f55ff4bae101e4eb34db1595156` |
| Reference `vendor/buwiz-validation` | `00fbf3de41742478560fc656fae11ff3e7a6abd5` |

### Shell

Read backend README/configuration/smoke script; no backend justfile exists. Built the actual `buwiz-cloud` server (1m43s), isolated PostgreSQL 17 container `bir-ui-audit-postgres-20261101` on loopback 54330, server **127.0.0.1:3008**. Read shell development docs/justfile and built only:

```text
cargo build --locked --manifest-path crates/bir-shell/Cargo.toml --bin bir-shell
```

Build took 7m45s. Launched the JavaScript application using `app/main.js` and `app/gpui-shell.json`. **The retired painted target was neither built nor used.**

Fresh HOME `/tmp/bir-shell-ui-audit-20261101/shell-home`, independent DB/account-file paths, private D-Bus and browser profile were absent before first launch: [shell absence log](../../evidence/bir-shell-ui-audit/fresh-state.txt). Fresh backend counts were accounts 0, web_sessions 0, oauth_grants 0, tin_claims 0. Captured genuine auth gate and setup before adding a taxpayer. Used native Create account → local email-code verification → device approval → solo setup; no bypass or fixture.

**Backend connectivity was verified before the page audit:** shell PID 96089 had an ESTABLISHED socket to 127.0.0.1:3008; server recorded POST `/auth/device/start` 200 and GET `/me` 200. [Socket evidence](../../evidence/bir-shell-ui-audit/local-socket-evidence.txt), [safe request metadata](../../evidence/bir-shell-ui-audit/local-request-evidence.txt). No authenticated curl flow was used.

### Confirmed TypeScript/Bun/GPUIX reference

Read README, package files and `scripts/setup-unix.sh`; installed Bun **1.4.4**, dependencies and pinned submodules; GPUIX **0.10.0**. Host GLIBC 2.35 could not load the package requiring GLIBC 2.38/2.39. Direct X11 native launch logged window creation without a visible managed app. Resolved without app changes using an **Ubuntu 24.04 Docker compatibility runtime**, read-only source mount, UID 1000, host IPC and nested Weston/Wayland. Root/SHM and initial input-seat attempts were setup failures, not application regression claims.

Fresh headed `/tmp/ebirforms-paired-audit-20261101/headed-data`, headed-home and database were verified absent before first launch: [reference absence log](../../evidence/bir-shell-ui-audit/paired/reference-fresh-state.txt). `EBIRFORMS_SEED` and `BIR_AUTH_BYPASS` were unset. Captured untouched Home, Calendar, Filings and “No taxpayer” panel before entering data. The reference starts directly in local Home; there is no cloud-auth or computer-owner setup gate.

Weston kiosk-shell allocated the reference **1440×900** visible surface because reference caption maximize controls are Windows-only (`apps/desktop/src/app.tsx:1453–1459`). The initial fresh Home was preserved before a compositor reconnect; the same empty store was reused with no taxpayer added until onboarding. Host clipboard paste did not reliably bridge to nested Wayland; after app reconnect, direct keyboard entry worked (`xdotool type --clearmodifiers --delay 50`). No fixture was substituted for UI entry.

### Safety and cleanup

- Only synthetic local account `audit@example.test`, TIN **123-456-788-00000**, taxpayer **SYNTHETIC AUDIT ONLY** and synthetic address were used.
- All inherited `BIR_SFTP_*` variables were stripped on each app launch; only `BIR_SFTP_DRY_RUN=1` set. Live transport remained unset. Reference Settings visibly showed **“SFTP: dry run”**: [capture](../../evidence/bir-shell-ui-audit/paired/ref-22-settings-dry-run.png).
- No production account/data, live send, queue/send action, Final Copy, update installation, PIN/TOTP/recovery-key generation or authenticated backend API shortcut.
- The apps/Weston/reference containers were stopped after capture. Local backend PID 36063 and isolated PostgreSQL container were left available for the lead.
- Runtime regenerated nine declarations in tracked `app/shell/gpui-kit.d.ts`; [exact generated-only diff](../../evidence/bir-shell-ui-audit/runtime-generated-types.diff) preserved. After shutdown these additions were restored; `git diff --exit-code -- app crates` verified no application-code changes. Reference working tree remained clean. Only audit documentation/evidence is included.

## 3. Coverage matrix

**Observed** means only the named visible states were exercised. **Partial** means substantive cases remain untested. **Absent-equivalent** means source routes and actual navigation do not offer a matching reference surface, not that launching the reference was blocked.

| Requested module | Shell runtime | Reference runtime/equivalence | Remaining limit |
|---|---|---|---|
| auth | Fresh gate, genuine local signup/code/device approval | Absent-equivalent; fresh local Home | Expiry/cancel/re-login untested |
| setup_change | Initial solo setup; change-owner/help-desk choices opened/cancelled | Absent-equivalent | Ownership handover/help-desk completion untested |
| home | Empty then populated | Fresh empty then populated | Submitted rows untested |
| form_page | Sample → October 2026 draft → Recent reopen; scroll; section 9 adds four annual editors/selected expanded sections | Supported 1601-C October 2026 editor; validation; scroll; save toast; all four annual variants explicitly not editable | Annual reference comparison blocked; all fields simultaneously expanded/calculations untested; shell clipping in section 9 |
| filings | Empty; local draft not counted as filed | Empty before draft | Submitted tables/long rows untested |
| periods | Monthly save dialog; October 2026 retained; section 9 annual TY2026 and year-ended12/2026 saves/reopens | Start-return monthly modal; October 2026 retained; annual TY2026 choices rejected as unsupported | Duplicate/amended period flows and supported reference annual save untested |
| calendar_page | Empty and populated Nov/Dec/Jan groups | Empty and populated groups; narrow clipping observed | Deadline/holiday correctness untested |
| recent | Sidebar creation and reopen | Sidebar creation and reopen | Large history/privacy/preference permutations untested |
| gallery | Components top and bottom scroll | Absent-equivalent | Not a library comparison |
| library | Empty selected-taxpayer filter; All forms 43 cards; sample; matched All/search supplement | Nearest equivalent: taxpayer Forms year/catalog/filter/selection | Catalogs/workflows differ; not identical pages |
| taxpayer | Validation, creation, profile scroll, 2026 form set; matched Profile/Forms supplement | Validation, creation, profile, Forms selection | Full reference address/RDO not completed; branch/history/import untested |
| settings | General/Account/Computer; light/dark | Settings light/dark; Data/dry-run/empty tasks | Restart persistence/import/export/job execution untested |
| security | Unset state, empty Set PIN validation | Absent-equivalent; explicit profile placeholder | PIN/TOTP/recovery/lock transitions untested |
| update | Automatic-download control only; source state inspection | Absent-equivalent | No shell banners/install/runtime phases |
| account | Local cloud account/server status and owner menu | Nearest equivalent: local display-name/email preferences | No reference cloud auth; portal/signout untested |
| chrome/gates | Docked panels, palette/menu, empty notifications, narrow overlay | Docked panels, populated notifications, narrow clipping/manual collapse | Exact 860 boundary, focus traps, native-platform parity untested |

## 4. Geometry: measured pixels, not preview or source assumptions

Desktop remained **2560×1920 at 60Hz**, unchanged. Tool previews at 1024×768 are not measurement pixels. PNGs were cropped from genuine native desktop captures; crops exclude compositor/window chrome and shadows but do not reconstruct app content.

### Matched final captures

| Capture group | Actual PNG dimensions | Visible app content / native allocation | Left | Right |
|---|---|---|---:|---:|
| `paired/shell-01`–`03`, `05`–`11` | **1440×900** | 1480×940 outer allocation minus 20px shadow/frame each edge; shadow-only crop | 248px when shown | 320px when shown |
| `paired/ref-01`–`15`, `18`–`23` | **1440×900** | Weston surface 1440×900, decoration excluded | 248px when shown | 320px when shown |
| `paired/shell-04` | **800×900** | 840×940 outer allocation minus shadow | auto-hidden | 320px overlay |
| `paired/ref-16`–`17` | **800×900** | Weston surface 800×900 | docked in16, manually collapsed in17 | docked 320px in16 |
| Original shell `02`–`12`, `15`–`18` | **2560×1856** | maximized app content 2560×1856 | 248px when shown | 320px when shown |
| `paired/ref-24` | **2560×1856** | genuine resized Weston X11 client x0/y0, fully visible on unchanged desktop | 248px | 320px |
| Original shell `01`, `23`–`33` | **1480×940** | includes 20px shadow/frame each edge; **1440×900 content**, not 1440×900 PNGs | state-dependent | state-dependent |
| Original shell `13`, `19`–`21` | **1520×980** | includes shadow/frame; **1480×940 content**, not matched default | state-dependent | state-dependent |
| Original shell `14` | **840×940** | includes shadow/frame; **800×900 content** | auto-hidden | overlay |
| Original shell `22` | **1440×900** | includes shadow/frame; **1400×860 content**, intermediate probe | 248px | 320px |

Every PNG's actual dimensions were inspected with PIL: [per-file PNG/content inventory](../../evidence/bir-shell-ui-audit/paired/png-dimensions.tsv). Originals retained unchanged; no screenshot scaled or reconstructed. Filenames saying “default” do not override geometry. Fresh shell setup/onboarding `02`/`03` remain original wide captures against 1440×900 reference onboarding: **unmatched-size limitation**, not exact spacing/modal-size parity. Later matched taxpayer Profile/Forms do not re-establish fresh onboarding.

### Exact panel observations at 1440×900

Native crop origins differ by one border pixel; do not compare absolute screen x-coordinates across windows. The rendered widths/inner rectangles are:

| Measurement | Shell | Reference | Delta |
|---|---:|---:|---|
| Left sidebar background region | 248px | 248px | 0 |
| Sidebar horizontal inset / inner row | 8px / 232px | 8px / 232px | 0 |
| Fixed taxpayer panel | 320px | 320px | 0 |
| Selected branch control | 296px | 288px | shell +8px |
| Taxpayer body horizontal padding | 12px per side | 16px per side | shell −4px per side |
| Pre-panel same-background gutter | not the reference's extra 8px strip | 8px outside panel | visual region ≠ panel width |
| Safe divider drag held/released | no observed resize | no observed resize | fixed-width behavior |

In cropped Home images, shell branch control is x1131–1426 inclusive (296px); reference x1136–1423 inclusive (288px). The latter leaves 16px to the 1440px edge. Reference right background appears 328px wide because it includes the preceding 8px gap. Group-label glyphs have their own inset; measuring their first ink pixel would overstate body padding. Divider drag during reference gesture is preserved in [ref-18](../../evidence/bir-shell-ui-audit/paired/ref-18-divider-drag-held.png).

At wide/default sizes both apps kept fixed sides while main content expanded. Original shell12 and supplemental reference24 are both genuine **2560×1856 content**. On native row y1400, shell background runs x0–247 (248px) and x2240–2559 (320px); reference left run is x0–247 and right-colored run x2232–2559 (328px including the measured8px exterior gutter). This confirms fixed widths rather than a wider shell panel. Reference's form occupies a substantially narrower column within the expanded main region than shell12; section/validation state differs, so this is **not a field-density/font-metric comparison**. Reference initially showed “Opening official form…”; clicking Validate rendered the existing incomplete-data errors. No new save occurred. At800px shell opening the right toggle displays an overlay/scrim; reference stays docked/clips, with Ctrl+B manual collapse available. Exact859/860/861 boundary and focus trapping untested.

### Source declarations (separate from measurements)

- Shell `app/shell/ui.js:106–137`: sidebar 248, panel 320, title bar 40, mainInset 8, narrow breakpoint 860; `:1228–1275`: fixed panel, body padding 12, header left 20/right 12. `app/shell/chrome.js:289–301`: sidebar padding 8.
- Shell `app/main.js:1722–1733`: deduct side widths and 2px, main horizontal padding 16 below mainWidth 640 else 32. Arithmetic at viewport 1440 with both sides gives mainWidth 870; at 860 gives 290. These are **source arithmetic**, not additional measured screenshots. `:257–287`: 200ms resize settling.
- Reference `apps/desktop/src/app.tsx:1493–1503`: sidebar 248/padding 8; `:1656`: notifications 320, header left 16/right 12; `:1724–1725`: taxpayer panel 320/body padding 16. `:3413–3440`: main rounded white region, 8px exterior margin, 28px vertical/32px horizontal padding. Initial window 1440×940 at `:3473–3475` was overridden by measured kiosk allocation.

### Typography and navigation density — source-only numeric values

Declared sizes below are **not measured glyph bounds or computed font metrics**. Linux fallback/renderer differences remain a limitation.

| Token/role | Shell `app/shell/ui.js:86–99` | Reference `apps/desktop/src/app.tsx:91–99` | Declared size delta |
|---|---:|---:|---:|
| Page heading | 20px / weight 600 | `F_PAGE` 20px | 0px |
| Section title | 16px / weight 600 | `F_TITLE` 16px | 0px |
| Emphasis | 14px / weight 600 | `F_EMPH` 14px | 0px |
| Body | 13px / weight 400 | `F_BODY` 13px | 0px |
| Control/nav | 13px / weight 500 | `F_BODY` 13px; nav active 600/inactive 400 | 0px; weights differ |
| Metadata | 12px / weight 400, metaStrong 500 | `F_META` 12px | 0px |
| Caption | 11px / weight 500 | `F_CAP` 11px | 0px |

Reference font stack declares `Segoe UI Variable Text, Segoe UI, Bricolage Grotesque, sans-serif`; registration can fall back to system fonts (`app.tsx:67–99`). Exact rendered-family parity is not established.

| Primary navigation source metric | Shell | Reference |
|---|---|---|
| Row height / horizontal inner padding | 32px /8px (`ui.js:337–365`) | 32px /8px (`app.tsx:1467–1488`) |
| Icon-to-text gap | 10px | 9px |
| Primary row gap / top gap | 2px /12px (`chrome.js:260`) | 1px /4px (`app.tsx:1533–1540`) |
| Primary order | Home → Form library → Calendar → Filings | Home → Calendar → Filings; Settings at bottom |
| Stack arithmetic, source-only | Four 32px rows + three 2px gaps = 134px | Three 32px rows + two 1px gaps = 98px |

The extra shell route plus 1px inter-row-gap delta—not a wider sidebar—accounts for the taller primary-navigation block. Recent's group-label spacing is additional to this arithmetic.

## 5. Findings and recommendations

Severity applies to the stated issue, not untested code. Reference paths below are in `hexuria/ebirforms`; shell paths are in `hexuria/buwiz-forms`.

### M1 — Medium: normal small-label contrast risk in light chrome

Observed small gray **Open**, **Branch** and **Recent** labels on the actual light shell. `GroupLabel` uses metaStrong 12px/500 with `fg3` (`app/shell/ui.js:370–372`), called by `app/shell/chrome.js:272,569,578` over the panel/sidebar background. Source tokens `#71717a` on `#f4f4f6` calculate **4.400:1** using WCAG sRGB luminance, below normal-text 4.5:1. The reference uses the same base light palette; this is not evidence that the shell uniquely introduced it.

**Expected/recommendation:** darken normal small-label foreground or use a stronger token; verify both themes and selected/hover states. Shell owner: `app/shell/ui.js:23–35,88–99,370–372`, `app/shell/theme.js`; reference `apps/desktop/src/theme.ts`. Ratio is **token-derived**, not an anti-aliased glyph-pixel measurement. Other calculated risks (light hover 4.062, selected 3.743; dark selected 4.286) are **not independently verified offending runtime text/background combinations**.

### M2 — Medium, reference-side: docked narrow layout clips Calendar

At matched **800×900**, reference Calendar plus taxpayer panel leaves an unusably narrow center and visibly cuts headings/rows. Shell auto-collapse/overlay preserves a readable main surface and offers a scrim. [Paired evidence below](#narrow-calendar--800900).

**Expected/recommendation:** preserve shell's responsive approach; if comparator convergence is desired, adapt the reference rather than copying its clipping. Shell responsible behavior: `app/main.js:257–287,1722–1733`, `app/shell/ui.js:106–137`; reference layout owner `apps/desktop/src/app.tsx:1493–1503,1724,3413–3440`. Not a finding that shell narrow mode failed. Keyboard trapping and exact breakpoint remain untested.

### L1 — Low: right-panel density differs despite equal widths

Shell's 12px inset makes the same fixed panel look denser than reference 16px; branch control is 8px wider. Reference's extra 8px gutter changes the apparent right region. **Do not “fix” this by reducing shell panel width:** both are already 320px.

**Expected/recommendation:** if exact taxpayer-panel parity is desired, choose shared 16px body padding (or explicitly retain denser 12px), then align nested headings and header separately. Shell `app/shell/ui.js:1228–1275`, `app/shell/chrome.js:389–608`; reference `apps/desktop/src/app.tsx:1724–1725`. This is a measured visual delta, not inherently a usability defect.

### L2 — Low, reference-side: catalog selection can lead to unsupported editor

The first filtered 1601C catalog entry led to an explicit **“isn't editable yet”** toast and returned to Forms. A second supported `1601-C` variant opened the editor. [Actual rejection](../../evidence/bir-shell-ui-audit/paired/ref-10-unsupported-catalog-form.png).

**Expected/recommendation:** mark unsupported variants before entry, clarify version/editability in catalog selection, or disable unsupported opening. Reference `apps/desktop/src/app.tsx:926–930,2350–2465`; shell comparison owner `app/shell/library.js:56`, `app/shell/form_page.js:3426–3452`. This is not a claim of shell catalog failure or that both catalogs are identical.

### Setup-only constraints, not established product regressions

- Shell's local test `xdg-open` wrapper initially blocked at “Opening browser…” until Chrome closed. `crates/bir-shell/src/cloud_account.rs:129–154` calls opener before polling. A detached opener returning promptly resolves the harness issue. Genuine auth completed; no bypass. Not established as a normal production opener defect.
- Reference required newer GLIBC/container/Wayland and direct-key input workaround. Native Linux setup is not yet covered by its Windows blueprint; Linux screenshot typography can differ because font registration falls back to system fonts (`apps/desktop/src/app.tsx:67–91`). Do not treat this run as Windows font-metric parity.
- Launch regenerated nine declaration additions in `app/shell/gpui-kit.d.ts`; diff preserved and generated additions restored after shutdown. Responsible scope: host/type-generation setup.

No other reproducible visual defect was established in these limited states. This does not certify unseen error paths, every form, financial calculations or all responsive layouts.

## 6. Page-by-page comparison — all 16 requested modules

### auth — shell observed; reference absent-equivalent

**Status:** fresh shell auth observed; reference absent-equivalent; expiry/re-login partial. **Severity:** no shell defect established.

Shell fresh welcome card offered Sign in/Create account without sidebar/taxpayer state: [auth capture](../../evidence/bir-shell-ui-audit/01-auth-fresh-default.png). Native local signup, email code and device approval reached setup. Sensitive intermediates were not retained in the recording.

Reference genuine first launch opened Home with “Nothing due…”/“No filed returns yet”, no auth card: [fresh Home](../../evidence/bir-shell-ui-audit/paired/ref-01-fresh-home.png). Its local-store initialization and routes do not provide equivalent cloud auth (`apps/desktop/src/app.tsx:49–58,636–663`). This is an intentional architecture difference, not missing comparison evidence. Shell owner `app/shell/gate.js:11,74`, `app/shell/account.js`, `crates/bir-shell/src/cloud_account.rs:129–154`. Expired/cancelled flows and re-login untested.

### setup_change — shell partial; reference absent-equivalent

**Status:** initial setup/change choices observed; transfer completion untested; reference absent-equivalent. **Severity:** no shell defect established.

Shell first-run “Who will use eBIRForms on this computer?” offered solo/helping others: [setup](../../evidence/bir-shell-ui-audit/02-setup-fresh.png). Solo led to taxpayer onboarding. Later Computer → Change exposed Another owner/A help desk/Cancel: [change choices](../../evidence/bir-shell-ui-audit/18-setup-change-options.png), cancelled without transfer.

Reference has local taxpayer creation, not computer-owner/help-desk setup. Do not pair its taxpayer dialog as ownership transfer. Owner `app/shell/auth.js:184`, `app/shell/setup_change.js:137–240`. Handover, help-desk PIN and recovery completion untested.

### home — paired empty and populated

**Status:** paired empty/populated observed; submitted history untested. **Severity:** Low density delta L1; no additional shell defect established.

Both show greeting/lede, Due in the next 30 days and Recently filed card areas. Empty states precede taxpayer/form selection. Adding 2026/1601C through each UI populated deadline rows and later a Draft status. Matched [shell](../../evidence/bir-shell-ui-audit/paired/shell-02-home-panel.png) / [reference](../../evidence/bir-shell-ui-audit/paired/ref-08-home-panel-populated.png).

Shell left nav adds Form library between Home and Calendar; reference uses Home/Calendar/Filings and places Settings near bottom. Both keep Recent below primary nav, a context pill in title bar and a taxpayer panel. Rounded bordered cards, relative due/status chips and roomy whitespace are broadly aligned; panel body is denser in shell (L1). Source main horizontal padding is 32 in both at this size; source vertical values and title/chrome placement are not identical. Due-list snapshots are not byte-identical states: the reference screenshot was taken before the later draft and its profile/address completeness differs.

No populated Recently filed history was created. Owner `app/shell/home.js:224`; reference `apps/desktop/src/app.tsx:1985–2075`.

### form_page — representative monthly draft, partial correctness coverage

**Status:** representative editor/reopen/save/scroll observed; full validity and other forms partial. **Severity:** Low reference-side unsupported variant L2; no shell form defect established.

Shell library opened an unsaved **Sample** with Save/Print/Delete, part navigation and disclosures. Saving chose October 2026, created a Recent entry, and reopening retained period. [Matched shell form](../../evidence/bir-shell-ui-audit/paired/shell-01-form-default.png).

Reference supported `1601-C` opened **Monthly Remittance Return of Income Taxes Withheld on Compensation**, October 2026, draft stage, part navigator and form fields: [matched reference](../../evidence/bir-shell-ui-audit/paired/ref-20-form-default.png). The first unsupported catalog variant showed a toast (L2), then the supported variant was chosen through UI. Form initially showed “Opening official form…” before rendering; this was a transient loading state, not timed as a performance benchmark.

Reference validation highlighted missing Item 3/registered-address data: [validation](../../evidence/bir-shell-ui-audit/paired/ref-11-form-validation.png). Lower fields were reachable by scroll: [scroll](../../evidence/bir-shell-ui-audit/paired/ref-12-form-scroll.png). After enabling “Ask to update the template on save”, Save draft visibly showed [Saved toast](../../evidence/bir-shell-ui-audit/paired/ref-23-draft-saved.png); Recent reopened the same period. Draft saving is therefore observed, **not blocked**, but a fully valid completed return is not established.

These are analogous monthly return workflows, not guaranteed identical form versions, field sets or output layouts. Shell has a more compact header/action arrangement; reference has an explicit workflow strip and guided-mode toggle. Both constrain long forms in the main region and allow scrolling. No print, Final Copy, queue/send, tax-calculation correctness, schedule persistence or all-form fidelity claim. Owner `app/shell/form_page.js:2987–3042,3426–3452`; reference `apps/desktop/src/app.tsx:1164–1168,2766–3140`.

### filings — paired empty only

**Status:** matched-content empty states observed; submitted tables untested. **Severity:** no shell defect established.

Shell centered bordered **“Filed returns show here”** empty card: [matched shell, 1440×900 PNG/content](../../evidence/bir-shell-ui-audit/paired/shell-11-filings-empty.png). Reference **“No filed returns yet”**: [reference, 1440×900](../../evidence/bir-shell-ui-audit/paired/ref-03-filings-empty.png). Both avoid treating empty state as an error; local drafts were not submitted. Shell shows its taxpayer panel while reference empty capture has it closed; content geometry is matched, panel/data state is not.

**Populated table columns/widths, long taxpayer rows, submitted-history scrolling and transport status remain untested**. Do not infer table parity from empty cards. Owner `app/shell/filings.js:8`; reference `apps/desktop/src/app.tsx:1985–2075`.

### periods — paired monthly, different entry points

**Status:** matched-content monthly modals observed; shell Cancel rechecked; other periods partial. **Severity:** no shell defect established.

Shell Save on Sample opens **“Which period is this return for?”** with Month/Year/Cancel/Save: [matched shell modal, 1440×900](../../evidence/bir-shell-ui-audit/paired/shell-08-period-chooser.png). Original run entered 10/2026 and saved. The supplement reopened an unsaved library sample and clicked **Cancel**, dismissing the modal without another saved period. [Original wide modal](../../evidence/bir-shell-ui-audit/11-period-save-modal.png) remains historical evidence, not the matched pair.

Reference plus/Start return opens monthly year/period selection before editing: [reference modal](../../evidence/bir-shell-ui-audit/paired/ref-09-period-modal.png). Chose October 2026 and opened form; same period retained in Recent/editor. Both center a focused modal, but prompt/actions and stage differ (save-sample versus start-return), so overlay sizes are not a direct identity assertion.

Annual/quarterly/as-needed, invalid/future inputs and duplicate-return choices untested. Periods is a helper/modal flow, not a separate page. Owner `app/shell/periods.js:18`, `app/shell/form_page.js:2987–3042`; reference `apps/desktop/src/app.tsx:3177–3265`.

### calendar_page — paired empty, populated and narrow

**Status:** paired empty/populated/narrow observed; date correctness untested. **Severity:** Medium reference-side clipping M2; no shell responsive defect established.

Shell initial no-forms state: [empty](../../evidence/bir-shell-ui-audit/29-calendar-default-panel.png); reference [“No pending deadlines”](../../evidence/bir-shell-ui-audit/paired/ref-02-calendar-empty.png). After 2026/form selection both show November 2026, December 2026 and January 2027 date groups, form/taxpayer, relative due and Draft/Not started status: [shell](../../evidence/bir-shell-ui-audit/paired/shell-03-calendar.png) / [reference](../../evidence/bir-shell-ui-audit/paired/ref-13-calendar-populated.png).

Date gutter, bordered rounded row and right-aligned status are comparable. At default size no broad column collision was observed; 800px reference clips while shell overlays (M2). This is list-row evidence, not a populated filings-table width test. VM date was November 1, 2026; holiday/deadline correctness not audited. Owner `app/shell/calendar_page.js:13`; reference `apps/desktop/src/app.tsx:1985–2075`.

### recent — paired creation and reopen

**Status:** creation/reopen observed at matched 1440×900 content; large histories/privacy permutations partial. **Severity:** no shell defect established.

Both sidebar histories gained a single October 2026 form entry after UI draft creation; selecting reopened that period. Shell long taxpayer title ellipsized without widening sidebar: [matched shell reopen](../../evidence/bir-shell-ui-audit/paired/shell-01-form-default.png). Reference entry visible with reopened editor: [matched reference](../../evidence/bir-shell-ui-audit/paired/ref-20-form-default.png).

Not a standalone page. Reference metadata formatting differs (month/year and abbreviated name), but both keep history beneath primary navigation. Reused shell-01 is 1440×900 PNG/content, matching ref-20; original21 remains only earlier unmatched evidence. Large histories, preference off/on and cross-taxpayer privacy untested. Owner `app/shell/chrome.js:270–282`, `app/shell/recent.js`.

### gallery — shell observed; reference absent-equivalent

**Status:** shell top/bottom scroll observed; reference absent-equivalent. **Severity:** no shell defect established on this surface.

Actual Components gallery reached via Ctrl+K → Components, **not Form library**. Top palette/icons/buttons and lower disclosure/nested-panel examples captured through scrolling: [top](../../evidence/bir-shell-ui-audit/19-gallery-default-top.png), [bottom](../../evidence/bir-shell-ui-audit/20-gallery-default-bottom.png). These are 1520×980 PNGs including shadow/frame, with 1480×940 app content, not a matched pair.

Reference declared pages are home/calendar/filings/taxpayer/settings/jobs/logs/form; no Components equivalent. No artificial gallery comparison or font-metric benchmark. Owner `app/shell/gallery.js:58`; reference page declaration `apps/desktop/src/app.tsx`.

### library — nearest equivalent is reference taxpayer Forms

**Status:** matched shell All/filter observed; reference nearest-equivalent Forms/catalog observed, not identical routes. **Severity:** Low reference-side L2; no shell library defect established.

Shell default selected-taxpayer filter originally began with **“No forms match”**: [original wide empty state](../../evidence/bir-shell-ui-audit/08-library-filtered-empty.png). In resumed populated synthetic state, exact 1440×900 captures show [All forms 43 with frequency controls/cards](../../evidence/bir-shell-ui-audit/paired/shell-06-library-all.png) and [1601C search filtering to 1 of 43](../../evidence/bir-shell-ui-audit/paired/shell-07-library-filtered.png). [Earlier wide All forms](../../evidence/bir-shell-ui-audit/09-library-all-gallery.png) remains historical evidence; its filename denotes Form library, not Components. The resumed filter is not claimed as a fresh/empty state.

Reference catalog lives inside taxpayer → Forms, with year, filter, selection checkboxes and saved form set: [reference filter/selection](../../evidence/bir-shell-ui-audit/paired/ref-07-library-filter-selected.png). Both can filter 1601C and establish selected forms, but reference has no standalone library route. Shell is browse/sample-oriented; reference is taxpayer/year assignment-oriented, with limited editable variants (L2). No exact card/table density equivalence claim. All search permutations and editors untested. Owner `app/shell/library.js:56`, `app/shell/taxpayer.js:1588–1612`; reference `apps/desktop/src/app.tsx:2350–2465`.

### taxpayer — paired authentic onboarding and profile

**Status:** authentic onboarding observed at unmatched sizes; later Profile/Forms matched at 1440×900; full profile/import partial. **Severity:** Low density delta L1; no additional shell defect established.

Shell fresh segmented TIN, name/address, branch 00000 and required validation: [shell onboarding, original 2560×1856](../../evidence/bir-shell-ui-audit/03-taxpayer-empty.png). Reference No taxpayer title panel → New taxpayer → empty Add exposed validation before creation: [empty panel](../../evidence/bir-shell-ui-audit/paired/ref-04-no-taxpayer-panel.png), [first validation, 1440×900](../../evidence/bir-shell-ui-audit/paired/ref-05-first-onboarding-validation.png). **Fresh onboarding is unmatched-size evidence**; no exact modal/spacing parity claim.

Entered only allowed synthetic TIN/name through visible fields. Both display taxpayer context/head office; selected year 2026 and form through Forms UI. Matched 1440×900 shell [Profile](../../evidence/bir-shell-ui-audit/paired/shell-09-taxpayer-profile.png) and [Forms with 2026/1601C](../../evidence/bir-shell-ui-audit/paired/shell-10-taxpayer-forms.png) show preserved taxpayer, branch and address/contact sections. [Original lower profile scroll](../../evidence/bir-shell-ui-audit/25-taxpayer-profile-scroll.png) remains a 1480×940 PNG including shadow with 1440×900 content. Reference [Profile](../../evidence/bir-shell-ui-audit/paired/ref-06-taxpayer-profile.png) and [Forms/filter selection](../../evidence/bir-shell-ui-audit/paired/ref-07-library-filter-selected.png) are 1440×900, with profile/forms/templates and disclosures.

**Data mismatch:** shell contains a synthetic address; reference address/RDO were left incomplete. This explains some reference validation and panel metadata differences; do not call missing reference address a renderer bug. Right-panel padding differs as measured (L1), both fixed width. COR import, address-history resolution, additional branch, archive/delete/export/TIN activation not exercised. Owner `app/shell/taxpayer.js:1588–1612`, `app/shell/chrome.js:389–608`; reference `apps/desktop/src/app.tsx:977–1010,1720–1845`.

### settings — paired light/dark, structurally different

**Status:** paired light/dark and selected settings observed; persistence/jobs partial. **Severity:** no shell settings defect established.

Shell uses a separate categorized settings sidebar and narrow centered General column: [matched General](../../evidence/bir-shell-ui-audit/paired/shell-05-settings-general.png), [earlier Dark](../../evidence/bir-shell-ui-audit/16-settings-dark.png). Reference keeps application sidebar and a stacked Settings column: [Light](../../evidence/bir-shell-ui-audit/paired/ref-14-settings-light.png), [Dark](../../evidence/bir-shell-ui-audit/paired/ref-15-settings-dark.png).

Both visibly switched theme and returned to Light for paired page captures. Shell uses a dropdown (System/Light/Dark); reference button choices. Both use light gray outer/white inner surfaces and dark neutral surfaces. Source base light colors match (`bg #f4f4f6`, `surface #fff`, `fg #18181b`, `fg2 #54545d`, `fg3 #71717a`), but typography is **not guaranteed pixel-identical**: Linux font fallback differs and reference font registration can use system fonts.

Reference lower Settings showed import/back up/jobs/logs, “SFTP: dry run” and no background tasks: [lower Settings](../../evidence/bir-shell-ui-audit/paired/ref-22-settings-dry-run.png). No jobs/import/export were executed. Restart persistence and exhaustive settings not tested. Owner `app/shell/settings.js:268–284,570`, `app/shell/settings_model.js`, `app/shell/theme.js`; reference `apps/desktop/src/app.tsx:2640–2720`, `apps/desktop/src/theme.ts`.

### security — shell partial; reference placeholder, not functional equivalent

**Status:** unset/empty-PIN validation observed; secret-bearing transitions untested; reference absent-equivalent placeholder. **Severity:** no shell defect established; not a security-correctness guarantee.

Shell Account Security showed unset masked PIN, Set PIN, authenticator Add and recovery explanation. Empty Set PIN produced **“Type all 4 digits of the new PIN first”**: [validation](../../evidence/bir-shell-ui-audit/26-security-empty-pin-validation.png). No PIN/TOTP/recovery credential generated or captured.

Reference profile “Email, security and export” explicitly says notification email, signing and export options **“arrive with the filing pipeline”**: [placeholder](../../evidence/bir-shell-ui-audit/paired/ref-21-profile-security-placeholder.png). This is not an equivalent authenticator/PIN page. Wrong/correct PIN lock/unlock, TOTP, recovery and account takeover resistance untested. Owner `app/shell/security.js:63,149–212`; reference `apps/desktop/src/app.tsx:2309–2315`.

### update — shell control/source only; reference absent-equivalent

**Status:** control observed; banner/install phases untested; source-only states; reference absent-equivalent. **Severity:** no shell defect established in unseen phases.

Shell General visibly offers Download updates automatically. No ready/up-to-date/failure banner appeared during this source-run build; no fake phase or install was injected. Source states inspected, **runtime banner layout and updater success untested**. Reference Settings/routes expose no equivalent app-updater surface.

Owner/source-only state definitions `app/shell/update.js:8,72–139`, `app/shell/settings.js:268–284`. Recommendation is coverage follow-up on a suitable update-enabled build, not a finding that unseen banners fail. Never install an update merely to manufacture evidence.

### account — shell cloud/local distinction; reference local preferences only

**Status:** shell account/server/menu observed; reference nearest-equivalent preferences; portal/signout partial. **Severity:** no shell defect established.

Shell Cloud Account showed synthetic email, local server **127.0.0.1:3008**, Refresh/Open account/Sign out; computer identity separately showed synthetic taxpayer as admin: [account/security](../../evidence/bir-shell-ui-audit/17-account-security-dark.png).

Reference Settings has local display name/email and bottom “Set your name / No email set”, not a logged-in server identity. Fresh empty preference state is intentional. This is nearest local-preferences equivalence, not cloud-account parity. No external account portal, production login, signout/re-login or activated TIN claim. Owner `app/shell/account.js:108`, `app/shell/settings.js:382`; reference `apps/desktop/src/app.tsx:2640–2720`.

### chrome/gate states — paired selected states

**Status:** selected docked/narrow/wide chrome and gates observed; exact boundary/focus traps partial. **Severity:** Medium shared contrast M1; Low density L1; reference-side Medium clipping M2.

Both have title/context pill, left navigation, optional taxpayer/notifications panel, Recent and account identity. Both sidebars are 248px, padding 8, with similar item density; shell includes Form library while reference promotes Settings at bottom. Shell exposes native menu/caption chrome on Linux; reference maximize/minimize/close buttons are Windows-only, so exact caption parity is not a valid Linux claim.

Shell empty notifications [capture](../../evidence/bir-shell-ui-audit/33-notifications-empty-default.png); reference draft activity produced populated notifications [capture](../../evidence/bir-shell-ui-audit/paired/ref-19-notifications-populated.png). These are **different notification states**, not a paired empty-state mismatch. Both use fixed 320px declared notification panels, but padding table above measures taxpayer body, not every notification nested element.

Held divider drag did not resize reference panel; shell attempted edge drag likewise remained fixed. Narrow auto-collapse/overlay versus reference docked clipping is M2. Fresh shell auth gate versus direct reference Home is genuine absent-equivalent. Exact breakpoint, keyboard focus traps, errors/retries, local PIN-locked frame and Windows captions untested. Owner `app/shell/chrome.js:80,208,225,389,608`, `app/main.js:257–287,1722–1733`; reference `apps/desktop/src/app.tsx:1453–1459,1493–1503,1656–1725,3413–3440`.

## 7. Visual evidence — genuine side-by-side captures

Columns are **shell / confirmed reference**, not before/after unless explicitly stated. Paired rows have matched content dimensions, not necessarily identical data/panel state. All links are repository-relative and work in the portable archive. No synthetic mockups, scaled content or composited UI.

### Home and taxpayer panel — 1440×900

| bir-shell | hexuria/ebirforms |
|---|---|
| ![Shell Home and 320px panel](../../evidence/bir-shell-ui-audit/paired/shell-02-home-panel.png) | ![Reference Home and 320px panel](../../evidence/bir-shell-ui-audit/paired/ref-08-home-panel-populated.png) |

### Representative October 2026 form — 1440×900

Analogous workflow, not guaranteed same form revision or profile completeness.

| bir-shell | hexuria/ebirforms |
|---|---|
| ![Shell October form](../../evidence/bir-shell-ui-audit/paired/shell-01-form-default.png) | ![Reference October form](../../evidence/bir-shell-ui-audit/paired/ref-20-form-default.png) |

### Populated Calendar — 1440×900

| bir-shell | hexuria/ebirforms |
|---|---|
| ![Shell Calendar](../../evidence/bir-shell-ui-audit/paired/shell-03-calendar.png) | ![Reference Calendar](../../evidence/bir-shell-ui-audit/paired/ref-13-calendar-populated.png) |

### Narrow Calendar — 800×900

| bir-shell: dismissible overlay | hexuria/ebirforms: docked/clipped |
|---|---|
| ![Shell narrow overlay](../../evidence/bir-shell-ui-audit/paired/shell-04-calendar-narrow-overlay.png) | ![Reference narrow clipping](../../evidence/bir-shell-ui-audit/paired/ref-16-calendar-narrow-clipped.png) |

### Settings — 1440×900

| bir-shell: separate settings navigation | hexuria/ebirforms: application navigation retained |
|---|---|
| ![Shell General](../../evidence/bir-shell-ui-audit/paired/shell-05-settings-general.png) | ![Reference Settings Light](../../evidence/bir-shell-ui-audit/paired/ref-14-settings-light.png) |

### Reference fresh state before synthetic data — 1440×900

Two sequential reference states, **not shell/reference pairing**.

| Untouched local Home | First empty taxpayer submission validation |
|---|---|
| ![Reference genuine fresh Home](../../evidence/bir-shell-ui-audit/paired/ref-01-fresh-home.png) | ![Reference genuine onboarding validation](../../evidence/bir-shell-ui-audit/paired/ref-05-first-onboarding-validation.png) |

### Library nearest-equivalent — 1440×900

Different workflows, matched content geometry. Results/count differ because catalogs differ.

| Shell All forms | Reference taxpayer Forms/catalog |
|---|---|
| ![Shell All 43](../../evidence/bir-shell-ui-audit/paired/shell-06-library-all.png) | ![Reference filtered Forms](../../evidence/bir-shell-ui-audit/paired/ref-07-library-filter-selected.png) |

| Shell 1601C search | Reference 1601C selection |
|---|---|
| ![Shell filtered 1 of 43](../../evidence/bir-shell-ui-audit/paired/shell-07-library-filtered.png) | ![Reference selection](../../evidence/bir-shell-ui-audit/paired/ref-07-library-filter-selected.png) |

### Period chooser — 1440×900

Shell sample-save versus reference start-return; shell supplement cancelled without another save.

| Shell period chooser | Reference period chooser |
|---|---|
| ![Shell period modal](../../evidence/bir-shell-ui-audit/paired/shell-08-period-chooser.png) | ![Reference period modal](../../evidence/bir-shell-ui-audit/paired/ref-09-period-modal.png) |

### Taxpayer Profile and Forms — 1440×900

| Shell Profile | Reference Profile |
|---|---|
| ![Shell Profile](../../evidence/bir-shell-ui-audit/paired/shell-09-taxpayer-profile.png) | ![Reference Profile](../../evidence/bir-shell-ui-audit/paired/ref-06-taxpayer-profile.png) |

| Shell saved Forms | Reference Forms/catalog selection |
|---|---|
| ![Shell 2026 Forms](../../evidence/bir-shell-ui-audit/paired/shell-10-taxpayer-forms.png) | ![Reference Forms](../../evidence/bir-shell-ui-audit/paired/ref-07-library-filter-selected.png) |

### Filings empty — 1440×900

Same content geometry; shell panel open versus reference panel closed, later versus earlier taxpayer state.

| Shell empty Filings | Reference empty Filings |
|---|---|
| ![Shell empty Filings](../../evidence/bir-shell-ui-audit/paired/shell-11-filings-empty.png) | ![Reference empty Filings](../../evidence/bir-shell-ui-audit/paired/ref-03-filings-empty.png) |

### Wide fixed panels — 2560×1856

Genuine equal-size content; editor section/validation state and form columns differ. Establishes fixed chrome widths, not field-by-field fidelity.

| Original shell12 wide | Supplemental reference wide |
|---|---|
| ![Shell wide fixed panels](../../evidence/bir-shell-ui-audit/12-form-panel-wide.png) | ![Reference wide fixed panels](../../evidence/bir-shell-ui-audit/paired/ref-24-form-wide.png) |

## 8. Deliverables, limitations and next decisions

- **Primary paired annotated recording (22 seconds edited):** `/home/ubuntu/screencasts/bir-shell-ebirforms-paired/bir-shell-ebirforms-paired-edited.mp4`. It shows shell Home/panel, Recent reopening and draft save, then reference Home/panel, Recent reopening/loading and validation. Its final reference annotation conservatively marks the valid-save path incomplete. The later save-toast screenshot proves draft save, not a fully valid return. Narrow/theme/fresh-state/lower-Settings/profile coverage is screenshot evidence, not in this video.
- **Report:** `/home/ubuntu/repos/buwiz-forms/docs/audits/bir-shell-vs-ebirforms-ui-audit.md`.
- **Evidence:** `/home/ubuntu/repos/buwiz-forms/evidence/bir-shell-ui-audit/`, with the original 68 PNGs plus **45 heavy-form PNGs (113 total)**, safe absence/request metadata, geometry/dimension inventories and generated-only diffs.
- **Portable archive:** `/home/ubuntu/bir-shell-ebirforms-ui-audit-20261101.zip`, containing report, evidence, the original 22-second comparison recording and the new heavy-form recordings. No account stores, databases, tokens or raw sensitive auth logs included.

Remaining **untested**: submitted filings/table column widths, all form versions/calculations/print fidelity, fully valid reference return, full reference profile/address/RDO, ownership transfer/help desk, PIN/TOTP/recovery/lock, updater banners/install, all responsive widths/exact breakpoint/focus traps, restart persistence beyond this preserved-state resume, all settings/jobs/import/export, Windows caption/font parity. Section 9 closes selected annual scrolling/save gaps, **not every collapsed field/section or complete horizontal-grid traversal**; reference annual editing remains unavailable. **Fresh onboarding remains unmatched 2560×1856 shell versus 1440×900 reference.** Wide 2560×1856 fixed-panel behavior is now observed; no exhaustive wide form-fidelity claim. Source-only observations labeled above.

No comparator clarification or user credential remains needed. Product decisions, not access blockers: whether to standardize taxpayer body padding to 16px and whether to address shared small-label contrast. Both apps/Weston are stopped. The original run's generated declarations were restored; the section 9 resume regenerated nine declaration lines, preserved as evidence and left untouched as requested. No intentional application-code edits or git writes were made.

Blueprints were read for both repos. The effective reference blueprint now includes the verified Linux Bun/Ubuntu24.04/Wayland compatibility path; the shell blueprint has Linux setup but still contains obsolete Windows painted-target run guidance that must not be selected for this JavaScript-shell audit. The heavy-form resume required no dependency installation or new setup proposal. No blueprint or app source was changed.

## 9. Heavy annual forms — 1700 family, full scrolling

### Scope, geometry and actual outcome

This follow-up reuses the **preserved synthetic stores**, not fresh onboarding. Shell branch `devin/1793507972-heavy-forms-ui-audit`, HEAD `d65a473ce7eed80065d082707923448245bcdc9e`; `app/` and `crates/bir-shell/` are unchanged versus the original `b833bdcc` audit. Reference remains `14fe8efb29e171cac4e794d0d1ee9afaf1069310`. After captures the branch integrated main `5300106d`; evidence describes tested `d65a473c`, not a retest of that later revision. Existing backend3008/Postgres were reused; the existing Ubuntu24.04/Weston reference container was resumed. Launch scripts stripped inherited `BIR_SFTP_*`, set only `BIR_SFTP_DRY_RUN=1`, and left seed/auth bypass unset.

All default heavy PNGs are genuine **1440×900 content**. Shell captures remove only20px shadow on each side of a1480×940 allocation; reference captures exclude compositor decoration. The five narrow1701 PNGs are **800×900** from an840×940 allocation. No screenshot was scaled or reconstructed. Reference rejection toasts were cropped from the **immediate native screenshot**, not a later frame after the toast disappeared. [PNG/geometry inventory](../../evidence/bir-shell-ui-audit/heavy/png-dimensions.tsv) and [observed measurements](../../evidence/bir-shell-ui-audit/heavy/observed-geometry.txt).

Wheel scrolling traversed top→middle→document end→top in each shell editor, with repeated end-scroll producing no further movement. The larger computation/schedule section was selected through part navigation, and additional disclosure headers expanded selected schedules. **Part-nav selection folds other parts**; this is not a claim that all296/946/269/663 fields were simultaneously rendered or every collapsed section inspected. Blank synthetic drafts with profile identity and2026 period were saved through UI, then all four reopened from Recent. No Queue/send/Final Copy or valid-return claim.

| Requested shell schema | Rendered badge / parts | Expanded scrolling exercised | Runtime status / severity |
|---|---|---|---|
| `1700-2018` | 2pages,296fields,6parts | Part V Computation of Tax (18items), then Part VI Schedule1 gross compensation | Save TY2026/reopen and scrolling observed; **medium** shared clipping below |
| `1701-2018` | 4pages,946fields,17parts; **Draft layout** badge | Part V Schedule4 Ordinary Allowable Itemized Deductions (21items), Schedule1 employer grid (2rows), narrow800px pass | Save TY2026/reopen and scrolling observed; **medium** shared clipping; Draft-layout fidelity not established |
| `1702rt-2018c` | 4pages,269fields,12parts | Part IV Computation of Tax (31items), Schedule III/IIIA NOLCO disclosures | Save year-ended12/2026/reopen and scrolling observed; **medium** shared clipping |
| `1702mx-2018c` | 4pages,663fields,17parts | Schedule2 income-tax regime computation (19items), expanded Schedule10 special-regime computation | Save year-ended12/2026/reopen and scrolling observed; **medium** shared clipping |

The badge numbers above are read from native-resolution **runtime PNGs**, not estimates from downscaled previews or original HTML file sizes.

**Validation limitation:**1702RT/MX12/2026 retained the visible warning **“Year (Page 1 Item 2) cannot be greater than or equal to current year when filing for Calen…”** (truncated by the UI). Save acknowledged/persisted the draft and Recent reopened it, but these are saved **invalid/incomplete drafts**, not fully valid annual returns. The reference fallback likewise retained incomplete-data validation. No warning was bypassed or submitted.

### Per-form top / middle / bottom / back-top evidence

“Bottom” means the end of the current document with the named section expanded and other sections retained as disclosure headers. Extra expanded-schedule endpoints are linked separately; no collapsed field is implicitly counted as tested.

| Form | Top | Middle | Bottom | Scrolled back to top |
|---|---|---|---|---|
| 1700 | [Top/panel open](../../evidence/bir-shell-ui-audit/heavy/shell-1700-top.png) | [Computation rows](../../evidence/bir-shell-ui-audit/heavy/shell-1700-middle.png) | [End after repeated wheel](../../evidence/bir-shell-ui-audit/heavy/shell-1700-bottom.png) | [Back-top](../../evidence/bir-shell-ui-audit/heavy/shell-1700-back-top.png) |
| 1701 | [Top/panel open](../../evidence/bir-shell-ui-audit/heavy/shell-1701-top.png) | [Schedule4 rows](../../evidence/bir-shell-ui-audit/heavy/shell-1701-middle.png) | [Last Schedule13 disclosure](../../evidence/bir-shell-ui-audit/heavy/shell-1701-bottom.png) | [Back-top](../../evidence/bir-shell-ui-audit/heavy/shell-1701-back-top.png) |
| 1702RT | [Top/panel open](../../evidence/bir-shell-ui-audit/heavy/shell-1702rt-top.png) | [Computation rows](../../evidence/bir-shell-ui-audit/heavy/shell-1702rt-middle.png) | [Last Schedule V disclosure](../../evidence/bir-shell-ui-audit/heavy/shell-1702rt-bottom.png) | [Back-top](../../evidence/bir-shell-ui-audit/heavy/shell-1702rt-back-top.png) |
| 1702MX | [Top/panel open](../../evidence/bir-shell-ui-audit/heavy/shell-1702mx-top.png) | [Schedule2 tax-regime rows](../../evidence/bir-shell-ui-audit/heavy/shell-1702mx-middle.png) | [Last Schedule10 disclosure](../../evidence/bir-shell-ui-audit/heavy/shell-1702mx-bottom.png) | [Back-top](../../evidence/bir-shell-ui-audit/heavy/shell-1702mx-back-top.png) |

Additional state evidence:
- 1700: [saved TY2026](../../evidence/bir-shell-ui-audit/heavy/shell-1700-saved.png), [gross-compensation grid viewport](../../evidence/bir-shell-ui-audit/heavy/shell-1700-schedule-left.png), [expanded schedule end](../../evidence/bir-shell-ui-audit/heavy/shell-1700-schedule-bottom.png).
- 1701: [saved TY2026](../../evidence/bir-shell-ui-audit/heavy/shell-1701-saved.png), [Schedule1 two-row employer grid](../../evidence/bir-shell-ui-audit/heavy/shell-1701-schedule.png), [same section/panel closed](../../evidence/bir-shell-ui-audit/heavy/shell-1701-panel-closed.png).
- 1702RT: [saved2026](../../evidence/bir-shell-ui-audit/heavy/shell-1702rt-saved.png), [expanded NOLCO sections](../../evidence/bir-shell-ui-audit/heavy/shell-1702rt-schedule.png), [panel closed](../../evidence/bir-shell-ui-audit/heavy/shell-1702rt-panel-closed.png).
- 1702MX: [saved2026](../../evidence/bir-shell-ui-audit/heavy/shell-1702mx-saved.png), [Schedule10 rows](../../evidence/bir-shell-ui-audit/heavy/shell-1702mx-schedule.png), [expanded Schedule10 end](../../evidence/bir-shell-ui-audit/heavy/shell-1702mx-expanded-bottom.png), [panel closed](../../evidence/bir-shell-ui-audit/heavy/shell-1702mx-panel-closed.png).

### Reference annual variants: actual attempts, not editable

Each exact catalog key was located through the Forms filter, selected for2026, then chosen in **New return → Open form**. All four displayed the explicit toast: **“[code] isn't editable yet — the editor covers1601-C,2551Q and0619-E for now.”** The catalog contains1702MX despite its absence in the initial profile-pack source search.

| Reference annual attempt —1440×900 | Reference annual attempt —1440×900 |
|---|---|
| ![1700v2018 rejection](../../evidence/bir-shell-ui-audit/heavy/ref-1700-not-editable.png) | ![1701v2018 rejection](../../evidence/bir-shell-ui-audit/heavy/ref-1701-not-editable.png) |
| ![1702RTv2018C rejection](../../evidence/bir-shell-ui-audit/heavy/ref-1702rt-not-editable.png) | ![1702MXv2018C rejection](../../evidence/bir-shell-ui-audit/heavy/ref-1702mx-not-editable.png) |

**Status: blocked for annual editor scrolling, draft saving and field-level comparison; audit-blocker, not an inferred shell defect.** There is no next supported annual variant in this reference build. Source confirmation is `apps/desktop/src/app.tsx:611` (three supported codes), `:925–929` (visible unsupported toast/route back) and `:1094` (open guard). Recommendation: only claim matched annual parity after the reference supports annual editors; do not swap an older annual catalog entry silently.

The next supported editor, existing **1601Cv2018 October2026**, was used as an explicitly **non-annual fallback**. Initial “Opening official form…” yielded the populated editor after Validate, as in the prior run; this required interaction is not a clean loading/performance result. Top→middle→end→top and right-panel toggle were exercised; Save draft produced the actual Saved toast. It has70fields, not the annual complexity. Owner: reference `apps/desktop/src/app.tsx:2766–3147` (form surface/actions), `:1164–1168` (save).

| Supported fallback top — not annual | Supported fallback middle — not annual |
|---|---|
| ![1601C fallback top](../../evidence/bir-shell-ui-audit/heavy/ref-fallback-1601c-top.png) | ![1601C fallback middle](../../evidence/bir-shell-ui-audit/heavy/ref-fallback-1601c-middle.png) |

[Fallback bottom](../../evidence/bir-shell-ui-audit/heavy/ref-fallback-1601c-bottom.png) · [back-top](../../evidence/bir-shell-ui-audit/heavy/ref-fallback-1601c-back-top.png) · [panel closed](../../evidence/bir-shell-ui-audit/heavy/ref-fallback-1601c-top-closed.png) · [Saved toast](../../evidence/bir-shell-ui-audit/heavy/ref-fallback-1601c-saved.png).

### Concrete deltas, clipping, grids and scrolling

**Medium — shell form cards clipped at the right seam.** With right panel docked at1440px, annual part cards terminate at the main-surface edge without their right-hand rounded border; explanatory text/rows are cut there. Closing the panel restores bounded rounded cards. This occurs across the four annual screenshots and on1701 narrow cards. This is separate from the intentional horizontal viewport inside a wide schedule table. Native1700 comparison below is a **workaround comparison**, not a fix or two different builds.

| Panel open: card/text right seam clipped | Panel closed: complete bounded cards |
|---|---|
| ![1700 clipped form column](../../evidence/bir-shell-ui-audit/heavy/shell-1700-top.png) | ![1700 closed panel workaround](../../evidence/bir-shell-ui-audit/heavy/shell-1700-top-panel-closed.png) |

Owner paths: `app/shell/form_page.js:3460–3484` (`drawFormSplit` sizes/body/insets, horizontal overflow hidden), `app/main.js:1722–1734` (viewport/layout arithmetic). Expected: card/content width respects actual scroll viewport and leaves its complete border/padding visible in both panel states. Recommendation: inspect native viewport versus shadow allocation and split-column arithmetic; **root cause not proven** by this audit.

Observed native-coordinate measurements (not all semantically identical fields):

| Measurement | Shell annual | Reference supported fallback | Interpretation |
|---|---|---|---|
| Fixed left / right chrome |248 /320px |248 /320px |0px width delta, unchanged from prior audit |
| Part-nav visible row width |208px (`x282…489`) |208px (`x281…488`) |0px; shell stays pinned while form scrolls |
| Visible part-card span, panel open |approximately600px (`x518…1117`), right edge clipped |574px (`x505…1078`), bounded |+26px visible span, **not** proof of intended width or annual parity |
| 1700 part-card span, panel closed |820px (`x569…1388`) |not a matched annual editor |+220px visible span vs shell open; centering changes too |
| Ordinary text-box height |32px in1700 return year (`y477…508`) |32px fallback year (`y502…533`) |0px on these examples; not a universal row-height equivalence |
| 1701 employer-grid row pitch |about43px, control30px high; column pitch140px |no annual grid available |No same-grid delta claim; source grid declares140px columns/30px editable controls |

Schedule controls stayed in their visible row cells; selected non-table money fields reflowed into two columns when space allowed. Wide employer grids show only a subset of columns at once. The horizontal-wheel probe also changed vertical page position, so **complete leftmost→rightmost reachability and all end columns remain unverified**. Do not classify cropped table columns alone as a defect or claim a horizontal-scroll pass. Owner for follow-up: `app/shell/form_page.js:2347–2415` (`scheduleTable`,140px columns and internal horizontal scroller).

Shell app chrome and its “On this form” column stayed pinned at1440px; form title/actions scroll away. The long1701/1702MX part list is independently scrollable, not evidence that all sections are expanded. Source `form_page.js:708–729` confirms nav chooses one part while disclosure toggles preserve multiple. In the reference fallback, “On this form” scrolls away with the document; no annual sticky-header comparison exists.

Qualitatively, the exercised shell wheel batches and return-to-top responded without a crash or persistent blank region. **No frame-time/input-latency measurement was taken**, and condensed recordings cannot establish smooth60fps scrolling or a performance delta. First render/validation calculations and all sections expanded at once remain outside coverage.

### 1701 at800×900

The sidebar collapsed and navigator became a wrapped chip list in the single scrolling document. Top→middle→end→top was repeated on Schedule1; right taxpayer panel became a dismissible overlay. Normal numeric fields were usable in this pass, but the right-edge card seam remains clipped and grid end-column reachability is unverified. The reference1701 narrow editor is **blocked by not-editable status**, not silently substituted.

| 800px top: chips and return header | 800px middle: grid and numeric rows |
|---|---|
| ![1701 narrow top](../../evidence/bir-shell-ui-audit/heavy/shell-1701-narrow-top.png) | ![1701 narrow middle](../../evidence/bir-shell-ui-audit/heavy/shell-1701-narrow-middle.png) |

[Narrow bottom](../../evidence/bir-shell-ui-audit/heavy/shell-1701-narrow-bottom.png) · [narrow back-top](../../evidence/bir-shell-ui-audit/heavy/shell-1701-narrow-back-top.png) · [right overlay](../../evidence/bir-shell-ui-audit/heavy/shell-1701-narrow-overlay.png). Responsive owners: `app/main.js:257–287,1722–1734`, `app/shell/form_page.js:3281–3349,3460–3484`, `app/shell/chrome.js` panel rendering.

### Recordings, cleanup and remaining coverage

One continuous annotated source recording was made, then four short genuine excerpts were produced:

| Form | Clip path |
|---|---|
|1700|`/home/ubuntu/screencasts/heavy-annual-forms/heavy-1700.mp4`|
|1701|`/home/ubuntu/screencasts/heavy-annual-forms/heavy-1701.mp4`|
|1702RT|`/home/ubuntu/screencasts/heavy-annual-forms/heavy-1702rt.mp4`|
|1702MX|`/home/ubuntu/screencasts/heavy-annual-forms/heavy-1702mx.mp4`|

Each clip crops the genuine **raw recording** application region, adds captions **below** it, retains action windows at the original15fps cadence while omitting idle gaps, and appends that form's reference Open-form rejection.1701 includes the narrow allocation inside the same unscaled desktop crop. These are UX evidence, **not timing benchmarks**. Combined viewing file: `/home/ubuntu/screencasts/heavy-annual-forms/heavy-annual-summary.mp4`; full annotated recording: `/home/ubuntu/screencasts/heavy-annual-forms/heavy-annual-forms-edited.mp4`. The archive includes clips, summary, full annotated run and provenance, alongside the unchanged original22-second paired video.

Apps/Weston/reference container stopped. Backend3008/Postgres remain available. No intentional app edits/git writes. Nine runtime-generated declaration lines were restored after shutdown; no app-code diff remains. The [generated diff](../../evidence/bir-shell-ui-audit/heavy/runtime-generated-types.diff) records them.

**Still incomplete:** actual reference annual scrolling/saving and800px1701, all fields/parts simultaneously expanded, every hidden table column and horizontal-scroll mechanism, full valid synthetic returns/calculations/print, precise scroll latency/paint budgets, and baseline security/update/submitted-history limits in section8. No user secret or production action is needed; closing annual comparator coverage requires a reference build implementing annual editors. The shell clipping finding remains unresolved.
