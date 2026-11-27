// Settings > Jobs at phone size and at desktop size: the status filters, Archive, Delete, Purge archived, Cancel queue, and
// the empty states. The log is the Logs section (logs.test.tsx). The job table is the fixture's (SettingsDemo.tsx jobs):
// the scratch database has no jobs.
import { expect, test } from "nsh/test";
import { calls, expectTexts, openSettings, SIZES } from "./testing";

const JOBS = {
  queued: "id:job-submission-41",
  inbox: "id:job-job-1",
  failed: "id:job-job-2",
  archived: "id:job-job-3",
  rebuilt: "id:job-job-4",
};
const ALL = Object.values(JOBS);

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
}
