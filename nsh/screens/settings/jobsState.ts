// Jobs and logs (old cron_tasks.js and the app's jobs actions): the tab, the status and level filters, the last error, the
// log read, and the writes (cancel a queued filing, archive, delete, purge). One owner: this file's private signals.
import { signal } from "nsh";
import { bir, type BackgroundJob, type JobLogTail } from "../../bir";
import { errorText } from "../../lib/errors";
import { app, cancelQueue, store } from "../../shell";

/** The two tabs of the Jobs section. */
export type JobsTab = "jobs" | "logs";
/** What the Logs tab reads: the host's tail, or the reason it could not. */
export type LogTail = JobLogTail & { error?: string };

const tab = signal<JobsTab>("jobs");
const filter = signal("All Jobs");
const logFilter = signal("All Logs");
const failure = signal<string | null>(null);
// The next session starts from the defaults (a lock, a sign-out or another taxpayer signing in).
app.onSessionChange(() => {
  tab.set("jobs");
  filter.set("All Jobs");
  logFilter.set("All Logs");
  failure.set(null);
});

/** The store key of the log read: forgotten to read the file again (the tab opens, Refresh). */
export const LOG_KEY = "job_log_tail";
const EMPTY_LOG: LogTail = { file_name: "", lines: 0, content: "" };

function readLog(): LogTail {
  try {
    return bir.job_log_tail();
  } catch (error) {
    return { ...EMPTY_LOG, error: errorText(error) };
  }
}

/** The view state, read-only. */
export const jobsUi = {
  tab: (): JobsTab => tab(),
  filter: (): string => filter(),
  logFilter: (): string => logFilter(),
  /** The last write that failed, in the host's words; the next one that works clears it. */
  failure: (): string | null => failure(),
  /** Every job: stored jobs and queued filings. Cached; the 30-second poll and every write read it again. */
  jobs: (): BackgroundJob[] => store.backgroundJobs(),
  /** The log's tail (today's file), cached until Refresh or the Logs tab opens. */
  log: (): LogTail => store.read(LOG_KEY, readLog, EMPTY_LOG),
};

/** Opening Logs reads today's file afresh. */
export function setTab(next: JobsTab): void {
  // Forgotten first: the tab reads the file as it opens, once.
  if (next === "logs") store.forget(LOG_KEY);
  tab.set(next);
}

export function setFilter(next: string): void {
  filter.set(next);
}

export function setLogFilter(next: string): void {
  logFilter.set(next);
}

export function refreshLog(): void {
  store.forget(LOG_KEY);
}

/** A write on one job; the error stays on the tab. */
function jobWrite(run: () => void): void {
  try {
    run();
    failure.set(null);
  } catch (error) {
    failure.set(errorText(error));
  }
}

export function deleteJob(id: number): void {
  jobWrite(() => store.write("delete_background_job", id));
}

export function archiveJob(id: number): void {
  jobWrite(() => store.write("archive_background_job", id));
}

/** Removes every archived job and shows all jobs again. */
export function purgeArchived(): void {
  jobWrite(() => {
    store.write("purge_archived_jobs");
    filter.set("All Jobs");
  });
}

/** Takes a queued filing out of the queue: the return is an editable draft again. */
export function cancelQueued(job: BackgroundJob): void {
  cancelQueue(job.tin ?? "", job.form_code ?? "", job.taxable_year ?? 0, job.quarter ?? job.month ?? null);
}
