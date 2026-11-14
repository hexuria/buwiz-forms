// The app's polls, one small helper (plan: "Refresh stays poll-based like today ... behind one small lib/poll.ts").
// Today's three: the submission worker's events every 30 s, the account server's device filings every 5 s, the updater's
// status every second. The app root starts them when it mounts and stops them when it unmounts; headless tests do not
// start them unless a test asks. No nsh import: plain timers, so it runs in a pure test.

/** Milliseconds between the runs of each poll. */
export const POLL = { events: 30_000, deviceFilings: 5_000, update: 1_000 } as const;

/** One repeating job. */
export interface PollTask {
  name: string;
  /** Milliseconds between the end of one run and the start of the next. */
  every: number;
  /** Milliseconds before the first run (default: every). */
  first?: number;
  run: () => void | Promise<void>;
}

/**
 * Starts every task and returns the function that stops them all. A task runs one turn at a time (the next waits for the
 * end of the one running, so a slow host call never piles up); a throw goes to onError and the task goes on.
 */
export function startPolls(tasks: readonly PollTask[], onError?: (name: string, error: unknown) => void): () => void {
  let stopped = false;
  const timers = new Map<string, ReturnType<typeof setTimeout>>();

  const schedule = (task: PollTask, delay: number) => {
    timers.set(
      task.name,
      setTimeout(async () => {
        if (stopped) return;
        try {
          await task.run();
        } catch (error) {
          onError?.(task.name, error);
        }
        if (!stopped) schedule(task, task.every);
      }, delay),
    );
  };

  for (const task of tasks) schedule(task, task.first ?? task.every);
  return () => {
    stopped = true;
    for (const timer of timers.values()) clearTimeout(timer);
    timers.clear();
  };
}
