---
description: Drive the current project goal to completion autonomously, stopping only when its done-condition passes
---

# /goal — drive an objective to completion

Read `.claude/GOAL.md`. It defines one objective, its **done-condition as a
command**, the method to follow, and the constraints that cannot be broken.

If `$ARGUMENTS` is non-empty, treat it as a new objective: rewrite
`.claude/GOAL.md` to describe it (same sections), then proceed.

## How to run it

1. **Check the done-condition first, every time.** Run the exact command in the
   `## Done when` section. Never answer "is this finished?" from memory or from
   what you believe you just did — that is how an objective stays open forever.
   If it passes, go to step 5.

2. **Check whether work is already in flight.** If `## In flight` names a
   background workflow or task, verify it is still alive before starting
   anything. If it died or was stopped, resume it rather than duplicating it. If
   it finished but the done-condition still fails, do the remaining work
   yourself.

3. **Do the next increment**, following `## Method` exactly. Prefer the smallest
   unit that can be verified on its own — one form, one file, one section. After
   each increment, run its verification and **revert anything that regresses**.
   A partial objective with a clean tree beats a complete one that broke
   something.

4. **Keep the loop alive.** End the turn with a `ScheduleWakeup` carrying
   `/goal` verbatim as the prompt. If a background task is the real wake signal,
   use a long fallback delay (1200–1800s) — polling harness-tracked work is
   waste, since its completion re-invokes you automatically. Update
   `## In flight` and `## Progress` in `.claude/GOAL.md` so the next wake-up (or
   the next session, or a different agent) picks up from fact rather than
   inference.

5. **When the done-condition passes**, stop the loop (`ScheduleWakeup` with
   `stop: true`), report what changed with concrete evidence, and ask the user
   what is next. This is the *only* point at which you ask a question.

## Rules

- **Do not ask the user anything until step 5.** That is the whole point of a
  goal: it runs without prompting. If you hit a genuine blocker that only the
  user can resolve, record it under `## Blocked` in `.claude/GOAL.md`, continue
  with everything not blocked by it, and surface it at step 5.
- **Never edit the done-condition to make it pass.** If it is wrong, fix it and
  say so plainly in the commit — but a done-condition that reports success while
  the work is unfinished is worse than no check at all.
- **Report honestly at every wake-up.** State what actually happened, including
  reverts and failures. Never present a partial or masked result as completion.
- Obey `CLAUDE.md` and the objective's own `## Constraints` at all times. They
  outrank speed and outrank finishing.
