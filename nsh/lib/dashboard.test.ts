import { describe, expect, test } from "nsh/test";
import { loadDashboard, nextText, pickedDeadlines, progressText, tileSpecs } from "./dashboard";
import type { DashboardDeadline, DashboardReads, DashboardReturn, DashboardTaxpayer } from "./dashboard";

// A clock in the afternoon of Wednesday, October 14, 2026. Fixtures are invented; no real taxpayer is named.
const NOW = new Date(2026, 9, 14, 15, 30);

function deadline(tinRoot: string, formCode: string, dueDate: string, status: string): DashboardDeadline {
  return {
    tin: `${tinRoot}-00000`,
    tin_root: tinRoot,
    branch_code: "00000",
    form_code: formCode,
    taxable_year: 2026,
    period: 0,
    due_date: dueDate,
    status,
  };
}

/** A store whose reads answer from fixtures; deadlines are filtered by due date, as the host filters them. */
function fakeStore(fixtures: {
  deadlines?: DashboardDeadline[];
  saved?: DashboardReturn[];
  filed?: DashboardReturn[];
  taxpayers?: DashboardTaxpayer[];
}): DashboardReads {
  const all = fixtures.deadlines ?? [];
  return {
    deadlines: (from, to) => all.filter((d) => d.due_date >= from && d.due_date <= to),
    savedReturns: () => fixtures.saved ?? [],
    returns: (which) => (which === "filed" ? (fixtures.filed ?? []) : []),
    taxpayers: () => fixtures.taxpayers ?? [],
  };
}

const d1 = deadline("T1", "1601C", "2026-10-10", "Not started"); // overdue
const d2 = deadline("T1", "2550Q", "2026-10-17", "Draft"); // due in 7 days, and on the strip
const d3 = deadline("T2", "1701", "2026-10-30", "Not started"); // later on the strip
const d4 = deadline("T2", "1601C", "2026-10-20", "Submitted"); // not open: never listed
const d5 = deadline("T1", "1701", "2026-03-15", "Submitted"); // this year, filed
const d6 = deadline("T1", "1701Q", "2026-04-15", "Draft"); // open and past: overdue
const d7 = deadline("T2", "1601C", "2025-08-01", "Not started"); // outside the 366-day window
const FIXTURES = {
  deadlines: [d1, d2, d3, d4, d5, d6, d7],
  saved: [
    { tin_root: "T1", status: "Draft", updated_at: "2026-10-01 12:00:00" },
    { tin_root: "T2", status: "Draft", updated_at: "2026-10-02 12:00:00" },
  ],
  filed: [
    { tin_root: "T1", status: "Queued", updated_at: "2026-10-13 12:00:00" },
    { tin_root: "T2", status: "Submitted", updated_at: "2026-10-01 12:00:00" },
    { tin_root: "T2", status: "Paid", updated_at: "2026-09-30 12:00:00" },
    { tin_root: "T1", status: "Confirmed", updated_at: "2026-10-05" },
    { tin_root: "T1", status: "Draft", updated_at: "2026-10-05 12:00:00" },
  ],
  taxpayers: [
    { tin_root: "T1", name: "Example One", tin_display: "T1-00000" },
    { tin_root: "T2", name: "Example Two" },
    { tin_root: "T3", name: "Example Three", archived: true },
  ],
};

describe("loadDashboard", () => {
  const data = loadDashboard(fakeStore(FIXTURES), NOW);
  test("today and the seven-day edge are local dates", () => {
    expect(data.today).toBe("2026-10-14");
    expect(data.in7).toBe("2026-10-21");
  });
  test("overdue: open and past due within the last 366 days; due in 7 days: open and in the window", () => {
    expect(data.overdue.map((d) => d.due_date)).toEqual(["2026-10-10", "2026-04-15"]);
    expect(data.week.map((d) => d.due_date)).toEqual(["2026-10-17"]);
  });
  test("the counts: drafts are the saved returns; waiting is Queued; filed this month is Submitted, Confirmed or Paid this month", () => {
    expect(data.drafts).toBe(2);
    expect(data.waiting).toBe(1);
    expect(data.filedMonth).toBe(2);
  });
  test("the strip is 30 days from today, each with the open deadlines that day", () => {
    expect(data.strip.length).toBe(30);
    expect(data.strip[0].iso).toBe("2026-10-14");
    expect(data.strip[29].iso).toBe("2026-11-12");
    expect(data.strip[3].rows.map((d) => d.due_date)).toEqual(["2026-10-17"]);
    expect(data.strip[16].rows.map((d) => d.due_date)).toEqual(["2026-10-30"]);
    expect(data.strip[1].rows).toEqual([]);
  });
  test("upcoming lists the open deadlines in the strip, not the submitted one", () => {
    expect(data.upcoming.map((d) => d.due_date)).toEqual(["2026-10-17", "2026-10-30"]);
    expect(data.anyDeadlines).toBe(true);
  });
  test("each taxpayer: filed of due this year, and the next open deadline in 60 days; an archived one is left out", () => {
    expect(data.taxpayers.map((row) => row.taxpayer.tin_root)).toEqual(["T1", "T2"]);
    expect(data.taxpayers[0]).toMatchObject({ due: 3, done: 1 });
    expect(data.taxpayers[0].next?.form_code).toBe("2550Q");
    expect(data.taxpayers[1]).toMatchObject({ due: 0, done: 0 });
    expect(data.taxpayers[1].next?.form_code).toBe("1701");
  });
  test("the clock given is not changed", () => {
    expect(NOW.getHours()).toBe(15);
  });
});

