# Linux proof — hexuria/nsh PR #39 (fix/otp-input-burst)

Tested commit: **62e436ff170ed6652b05815deb6799077285cad2** (branch head at
test time; two commits past the 5fa33438 named in the order — a doc-comment
commit and an engine-test expectation, neither touching runtime code).
Base: **8c78cfd6d** (origin/buwiz-next). Clone: a separate copy at
~/repos/nsh-kit39; the nsh checkout the app builds against was never touched.

## Result table

| step | commit | command | PASS/FAIL | counts |
|---|---|---|---|---|
| 1. Linux platform build | 62e436ff | `cargo clippy -p nsh-platform-linux -p nsh-linux-host -p nsh-linux-app --all-targets` (the exact command in the kit's AGENTS.md), run in the buwiz-nsh-2404 docker image (Ubuntu 24.04, GTK 4 ≥ 4.14; the host's 4.6.9 is too old) | **PASS** | 0 warnings, 0 errors in 28 crates — nothing to quote; no warning or error text exists in the changed files or anywhere in the run |
| 2a. otp input tests | 62e436ff | `cargo test -p nsh-base --test base otp_input` | **PASS** | 19 passed, 0 failed — incl. all four new tests (a_burst_of_keys_keeps_its_order, a_composition_mid_entry_drops_no_key, a_key_during_the_rewrite_is_not_dropped, a_users_selection_is_not_pinned_away) |
| 2b. text input tests | 62e436ff | `cargo test -p nsh-testing` (the headless input/screen modeling incl. typing_into_the_focused_field_keeps_its_caret_and_selection) | **PASS** | 114 passed, 0 failed |
| 2c. command encoding tests | 62e436ff | `cargo test -p nsh-core ops::tests` / `components::host_elements_take_typed_props_and_events`; `cargo test -p nsh-js` | **PASS** | ops 1/1; host_elements 1/1 (asserts the count field on Command::SetSelection); nsh-js 243+38 passed, 0 failed |
| 2d. engine otp test | 62e436ff | `cargo test -p nsh-engine --test engine a_base_otp_input_types_into_one_field_over_its_cells` (+ keyboard) | **PASS** | 1/1 + 3/3 — asserts the caret is present but transparent |
| 3. otp tests pre-fix | see below | `cargo test -p nsh-base --test base otp_input` | see below | see below |
| 4. Linux selection write | 62e436ff | read | — | see below |
| 5. xdotool demo, head | 62e436ff | `cargo run -- OtpInput` (kit's own gallery launcher) in the buwiz-nsh-2404 image on Xvfb, `xdotool type --delay 10 "123456789"` x10, click Clear between runs | **PASS** | 10/10 runs the six cells read 1 2 3 4 5 6 in order (the input caps at its length of six; the extra digits are dropped) — otp-head-run01..10.png |
| 5. xdotool demo, base | 8c78cfd6 | identical procedure | **in order 10/10 — see the caveat below** | 10/10 runs read 1 2 3 4 5 6 in order — otp-base-run01..10.png |

## Step 3 — the same tests before the fix

The four new tests **cannot run at all on pure buwiz-next**: they are written
against harness APIs the PR itself adds (`type_in_burst`, `compose`, the
modeled caret/selection), and the headless screen at the head does not even
compile against the base core (`Command::SetSelection` has no `count` field
there — quoted verbatim: `variant `SetSelection` does not have this field`).
What runs on pure buwiz-next is the 15 pre-existing otp tests; they pass
15/15 — they never exercised a burst.

The author's own pre-fix demonstration is the series' first commit
(5b2f70552, harness + tests, no fix). To test all four new tests pre-fix the
closest faithful states were constructed in a throwaway worktree (nothing
pushed anywhere):

| state | command | result |
|---|---|---|
| 5b2f70552 exactly (harness+burst test, no fix) | `cargo test -p nsh-base --test base otp_input` | 15 passed, 1 failed: **a_burst_of_keys_keeps_its_order FAILED** |
| c68412db7 with the fix commit's two files (otp_input.rs, nsh-testing/src/screen.rs) reverted to their pre-fix content | same | 16 passed, 2 failed: **a_burst_of_keys_keeps_its_order FAILED**, **a_composition_mid_entry_drops_no_key FAILED**; **a_key_during_the_rewrite_is_not_dropped PASSED** — this is the one that passes both ways |
| f221d8168 with the same two files reverted | `cargo test -p nsh-base --test base otp_input::a_users_selection_is_not_pinned_away` | **FAILED** |

Re-runs of the pre-fix failures, alone, three times each (the owner's rule):
a_burst_of_keys_keeps_its_order FAILED/FAILED/FAILED (2.77s, 2.76s, 2.76s);
a_composition_mid_entry_drops_no_key FAILED/FAILED/FAILED (3.51s, 3.53s,
3.53s) — deterministic, not timing flakes.

The composition failure text in full — same permutation:

```
thread 'otp_input::a_composition_mid_entry_drops_no_key' (1023063) panicked at crates/nsh-base/tests/base/otp_input.rs:268:55:
called `Result::unwrap()` on an `Err` value: "`Changes [1,12,123,1234,12345,123456]` showing never happened in 2000 ms; on screen: [\"1\", \"3\", \"4\", \"5\", \"6\", \"2\", \"Changes [1,12,132,1342,13452,134562]\", \"Completes [134562]\", \"Focus 1 Blur 0\"]"
```

The user-selection failure text in full — a pin (start=2 end=2) answers the
user's 0..1 selection, exactly what the fix forbids:

```
thread 'otp_input::a_users_selection_is_not_pinned_away' (1023364) panicked at crates/nsh-base/tests/base/otp_input.rs:580:5:
no pin answers a selection: [(27, SetSelection { start: 2, end: 2, count: 2 })]
```

The burst failure text in full:

```
thread 'otp_input::a_burst_of_keys_keeps_its_order' (990750) panicked at crates/nsh-base/tests/base/otp_input.rs:268:55:
called `Result::unwrap()` on an `Err` value: "`Changes [1,12,123,1234,12345,123456]` showing never happened in 2000 ms; on screen: [\"1\", \"3\", \"4\", \"5\", \"6\", \"2\", \"Changes [1,12,132,1342,13452,134562]\", \"Completes [134562]\", \"Focus 1 Blur 0\"]"
```

i.e. the cells read 1,3,4,5,6,2 — the exact permutation the PR description
reports from macOS, reproduced here on Linux.

## Step 4 — the Linux selection write at the head

`crates/nsh-platform-linux/src/text_input.rs` `set_selection(view, start,
end, count)`: if the count the write was made against is behind the input's
own edit count (`count < input.count.get()`) the write is dropped silently —
a late selection would pull the caret into live typing, exactly as a stale
value write would; otherwise it converts the UTF-16 offsets to char offsets
and applies `editor.select`. This matches the macOS one
(`crates/nsh-platform-macos/src/text_input.rs`): the identical staleness
guard with the identical comparison, differing only in how the surviving
write is applied (macOS stashes it as pending when the field is not being
edited; GTK applies it to the editor directly).

## Incidental, pre-existing (matches the author's note, not caused by the PR)

An accidental full `cargo test -p nsh-core` at the head: 1715 passed, 1
failed — `core::tests::components::keyed_list_cell_whose_row_goes_can_read_first`
(`left: [], right: ["r0", "r2"]` at tests.rs:16684). Re-run alone three times
per the owner's rule: FAILED/FAILED/FAILED, deterministic. The author lists
this test among three that fail on buwiz-next and at the head alike on his
Mac; consistent with that, it is pre-existing and unrelated to the change
(the PR touches no keyed-list code).

## Machine differences from a standard Linux build machine

- The build machine is the docker image buwiz-nsh-2404 (Ubuntu 24.04
  userland, GTK4 ≥ 4.14) on an Ubuntu 22.04.5 host — the host's own GTK 4.6.9
  is below the kit's 4.14 minimum, so host-direct builds of the platform
  crate are impossible here; the kit's own harness and core crates were also
  built/tested on the host directly (they are platform-independent).
- Kernel is the host's 6.8.0-1060-aws (AWS VM), shared by the container.
- Rust is rustup stable 1.99.0, not a pinned toolchain; cargo/rust caches
  were warm, not cold.
- Xvfb (1280x1024/1920x1080) provides the display; no real GPU, GTK uses the
  cairo renderer.
- The disk filled twice during the work (shared machine, a 61 GB app build
  cache lives here); the affected cargo runs were re-run after freeing space.
  No test result above comes from a run that hit the disk-full.


## Step 5 — the live demo, and an honest caveat

The kit's gallery has an OtpInput demo ("One-time code", six boxed cells) and
it runs on Linux (GTK). Procedure, per the order: xdotool real key events,
nine digits at 10 ms, ten times, clicking the demo's Clear button between
runs. The procedure was validated: Clear empties the cells and the first cell
takes focus; a control entry of 987654321 landed 9 8 7 6 5 4 in order.

Result at the head (62e436ff): **10/10 runs read 1 2 3 4 5 6 in order.**
Result at buwiz-next (8c78cfd6): **10/10 runs read 1 2 3 4 5 6 in order as
well** — the fault did not reproduce in the live demo on this machine at
10 ms, nor in three exploratory runs each at 1 ms and 20 ms. That is not
evidence against the PR: the fault is a race — a keystroke's pin must land
back while later keys of the burst are still in flight — and whether that
window opens depends on the machine's event roundtrip timing (the author
reproduced it on macOS at 20 ms spacing, never at 50 ms). On this VM's GTK /
Xvfb / cairo stack the window apparently does not open at these spacings. The
deterministic Linux evidence is the harness-modeled burst of step 3, which
fails pre-fix 3/3 with exactly the permutation the PR describes and passes at
the head. The demo screenshots still pull their weight: they show the fixed
input behaving correctly end-to-end with real key events on a real Linux
window, and they are the control proving the typing procedure itself lands
digits in order.
