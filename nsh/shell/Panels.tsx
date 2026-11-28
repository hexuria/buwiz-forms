// The right panel (old chrome.js): the tax-profile summary or the notifications. Both bodies are also what the narrow
// layout's sheet shows, so each is a component of its own; RightPanel is the docked or floating 320-point column.
import { View } from "nsh";
import { Button } from "nsh/base";
import type { Taxpayer } from "../bir";
import { shortDate } from "../lib/dates";
import { Avatar, AppButton, Chip, Code, EmptyRow, GroupLabel, Icon, NavRow, SidePanel, StepCircle, Text } from "../ui";
import { TinChip } from "../screens/taxpayer/TinChip";
import { app, type Notification } from "./app";
import { FactRow } from "./parts";
import { store } from "./store";

// ---------------------------------------------------------------- notifications

/** One notification: a tinted icon circle, title and sub line, the date and an unread dot. */
function NotificationRow(props: { item: Notification }) {
  const ok = () => props.item.tone === "success";
  return (
    <Button
      testID={`notif-${props.item.id}`}
      accessibilityLabel={props.item.title}
      class="w-full flex-row gap-3 rounded-(--radius-row) px-2 py-2.5 enabled:hover:bg-hover active:bg-selected focus-visible:ring-2 focus-visible:ring-accent-blue"
      onPress={() => app.openNotification(props.item.id)}
    >
      <View class={`size-7 shrink-0 items-center justify-center rounded-full ${ok() ? "bg-success-soft" : "bg-hover"}`}>
        <Icon name={props.item.icon} tone={ok() ? "success" : "fg2"} />
      </View>
      <View class="min-w-0 flex-1">
        <Text variant="control">{props.item.title}</Text>
        <Text variant="meta" tone="fg3">{props.item.sub}</Text>
      </View>
      <View class="shrink-0 items-end gap-1.5">
        <Text variant="cap" tone="fg3">{shortDate(props.item.when)}</Text>
        <If when={props.item.unread}>
          <View testID={`notif-unread-${props.item.id}`} class="size-1.5 rounded-full bg-accent-blue" />
        </If>
      </View>
    </Button>
  );
}

/** The notifications list: submissions sent and BIR confirmations received, newest first. */
export function NotificationsBody() {
  return (
    <View testID="notifications-list" class="gap-0.5">
      <If when={app.notifications().length === 0}>
        <EmptyRow text="No notifications yet." testID="notifications-empty" />
      </If>
      <Each of={app.notifications()} key={(n) => n.id}>{(n) => <NotificationRow item={n()} />}</Each>
    </View>
  );
}

/** The header action of the notifications panel. */
export function MarkAllRead() {
  return <AppButton testID="notif-mark-all" label="Mark all read" variant="ghost" size="sm" disabled={app.unreadCount() === 0} onPress={app.markAllRead} />;
}

// ---------------------------------------------------------------- tax profile

function EmptyProfile() {
  return (
    <View testID="profile-empty" class="items-center gap-2 px-2 pt-8">
      <StepCircle size={56}>
        <Icon name="user" tone="fg2" />
      </StepCircle>
      <Text variant="title" class="mt-1">{app.taxpayers().length ? "No taxpayer open" : "No taxpayer yet"}</Text>
      <Text variant="meta" tone="fg2" class="max-w-60 text-center">
        {app.taxpayers().length
          ? "Choose one on Home or with search. The one you open shows here, with its branches, tax years, forms and templates."
          : "The taxpayer you file for shows here, with its branches, tax years, forms and templates."}
      </Text>
      <If when={app.canAddTaxpayer()}>
        <View class="mt-2">
          <AppButton testID="panel-new-taxpayer" label="New tax profile" variant="primary" size="sm" icon="plus" onPress={() => app.go("new_taxpayer")} />
        </View>
      </If>
    </View>
  );
}

