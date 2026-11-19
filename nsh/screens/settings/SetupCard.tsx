// Settings > Computer > Computer setup (old SetupCard; admin only): who this computer is set up for, and the steps to change it.
import { View } from "nsh";
import { app } from "../../shell";
import { AppButton, Card, Code, PinCells, SettingRow, TOUCH } from "../../ui";
import { StackRow } from "./parts";
import {
  candidates, confirmChange, finishChange, goStep, pickHelpdeskPin, pickOwner, resetSetup, setupChange, setupSummary, toggleCode,
} from "./setup";

/** Cancel: closes the change and forgets what was typed. */
function Cancel() {
  return <AppButton testID="setup-change-cancel" label="Cancel" variant="ghost" size="sm" onPress={resetSetup} />;
}

function Choose() {
  const helpdesk = () => app.isHelpdesk();
  return (
    <StackRow
      testID="setup-change-choose"
      title="Change to"
      hint={
        helpdesk()
          ? "Make one of the taxpayers the owner. They become the admin and sign in with their own PIN."
          : `Hand the ${TOUCH ? "device" : "computer"} to another taxpayer, or use it to help other taxpayers.`
      }
    >
      <AppButton testID="setup-change-owner" label={helpdesk() ? "An owner" : "Another owner"} size="sm" onPress={() => goStep("owner")} />
      <If when={!helpdesk()}>
        <AppButton testID="setup-change-helpdesk" label="A help desk" size="sm" onPress={() => goStep("helpdesk")} />
      </If>
      <Cancel />
    </StackRow>
  );
}

function PickOwner() {
  return (
    <>
      <SettingRow testID="setup-change-owner-head" title="New owner" hint={`The owner is the admin of this ${TOUCH ? "device" : "computer"}. Only a taxpayer with a PIN or authenticator can be chosen.`}>
        <Cancel />
      </SettingRow>
      <If when={candidates().length === 0}>
        <SettingRow testID="setup-change-none" title="No other taxpayers" hint="Add the taxpayer first, then come back here." />
      </If>
      <Each of={candidates()} key={(t) => t.tin_root}>
        {(t) => (
          <SettingRow
            testID={`setup-change-cand-${t().tin_root}`}
            title={t().name || t().tin_display}
            hint={t().ready ? `TIN ${t().tin_display}` : `TIN ${t().tin_display}. No PIN yet: they set one in Settings after signing in`}
          >
            <If when={t().ready}>
              <AppButton testID={`setup-change-pick-${t().tin_root}`} label="Choose" size="sm" onPress={() => pickOwner(t())} />
            </If>
          </SettingRow>
        )}
      </Each>
    </>
  );
}

function HelpdeskPin() {
  return (
    <>
      <SettingRow
        testID="setup-change-helpdesk-head"
        title="Switch to a help desk"
        hint="The admin signs in with TIN 000-000-000. The current owner stays a taxpayer with their own PIN."
      >
        <Cancel />
      </SettingRow>
      <StackRow testID="setup-change-newpin" title="New admin PIN" hint="4 digits, for TIN 000-000-000">
        <PinCells testID="setup-change-newpin-cells" length={4} masked label="New admin PIN" value={setupChange.newPin()} onChange={setupChange.setNewPin} />
        <AppButton testID="setup-change-newpin-next" label="Continue" variant="primary" size="sm" onPress={pickHelpdeskPin} />
      </StackRow>
    </>
  );
}

function Confirm() {
  const what = () => {
    const t = setupChange.target();
    return t?.kind === "owner"
      ? `Make ${t.name || "this taxpayer"} (TIN ${t.tin_display}) the owner and admin`
      : "Switch to a help desk with the new admin PIN";
  };
  return (
    <>
      <SettingRow testID="setup-change-what" title="Confirm the change" hint={what()}>
        <Cancel />
      </SettingRow>
      <StackRow
        testID="setup-change-confirm"
        title={setupChange.useCode() ? "Your admin authenticator code" : "Your current admin PIN"}
        hint="Then a new recovery key, and you are signed out"
      >
        <If
          when={setupChange.useCode()}
          else={
            <PinCells testID="setup-change-pin-cells" length={4} masked label="Current admin PIN" value={setupChange.confirmPin()} onChange={setupChange.setConfirmPin} />
          }
        >
          <PinCells testID="setup-change-code-cells" length={6} label="Admin authenticator code" value={setupChange.confirmCode()} onChange={setupChange.setConfirmCode} />
        </If>
        <AppButton testID="setup-change-confirm-btn" label="Confirm change" variant="primary" size="sm" onPress={confirmChange} />
        <If when={Boolean(app.authState().has_admin_totp)}>
          <AppButton testID="setup-change-toggle" label={setupChange.useCode() ? "Use PIN" : "Use code"} variant="ghost" size="sm" onPress={toggleCode} />
        </If>
      </StackRow>
    </>
  );
}

function Key() {
  return (
    <>
      <StackRow testID="setup-change-key" title="New recovery key" hint="Write it down now. It is shown once and works once; the old key no longer works.">
        <Code testID="setup-change-key-value">{setupChange.recoveryKey() ?? ""}</Code>
      </StackRow>
      <SettingRow testID="setup-change-done-row" title="Setup changed" hint="You are signed out next, so the admin signs in with their own PIN">
        <AppButton testID="setup-change-done" label="I wrote it down" variant="primary" size="sm" onPress={finishChange} />
      </SettingRow>
    </>
  );
}

/** The Computer setup card: the summary and Change, then whichever step the change is at. */
export function SetupCard() {
  const step = () => setupChange.step();
  return (
    <Card testID="settings-setup">
      <If
        when={step()}
        else={
          <SettingRow testID="setup-change" title="Set up for" hint={setupSummary()}>
            <AppButton testID="setup-change-btn" label="Change" size="sm" onPress={() => goStep("choose")} />
          </SettingRow>
        }
      >
        <View class="w-full">
          <If when={step() === "choose"}>
            <Choose />
          </If>
          <If when={step() === "owner"}>
            <PickOwner />
          </If>
          <If when={step() === "helpdesk"}>
            <HelpdeskPin />
          </If>
          <If when={step() === "confirm"}>
            <Confirm />
          </If>
          <If when={step() === "key"}>
            <Key />
          </If>
        </View>
      </If>
    </Card>
  );
}
