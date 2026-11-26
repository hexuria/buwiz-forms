// Settings: a page in the main area (plan, UI decisions), opened from the account menu, the palette and Cmd/Ctrl+comma. From 600
// points: a section list on the left (Personal: General, Account; Data: Import / Export, Jobs; System: Keyboard, Computer) with the search
// over rows, and the section's cards on the right. Under 600: the section list first, then the section as its own page: a stop of the
// app's history, with Back and its name in the narrow header (a stacked page under the wide title bar keeps its own Back line and title).
// Ported from the old shell's settings.js. Params: `section` opens a section (the old cron_tasks view is `section: "jobs"`).
import { cleanup, Scroll, View } from "nsh";
import { sectionById, settingsView, shownSection } from "../../lib/settings_model";
import { app } from "../../shell";
import { masBuild, storeManaged } from "../../shell/desktop/native";
import { AppButton, GroupLabel, Icon, NavRow, PageTitle, Text, TextField, TOUCH, type IconName } from "../../ui";
import { AccountSection } from "./AccountSection";
import { ComputerSection } from "./Computer";
import { GeneralSection } from "./General";
import { JobsSection } from "./Jobs";
import { KeyboardSection } from "./Keyboard";
import { groupSections } from "./logic";
import { nav } from "./nav";
import { Labeled } from "./parts";
import { leaveSecurity } from "./securityState";
import { SetupCard } from "./SetupCard";
import { leaveSetup, resetSetup, setupChange } from "./setup";
import { TransferSection } from "./Transfer";

/** The cards' column on the right of the list: one width for every section, so the title never moves. The old Jobs column was 680 (JOBS_COLUMN in the old shell's settings.js): Jobs and Logs need it, the rest take it too. */
const COLUMN = 680;
/** The section list's width beside the cards. */
const RAIL = 220;
/** The narrowest the cards' pane may be beside the list: its 32-point sides and a row whose words still fit beside a 120-point control
 * (a 640-point window, the list beside narrow cards, still has room: shots/settings.test.tsx). */
const PANE_MIN = 380;

/**
 * The list first and a section as its own page: on a phone, and wherever the main surface is too narrow for the list beside the cards.
 * The surface, not the window, decides: an iPad (or a desktop window) of 1032 points with the sidebar and the tax profile docked leaves
 * the page about 460, and the cards beside the list then had room for one letter a line.
 */
const settingsStacked = (): boolean => app.layout.narrow() || app.layout.mainWidth() < RAIL + PANE_MIN;

/** The change of who this computer is for has made its recovery key: only the key shows, as the old screen did. */
const keyOnly = (): boolean => setupChange.step() === "key";
/** The sections and rows the search keeps for this person. */
const view = () => {
  if (keyOnly()) return [];
  return settingsView(nav.query(), app.isAdmin(), TOUCH, masBuild(), storeManaged());
};
const shownId = () => (keyOnly() ? null : shownSection(nav.section(), view()));
const shown = () => {
  const id = shownId();
  return id ? sectionById(view(), id) : null;
};

/** What a section is called here ("Device" on a phone, "Computer" on a desktop): the narrow header names the open section with it. */
export const sectionTitle = (id: string): string => sectionById(view(), id)?.label ?? "Settings";

/**
 * From a section's page to the list. The section is a stop of the app's history when it was opened from the list, so this is Back;
 * a section opened by name (a link from another page, the dev route) has no list behind it, and the list takes its place.
 */
export function backToSettingsList(): void {
  const before = app.previous();
  if (before?.view === "settings" && typeof before.params.section !== "string") app.back();
  else app.replace("settings", {});
}

function Search() {
  return (
    <TextField testID="settings-search" icon="search" placeholder="Search settings" accessibilityLabel="Search settings" value={nav.query()} onChange={nav.setQuery} />
  );
}

/** The sections the search keeps, under their group names. Narrow: each row ends in a chevron. */
function SectionList(props: { narrow: boolean; onPick: (id: string) => void }) {
  return (
    <View testID="settings-nav" class="w-full">
      <If when={view().length} else={<Text testID="settings-nav-empty" variant="meta" tone="fg3" class="px-2 pt-4">No matching settings.</Text>}>
        <Each of={groupSections(view())} key={(g) => g.group}>
          {(g) => (
            <View>
              <GroupLabel>{g().group}</GroupLabel>
              <Each of={g().sections} key={(s) => s.id}>
                {(s) => (
                  <NavRow
                    testID={`settings-nav-${s().id}`}
                    leading={s().icon as IconName}
                    label={s().label}
                    current={!props.narrow && s().id === shownId()}
                    trailing={props.narrow ? <Icon name="chevron-right" tone="fg3" /> : undefined}
                    onPress={() => props.onPick(s().id)}
                  />
                )}
              </Each>
            </View>
          )}
        </Each>
      </If>
    </View>
  );
}

