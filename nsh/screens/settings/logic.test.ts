// The pure rules of the Settings screens: log lines and levels, the job filters, how a job reads, and the choices of the selects.
import { expect, test } from "nsh/test";
import type { BackgroundJob } from "../../bir";
import {
  filterJobs, groupSections, jobActions, jobKey, jobSub, jobTone, levelTone, LOG_LINES, logHeadline, logLineLevel, logView,
  retentionChoices, serverHint,
} from "./logic";

const job = (extra: Partial<BackgroundJob>): BackgroundJob => ({
  kind: "job", id: 1, name: "n", job_type: "Manual", cron_expr: null, status: "Completed", retries: 0, last_run_at: null, next_run_at: null, created_at: "", output_log: null, ...extra,
});

test("a log line's level is read from the markers bir-core classifies on", () => {
  expect(logLineLevel("2026-11-15T08:00:00Z ERROR bir::x boom")).toBe("Error");
  expect(logLineLevel("2026-11-15T08:00:00Z FATAL bir::x boom")).toBe("Error");
  expect(logLineLevel("2026-11-15T08:00:00Z  WARN bir::x careful")).toBe("Warn");
  expect(logLineLevel("2026-11-15T08:00:00Z  INFO bir::x fine")).toBe("Info");
  expect(logLineLevel("a continuation line")).toBe("Other");
  // A word that merely contains a marker is not one.
  expect(logLineLevel("INFORMATION about ERRORS")).toBe("Other");
  expect(levelTone("Error")).toBe("danger");
  expect(levelTone("Warn")).toBe("warning");
  expect(levelTone("Info")).toBe("fg");
  expect(levelTone("Other")).toBe("fg2");
});

test("the log view drops the empty last line, filters by level and keeps the newest 200", () => {
  const view = logView("a\n INFO b\n ERROR c\n", "All Logs");
  expect(view.shown.map((l) => l.line)).toEqual(["a", " INFO b", " ERROR c"]);
  expect(view.matching).toBe(3);
  expect(logView("a\n INFO b\n ERROR c\n", "Error").shown.map((l) => l.line)).toEqual([" ERROR c"]);
  expect(logView("", "All Logs").shown).toEqual([]);
  expect(logView(null, "Info").matching).toBe(0);
  const many = Array.from({ length: 450 }, (_, i) => ` INFO line ${i}`).join("\n");
  const big = logView(many, "Info");
  expect(big.matching).toBe(450);
  expect(big.shown).toHaveLength(LOG_LINES);
  expect(big.shown[0].line).toBe(" INFO line 250");
  expect(big.shown[0].index).toBe(0);
  expect(big.shown[LOG_LINES - 1].line).toBe(" INFO line 449");
  expect(logHeadline("f.log", 450, 450)).toBe("f.log · 450 lines · showing the last 200");
  expect(logHeadline("f.log", 12, 3)).toBe("f.log · 12 lines");
  expect(logHeadline("", 0, 0)).toBe("No log file yet · 0 lines");
});

test("a status filter keeps the jobs of that status; All Jobs keeps every one", () => {
  const jobs = [job({ id: 1, status: "Queued" }), job({ id: 2, status: "Failed" }), job({ id: 3, status: "Archived" }), job({ id: 4, status: "Failed" })];
  expect(filterJobs(jobs, "All Jobs")).toHaveLength(4);
  expect(filterJobs(jobs, "Failed").map((j) => j.id)).toEqual([2, 4]);
  expect(filterJobs(jobs, "Completed")).toEqual([]);
});

test("a job reads as its schedule, then its last and next run", () => {
  expect(jobSub(job({ cron_expr: "0 8 * * 1", last_run_at: "2026-11-10", next_run_at: "2026-11-17" }))).toBe("Schedule: 0 8 * * 1 · last 2026-11-10 · next 2026-11-17");
  expect(jobSub(job({ job_type: "System" }))).toBe("Runs when the queue wakes");
  expect(jobSub(job({ job_type: "Manual", last_run_at: "2026-11-10" }))).toBe("Manual · last 2026-11-10");
  expect(jobTone("Queued")).toBe("accent");
  expect(jobTone("Running")).toBe("accent");
  expect(jobTone("Failed")).toBe("danger");
  for (const ok of ["Completed", "Submitted", "Confirmed", "Paid"]) expect(jobTone(ok)).toBe("success");
  expect(jobTone("Archived")).toBe("neutral");
});

test("a job row offers Cancel queue, Archive and Delete as the old row did", () => {
  expect(jobActions(job({ kind: "submission", status: "Queued", id: 5 }))).toEqual({ cancel: true, archive: false, remove: false });
  expect(jobActions(job({ kind: "submission", status: "Submitted", id: 5 }))).toEqual({ cancel: false, archive: false, remove: false });
  expect(jobActions(job({ status: "Completed" }))).toEqual({ cancel: false, archive: true, remove: true });
  expect(jobActions(job({ status: "Archived" }))).toEqual({ cancel: false, archive: false, remove: true });
  expect(jobActions(job({ id: null }))).toEqual({ cancel: false, archive: false, remove: false });
  expect(jobKey(job({ id: 7 }))).toBe("job-7");
  expect(jobKey(job({ kind: "submission", id: 7 }))).toBe("submission-7");
});

test("the server hint says where the address came from", () => {
  expect(serverHint("env")).toBe("From BUWIZ_SERVER_URL for this run.");
  expect(serverHint("build")).toBe("Set when this app was built.");
  expect(serverHint("invalid")).toBe("That address was rejected.");
  expect(serverHint("default")).toBe("Chosen for this build.");
  expect(serverHint(undefined)).toBe("Chosen for this build.");
});

test("the retention choices are the host's, with an unusual current one added", () => {
  expect(retentionChoices({ log_retention_choices: [3, 7, 14, 30, 90], log_retention_days: 7 }).map((o) => o.label)).toEqual(["3 days", "7 days", "14 days", "30 days", "90 days"]);
  expect(retentionChoices({ log_retention_choices: [3, 7], log_retention_days: 21 }).map((o) => o.value)).toEqual(["3", "7", "21"]);
  expect(retentionChoices(null).map((o) => o.value)).toEqual(["3", "7", "14", "30", "90"]);
  expect(retentionChoices({ log_retention_choices: [1, 7], log_retention_days: 1 })[0].label).toBe("1 day");
});

test("sections are grouped in the order of the view", () => {
  const s = (id: string, group: string) => ({ id, label: id, icon: "x", group, rows: [] });
  expect(groupSections([s("a", "One"), s("b", "One"), s("c", "Two")]).map((g) => [g.group, g.sections.map((x) => x.id)])).toEqual([
    ["One", ["a", "b"]],
    ["Two", ["c"]],
  ]);
  expect(groupSections([])).toEqual([]);
});