describe("loadDashboard: a dry run sent nothing, so it is not filed and its deadline stays open", () => {
  const dryD = (due: string): DashboardDeadline => ({ ...deadline("T1", "1601C", due, "Submitted"), submitted_dry_run: true });
  const realD = (due: string): DashboardDeadline => ({ ...deadline("T1", "0619E", due, "Submitted"), submitted_dry_run: false });
  const send = (stage: string) => ({ stage });
  const data = loadDashboard(
    fakeStore({
      // Past and still due this year, due within a week, and later on the strip: each as a dry run and as a real send.
      deadlines: [dryD("2026-10-10"), realD("2026-10-10"), dryD("2026-10-17"), realD("2026-10-17"), dryD("2026-10-30"), realD("2026-10-30")],
      filed: [
        { tin_root: "T1", status: "Submitted", send: send("test_sent"), updated_at: "2026-10-05 12:00:00" },
        { tin_root: "T1", status: "Submitted", send: null, updated_at: "2026-10-05 12:00:00" },
        { tin_root: "T1", status: "Submitted", send: send("sent"), updated_at: "2026-10-05 12:00:00" },
        { tin_root: "T1", status: "Confirmed", updated_at: "2026-10-05 12:00:00" },
        { tin_root: "T1", status: "Paid", updated_at: "2026-10-05 12:00:00" },
      ],
      taxpayers: [{ tin_root: "T1", name: "Example One" }],
    }),
    NOW,
  );
  test("the dry run is overdue, due in 7 days and on the strip; the real send is none of them", () => {
    expect(data.overdue.map((d) => d.form_code)).toEqual(["1601C"]);
    expect(data.week.map((d) => d.form_code)).toEqual(["1601C"]);
    expect(data.upcoming.map((d) => `${d.form_code} ${d.due_date}`)).toEqual(["1601C 2026-10-17", "1601C 2026-10-30"]);
  });
  test("Filed this month leaves the dry run out; a real send, a relay send, Confirmed and Paid count", () => {
    expect(data.filedMonth).toBe(4);
  });
  test("the taxpayer's progress: the dry run's past deadline is due and not done; its next deadline is the dry run", () => {
    expect(data.taxpayers[0]).toMatchObject({ due: 2, done: 1 });
    expect(data.taxpayers[0].next?.form_code).toBe("1601C");
  });
});

describe("loadDashboard edges", () => {
  test("no deadlines, no returns, no taxpayers: everything is empty and nothing is due", () => {
    const data = loadDashboard(fakeStore({}), NOW);
    expect(data.overdue).toEqual([]);
    expect(data.week).toEqual([]);
    expect(data.drafts).toBe(0);
    expect(data.waiting).toBe(0);
    expect(data.filedMonth).toBe(0);
    expect(data.upcoming).toEqual([]);
    expect(data.anyDeadlines).toBe(false);
    expect(data.taxpayers).toEqual([]);
  });
  test("a month boundary: a filing on the 1st counts, a filing on the 30th of the month before does not", () => {
    const store = fakeStore({
      filed: [
        { tin_root: "T1", status: "Submitted", updated_at: "2026-10-01 12:00:00" },
        { tin_root: "T1", status: "Submitted", updated_at: "2026-09-30 12:00:00" },
      ],
    });
    expect(loadDashboard(store, NOW).filedMonth).toBe(1);
  });
  test("the strip and the window cross New Year", () => {
    const data = loadDashboard(fakeStore({}), new Date(2026, 11, 28, 9, 0));
    expect(data.in7).toBe("2027-01-04");
    expect(data.strip[4].iso).toBe("2027-01-01");
    expect(data.strip[29].iso).toBe("2027-01-26");
  });
  test("progress counts this year only: a new year starts from January 1", () => {
    const store = fakeStore({
      deadlines: [deadline("T1", "1601C", "2026-03-15", "Submitted"), deadline("T1", "1601C", "2027-01-02", "Submitted")],
      taxpayers: [{ tin_root: "T1", name: "Example One" }],
    });
    const row = loadDashboard(store, new Date(2027, 0, 3, 9, 0)).taxpayers[0];
    expect(row.due).toBe(1);
    expect(row.done).toBe(1);
  });
});

