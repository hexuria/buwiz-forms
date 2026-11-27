// The migration pages (A3a, sign-in look B: OpenPage, PageHeading, PageButton, BackControl, the whole BrandLogo,
// never the emblem alone): what stands between the account gate and the frame while the legacy claim decides —
// Moving your tax profiles (progress), Could not be moved yet (transient: Try again / Sign out), Moved, not opened
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
  kind: "none" | "moving" | "transient" | "permanent" | "moved_not_opened" | "offered";
  copySaved: boolean;
  detail: string;
}
const state = signal<MigrationPage>({ kind: "none", copySaved: false, detail: "" });

/** The migration page now up, if one is: the frame waits behind it. Permanent is not a page — the empty room opens. */
export const migrationPage = () => {
  const kind = state().kind;
  return kind === "moving" || kind === "transient" || kind === "moved_not_opened" || kind === "offered" ? state() : null;
};

/** The permanent failure's detail for the Home note and the Settings › Account row, or null. */
export const migrationPermanent = () => (state().kind === "permanent" ? state().detail : null);

/** Ask the host what the migration state is now (after a session open, a Try again, an offer answer). */
export function rereadMigration(): void {
  try {
    const answer = bir.migration_state() as { kind: MigrationPage["kind"]; copy_saved: boolean; detail: string };
    state.set({ kind: answer.kind ?? "none", copySaved: answer.copy_saved === true, detail: answer.detail ?? "" });
  } catch {
    state.set({ kind: "none", copySaved: false, detail: "" });
  }
}

/** Sign out of the migration pages: the account gate comes back. */
function signOut(): void {
  app.lockCloud();
}

/** "Moving your tax profiles": the progress page while the claim runs (a spinner, no buttons). */
export function MovingPage() {
  return (
    <OpenPage testID="migration-moving">
      <View class="items-center gap-4">
        <PageHeading testID="migration-moving-title">Moving your tax profiles</PageHeading>
        <SoftLine testID="migration-moving-line">They are being moved to this account. This takes a moment.</SoftLine>
        <Spinner size={20} label="Moving your tax profiles…" />
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
      </View>
      <View class="w-full gap-2.5">
        <PageButton testID="migration-moved-sign-out" label="Sign out" onPress={signOut} />
      </View>
    </OpenPage>
  );
}

/** The offer (§A.3.6): "Tax profiles from an older version of eBIRForms were found on this device."
 *  Add them to this account / Not now. */
export function OfferPage() {
  const busy = signal(false);
  const error = signal<string | null>(null);
  const answer = async (add: boolean) => {
    busy.set(true);
    error.set(null);
    try {
      await bir.migration_offer_answer(add);
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
        <If when={error() !== null}>
          <Text variant="meta" tone="danger" testID="migration-offer-error">{error()}</Text>
        </If>
      </View>
      <View class="w-full gap-2.5">
        <PageButton testID="migration-offer-add" label="Add them to this account" busy={busy()} onPress={() => void answer(true)} />
        <View class="items-center">
          <TextLink testID="migration-offer-not-now" label="Not now" onPress={() => void answer(false)} />
        </View>
      </View>
    </OpenPage>
  );
}

/** The migration page for the state now up, if one is (LockedScreen draws it before everything else). */
export function MigrationPages() {
  const page = migrationPage();
  if (page === null) return null;
  if (page.kind === "moving") return <MovingPage />;
  if (page.kind === "transient") return <TransientPage copySaved={page.copySaved} detail={page.detail} />;
  if (page.kind === "moved_not_opened") return <MovedNotOpenedPage detail={page.detail} />;
  return <OfferPage />;
}
