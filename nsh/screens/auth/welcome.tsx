// The welcome line after the first sign-in: "Welcome. Your Buwiz account is ready." as a green note at the top of the first
// page after it (Home, or the first-run setup or PIN sign-in when one comes first), never a page of its own.
//
// "The first time" is the first sign-in of this account on this device: the host's verify answer does not say whether the
// account was just made, so the page asks the host (`welcome_first_time`), which checks and records the signed-in account
// itself and answers only true or false. The page never names an account and reads no list of them (accounts-and-lock W3).
// The note shows on that one page and goes once the person leaves it; it never shows again for that account on this device.
import { cleanup, effect, signal, View } from "nsh";
import { bir } from "../../bir";
import { Icon, Text, TOUCH } from "../../ui";
import { setAbovePage } from "./parts";
import { signInFlow } from "./signInFlow";
import { openColumnMax } from "./signInLook";

export const WELCOME_TEXT = "Welcome. Your Buwiz account is ready.";

/** The note waits for the first page after the first sign-in. */
const pending = signal(false);

/** Signed in: the first time for this account here (the host's answer, recorded there), the note waits for the page. */
function noteSignIn(): void {
  try {
    if (bir.welcome_first_time()) pending.set(true);
  } catch {
    // No answer: no note rather than a note that could come again.
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
