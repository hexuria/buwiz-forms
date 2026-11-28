// Home (the old shell's home.js and dashboard.js): the greeting, four status tiles, the next 30 days, the deadlines they pick,
// each taxpayer at a glance, the as-needed forms, recently filed; on a fresh install, the welcome with three numbered steps.
// The numbers come from lib/dashboard.ts over the store's cached reads. Tiles: Overdue and Due in 7 days narrow the list under
// the strip, Drafts and Waiting to send open Filings with a preset (preset.ts).
import { cleanup, computed, signal, View, windowSize } from "nsh";
import type { AsNeededForm, AsNeededReturn, DeadlineRow, ReturnRow } from "../../bir";
import { loadDashboard, pickedDeadlines, tileSpecs, type HomeView, type TileSpec } from "../../lib/dashboard";
import { MONTHS_LONG } from "../../lib/dates";
import { app, contentColumn, go, store, today } from "../../shell";
import { SAVED_RETURNS_KEY } from "../../shell/store";
import { recentLabel, visibleRecent } from "../../shell/Sidebar";
import { recentOpenOptions } from "../../lib/recent";
import { AppButton, deviceCopy, EmptyRow, Icon, type IconName, ListGroup, ListRow, NameCell, PageHeader, SectionTitle, StepCircle, Text, TOUCH } from "../../ui";
import { WelcomeNote } from "../auth/welcome";
import { HomeMigrationNote } from "../auth/migrationNote";
import { AsNeededHead, AsNeededUndated, DeadlineStrip, DueRow, FiledRow, StatTile, TaxpayerRow } from "./parts";
import { homePick } from "./pick";
import { openFilings } from "./preset";
import { deadlineKey } from "./rows";

const WEEKDAYS = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];

function greeting(now: Date): string {
  const h = now.getHours();
  return h < 12 ? "Good morning" : h < 18 ? "Good afternoon" : "Good evening";
}

function longToday(now: Date): string {
  return `${WEEKDAYS[now.getDay()]}, ${MONTHS_LONG[now.getMonth()]} ${now.getDate()}, ${now.getFullYear()}`;
}

/** The page's width before its first layout: the window less the frame, as the frame's page column would have it. */
function firstWidth(): number {
  if (app.layout.narrow()) return windowSize().width - 32;
  return contentColumn("page", app.layout.mainWidth());
}

/** Tiles per row: four from 640 wide, two under it. */
function tileColumns(width: number): number {
  return width >= 640 ? 4 : 2;
}

const TILE_GAP = 8;

// ---------------------------------------------------------------- welcome

const STEPS: { icon: IconName; title: string; line: string }[] = [
  { icon: "user", title: "New tax profile", line: "TIN, registered name and head-office address" },
  { icon: "calendar", title: "Add a tax year", line: "Any year up to next year, per branch" },
  { icon: "file-text", title: "Choose its forms", line: "All 43 BIR forms, or start from a pack" },
];

/** A fresh install: what to do first, in three numbered steps, and the button for the first. */
function Welcome() {
  return (
    <View testID="home-welcome" class="w-full">
      <PageHeader
        title="Welcome to eBIRForms"
        lede={deviceCopy("Prepare, print and track BIR returns for every taxpayer you handle. Everything stays on this computer.")}
      />
      <View class="mt-7">
        <ListGroup testID="welcome-steps">
          <Each of={STEPS} key={(s) => s.title}>
            {(s, i) => (
              <ListRow testID={`welcome-step-${i() + 1}`}>
                <StepCircle label={String(i() + 1)} />
                <Icon name={s().icon} />
                <NameCell title={s().title} sub={s().line} />
              </ListRow>
            )}
          </Each>
          <ListRow testID="welcome-cta">
            <AppButton testID="welcome-add-taxpayer" label="New tax profile" variant="primary" icon="plus" onPress={() => go("new_taxpayer")} />
          </ListRow>
        </ListGroup>
      </View>
    </View>
  );
}

// ---------------------------------------------------------------- dashboard

/** A row of the as-needed list: a form's own row, or one of its returns that holds no trigger date yet. */
interface AsNeededEntry {
  key: string;
  form: AsNeededForm;
  undated: AsNeededReturn | null;
}

function asNeededEntries(forms: AsNeededForm[]): AsNeededEntry[] {
  const out: AsNeededEntry[] = [];
  for (const form of forms) {
    const id = `as-needed-${form.tin}-${form.form_code}`;
    out.push({ key: id, form, undated: null });
    for (const r of form.undated) out.push({ key: `${id}-${r.draft_id}`, form, undated: r });
  }
  return out;
}

/** The tile's press: Overdue and Due in 7 days narrow the list below; the others open Filings with their preset. */
function pressTile(spec: TileSpec): void {
  if (spec.key === "overdue" || spec.key === "week") homePick.toggleTile(spec.key);
  else if (spec.key === "drafts") openFilings({ statuses: ["Draft"] });
  else if (spec.key === "waiting") openFilings({ statuses: ["Queued"] });
}

