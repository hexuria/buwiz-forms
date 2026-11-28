// The migration pages (A3a, sign-in look B: OpenPage, PageHeading, PageButton, BackControl, the whole BrandLogo,
// never the emblem alone): what stands between the account gate and the frame while the legacy claim decides —
// Opening/Moving (progress), Could not be moved yet (transient: Try again / Sign out), Moved, not opened
// (support, Sign out), and the offer (Add them to this account / Not now). Could not be moved (permanent) does not
// block the frame: the account opens its empty room, and the note shows on Home once per account, then under
// Settings › Account (migrationNote.tsx). Every control carries a stable test id.
import { signal, View } from "nsh";
import { bir } from "../../bir";
import { errorText } from "../../lib/errors";
import { app } from "../../shell";
import { Spinner, Text } from "../../ui";
import { OpenPage, PageButton, PageHeading, SoftLine, TextLink } from "./parts";

/** The migration state the host answered after the last session open or answer (migration_state()). `kind` "none" is no page. */
export interface MigrationPage {
  kind: "none" | "opening" | "moving" | "transient" | "permanent" | "moved_not_opened" | "offered" | "device_store";
  copySaved: boolean;
  detail: string;
}
const state = signal<MigrationPage>({ kind: "none", copySaved: false, detail: "" });

/** The migration page now up, if one is: the frame waits behind it. Permanent is not a page — the empty room opens. */
export const migrationPage = () => {
  const kind = state().kind;
  return kind === "opening" || kind === "moving" || kind === "transient" || kind === "moved_not_opened" || kind === "offered" || kind === "device_store" ? state() : null;
};

/** The permanent failure's detail for the Home note and the Settings › Account row, or null. */
export const migrationPermanent = () => (state().kind === "permanent" ? state().detail : null);

let activeSince: number | null = null;
let activeKind: "opening" | "moving" | null = null;
const progress = signal<string | null>(null);
const signOutError = signal<string | null>(null);

/** The one Opening/Moving sentence, or null during its first second. It never flips between the two while the claim runs. */
export const migrationProgress = () => progress();

function noteProgress(kind: MigrationPage["kind"]): void {
  const active = kind === "opening" || kind === "moving";
  if (!active) {
    activeSince = null;
    activeKind = null;
    progress.set(null);
    return;
  }
  if (activeSince === null || activeKind === null) {
    activeSince = Date.now();
    activeKind = kind;
    progress.set(null);
    return;
  }
  if (progress() === null && Date.now() - activeSince >= 1000) {
    progress.set(activeKind === "opening" ? "Opening your account…" : "Moving your returns…");
  }
}

/** Ask the host what the migration state is now (after a session open, a Try again, an offer answer). */
export function rereadMigration(): void {
  try {
    const answer = bir.migration_state() as { kind: MigrationPage["kind"]; copy_saved: boolean; detail: string };
    const kind = answer.kind ?? "none";
    state.set({ kind, copySaved: answer.copy_saved === true, detail: answer.detail ?? "" });
    signOutError.set(null);
    noteProgress(kind);
  } catch {
    state.set({ kind: "none", copySaved: false, detail: "" });
    noteProgress("none");
  }
}

/** The host's sign-out refusal, shown on the migration page that stays up. */
function SignOutErrorLine() {
  return (
    <If when={signOutError() !== null}>
      <Text variant="meta" tone="danger" testID="migration-sign-out-error">{signOutError()}</Text>
    </If>
  );
}

/** Sign out of the migration pages: the real sign-out removes the stored account first, then the gate comes back. */
function signOut(): void {
  signOutError.set(null);
  void (async () => {
    try {
      await bir.cloud_account_sign_out();
      app.lockCloud();
    } catch (error) {
      signOutError.set(errorText(error));
    }
  })();
}

/** Opening/Moving: the progress page while the claim runs (a spinner, no buttons). */
export function MovingPage() {
  const neutral = signal(false);
  setTimeout(() => neutral.set(true), 1000);
  const title = () => migrationProgress() ?? (neutral() ? "Opening your account…" : null);
  return (
    <OpenPage testID="migration-moving">
      <View class="items-center gap-4">
        <If when={title() !== null}>
          <PageHeading testID="migration-moving-title">{title()}</PageHeading>
        </If>
        <Spinner size={20} label="Opening your account…" />
      </View>
    </OpenPage>
  );
}