/** What the open section holds, by its id. */
function SectionBody(props: { id: string; rows: string[] }) {
  return (
    <>
      <If when={props.id === "general"}>
        <GeneralSection rows={props.rows} />
      </If>
      <If when={props.id === "account"}>
        <AccountSection rows={props.rows} />
      </If>
      <If when={props.id === "transfer"}>
        <TransferSection rows={props.rows} />
      </If>
      <If when={props.id === "jobs"}>
        <JobsSection />
      </If>
      <If when={props.id === "keyboard"}>
        <KeyboardSection rows={props.rows} />
      </If>
      <If when={props.id === "computer"}>
        <ComputerSection rows={props.rows} />
      </If>
    </>
  );
}

/** The cards of the open section (or the key of a finished setup change), in a column that scrolls by itself. */
function Pane(props: { narrow: boolean }) {
  // The narrow layout's header carries Back and the section's name (one Back control, as on a phone); a stacked page under the
  // wide title bar, which names no page, keeps its own line and title.
  const ownHeader = () => !app.layout.narrow();
  return (
    <Scroll
      testID="settings-scroll"
      class="min-h-0 flex-1"
      contentClass={props.narrow ? "px-4 pt-3 pb-14" : "items-center px-8 pt-7 pb-14"}
    >
      <View class="w-full min-w-0 gap-5" style={{ maxWidth: COLUMN }}>
        <If when={props.narrow && ownHeader() && !keyOnly()}>
          <View class="self-start">
            <AppButton testID="settings-back" label="Settings" icon="chevron-left" variant="ghost" size="sm" onPress={backToSettingsList} />
          </View>
        </If>
        <If when={!props.narrow || ownHeader() || keyOnly()}>
          <View testID="settings-title">
            <PageTitle>{keyOnly() ? (TOUCH ? "Device" : "Computer") : (shown()?.label ?? "Settings")}</PageTitle>
          </View>
        </If>
        <If
          when={keyOnly()}
          else={
            <If when={shown()} else={<Text testID="settings-empty" tone="fg3">No settings match that search.</Text>}>
              <SectionBody id={shown()?.id ?? ""} rows={shown()?.rows ?? []} />
            </If>
          }
        >
          <Labeled label={TOUCH ? "Device setup" : "Computer setup"}>
            <SetupCard />
          </Labeled>
        </If>
      </View>
    </Scroll>
  );
}

/** The Settings page. */
export function Settings() {
  const asked = app.params().section;
  const section = typeof asked === "string" ? asked : null;
  if (section) nav.setSection(section);
  /**
   * Stacked: the list shows, or the section the route names. A section picked from the list is a move of its own, so the header's
   * Back, Android's back button and Forward walk it like any page. Beside the list, a pick only changes which cards show.
   */
  const open = section !== null;
  const pick = (id: string) => {
    if (settingsStacked()) app.go("settings", { section: id });
    else nav.setSection(id);
  };
  // Leaving Settings: the half-typed codes and the recovery key on screen go; a finished setup change signs the computer out.
  cleanup(app.beforeLeave(leaveSetup));
  cleanup(() => {
    leaveSecurity();
    resetSetup();
  });
  return (
    <View testID="settings" class="min-h-0 w-full flex-1">
      <If
        when={settingsStacked()}
        else={
          <View class="min-h-0 flex-1 flex-row">
            <If when={!keyOnly()}>
              <View testID="settings-rail" class="h-full shrink-0 border-r border-border pt-4 pr-3 pb-2 pl-3" style={{ width: RAIL }}>
                <Text variant="emph" class="px-2 pb-3">Settings</Text>
                <Search />
                <Scroll testID="settings-rail-scroll" class="mt-1 min-h-0 flex-1">
                  <SectionList narrow={false} onPick={pick} />
                </Scroll>
              </View>
            </If>
            <Pane narrow={false} />
          </View>
        }
      >
        <If
          when={open || keyOnly()}
          else={
            <Scroll testID="settings-list-scroll" class="min-h-0 flex-1" contentClass="gap-2 px-4 pt-4 pb-14">
              <Search />
              <SectionList narrow onPick={pick} />
            </Scroll>
          }
        >
          <Pane narrow />
        </If>
      </If>
    </View>
  );
}
