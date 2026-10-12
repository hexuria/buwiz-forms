# ui-walkthrough

Screenshots every top-level page of the desktop app on Linux, without touching
your running app or your data. It replaces the slow manual walkthrough.

`walkthrough.py` does the following:

1. Starts `Xvfb` on the first free display from `:110` (1440x900 by default).
2. Starts the app there with its own `HOME`, `XDG_DATA_HOME`, `XDG_CONFIG_HOME`
   and `XDG_CACHE_HOME` in a temporary folder. Because the single-instance
   port file (`$XDG_DATA_HOME/ebirforms/instance.port`) lives in that folder,
   this copy never hands off to an app you already have open. The agent listens
   on a random loopback port with a random token.
3. Drives the app through its gpui-agent automation interface (protocol v2,
   HMAC-authenticated), then saves a screenshot of each page with
   `import -display :N -window root`.
4. Writes `index.html` (a contact sheet), `pages.txt`, `layout.json` and
   `app.log` to the output folder. Then it shuts the app down, stops Xvfb and
   deletes the temporary HOME.

## Build

The binary must include the agent. Debug builds also include Dev Mode:

```sh
cargo build -p bir-desktop --bin bir --features dev-tools,agent
```

## Run

```sh
tools/ui-walkthrough/walkthrough.py --out /tmp/walk                  # empty database
tools/ui-walkthrough/walkthrough.py --out /tmp/walk --demo-profile   # adds a demo taxpayer
tools/ui-walkthrough/walkthrough.py --out /tmp/walk --data-from ~/.taxman-ebir
```

Then open `<out>/index.html` in a browser. Each card links to the full-size PNG.

| Option | Default | Meaning |
|---|---|---|
| `--out DIR` | `target/ui-walkthrough` | output folder |
| `--bin PATH` | `$CARGO_TARGET_DIR/debug/bir` | app binary built with `--features agent` |
| `--data-from DIR` | none | copies `bir_data.db` (+ `-wal`, `-shm`) and `bir_key.txt` into `<HOME>/.taxman-ebir`. The source is never opened or modified. |
| `--demo-profile` | off | if the database has no active profile, creates "Walkthrough Demo Taxpayer" (TIN 123-456-788-00002) through the agent |
| `--tin TIN` | first active profile | taxpayer for the dashboard and form pages |
| `--forms LIST` | `2550Q:1,1800:1` | form pages to open, as `CODE[:PERIOD]` |
| `--year N` | current year | tax year for `form.open` |
| `--size WxH` | `1440x900` | screen size. The window is moved to 0,0 and resized to match. |
| `--scroll-notches N` | 40 | mouse-wheel notches used to reach the end of Settings and Dev Mode |
| `--no-dev-mode` | off | skip Dev Mode (use this for release builds) |
| `--keep-home` | off | keep the temporary HOME for inspection |

Exit codes:

- `0`: every page was reached and captured.
- `1`: a page could not be reached (the page root never appeared in the agent
  tree, an invoke failed, or a PIN overlay blocked it), or `--data-from` could
  not be loaded.
- `2`: the app crashed, exited, or its agent never answered.
- `3`: a required tool is missing.

The reasons are printed at the end of the run and listed at the top of
`index.html`. A page that fails still gets a `NN_<page>_failed.png`
screenshot as evidence.

## Pages visited

1. Global Dashboard
2. Profile dashboard, Calendar tab
3. Profile dashboard, Tax Form Library tab
4. Import Data
5. Notifications
6. Background Tasks, Jobs tab
7. Background Tasks, Logs tab
8. Tax Calendars
9. Settings, top of the page and after scrolling to the end
10. Dev Mode, top of the page and after scrolling to the end (debug builds only)
11. Form 2550Q (the form header style)
12. Form 1800 (the event-kit header style)

Without a profile, pages 2, 3, 11 and 12 are skipped and the run fails.

## How it navigates