describe("tileSpecs", () => {
  test("five tiles, in order, with their captions, icons, tones and counts", () => {
    expect(tileSpecs(loadDashboard(fakeStore(FIXTURES), NOW))).toEqual([
      { key: "overdue", label: "Overdue", sub: "past the due date", tone: "danger", icon: "triangle-alert", n: 2 },
      { key: "week", label: "Due in 7 days", sub: "by Oct 21", tone: "warning", icon: "calendar", n: 1 },
      { key: "drafts", label: "Drafts", sub: "started, not queued", tone: "accent", icon: "pencil", n: 2 },
      { key: "waiting", label: "Waiting to send", sub: "queued to send", tone: "fg2", icon: "send", n: 1 },
      { key: "filed", label: "Filed this month", sub: "submitted in Oct", tone: "success", icon: "circle-check", n: 2 },
    ]);
  });
  test("a due-by date in the next month and the filed month of today (a month boundary)", () => {
    const specs = tileSpecs({
      today: "2026-10-28",
      in7: "2026-11-04",
      overdue: [],
      week: [],
      drafts: 0,
      waiting: 0,
      filedMonth: 0,
    });
    expect(specs[1].sub).toBe("by Nov 4");
    expect(specs[4].sub).toBe("submitted in Oct");
  });
});

describe("pickedDeadlines", () => {
  const data = loadDashboard(fakeStore(FIXTURES), NOW);
  test("with nothing picked, the open deadlines of the strip", () => {
    const picked = pickedDeadlines({ homePick: null, homeDay: null }, data);
    expect(picked.title).toBe("Open deadlines");
    expect(picked.rows.map((d) => d.due_date)).toEqual(["2026-10-17", "2026-10-30"]);
    expect(picked.empty).toBe("Nothing is due in the next 30 days.");
  });
  test("the empty line says how to get deadlines when there are none at all", () => {
    const empty = loadDashboard(fakeStore({}), NOW);
    expect(pickedDeadlines({ homePick: null, homeDay: null }, empty).empty).toBe(
      "Deadlines show here once a taxpayer has forms for a tax year.",
    );
  });
  test("forms with nothing due in the next 30 days: the empty line names the next deadline, not a missing tax year", () => {
    // A business that began on October 1: its first deadlines are the fourth quarter's, in January.
    const late = loadDashboard(
      fakeStore({
        deadlines: [
          deadline("T1", "1701", "2027-04-15", "Not started"),
          deadline("T1", "2551Q", "2027-01-25", "Draft"),
          deadline("T1", "1601C", "2026-12-10", "Submitted"),
        ],
      }),
      NOW,
    );
    expect(late.upcoming).toEqual([]);
    expect(late.later?.form_code).toBe("2551Q");
    expect(late.anyDeadlines).toBe(true);
    expect(pickedDeadlines({ homePick: null, homeDay: null }, late).empty).toBe(
      "Nothing is due in the next 30 days. The next is 2551Q on January 25, 2027.",
    );
    // Everything later already filed: still deadlines, so not the fresh line, and no next one to name.
    const filed = loadDashboard(fakeStore({ deadlines: [deadline("T1", "2551Q", "2027-01-25", "Submitted")] }), NOW);
    expect(filed.later).toBe(null);
    expect(pickedDeadlines({ homePick: null, homeDay: null }, filed).empty).toBe("Nothing is due in the next 30 days.");
  });
  test("overdue and due-in-7-days pick their own lists", () => {
    const overdue = pickedDeadlines({ homePick: "overdue", homeDay: null }, data);
    expect(overdue).toMatchObject({ title: "Overdue", empty: "Nothing is overdue." });
    expect(overdue.rows.length).toBe(2);
    const week = pickedDeadlines({ homePick: "week", homeDay: null }, data);
    expect(week).toMatchObject({ title: "Due in the next 7 days", empty: "Nothing is due in the next 7 days." });
    expect(week.rows.map((d) => d.due_date)).toEqual(["2026-10-17"]);
  });
  test("a picked day titles itself with its weekday and date", () => {
    const saturday = pickedDeadlines({ homePick: "day", homeDay: "2026-10-17" }, data);
    expect(saturday.title).toBe("Saturday, October 17");
    expect(saturday.rows.map((d) => d.due_date)).toEqual(["2026-10-17"]);
    const sunday = pickedDeadlines({ homePick: "day", homeDay: "2026-10-18" }, data);
    expect(sunday).toEqual({ title: "Sunday, October 18", rows: [], empty: "Nothing due this day." });
  });
  test("a day that is not on the strip falls back to the open deadlines", () => {
    expect(pickedDeadlines({ homePick: "day", homeDay: "2027-06-01" }, data).title).toBe("Open deadlines");
    expect(pickedDeadlines({ homePick: "day", homeDay: null }, data).title).toBe("Open deadlines");
  });
});

describe("progressText and nextText", () => {
  test("progress reads filed of due, or nothing due yet", () => {
    expect(progressText({ due: 3, done: 1 })).toBe("1 of 3 filed this year");
    expect(progressText({ due: 0, done: 0 })).toBe("Nothing was due yet this year");
  });
  test("the next deadline names its form and date, or that nothing is due in 60 days", () => {
    expect(nextText({ form_code: "2550Q", due_date: "2026-10-17" })).toBe("Next: 2550Q · Oct 17");
    expect(nextText({ form_code: "1601C", due_date: "2027-01-02" })).toBe("Next: 1601C · Jan 2");
    expect(nextText(null)).toBe("Nothing due in 60 days");
  });
});

