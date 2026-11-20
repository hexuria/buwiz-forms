// Settings > Account > who is signed in (old accountCards "identity"): the card with the name, the email and the TIN, Add account
// and Sign out; and, under it, the registered name and email of the selected profile with a pencil to the tax profile's editor.
import { View } from "nsh";
import { accountSummary } from "../../lib/settings_model";
import { app, go } from "../../shell";
import { AppButton, Avatar, Card, deviceCopy, IconButton, Line, SettingRow } from "../../ui";

/** The account card's words (old accountSummary): read inside JSX, so it follows the session, the taxpayers and the branch. */
function summary() {
  const session = app.session();
  const card = accountSummary({
    isAdmin: app.isAdmin(),
    isHelpdesk: app.isHelpdesk(),
    adminRoot: app.authState().admin_root,
    sessionKind: session.kind,
    sessionRoot: session.kind === "taxpayer" ? session.root : null,
    taxpayers: app.taxpayers(),
    branch: app.branch(),
    canAddTaxpayer: app.canAddTaxpayer(),
  });
  return { ...card, detail: deviceCopy(card.detail) };
}

/** A value with a pencil: the profile editor opens on that section. */
function ValueEdit(props: { id: string; value: string; section: string }) {
  return (
    <View class="flex-row items-center gap-0.5">
      <Line variant="body" tone="fg2" class="max-w-40">{props.value || "Not set"}</Line>
      <IconButton testID={props.id} icon="pencil" tooltip="Edit" onPress={() => app.openProfile(props.section)} />
    </View>
  );
}

export function IdentityBlock() {
  const profile = () => app.taxpayer();
  const branch = () => app.branch();
  return (
    <View class="w-full gap-4">
      <View testID="settings-account-card" class="w-full gap-4 rounded-(--radius-card) border border-border bg-card p-4">
        <View class="flex-row items-center gap-3">
          <Avatar name={summary().name} size={40} hueKey={summary().name} />
          <View class="min-w-0 flex-1">
            <Line variant="emph">{summary().name}</Line>
            <Line variant="meta" tone="fg3">{summary().email || summary().detail}</Line>
            <If when={summary().email}>
              <Line variant="meta" tone="fg3">{summary().detail}</Line>
            </If>
          </View>
        </View>
        <View class="flex-row flex-wrap gap-2">
          <If when={summary().showAdd}>
            <AppButton testID="settings-add-account" label="Add account" size="sm" icon="plus" onPress={() => go("new_taxpayer")} />
          </If>
          <AppButton testID="settings-sign-out" label="Sign out" size="sm" icon="log-out" onPress={app.logOut} />
        </View>
      </View>
      <If when={profile() && branch()}>
        <Card testID="settings-profile">
          <SettingRow
            testID="settings-profile-name"
            title="Registered name"
            hint={profile()?.tin_display ? `TIN ${profile()?.tin_display}` : summary().detail}
          >
            <ValueEdit id="settings-profile-name-edit" value={profile()?.name ?? ""} section="taxpayer" />
          </SettingRow>
          <SettingRow testID="settings-profile-email" title="Email" hint="Printed on this profile's returns">
            <ValueEdit id="settings-profile-email-edit" value={branch()?.facts.email ?? ""} section="address" />
          </SettingRow>
        </Card>
      </If>
    </View>
  );
}
