# UI beach-ball with `--features agent` — root cause and fix

Follow-up to `6c62f52c` ("stop Draft-save spam on Queued/Submitted 1601-C").
The toast spam was real and is fixed; the freeze that followed it was a
separate, pre-existing problem in the agent control plane that the toast fix
simply made visible (a `form.fields` poll loop became usable, so it got run).

## Symptom

Painted `bir` built with `--features dev-tools,agent` and started with
`GPUI_AGENT=1` lagged, then stopped responding entirely (system spinner over
the window, no input accepted). Force-killing `bir` released it. The agent
control plane itself kept answering.

## Root cause

`apply_agent` runs **inside `AppState::render`**, on the macOS main thread.
Two things put unbounded work there.

### 1. The agent refresh loop forced a full redraw 60×/s, forever

`AppState::attach_agent` spawned a 16 ms timer that called `cx.notify()`
unconditionally, purely so the next frame would drain the mailbox. GPUI
answers a notify by marking the window dirty and scheduling a frame, so the
whole window was laid out and repainted 60 times a second for as long as the
agent was attached — whether or not the agent had sent anything.

Measured on this machine (debug build, seeded scratch DB, window idle on the
Global Dashboard):

| build | idle CPU |
| --- | --- |
| `bir` without `GPUI_AGENT` | **1.2 %** |
| `bir` with `GPUI_AGENT=1`, agent idle | **50 %** |

A `sample` of the idle process shows the main thread inside
`gpui_macos::window::step` → `Window::draw` → `compute_layout` continuously.
That leaves the main thread with no headroom: once one frame costs more than
the 16 ms period — a real database, the 1601-C form (the heaviest view), a
debug build — the main dispatch queue never drains and AppKit input starves.
That is the beach ball.

### 2. `snapshot_host` rebuilt jobs + full submission history on *every* request

`snapshot_host` called `reload_jobs_and_submissions()` for every request,
including read-only ones. That is `list_jobs()`, `list_all_queued_submissions()`
and `list_submissions_for_tin()` for the selected TIN — and
`list_submissions_for_tin` **JSON-decodes every row's `form_data` blob**, which
none of the agent's callers ever read. The merge into the queued rows was also
an O(n²) linear scan.

`sample` of the main thread during a `form.fields` poll loop, before the fix
(seeded DB: 15 profiles, 60 jobs, 200 submission rows):

```
Window::draw                        1747 samples
└ apply_agent                       1260   (72 % of the frame)
  └ snapshot_host                   1125   (64 %)
    └ reload_jobs_and_submissions    854   (49 %)
      └ list_submissions_for_tin     607   ← serde_json::from_str per row
```

Roughly half of every frame was spent re-reading submission history to answer
`form.fields`, and the cost grows with the taxpayer's filing history — i.e. it
gets worse over time and is unbounded on a real database. Nothing reads
`host.jobs` / `host.submissions` back out except `jobs.list` / `submissions.list`
(which already reload themselves) and the Background Tasks snapshot tree.

### 3. Read-only invokes re-applied the fill patch to an immutable return

`apply_host` runs for every `Op::Invoke`, `form.fields` included. On a
Queued/Submitted/Confirmed 1601-C it still pushed `tax_14` / `tax_25` /
`sheets` into the painted inputs. `InputState::set_value` emits
`InputEvent::Change` even for an identical value, and this view answers Change
with `sync_from_inputs` — a full re-parse of every input plus `compute()` and
`validate()`. So each poll of a return that cannot change ran three of those.
`apply_1601c_host_header_patch` reported "dirty" unconditionally, forcing a
fourth even when the header flags were untouched.

## Fix

| file | change |
| --- | --- |
| `agent/drain.rs` | `attach_agent` notifies only when the mailbox is non-empty. |
| `agent/drain.rs` | `snapshot_host` loads jobs/submissions only for the Background Tasks view; `jobs.list` / `submissions.list` keep their own reload. |
| `agent/host.rs` | `reload_jobs_and_submissions` reads summaries (no `form_data`) and dedupes through a `HashSet`. |
| `db/submissions.rs` | new `list_submission_summaries_for_tin` + `SubmissionSummary` (id/tin/form_type/period/status only). |
| `views/form_1601c_view.rs` | `agent_apply_from_host` returns immediately for a non-editable return; input writes and header flags are no-ops when the value is unchanged. |

