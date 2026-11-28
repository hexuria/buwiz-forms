// First launch (the old shell's auth.js renderSetup and main.js renderPendingTaxpayer): who uses this computer decides who
// the admin is. "Just me or my business" makes the owner's own taxpayer the admin (typed here first on an empty computer,
// on the New taxpayer page with no chrome); "I help other taxpayers" makes the reserved TIN 000-000-000 the admin, with a
// 4-digit PIN and a recovery key shown once.
import { effect, platform, untracked, View } from "nsh";
import { BackHandler, isAvailable as mobileAvailable } from "nsh/mobile";
import { app, store } from "../../shell";
import { AppButton, Code, PinCells, Text, TOUCH } from "../../ui";
import { NewTaxpayerPage } from "../taxpayer";
import { Actions, AuthCard, ChoiceTile, deviceCopy, ErrorLine, Heading, Labelled, OpenPage } from "./parts";
import { auth } from "./state";

/** The choice, the help desk's PIN, or its recovery key: the step app.setup() names. */
export function Setup() {
  // The newest listener hears the press first (the frame's, Shell.tsx, is older). auth.back answers false on the choice, so
  // the press still leaves the app there.
  if (mobileAvailable && platform.os === "android") {
    effect(() => {
      BackHandler.on("press", () => auth.back());
    });
  }
  return (
    <View testID="setup-root" class="min-h-0 flex-1">
      <If when={app.setup().step === "helpdesk-pin"}>
        <HelpdeskPin />
      </If>
      <If when={app.setup().step === "helpdesk-key"}>
        <RecoveryKey />
      </If>
      <If when={app.setup().step !== "helpdesk-pin" && app.setup().step !== "helpdesk-key"}>
        <Choose />
      </If>
    </View>
  );
}

function Choose() {
  return (
    <AuthCard testID="setup-card">
      <Heading title={deviceCopy("Who will use eBIRForms on this computer?")} line="This decides who the admin is. Nobody can lock the others out." />
      <ChoiceTile
        testID="setup-solo"
        icon="user"
        title="Just me or my business"
        line="You are the admin. Your own tax profile is the admin account, with all its branches."
        onPress={auth.chooseSolo}
      />
      <ChoiceTile
        testID="setup-helpdesk"
        icon="users"
        title="I help other taxpayers"
        line="An RDO help desk, an accountant or a bookkeeper. You get a separate admin account, and each taxpayer signs in as themselves."
        onPress={auth.chooseHelpdesk}
      />
    </AuthCard>
  );
}

function HelpdeskPin() {
  return (
    <AuthCard testID="setup-pin-card">
      <Heading
        title="Choose the admin PIN"
        line="The help desk admin signs in with TIN 000-000-000 and this PIN. It also opens any taxpayer who forgot theirs."
      />
      <Labelled label="Admin PIN">
        <PinCells testID="setup-admin-pin" label="Admin PIN" length={4} masked value={auth.setupPin()} onChange={auth.setSetupPin} error={Boolean(auth.error())} />
      </Labelled>
      <ErrorLine message={auth.error()} />
      <Actions
        left={<AppButton testID="setup-back" label="Back" variant="ghost" onPress={() => void auth.back()} />}
        right={<AppButton testID="setup-pin-submit" label="Continue" variant="primary" onPress={auth.submitHelpdeskPin} />}
      />
    </AuthCard>
  );
}

function RecoveryKey() {
  return (
    <AuthCard testID="setup-key-card">
      <Heading
        title="Write down your recovery key"
        line="If you forget the admin PIN, this key signs you in once. It is shown only now. Keep it away from this computer."
      />
      <View class="items-center py-2">
        <Code testID="setup-key">{auth.setupKey() ?? ""}</Code>
      </View>
      <If when={!TOUCH}>
        <Text variant="meta" tone="fg2" class="text-center">Lost both? Whoever can log in to this Mac can still reset the admin PIN.</Text>
      </If>
      <Actions right={<AppButton testID="setup-key-done" label="I wrote it down" variant="primary" onPress={auth.finishHelpdeskSetup} />} />
    </AuthCard>
  );
}

/**
 * "Just me" on an empty computer: the New taxpayer page in the frame's page column (880, 16 or 32 in from the sides), with no
 * toolbar or sidebar (they open a profile that does not exist yet). Its Create finishes the setup; leaving it goes back to
 * the choice.
 */
export function SoloTaxpayer() {
  // The page is left once the route moved off New taxpayer after reaching it (its Cancel goes back).
  let reached = false;
  effect(() => {
    const list = store.taxpayers();
    const view = app.route().view;
    untracked(() => {
      if (app.setup().pending !== "solo") return;
      if (list.length) auth.finishSoloSetup(list[0].tin_root);
      else if (view === "new_taxpayer") reached = true;
      else if (reached) auth.cancelSetup();
    });
  });
  // The frame's page column on the open page: the logo over the New taxpayer page, which is longer than any window, so the
  // logo takes its smallest step and the page scrolls.
  return (
    <OpenPage testID="setup-taxpayer" wide>
      <NewTaxpayerPage />
    </OpenPage>
  );
}
