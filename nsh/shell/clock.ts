// Today, as a reactive read (F8-13). A screen that shows "today", "days until" or this year reads `today()` inside JSX or a computed: it
// runs again when the day changes (a timer to the next midnight) and when the app comes back to the foreground on another day (a
// laptop opened the next morning: timers do not run while it sleeps). A read made at import time goes stale across midnight and the
// year end; this does not.
import { effect, lifecycle, signal, untracked } from "nsh";
import { isoDate } from "../lib/dates";

const day = signal(isoDate(new Date()));
let timer: ReturnType<typeof setTimeout> | null = null;

/** Arms the timer to one second past the next local midnight (armed on the first read, so a module that is only imported starts none). */
function arm(): void {
  if (timer !== null) clearTimeout(timer);
  const now = new Date();
  const next = new Date(now.getFullYear(), now.getMonth(), now.getDate() + 1, 0, 0, 1);
  timer = setTimeout(() => {
    timer = null;
    refreshToday();
    arm();
  }, Math.max(1000, next.getTime() - now.getTime()));
}

/** Reads the clock again: the readers of today() run again when the day is another one. */
export function refreshToday(): void {
  const iso = isoDate(new Date());
  if (iso !== untracked(day)) day.set(iso);
}

/** Today's date, "YYYY-MM-DD" in local time (reactive: changes at midnight and on coming back to the foreground). */
export function todayIso(): string {
  if (timer === null) arm();
  return day();
}

/** The time now, as a reactive read that changes with the day (not with every second): what "today" and "days until" are computed from. */
export function today(): Date {
  todayIso();
  return new Date();
}

// Back in the foreground: the day may have changed while the app slept.
effect(() => {
  if (lifecycle() === "active") untracked(refreshToday);
});