The filing CAS and the Draft-only save gates are untouched: `save_draft` still
refuses a non-Draft, `form_1601c_host_patch()` still carries `save` only while
the return is editable, and `reconcile_open_forms_from_db` still runs on every
request so a stale local Draft cannot mask Queued+claimed.

### Result (same machine, same seeded DB)

| measurement | before | after |
| --- | --- | --- |
| idle CPU, agent attached | 50 % | **2 %** (no-agent baseline is 1.2 %) |
| `form.fields` poll, worst round trip | 61 ms | **26 ms** |
| 16 concurrent floods: unrelated `hello` latency | 90–150 ms | **~20 ms** |
| 16 concurrent floods: requests in 25 s | 4 314 | **17 420** |
| `apply_agent` share of a frame | 72 % | **29 %** |
| `reload_jobs_and_submissions` share of a frame | 49 % | **0 %** (not called) |

Soak: 24 concurrent connections, 23 098 invokes in 30 s against a **Submitted**
September 1601-C while 336 `DatabaseChanged` broadcasts landed — no stall, no
errors, window responsive throughout, back to idle CPU afterwards.

## Considered and rejected

- **`try_lock` on `AppEvent::DatabaseChanged` in `Form1601CView`** (a WIP patch
  that existed before this work). It trades a microsecond wait for silently
  dropping a Draft→Queued→Submitted status refresh, which is the one thing that
  handler exists to deliver. No measurement showed a cron thread holding the DB
  mutex long enough to matter — every `bir-core` path scopes the guard around a
  single query — while the contention that *was* measurable came from the UI
  thread hammering the same mutex 300×/s, which this change removes. Reverted to
  the blocking lock.
- **Gating `agent_apply_from_host` from `drain.rs`** (the other WIP patch). Same
  intent, but the guard belongs with the view that knows what "editable" means;
  keeping it in one place avoids the two drifting apart.

## Verifying by hand

```bash
export GPUI_AGENT=1 GPUI_AGENT_TOKEN='dev-secret' GPUI_AGENT_ADDR='127.0.0.1:17421'
cargo run --locked --bin bir --features dev-tools,agent
```

1. **Idle cost.** Leave the app alone on the Global Dashboard with no agent
   traffic. `ps -o pcpu -p $(pgrep -f 'target/debug/bir$')` should sit at a few
   percent, not ~50. Window drag / scroll should feel the same as a build
   without `GPUI_AGENT`.
2. **Submitted return.** Select the Juan dummy TIN `00000000000000`, open the
   Submitted 1601-C, and poll `form.fields` in a loop for a minute. Expected: no
   toast of any kind, round trips around 16–25 ms, and the window stays
   draggable and scrollable the whole time.
3. **Other read-only verbs.** Same loop with `submissions.list` and `jobs.list`.
   Both should stay responsive; `submissions.list` must still list the same rows
   it did before (covered by `summaries_match_full_rows_without_form_data` and
   `submissions_list_merges_history_rows_once`).
4. **Background Tasks still populated.** `nav.go page=cron-tasks`, then
   `snapshot` — the tree must still contain `jobs-list` and `submissions-list`
   with children. This is the one view that needs the eager reload.
5. **Draft editing unchanged.** Open a Draft period, `form.fill` `tax_14` /
   `tax_25` / `sheets` and the Item 11 header flags, then `form.save_draft`.
   Values must appear in `form.fields` and survive the save.
6. **Cron alongside.** With the poll loop running, let the 60 s cron tick fire
   (or post `dev.goldcoders.bir.DatabaseChanged`). Dashboard and Calendar must
   stay interactive.

## Adjacent finding, not fixed here

`Form1601CView::save_draft` does not copy the row id returned by
`save_1601c_draft` back into `self.draft`. For a period that did **not** exist
when the form was opened, the view's draft keeps `id: None` while the stored row
has an id, so `stored_1601c_wins` is permanently true and
`reconcile_open_forms_from_db` overwrites the in-memory draft from the database
on every request. Visible effect: a `form.fill` of the Item 11 header flags on a
brand-new, never-before-saved period is reverted before the next `form.fields`
reads it. Re-opening the form (so it loads the stored row with its id) makes the
same fill round-trip correctly. This predates the toast fix, is unrelated to the
freeze, and touches the save path, so it is left for its own change.