function Dashboard(props: { width: () => number }) {
  // Rows under 560 points drop their when column and code; the filed row's actions need 640 and the taxpayer row's bar 720.
  const compact = () => props.width() < 560;
  const filedCompact = () => props.width() < 640;
  const taxpayerCompact = () => props.width() < 720;
  const data = computed(() => loadDashboard(store, today()));
  const view = (): HomeView => ({ homePick: homePick.pick(), homeDay: homePick.day() });
  const picked = computed(() => pickedDeadlines(view(), data()));
  const rows = () => picked().rows as DeadlineRow[];
  const specs = computed(() => tileSpecs(data()));
  const filed = computed<ReturnRow[]>(() => store.returns("filed").slice(0, 3));
  /** The Recent list a phone or tablet shows here (no sidebar): the sidebar's returns, five at most. */
  const recentRows = computed(() => visibleRecent().slice(0, 5));
  const asNeeded = computed(() => asNeededEntries(store.asNeeded(today().getFullYear())));
  const tileWidth = () => {
    const cols = tileColumns(props.width());
    return Math.floor((props.width() - TILE_GAP * (cols - 1)) / cols);
  };
  return (
    <View testID="home-dashboard" class="w-full">
      <PageHeader title={greeting(today())} lede={`${longToday(today())}.`} />
      {/* The permanent failure's note, once per account (§A.3.4); afterwards it lives under Settings › Account. */}
      <HomeMigrationNote />
      <View testID="home-tiles" class="mt-6 w-full flex-row flex-wrap" style={{ gap: TILE_GAP }}>
        <Each of={specs()} key={(s) => s.key}>
          {(s) => (
            <StatTile
              spec={s()}
              width={tileWidth()}
              picked={homePick.pick() === s().key}
              onPress={() => pressTile(s())}
            />
          )}
        </Each>
      </View>
      <View class="mt-7">
        <SectionTitle first>Next 30 days</SectionTitle>
      </View>
      <DeadlineStrip
        days={data().strip}
        everyDay={props.width() >= 560}
        pickedDay={homePick.pick() === "day" ? homePick.day() : null}
        onPickDay={homePick.toggleDay}
      />
      <View class="mt-4 mb-2 w-full flex-row items-center gap-2">
        <Text variant="emph" class="min-w-0 flex-1">{picked().title}</Text>
        <If when={homePick.pick() !== null}>
          <AppButton testID="home-due-show-all" label="Show all" variant="ghost" size="sm" onPress={homePick.clear} />
        </If>
        <AppButton testID="home-open-calendar" label="Open Calendar" variant="ghost" size="sm" icon="calendar" onPress={() => go("calendar")} />
      </View>
      <ListGroup testID="home-due">
        <If when={rows().length > 0} else={<EmptyRow testID="home-due-empty" text={picked().empty} />}>
          <Each of={rows()} key={deadlineKey}>
            {(d) => <DueRow d={d()} compact={compact()} />}
          </Each>
        </If>
      </ListGroup>
      <SectionTitle>Each taxpayer at a glance</SectionTitle>
      <ListGroup testID="home-taxpayers">
        <If when={data().taxpayers.length > 0} else={<EmptyRow testID="home-taxpayers-empty" text="Archived taxpayers are left out of Home." />}>
          <Each of={data().taxpayers} key={(t) => t.taxpayer.tin_root}>
            {(t) => <TaxpayerRow row={t()} compact={taxpayerCompact()} />}
          </Each>
        </If>
      </ListGroup>
      <If when={asNeeded().length > 0}>
        <SectionTitle>As needed</SectionTitle>
        <ListGroup testID="home-as-needed">
          <Each of={asNeeded()} key={(e) => e.key}>
            {(e) => (
              <If when={e().undated} else={<AsNeededHead f={e().form} compact={compact()} />}>
                <AsNeededUndated f={e().form} r={e().undated as AsNeededReturn} compact={compact()} />
              </If>
            )}
          </Each>
        </ListGroup>
      </If>
      {/* A phone or a tablet has no sidebar: its Recent list (the same returns, five at most) is here, above Recently filed. */}
      <If when={TOUCH && app.prefs().showRecent && recentRows().length > 0}>
        <SectionTitle>Recent</SectionTitle>
        <ListGroup testID="home-recent">
          <Each of={recentRows()} key={(r) => r.key}>
            {(r) => (
              <ListRow
                testID={`home-recent-${String(r().key).replace(/[^A-Za-z0-9]/g, "-")}`}
                accessibilityLabel={`Open ${recentLabel(r())}`}
                onPress={() => app.openReturn(recentOpenOptions(r()))}
              >
                <Icon name="file-text" tone="fg3" />
                <NameCell title={recentLabel(r())} />
                <Icon name="chevron-right" tone="fg3" />
              </ListRow>
            )}
          </Each>
        </ListGroup>
      </If>
      <SectionTitle>Recently filed</SectionTitle>
      <ListGroup testID="home-filed">
        <If when={filed().length > 0} else={<EmptyRow testID="home-filed-empty" text="Filed returns show here." />}>
          <Each of={filed()} key={(r) => r.id}>
            {(r) => <FiledRow r={r()} compact={filedCompact()} />}
          </Each>
        </If>
      </ListGroup>
    </View>
  );
}

/** The Home view: the dashboard, or the welcome while no taxpayer exists. */
export function Home() {
  // Home draws from what the store holds at once; the drafts (kept outside the poll's sweep) are read again once per visit, in the
  // background, after the page is up, so the visit never waits for a read.
  const held = store.has(SAVED_RETURNS_KEY);
  const refresh = setTimeout(() => held && store.forget(SAVED_RETURNS_KEY), 0);
  cleanup(() => clearTimeout(refresh));
  const width = signal(firstWidth());
  return (
    <View testID="home" class="w-full" onLayout={(e) => e.bounds.size.width > 0 && width.set(Math.round(e.bounds.size.width))}>
      <WelcomeNote />
      <If when={app.isFresh()} else={<Dashboard width={width} />}>
        <Welcome />
      </If>
    </View>
  );
}