| What | How |
|---|---|
| Pages | agent `invoke nav.go {page: <slug>}` (slugs in `crates/bir-desktop/src/agent/ids.rs`). The script waits until the page's root id (for example `page-settings`) is in the agent snapshot. |
| Profile dashboard | `profile.set {tin, view: "dashboard"}` |
| Dashboard tabs | `dashboard.tab {tab: "calendar" or "forms"}`, which only the painted app answers (handled in `agent/drain.rs`) |
| Background Tasks tabs | agent `click jobs_tab` / `click logs_tab` |
| 2550Q | `form.open {code, tin, period}` |
| Event-kit forms (1800) | `form.open` does not know them, so the script calls `profile.set` and then `nav.go 1800` |
| Scrolling | **xdotool** mouse wheel over the centre of the page. The agent has no scroll operation for these pages, and its scrolled screenshots only work on macOS. |
| Screenshots | **ImageMagick `import`**, because the agent's `screenshot` op returns `screenshot_unavailable` on Linux |
| Demo profile | `profile.create`, `set_value` on each field, then `profile.save`. The save reloads the profile into the editor, so no "Unsaved profile changes" banner is left and navigation continues without any coordinate click. |

## Environment variables

The defaults suit a normal Linux machine that has `Xvfb`, `xdotool` and
ImageMagick installed. Override them for unusual setups:

| Variable | Default | Purpose |
|---|---|---|
| `WALK_BIN` | `$CARGO_TARGET_DIR/debug/bir` | same as `--bin` |
| `WALK_OUT` | `target/ui-walkthrough` | same as `--out` |
| `WALK_XVFB` | `Xvfb` | Xvfb executable |
| `WALK_IMPORT` | `import` | ImageMagick `import` |
| `WALK_XDOTOOL` | `xdotool` | xdotool executable |
| `WALK_XDOTOOL_LD_LIBRARY_PATH` | none | extra library path, applied to xdotool only |
| `WALK_APP_LD_LIBRARY_PATH` | inherited | `LD_LIBRARY_PATH` for the app |
| `WALK_APP_LD_PRELOAD` | none | `LD_PRELOAD` for the app |
| `WALK_VK_ICD_FILENAMES` | inherited | Vulkan ICD for the app (for example lavapipe) |
| `WALK_EMULATED_GPU` | `1` | sets `ZED_ALLOW_EMULATED_GPU=1` and `MESA_SHADER_CACHE_DISABLE=true`, which software Vulkan on Xvfb needs |
| `WALK_APP_PATH_PREPEND` | none | prepended to the app's `PATH` (for example an `xdg-open` shim) |
| `WALK_TMPDIR` | system temp | where the temporary HOME is created |

Example for the aarch64 build sandbox, which has runtime libraries in
`~/.cache/rtroot` and needs the lavapipe no-SVE preload:

```sh
S=<session scratchpad>; RL=$HOME/.cache/rtroot/usr/lib/aarch64-linux-gnu
WALK_XDOTOOL=$HOME/.cache/rtroot/usr/bin/xdotool WALK_XDOTOOL_LD_LIBRARY_PATH=$RL \
WALK_APP_LD_LIBRARY_PATH=$RL:$S/sysroot/usr/lib/aarch64-linux-gnu \
WALK_APP_LD_PRELOAD=$S/vnc/libnosve.so WALK_VK_ICD_FILENAMES=$S/vnc/lvp_icd.json \
WALK_BIN=$HOME/.cache/buwiz-target/debug/bir WALK_TMPDIR=$S \
python3 tools/ui-walkthrough/walkthrough.py --out $S/ui-walk --demo-profile
```

## Overflow check: not done

The agent tree cannot express text or control overflow. Only five nodes have
painted bounds: the window, the sidebar, the page root and the two form or
preview scrollers (see `tree_layout` in `agent/drain.rs`). Cards, text and
controls have no bounds, so the tree cannot show a label spilling out of its
card. Making that possible would mean recording bounds for every widget in
every view.

`layout.json` records the window bounds for each page, plus any bounded node
that extends past the window. It is informational only and does not change the
exit code.

## Known limits

- **`--data-from` does not work on Linux builds without a keyring backend.**
  On Linux, `bir-core` depends on `keyring` 3 with no features, which means an
  in-memory mock store. Each launch generates a new SQLCipher master key and
  never reads `bir_key.txt`, because that file is only used when the keyring
  cannot be created at all. As a result, a copied database cannot be
  decrypted: the app moves it aside as `bir_data.db.corrupt-*.bak` and starts
  empty. The script detects this and fails with a message. Use
  `--demo-profile` until Linux has a persistent key store or reads
  `bir_key.txt` first.
- The first screenshot after the opt-in demo profile can still show the
  "profile was cleared" toast.
- Dashboard tabs need the painted app. `bir-headless` does not answer
  `dashboard.tab`.