/** The summary of the open taxpayer and branch: read-only, every edit is on the Profile page. */
function ProfileSummary(props: { tp: Taxpayer }) {
  const b = () => app.branch();
  const chips = () => {
    const br = b();
    const y = app.year();
    return br && y ? store.taxTypes(br.tin, y) : [];
  };
  const missing = () => {
    const br = b();
    const y = app.year();
    if (!br || !y) return [];
    const view = store.yearFacts(br.tin, y);
    const readiness = store.electionReadiness(br.tin, y);
    return (view?.not_set ?? []).filter((f) => f !== "income_tax_election" || readiness?.gap === "ready");
  };
  return (
    <View class="gap-0.5">
      <View class="items-center gap-2 px-2 pt-3 pb-4">
        <Avatar name={props.tp.name} size={56} hueKey={props.tp.tin_root} />
        <Text variant="title" class="mt-1 text-center">{props.tp.name}</Text>
        <Code tone="fg2">{b()?.tin_display ?? props.tp.tin_display}</Code>
        <If when={b()}>
          <View class="items-center">
            <TinChip testID="panel-tin-status" tinRoot={props.tp.tin_root} branch={b()!.code} tinFull={`${props.tp.tin_root}${b()!.code}`} />
          </View>
        </If>
      </View>
      <View testID="panel-summary" class="mx-2 mb-1 gap-2 border-y border-border py-3">
        <FactRow icon="git-branch" label="Branch">
          <View class="flex-row gap-1">
            <Code>{b()?.code ?? ""}</Code>
            <Text>{`· ${b()?.name ?? ""}`}</Text>
          </View>
        </FactRow>
        <FactRow icon="map-pin" label="Address">
          <Text>{b()?.facts.registered_address || "Not set"}</Text>
        </FactRow>
        <FactRow icon="building" label="RDO">
          <Text>{b()?.facts.rdo_code || "Not set"}</Text>
        </FactRow>
        <If when={app.year()}>
          <FactRow icon="calendar" label={String(app.year())} testID="panel-tax-types">
            <If when={chips().length > 0} else={<Text>No forms yet</Text>}>
              <View class="flex-row flex-wrap gap-1">
                <Each of={chips()} key={(c) => c.type}>{(c) => <Chip label={c().label} />}</Each>
              </View>
            </If>
          </FactRow>
          <If when={missing().length > 0}>
            <FactRow icon="triangle-alert" label="Facts" testID="panel-not-set">
              <View class="flex-row">
                <Chip label={`${missing().length} not set`} tone="warning" />
              </View>
            </FactRow>
          </If>
        </If>
      </View>
      <View class="px-2 pt-2">
        <AppButton testID="panel-edit-profile" label="Edit in profile" size="sm" icon="pencil" onPress={() => app.openProfile()} />
      </View>
    </View>
  );
}

function ProfileLists(props: { tp: Taxpayer }) {
  const others = () => app.taxpayers().filter((x) => x.tin_root !== props.tp.tin_root);
  return (
    <View class="gap-0.5">
      <GroupLabel>Open</GroupLabel>
      <NavRow testID="panel-open-profile" leading="user" label="Profile" current={app.view() === "profile"} onPress={() => app.go("profile")} />
      <NavRow testID="panel-open-forms" leading="file-text" label="Forms" count={app.yearForms().length} current={app.view() === "forms"} onPress={() => app.go("forms")} />
      <NavRow testID="panel-open-templates" leading="layout-template" label="Templates" count={app.templates().length} current={app.view() === "templates"} onPress={() => app.go("templates")} />
      <GroupLabel>{props.tp.branches.length > 1 ? "Branches" : "Branch"}</GroupLabel>
      <Each of={props.tp.branches} key={(br) => br.code}>
        {(br) => (
          <NavRow
            testID={`panel-branch-${br().code}`}
            leading={<Code class="w-11">{br().code}</Code>}
            label={br().name}
            current={br().code === app.branch()?.code}
            trailing={
              <View class="flex-row items-center gap-1">
                <TinChip compact testID={`panel-branch-tin-${br().code}`} tinRoot={props.tp.tin_root} branch={br().code} tinFull={`${props.tp.tin_root}${br().code}`} />
                <If when={br().code === app.branch()?.code}>
                  <Icon name="check" tone="fg3" />
                </If>
              </View>
            }
            onPress={() => app.selectBranch(br().code)}
          />
        )}
      </Each>
      <NavRow testID="panel-add-branch" leading="plus" label="Add branch" onPress={() => app.go("new_branch")} />
      <If when={app.isAdmin()}>
        <If when={others().length > 0}>
          <GroupLabel>Switch taxpayer</GroupLabel>
        </If>
        <Each of={others()} key={(x) => x.tin_root}>
          {(x) => (
            <NavRow
              testID={`panel-tp-${x().tin_root}`}
              leading={<Avatar name={x().name} size={20} hueKey={x().tin_root} />}
              label={x().name}
              count={x().tin_root.slice(0, 3)}
              trailing={
                <If when={x().branches.find((br) => br.is_head_office)}>
                  <TinChip
                    compact
                    testID={`panel-tp-tin-${x().tin_root}`}
                    tinRoot={x().tin_root}
                    branch={x().branches.find((br) => br.is_head_office)!.code}
                    tinFull={`${x().tin_root}${x().branches.find((br) => br.is_head_office)!.code}`}
                  />
                </If>
              }
              onPress={() => app.selectTaxpayer(x().tin_root)}
            />
          )}
        </Each>
        <If when={app.canAddTaxpayer()}>
          <NavRow testID="panel-new-tp" leading="plus" label="New tax profile" onPress={() => app.go("new_taxpayer")} />
        </If>
      </If>
    </View>
  );
}

/** The tax-profile summary and its lists. */
export function ProfileBody() {
  return (
    <If when={app.taxpayer()} else={<EmptyProfile />}>
      {(tp) => (
        <View class="gap-0.5">
          <ProfileSummary tp={tp()} />
          <ProfileLists tp={tp()} />
        </View>
      )}
    </If>
  );
}

// ---------------------------------------------------------------- the column

/** The 320-point right panel: notifications when that is the panel, else the tax profile. */
export function RightPanel() {
  return (
    <If
      when={app.panel() === "notifs"}
      else={<SidePanel testID="panel-profile"><ProfileBody /></SidePanel>}
    >
      <SidePanel testID="panel-notifications" title="Notifications" action={<MarkAllRead />}>
        <NotificationsBody />
      </SidePanel>
    </If>
  );
}
