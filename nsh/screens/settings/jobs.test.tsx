// Settings > Jobs at phone size and at desktop size: the Jobs tab (the status filters, Archive, Delete, Purge archived, Cancel queue, the
// empty states) and the Logs tab (the level filters, the last 200 lines, Refresh, no file, an unreadable file). The job table and the log
// are the fixture's (SettingsDemo.tsx jobs, log): the scratch database has no jobs, and its log file is the host's.
import { expect, test } from "nsh/test";
import { shoot } from "../../shots/shoot";
import { calls, expectTexts, openSettings, SIZES, WIDE } from "./testing";

const JOBS = {
  queued: "id:job-submission-41",
  inbox: "id:job-job-1",
  failed: "id:job-job-2",
  archived: "id:job-job-3",
  rebuilt: "id:job-job-4",
};
const ALL = Object.values(JOBS);

test("startup writes today's log and Jobs shows its lines", async () => {
  const app = await openSettings(WIDE, {}, "jobs");
  await app.press("id:jobs-tabs >> text:Logs");
  await app.waitFor("text:/eBIRForms started/");
  const today = new Date().toISOString().slice(0, 10);
  expect(app.texts().some((text) => text.includes(`ebirforms.${today}.log`))).toBe(true);
  await shoot(app, "nsh-startup-log");
});

/** Which of the five rows show. */
function shown(app: Awaited<ReturnType<typeof openSettings>>): string[] {
  return ALL.filter((id) => app.shows(id));
}

