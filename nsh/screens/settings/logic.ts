// The pure rules of the Settings screens (old settings.js, cron_tasks.js, account.js): which words a log line is, which
// jobs a filter keeps, how a job reads, the choices of the retention select. No nsh import, no host call.
import type { BackgroundJob, SystemInfo } from "../../bir";
import type { ChipTone } from "../../ui";
import { retentionLabel, type SettingsPreset, type SettingsViewSection } from "../../lib/settings_model";

/** The status filters of the Jobs list, in the order they show. */
export const STATUS_FILTERS = ["All Jobs", "Queued", "Failed", "Completed", "Archived"] as const;
/** The log's level filters: the stored value and the label. */
export const LOG_FILTERS = [
  { value: "All Logs", label: "All" },
  { value: "Error", label: "Error" },
  { value: "Warn", label: "Warn" },
  { value: "Info", label: "Info" },
] as const;
/** Lines drawn from the tail of the log; older ones stay in the file. */
export const LOG_LINES = 200;

/** A log line's level, by the same markers bir-core classifies on (log_line_level): " ERROR ", " FATAL ", " WARN ", " INFO ". */
export type LogLevel = "Error" | "Warn" | "Info" | "Other";

/** The level of one log line. */
export function logLineLevel(line: string): LogLevel {
  if (line.includes(" ERROR ") || line.includes(" FATAL ")) return "Error";
  if (line.includes(" WARN ")) return "Warn";
  if (line.includes(" INFO ")) return "Info";
  return "Other";
}

/** The text tone a level is drawn in. */
export function levelTone(level: LogLevel): "danger" | "warning" | "fg" | "fg2" {
  if (level === "Error") return "danger";
  if (level === "Warn") return "warning";
  if (level === "Info") return "fg";
  return "fg2";
}

/** What the Logs tab shows of one file's text: the matching lines, and the last LOG_LINES of them. */
export interface LogView {
  /** Lines at the chosen level, every one of the file's tail. */
  matching: number;
  /** The newest LOG_LINES of them, oldest first. */
  shown: { index: number; line: string; level: LogLevel }[];
}

/** Splits the tail the host gave (a trailing newline leaves no empty last line, as str::lines) and keeps one level. */
export function logView(content: string | null | undefined, filter: string): LogView {
  const lines = String(content ?? "").split("\n");
  if (lines.length && lines[lines.length - 1] === "") lines.pop();
  const matching = lines.filter((line) => filter === "All Logs" || logLineLevel(line) === filter);
  const shown = matching.slice(-LOG_LINES).map((line, index) => ({ index, line, level: logLineLevel(line) }));
  return { matching: matching.length, shown };
}

/** The Logs tab's header line: the file, how many lines it has, and that only the last ones show. */
export function logHeadline(file: string | null | undefined, lines: number, matching: number): string {
  return `${file || "No log file yet"} · ${lines} lines${matching > LOG_LINES ? ` · showing the last ${LOG_LINES}` : ""}`;
}

/** The jobs a status filter keeps. */
export function filterJobs(jobs: readonly BackgroundJob[], filter: string): BackgroundJob[] {
  return filter === "All Jobs" ? [...jobs] : jobs.filter((job) => job.status === filter);
}

/** The chip tone of a job's status. */
export function jobTone(status: string): ChipTone {
  if (status === "Queued" || status === "Running") return "accent";
  if (status === "Failed") return "danger";
  if (["Completed", "Submitted", "Confirmed", "Paid"].includes(status)) return "success";
  return "neutral";
}

/** The line under a job's name: its schedule, then when it last ran and runs next. */
export function jobSub(job: BackgroundJob): string {
  const schedule = job.cron_expr ? `Schedule: ${job.cron_expr}` : job.job_type === "System" ? "Runs when the queue wakes" : "Manual";
  const runs = [job.last_run_at ? `last ${job.last_run_at}` : null, job.next_run_at ? `next ${job.next_run_at}` : null].filter(Boolean).join(" · ");
  return [schedule, runs].filter(Boolean).join(" · ");
}

/** What a job row offers: a queued filing can be cancelled; a stored job can be archived (unless it is) and deleted. */
export function jobActions(job: BackgroundJob): { cancel: boolean; archive: boolean; remove: boolean } {
  const stored = job.kind === "job" && job.id != null;
  return { cancel: job.kind === "submission" && job.status === "Queued", archive: stored && job.status !== "Archived", remove: stored };
}

/** The key a job row is identified by. */
export function jobKey(job: BackgroundJob): string {
  return `${job.kind}-${job.id}`;
}

const PRODUCTION_SERVER = "https://api.buwiz.com";

/**
 * Whether Settings shows the account server's address. The ordinary build talks to Buwiz's own server, and naming it
 * reads like a developer setting; the row is for a build or a run that points somewhere else, or at a rejected address.
 */
export function showsServer(status: { server_url: string; server_url_source: string } | null | undefined): boolean {
  if (!status) return false;
  if (status.server_url_source === "invalid" || status.server_url_source === "env") return true;
  return status.server_url.trim().replace(/\/+$/, "") !== PRODUCTION_SERVER;
}

/** The line under who is signed in: the host's detail, unless it only repeats the title ("Not signed in" / "Not signed in."). */
export function accountHint(who: string, detail: string): string {
  return detail.trim().replace(/\.$/, "") === who.trim() ? "" : detail;
}

/** The words under the cloud account's server row. */
export function serverHint(source: string | null | undefined): string {
  if (source === "env") return "From BUWIZ_SERVER_URL for this run.";
  if (source === "build") return "Set when this app was built.";
  if (source === "invalid") return "That address was rejected.";
  return "Chosen for this build.";
}

/** The retention select's choices: the host's, and the current one when it is none of them. */
export function retentionChoices(info: Pick<SystemInfo, "log_retention_choices" | "log_retention_days"> | null | undefined): SettingsPreset[] {
  const days = info?.log_retention_choices?.length ? info.log_retention_choices : [3, 7, 14, 30, 90];
  const choices = days.map((d) => ({ value: String(d), label: retentionLabel(d) }));
  const current = String(info?.log_retention_days || 7);
  if (!choices.some((option) => option.value === current)) choices.push({ value: current, label: retentionLabel(current) });
  return choices;
}

/** The sections of a settings view grouped by their group name, in order. */
export function groupSections(view: readonly SettingsViewSection[]): { group: string; sections: SettingsViewSection[] }[] {
  const out: { group: string; sections: SettingsViewSection[] }[] = [];
  for (const section of view) {
    const last = out[out.length - 1];
    if (last && last.group === section.group) last.sections.push(section);
    else out.push({ group: section.group, sections: [section] });
  }
  return out;
}

/** Where a request to delete the account goes: an email to support, naming the account when it is known. */
export function deletionRequestUrl(email: string | null | undefined): string {
  const who = email ? " for " + email : "";
  const body = "Please delete my Buwiz account" + who + " and the email address held with it.";
  return "mailto:support@buwiz.com?subject=" + encodeURIComponent("Delete my Buwiz account") + "&body=" + encodeURIComponent(body);
}
