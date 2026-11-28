// The TIN status chip (D2): one tax profile's TIN and branch against the account's list (tins.ts is the owner). Activated;
// Pending with the row's message; Fix with the message and its action (opened through the host-pinned account link only);
// Not activated with the "Activate this TIN to send" button; a Test TIN chip when test_only; stale rows say when they were
// checked. The refusals are the host's own sentences, shown as thrown.
import { openUrl } from "nsh";
import type { JSX } from "nsh";
import { View } from "nsh";
import { isoDate } from "../../lib/dates";
import { AppButton, Chip, Text } from "../../ui";
import { activateTin, clearTinRefusal, markAccountSiteOpened, refreshTins, tinActivating, tinRefusal, tinRow } from "./tins";

/** "Checked <date>" for a stale row: the list's read day. */
function checkedDate(fetchedAt: number): string {
  return isoDate(new Date(fetchedAt * 1000));
}

const TONES = { activated: "success", pending: "warning", fix: "danger", not_activated: "neutral" } as const;
const LABELS = { activated: "Activated", pending: "Pending", fix: "Fix", not_activated: "Not activated" } as const;

/** Opens the row's account-site action, already pinned host-side to the account site. The list is asked again when
 * the app becomes active on the filer's return (the shell's listener), never here: nothing has changed yet at open. */
function openAction(url: string): void {
  markAccountSiteOpened();
  void openUrl(url);
}

/** The Activate button and its refusal line, in the host's own words exactly as thrown. */
function ActivateRow(props: { tinFull: string; testID: string }) {
  const refusal = () => tinRefusal(props.tinFull);
  const busy = () => tinActivating(props.tinFull);
  return (
    <View class="gap-1">
      <AppButton
        testID={`${props.testID}-activate`}
        label="Activate this TIN to send"
        size="sm"
        disabled={busy()}
        onPress={() => void activateTin(props.tinFull)}
      />
    </View>
  );
}

/** The refusal sentence of the last Activate, in the host's own words, with its Dismiss. Rendered for as long as it
 * stands, whatever the row's new state: a refresh flipping the row to pending or activated must not hide what the
 * server answered to the filer's own button press — that answer, not the row, is what explains the moment. */
function RefusalRow(props: { tinFull: string; testID: string }) {
  const refusal = () => tinRefusal(props.tinFull);
  return (
    <If when={refusal()}>
      <View class="flex-row items-center gap-2">
        <Text testID={`${props.testID}-refusal`} variant="meta" tone="danger">{refusal()}</Text>
        <AppButton testID={`${props.testID}-refusal-dismiss`} label="Dismiss" size="sm" onPress={() => clearTinRefusal(props.tinFull)} />
      </View>
    </If>
  );
}

/**
 * The chip for one TIN root (9 digits) and branch (5 digits). Nothing when there is no list or no row for the pair
 * (an unreadable or incomplete list says nothing on screen; Queue decides then).
 */
export function TinChip(props: { tinRoot: string; branch: string; tinFull: string; testID?: string; compact?: boolean }): JSX.Element {
  // Screens read with max_age 60 (the host serves memory younger than that); the refresh points are the shell's.
  void refreshTins(60);
  const id = () => props.testID ?? "tin-chip";
  const row = () => tinRow(props.tinRoot, props.branch);
  return (
    <If when={row()}>
      <View testID={id()} class="gap-1">
        <View class="flex-row flex-wrap items-center gap-1">
          <Chip testID={`${id()}-state`} label={LABELS[row()!.state]} tone={TONES[row()!.state]} />
          <If when={row()!.test_only}>
            <Chip testID={`${id()}-test`} label="Test TIN" tone="accent" />
          </If>
          <If when={row()!.stale}>
            <Text testID={`${id()}-stale`} variant="cap" tone="fg3">{`Checked ${checkedDate(row()!.fetched_at)}`}</Text>
          </If>
        </View>
        <If when={!props.compact}>
          <If when={row()!.message}>
            <Text testID={`${id()}-message`} variant="meta" tone="fg2">{row()!.message}</Text>
          </If>
          <If when={row()!.state === "fix" && row()!.action_url}>
            <AppButton
              testID={`${id()}-action`}
              label="Fix on your account"
              size="sm"
                onPress={() => openAction(row()!.action_url!)}
            />
          </If>
          <If when={row()!.state === "not_activated"}>
            <ActivateRow tinFull={props.tinFull} testID={id()} />
          </If>
          <RefusalRow tinFull={props.tinFull} testID={id()} />
        </If>
      </View>
    </If>
  );
}