/** "Could not be moved yet" (transient): Try again / Sign out; the copy line only after step 3 passed. */
export function TransientPage(props: { copySaved: boolean; detail: string }) {
  const busy = signal(false);
  const error = signal<string | null>(null);
  const tryAgain = async () => {
    busy.set(true);
    error.set(null);
    try {
      await bir.migration_try_again();
      rereadMigration();
      await app.watchAccountOpen();
    } catch (e) {
      error.set(errorText(e));
    } finally {
      busy.set(false);
    }
  };
  return (
    <OpenPage testID="migration-transient">
      <View class="items-center gap-3">
        <PageHeading testID="migration-transient-title">Your tax profiles could not be moved to this account yet.</PageHeading>
        <SoftLine testID="migration-transient-copy">
          {props.copySaved
            ? "Nothing was changed: they are still where they were, and a copy is saved in this app's storage."
            : "Nothing was changed: they are still where they were."}
        </SoftLine>
        <If when={props.detail.length > 0}>
          <SoftLine testID="migration-transient-detail" size={12}>{props.detail}</SoftLine>
        </If>
        <If when={error() !== null}>
          <Text variant="meta" tone="danger" testID="migration-transient-error">{error()}</Text>
        </If>
        <SignOutErrorLine />
      </View>
      <View class="w-full gap-2.5">
        <PageButton testID="migration-try-again" label="Try again" busy={busy()} onPress={() => void tryAgain()} />
        <View class="items-center">
          <TextLink testID="migration-transient-sign-out" label="Sign out" onPress={signOut} />
        </View>
      </View>
    </OpenPage>
  );
}


/** The device store cannot be read: its own plain page, support plus Try again. */
export function DeviceStorePage(props: { detail: string }) {
  const busy = signal(false);
  const error = signal<string | null>(null);
  const tryAgain = async () => {
    busy.set(true);
    error.set(null);
    try {
      await bir.migration_try_again();
      rereadMigration();
      await app.watchAccountOpen();
    } catch (e) {
      error.set(errorText(e));
    } finally {
      busy.set(false);
    }
  };
  return (
    <OpenPage testID="migration-device-store">
      <View class="items-center gap-3">
        <PageHeading testID="migration-device-store-title">This device's account store cannot be read.</PageHeading>
        <SoftLine testID="migration-device-store-line">Your tax profiles are still on this device, unchanged. Write to support@buwiz.com.</SoftLine>
        <If when={props.detail.length > 0}>
          <SoftLine testID="migration-device-store-detail" size={12}>{props.detail}</SoftLine>
        </If>
        <If when={error() !== null}>
          <Text variant="meta" tone="danger" testID="migration-device-store-error">{error()}</Text>
        </If>
        <SignOutErrorLine />
      </View>
      <View class="w-full gap-2.5">
        <PageButton testID="migration-device-store-try-again" label="Try again" busy={busy()} onPress={() => void tryAgain()} />
        <View class="items-center">
          <TextLink testID="migration-device-store-sign-out" label="Sign out" onPress={signOut} />
        </View>
      </View>
    </OpenPage>
  );
}

/** "Moved, not opened" (a step-6 failure): support, Sign out. */
export function MovedNotOpenedPage(props: { detail: string }) {
  return (
    <OpenPage testID="migration-moved-not-opened">
      <View class="items-center gap-3">
        <PageHeading testID="migration-moved-title">Your tax profiles were moved, but could not be opened.</PageHeading>
        <SoftLine testID="migration-moved-line">
          They are safe in this account's storage, and a copy is saved. Write to support@buwiz.com.
        </SoftLine>
        <If when={props.detail.length > 0}>
          <SoftLine testID="migration-moved-detail" size={12}>{props.detail}</SoftLine>
        </If>
        <SignOutErrorLine />
      </View>
      <View class="w-full gap-2.5">
        <PageButton testID="migration-moved-sign-out" label="Sign out" onPress={signOut} />
      </View>
    </OpenPage>
  );
}

/** The offer (§A.3.6): "Tax profiles from an older version of eBIRForms were found on this device."
 *  No / Not now. Add is not available in this version. */
export function OfferPage() {
  const busy = signal(false);
  const error = signal<string | null>(null);
  const answer = async (answer: "no" | "not_now") => {
    busy.set(true);
    error.set(null);
    try {
      await bir.migration_offer_answer(answer);
      rereadMigration();
    } catch (e) {
      error.set(errorText(e));
    } finally {
      busy.set(false);
    }
  };
  return (
    <OpenPage testID="migration-offer">
      <View class="items-center gap-3">
        <PageHeading testID="migration-offer-title">Tax profiles from an older version of eBIRForms were found on this device.</PageHeading>
        <Text variant="body" tone="fg2">Choose No only if this isn't yours. The older data stays untouched and won't be offered to this account again; there is no undo.</Text>
        <If when={error() !== null}>
          <Text variant="meta" tone="danger" testID="migration-offer-error">{error()}</Text>
        </If>
      </View>
      <View class="w-full gap-2.5">
        <PageButton testID="migration-offer-no" label="No" busy={busy()} onPress={() => void answer("no")} />
        <View class="items-center">
          <TextLink testID="migration-offer-not-now" label="Not now" onPress={() => void answer("not_now")} />
        </View>
      </View>
    </OpenPage>
  );
}

/** The migration page for the state now up, if one is (LockedScreen draws it before everything else). */
export function MigrationPages() {
  const page = migrationPage();
  if (page === null) return null;
  if (page.kind === "opening" || page.kind === "moving") return <MovingPage />;
  if (page.kind === "transient") return <TransientPage copySaved={page.copySaved} detail={page.detail} />;
  if (page.kind === "moved_not_opened") return <MovedNotOpenedPage detail={page.detail} />;
  if (page.kind === "device_store") return <DeviceStorePage detail={page.detail} />;
  return <OfferPage />;
}
