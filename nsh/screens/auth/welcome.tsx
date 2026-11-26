// The welcome line after the first sign-in: "Welcome. Your Buwiz account is ready." as a green note at the top of the first
// page after it (Home, or the first-run setup or PIN sign-in when one comes first), never a page of its own.
//
// "The first time" is the first sign-in of this account on this device: the host's verify answer does not say whether the
// account was just made, so each account (its user id, else its address) is written to the setting `welcomed_accounts` the
// first time it signs in here, and the note shows only when it was not there yet. It shows on that one page and goes once the
// person leaves it; it never shows again for that account on this device.
import { cleanup, effect, signal, View } from "nsh";
import type { CloudAccountStatus } from "../../bir";
import { store } from "../../shell";
import { Icon, Text, TOUCH } from "../../ui";
import { setAbovePage } from "./parts";
import { signInFlow } from "./signInFlow";
import { openColumnMax } from "./signInLook";

/** The setting that lists the accounts welcomed on this device (a JSON array of user ids or addresses). */
export const WELCOMED_KEY = "welcomed_accounts";
export const WELCOME_TEXT = "Welcome. Your Buwiz account is ready.";

/** The account as this device knows it: its user id, else its address in lower case; null when the answer has neither. */
export function accountKey(status: Pick<CloudAccountStatus, "user_id" | "email">): string | null {
  if (status.user_id) return status.user_id;
  const email = status.email?.trim().toLowerCase();
  return email ? email : null;
}

/** Whether `key` is new to the stored list, and the list to store then (a list that does not read is taken as empty). */
export function welcomedAfter(stored: string | null, key: string): { first: boolean; next: string } {
  let list: string[] = [];
  try {
    const parsed: unknown = stored ? JSON.parse(stored) : [];
    if (Array.isArray(parsed)) list = parsed.filter((v): v is string => typeof v === "string");
  } catch {
    list = [];
  }
  if (list.includes(key)) return { first: false, next: stored ?? "[]" };
  return { first: true, next: JSON.stringify([...list, key]) };
}

/** The note waits for the first page after the first sign-in. */
const pending = signal(false);

/** Signed in: the first time for this account here, it is written down and the note waits for the page. */
function noteSignIn(status: CloudAccountStatus): void {
  const key = accountKey(status);
  if (!key) return;
  try {
    const { first, next } = welcomedAfter(store.setting(WELCOMED_KEY), key);
    if (!first) return;
    store.writeFor(`setting:${WELCOMED_KEY}`, "set_setting", WELCOMED_KEY, next);
    pending.set(true);
  } catch {
    // Not written down: no note rather than a note that could come again.
  }
}

signInFlow.onApproved(noteSignIn);

/**
 * The note, at the top of the page that draws it, while it waits and `when` holds. Once it has shown, the page going away
 * ends it.
 */
export function WelcomeNote(props: { when?: () => boolean; centred?: boolean }) {
  const shows = () => pending() && (props.when ? props.when() : true);
  let shown = false;
  effect(() => {
    if (shows()) shown = true;
    // Over an open page, the page's top is under the note: it counts the note out of its room while it shows.
    else if (props.centred) setAbovePage(0);
  });
  cleanup(() => {
    if (shown) pending.set(false);
    if (props.centred) setAbovePage(0);
  });
  const note = () => (
    <View testID="welcome-note" class="mb-4 w-full flex-row items-center gap-2 rounded-(--radius-card) bg-success-soft px-4 py-3">
      <Icon name="circle-check" tone="success" />
      <Text variant="control" tone="success" class="min-w-0 flex-1">{WELCOME_TEXT}</Text>
    </View>
  );
  // `centred`: over an open page (the pages before the app), in a column as wide as the page's.
  return (
    <If when={shows()}>
      <If when={props.centred} else={note()}>
        <View class="w-full items-center px-6 pt-3" onLayout={(e) => setAbovePage(Math.round(e.bounds.size.height))}>
          <View class="w-full" style={{ maxWidth: openColumnMax(TOUCH) }}>
            {note()}
          </View>
        </View>
      </If>
    </If>
  );
}