for (const { name, size } of SIZES) {
  test(`${name}: every job is listed with its schedule, runs and status; the status filters keep their own`, async () => {
    const app = await openSettings(size, { jobs: true }, "jobs");
    await app.waitFor("id:jobs-list");
    expectTexts(app, [
      "Submit 2551Q Q1 2026 for Juana Santos", "Runs when the queue wakes", "Queued",
      "Check the inbox for BIR confirmations", "Schedule: */15 * * * * · last 2026-11-15 08:45:00 · next 2026-11-15 09:00:00",
      "Send reminder digest", "Schedule: 0 8 * * 1 · last 2026-11-10 08:00:00", "Failed",
      "Old export", "Manual", "Archived", "Rebuild the form index", "Completed",
    ]);
    expect(shown(app)).toEqual(ALL);
    // The name and the line under it are one wrapping block (a job's name carries a TIN and a mailbox: never cut to one line with an
    // ellipsis, as the 640-wide column did).
    for (const id of ALL) expect(app.shows(`${id}-words`)).toBe(true);
    const only = async (filter: string, rows: string[]) => {
      await app.press(`id:jobs-filter >> text:${filter}`);
      expect(shown(app)).toEqual(rows);
    };
    await only("Queued", [JOBS.queued]);
    await only("Failed", [JOBS.failed]);
    await only("Completed", [JOBS.inbox, JOBS.rebuilt]);
    await only("Archived", [JOBS.archived]);
    await only("All Jobs", ALL);
  }, 60_000);

  test(`${name}: Archive moves a job to Archived, Delete removes it, Purge archived clears the archive and shows all jobs again`, async () => {
    const app = await openSettings(size, { jobs: true }, "jobs");
    await app.waitFor("id:jobs-list");
    // Only a stored job that is not archived has Archive; a queued filing has Cancel queue and nothing else.
    expect(app.shows("id:job-job-1-archive")).toBe(true);
    expect(app.shows("id:job-job-3-archive")).toBe(false);
    expect(app.shows("id:job-job-3-delete")).toBe(true);
    expect(app.shows("id:job-submission-41-archive")).toBe(false);
    expect(app.shows("id:job-submission-41-delete")).toBe(false);
    expect(app.shows("id:job-submission-41-cancel")).toBe(true);

    await app.press("id:job-job-1-archive");
    await app.waitFor("text:calls: archive_background_job 1");
    expect(app.shows("id:job-job-1-archive")).toBe(false);
    await app.press("id:jobs-filter >> text:Archived");
    expect(shown(app)).toEqual([JOBS.inbox, JOBS.archived]);

    await app.press("id:job-job-3-delete");
    await app.waitFor("text:calls: archive_background_job 1 | delete_background_job 3");
    expect(shown(app)).toEqual([JOBS.inbox]);

    await app.press("id:jobs-purge");
    await app.waitFor("text:/purge_archived_jobs/");
    // The filter went back to All Jobs, and no archived job is left.
    expect(shown(app)).toEqual([JOBS.queued, JOBS.failed, JOBS.rebuilt]);
    await app.press("id:jobs-filter >> text:Archived");
    await app.waitFor("id:jobs-empty");
    expect(app.texts()).toContain("No background jobs in this view.");
  }, 60_000);

  test(`${name}: Cancel queue takes a queued filing out of the queue and says the return is a draft again`, async () => {
    const app = await openSettings(size, { jobs: true }, "jobs");
    await app.waitFor("id:job-submission-41-cancel");
    await app.press("id:job-submission-41-cancel");
    await app.waitFor("text:Queue cancelled. The return is an editable draft again.");
    expect(calls(app)).toEqual(["cancel_queued 00012345300000,2551Q,2026,1"]);
    expect(app.shows("id:job-submission-41")).toBe(false);
  }, 60_000);

  test(`${name}: with no jobs the list says so, under every filter`, async () => {
    const app = await openSettings(size, {}, "jobs");
    await app.waitFor("id:jobs-empty");
    expect(app.texts()).toContain("No background jobs in this view.");
    await app.press("id:jobs-filter >> text:Failed");
    expect(app.shows("id:jobs-empty")).toBe(true);
  }, 60_000);

  test(`${name}: the Logs tab tails the log, colours its levels, filters by level and reads the file again on Refresh`, async () => {
    const app = await openSettings(size, { log: "mixed" }, "jobs");
    await app.waitFor("id:jobs-tab");
    await app.press("id:jobs-tabs >> text:Logs");
    await app.waitFor("id:logs-tab");
    expectTexts(app, ["ebirforms.2026-11-15.log · 8 lines"]);
    const lines = () => app.texts().filter((t) => /^2026-11-15T|^a continuation/.test(t));
    expect(lines()).toHaveLength(8);
    expect(lines()[lines().length - 1]).toMatch(/fixture log read 1$/);

    await app.press("id:logs-filter >> text:Error");
    expect(lines().map((l) => l.includes(" ERROR ") || l.includes(" FATAL "))).toEqual([true, true]);
    await app.press("id:logs-filter >> text:Warn");
    expect(lines()).toHaveLength(1);
    expect(lines()[0]).toContain(" WARN ");
    await app.press("id:logs-filter >> text:Info");
    expect(lines()).toHaveLength(4);
    // The line with no level is only in All.
    await app.press("id:logs-filter >> text:All");
    expect(app.texts()).toContain("a continuation line with no level");
    // The headline counts the whole file, and Refresh reads it again (the fixture adds a line saying which read this is).
    await app.press("id:logs-refresh");
    await app.waitFor("text:/fixture log read 2$/");
    expect(app.texts().some((t) => /fixture log read 1$/.test(t))).toBe(false);
    // Opening Logs again reads the file once more.
    await app.press("id:jobs-tabs >> text:Jobs");
    await app.waitFor("id:jobs-tab");
    await app.press("id:jobs-tabs >> text:Logs");
    await app.waitFor("text:/fixture log read 3$/");
  }, 60_000);

  test(`${name}: only the last 200 lines of a long log show, and the headline says so`, async () => {
    const app = await openSettings(size, { log: "long" }, "jobs");
    await app.press("id:jobs-tabs >> text:Logs");
    await app.waitFor("id:logs-tab");
    expect(app.texts()).toContain("ebirforms.2026-11-15.log · 301 lines · showing the last 200");
    const lines = app.texts().filter((t) => /bir::app line \d+$|fixture log read/.test(t));
    expect(lines).toHaveLength(200);
    expect(lines.some((l) => /bir::app line 300$/.test(l))).toBe(true);
    expect(lines.some((l) => /bir::app line 101$/.test(l))).toBe(false);
    expect(lines.some((l) => /bir::app line 102$/.test(l))).toBe(true);
    // Everything in it is Info: no Error line, and the filter says so.
    await app.press("id:logs-filter >> text:Error");
    await app.waitFor("id:logs-empty");
    expect(app.texts()).toContain("No lines at this level.");
  }, 60_000);

  test(`${name}: a log with no file yet, and a log that cannot be read`, async () => {
    const none = await openSettings(size, { log: "empty" }, "jobs");
    await none.press("id:jobs-tabs >> text:Logs");
    await none.waitFor("id:logs-empty");
    expect(none.texts()).toContain("No log entries yet today.");
    expect(none.texts()).toContain("No log file yet · 0 lines");
    const broken = await openSettings(size, { log: "error" }, "jobs");
    await broken.press("id:jobs-tabs >> text:Logs");
    await broken.waitFor("id:logs-error");
    expect(broken.texts()).toContain("The log file could not be read.");
  }, 60_000);
}
