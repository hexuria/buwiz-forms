// A test fixture of the Calendar (C3), not a screen: screens/calendar/*.test.tsx and shots/calendar.test.tsx open it. It lives at
// the nsh root because a real-window run serves assets (fonts, icons) from the entry's folder down.
// It is the frame (Shell) on the Calendar over generated data (screens/calendar/seed.ts). What a test asks for goes in
// `open(entry, { host: { params } })`:
//   demo    default true: Juana Santos (the demo taxpayer) with this year and 1601C, 2551Q, 0619E, 2550Q; false: an empty database
//   years   false: the taxpayers have no tax year (so no deadline)
//   branch  true: a second branch of hers (00001, Cebu branch) with 1601C and 2551Q: her rows then name the branch
//   second  true: a second taxpayer (generated: Sample Trading Corp) with this year and 1601C, 2550Q
//   drafts  true: two saved drafts (2551Q Q3, 2550Q Q2), read as Draft once the host has saved them
//   filed   true: the past 0619E returns and the first half-year of 1601C read as filed (generated statuses; nothing is stored)
//   sent    "dry": every deadline due this month reads as a Submitted dry run (only simulated); "real": as a real send (generated)
//   view    "month": the Month view first (the List is the start)
//   start   the dev route (default "view=calendar")
import { host, mount, signal, View } from "nsh";
import "./theme/fonts";
import { createDemoTaxpayer } from "./bir/testing";
import { calendarState } from "./screens/calendar/filters";
import { readSomeAsFiled, readThisMonthAsSent, saveDraft, seed, type SeedOptions } from "./screens/calendar/seed";
import { Shell } from "./shell/Shell";

interface Params extends SeedOptions {
  drafts?: boolean;
  filed?: boolean;
  sent?: "dry" | "real";
  view?: "month";
  start?: string;
}

function Demo() {
  const p = host.params as Params;
  const ready = signal(p.drafts !== true || p.demo === false);
  seed(p);
  if (p.filed && p.demo !== false) readSomeAsFiled();
  if (p.sent && p.demo !== false) readThisMonthAsSent(p.sent === "dry");
  if (p.drafts && p.demo !== false) {
    const tin = createDemoTaxpayer().branches[0].tin;
    void (async () => {
      await saveDraft(tin, "2551Q", 3);
      await saveDraft(tin, "2550Q", 2);
      ready.set(true);
    })();
  }
  if (p.view === "month") calendarState.setView("month");
  return (
    <View class="flex-1">
      <If when={ready()}>
        <Shell start={p.start ?? "view=calendar"} polling={false} />
      </If>
    </View>
  );
}

mount(Demo);
